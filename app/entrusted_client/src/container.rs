use serde_json;
use std::error::Error;
use std::fs;
use std::io::{self};
use std::process::Child;
use filetime::FileTime;
use std::path::PathBuf;
use std::env;
use std::process::{Command, Stdio};
use std::io::{BufReader, BufRead, Read};
use std::thread::JoinHandle;
use std::thread;

use entrusted_l10n as l10n;
use crate::common;

#[derive(Clone)]
struct NoOpEventSender;

impl common::EventSender for NoOpEventSender {
    fn send(&self, _: crate::common::AppEvent) -> Result<(), std::sync::mpsc::SendError<crate::common::AppEvent>> {
        Ok(())
    }

    fn clone_box(&self) -> Box<dyn common::EventSender> {
        Box::new(self.clone())
    }
}

fn read_cmd_output<R>(thread_name: &str, stream: R, tx: Box<dyn common::EventSender>) -> Result<JoinHandle<Result<(), Box<dyn Error + Send + Sync>>>, io::Error>
where
    R: Read + Send + 'static {
    thread::Builder::new()
        .name(thread_name.to_string())
        .spawn(move || {
            let reader = BufReader::new(stream);
            reader.lines()
                .try_for_each(|line| {
                    if let Err(ex) = tx.send(common::AppEvent::ConversionProgressEvent(line?)) {
                        return Err(ex.into());
                    }

                    Ok(())
                })
        })
}

#[cfg(not(any(target_os = "windows")))]
fn spawn_command(cmd: &str, cmd_args: Vec<String>) -> std::io::Result<Child> {
    Command::new(cmd)
        .args(cmd_args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
}

#[cfg(target_os = "windows")]
fn spawn_command(cmd: &str, cmd_args: Vec<String>) -> std::io::Result<Child> {
    use std::os::windows::process::CommandExt;
    Command::new(cmd)
        .args(cmd_args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(0x08000000)
        .spawn()
}

fn exec_crt_command (cmd_desc: String, container_program: common::ContainerProgram, args: Vec<String>, tx: Box<dyn common::EventSender>, capture_output: bool, printer: Box<dyn LogPrinter>, trans: l10n::Translations) -> Result<(), Box<dyn Error>> {
    let rt_path = container_program.exec_path;
    let sub_commands = container_program.sub_commands.iter().map(|i| i.to_string());
    let rt_executable: &str = &rt_path.display().to_string();

    let mut cmd = Vec::with_capacity(sub_commands.len() + args.len());
    cmd.extend(sub_commands);
    cmd.extend(args.clone());

    let cmd_masked: Vec<&str> = cmd
        .iter()
        .map(|i| {
            if i.contains(common::ENV_VAR_ENTRUSTED_DOC_PASSWD) {
                "ENTRUSTED_DOC_PASSWD=***"
            } else {
                i
            }
        }).collect();

    let masked_cmd = cmd_masked.join(" ");

    tx.send(common::AppEvent::ConversionProgressEvent(printer.print(1, trans.gettext_fmt("Running command: {0}", vec![&format!("{} {}", rt_executable, masked_cmd)]))))?;
    tx.send(common::AppEvent::ConversionProgressEvent(printer.print(1, cmd_desc)))?;

    let mut cmd = spawn_command(rt_executable, cmd)?;

    let stdout_handles = vec![
        read_cmd_output("entrusted.stdout",  cmd.stdout.take().expect("!stdout"), if capture_output {
            tx.clone_box()
        } else {
            Box::new(NoOpEventSender)
        }),
        read_cmd_output("entrusted.stderr" , cmd.stderr.take().expect("!stderr"), if capture_output {
            tx.clone_box()
        } else {
            Box::new(NoOpEventSender)
        })
    ];

    for stdout_handle in stdout_handles {
        if let Err(ex) = stdout_handle?.join() {
            return Err(format!("{} {:?}", trans.gettext("Command output capture failed!"), ex).into());
        }
    }

    loop {
        match cmd.try_wait() {
            Ok(Some(exit_status)) => {
                if exit_status.success() {
                    return Ok(());
                } else {
                    if let Some(exit_code) = exit_status.code() {
                        // Please see https://betterprogramming.pub/understanding-docker-container-exit-codes-5ee79a1d58f6
                        if exit_code == 139 || exit_code == 137 {
                            let mut explanation = trans.gettext("Container process terminated abruptly potentially due to memory usage. Are PDF pages too big? Try increasing the container engine memory allocation?");

                            if exit_code == 139 {
                                explanation = trans.gettext("Container process terminated abruptly potentially due to a memory access fault. Please report the issue at: https://github.com/rimerosolutions/entrusted/issues");
                            }

                            let lm = common::LogMessage {
                                data: format!("{} {}", trans.gettext("Conversion failed!"), explanation),
                                percent_complete: 100
                            };

                            if let Ok(lm_string) = serde_json::to_string(&lm) {
                                let _ = tx.send(common::AppEvent::ConversionProgressEvent(lm_string));
                            }
                        }
                    }

                    return Err(trans.gettext("Command failed!").into());
                }
            },
            Ok(None) => {
                thread::yield_now();
            },
            Err(ex) => {
                return Err(format!("{} {}", trans.gettext("Command failed!"), ex.to_string()).into());
            }
        }
    }
}

trait LogPrinter: Send + Sync {
    fn print(&self, percent_complete: usize, data: String) -> String;

    fn clone_box(&self) -> Box<dyn LogPrinter>;
}

impl Clone for Box<dyn LogPrinter> {
    fn clone(&self) -> Box<dyn LogPrinter> {
        self.clone_box()
    }
}

#[derive(Debug, Copy, Clone)]
struct PlainLogPrinter;

#[derive(Debug, Copy, Clone)]
struct JsonLogPrinter;

impl LogPrinter for PlainLogPrinter {
    fn print(&self, percent_complete: usize, data: String) -> String {
        format!("{}% {}", percent_complete, data)
    }

    fn clone_box(&self) -> Box<dyn LogPrinter> {
        Box::new(self.clone())
    }
}

impl LogPrinter for JsonLogPrinter {
    fn print(&self, percent_complete: usize, data: String) -> String{
        let log_msg = &common::LogMessage {
            percent_complete, data
        };
        serde_json::to_string(log_msg).unwrap()
    }

    fn clone_box(&self) -> Box<dyn LogPrinter> {
        Box::new(self.clone())
    }
}

pub fn convert(input_path: PathBuf, output_path: PathBuf, convert_options: common::ConvertOptions, tx: Box<dyn common::EventSender>,  trans: l10n::Translations) -> Result<bool, Box<dyn Error>> {
    if !input_path.exists() {
        return Err(trans.gettext_fmt("The selected file does not exists: {0}!", vec![&input_path.display().to_string()]).into());
    }

    let printer: Box<dyn LogPrinter> = if convert_options.log_format == "plain".to_string() {
        Box::new(PlainLogPrinter)
    } else {
        Box::new(JsonLogPrinter)
    };

    tx.send(common::AppEvent::ConversionProgressEvent(printer.print(1, format!("{} {}", trans.gettext("Converting"), input_path.display()))))?;

    let mut success = false;

    fn mkdirp(p: &PathBuf, trans: l10n::Translations) -> Result<(), Box<dyn Error>> {
        if !p.exists() {
            if let Err(ex) = fs::create_dir_all(&p) {
                return Err(trans.gettext_fmt("Cannot create directory: {0}! Error: {1}", vec![&p.display().to_string(), &ex.to_string()]).into());
            }
        }

        Ok(())
    }

    pub fn cleanup_dir(dir: &PathBuf) -> Result<(), Box<dyn Error>> {
        if dir.exists() && dir.is_dir() {
            let mut files = vec![dir.to_owned()];

            while let Some(f) = files.pop() {
                if f.is_file() && f.exists() {
                    fs::remove_file(f)?;
                } else {
                    for p in fs::read_dir(&f)? {
                        files.push(p?.path());
                    }
                }
            }
        }

        Ok(())
    }

    let run_args:Vec<String> = vec![
        "run".to_string(),
        "--rm".to_string(),
        "--network".to_string() , "none".to_string(),
        "--cap-drop".to_string(), "all".to_string()
    ];

    let input_file_volume = format!("{}:/tmp/input_file:Z", input_path.display());
    let mut err_msg = String::new();

    // TODO for Lima we assume the default VM instance, that might not be true all the time...
    if let Some(container_rt) = common::container_runtime_path() {
        let mut ensure_image_args = vec!["inspect".to_string(), convert_options.container_image_name.to_owned()];
        if let Err(ex) = exec_crt_command(trans.gettext("Checking if container image exists"), container_rt.clone(), ensure_image_args, tx.clone_box(), false, printer.clone_box(), trans.clone()) {
            tx.send(common::AppEvent::ConversionProgressEvent(printer.print(1, trans.gettext_fmt("The container image was not found. {0}", vec![&ex.to_string()]))))?;
            ensure_image_args = vec!["pull".to_string(), convert_options.container_image_name.to_owned()];

            if let Err(exe) = exec_crt_command(trans.gettext("Please wait, downloading sandbox image (roughly 600 MB)"), container_rt.clone(), ensure_image_args, tx.clone_box(), false, printer.clone_box(), trans.clone()) {
                tx.send(common::AppEvent::ConversionProgressEvent(printer.print(100, trans.gettext("Couldn't download container image!"))))?;
                return Err(exe.into());
            }

            tx.send(common::AppEvent::ConversionProgressEvent(printer.print(5, trans.gettext("Container image download completed..."))))?;
        }

        let mut convert_args = Vec::with_capacity(run_args.len() + container_rt.suggested_run_args.len());
        convert_args.append(&mut run_args.clone());
        convert_args.append(&mut container_rt.suggested_run_args.iter().map(|i| i.to_string()).collect());

        // TODO potentially this needs to be configurable
        // i.e. for Lima with assume that /tmp/lima is the configured writable folder...
        let mut dz_tmp = match container_rt.suggested_tmp_dir {
            Some(ref suggested_dir) => suggested_dir.clone(),
            None => env::temp_dir(),
        };
        dz_tmp.push("entrusted");
        mkdirp(&dz_tmp, trans.clone())?;
        cleanup_dir(&dz_tmp)?;

        // TODO Make the seccomp profile work for arm64
        // There are ongoing tech challenges generating the seccomp profile under QEMU from an amd64 host (Alpine Linux aarch64 ISO)...
        // Ideally we'll maintain a single JSON seccomp profile JSON file that works for both arm64 and amd64
        // Also noticed some issue with Docker on Mac OS that are apparently related to the seccomp profile, so it's not just an arm64 issue...
        // #[cfg(not(target_arch = "aarch64"))]
        // {
        //     use std::io::Write;

        //     let seccomp_profile_data = include_bytes!("../seccomp-entrusted-profile.json");
        //     let seccomp_profile_filename = format!("seccomp-entrusted-profile-{}.json", option_env!("CARGO_PKG_VERSION").unwrap_or("Unknown"));
        //     let seccomp_profile_pathbuf = PathBuf::from(dz_tmp.join(seccomp_profile_filename));
        //     convert_args.push("--security-opt".to_string());
        //     convert_args.push(format!("seccomp={}", seccomp_profile_pathbuf.display()));

        //     if !seccomp_profile_pathbuf.exists() {
        //         let f_ret = fs::File::create(&seccomp_profile_pathbuf);

        //         match f_ret {
        //             Ok(mut f) => {
        //                 if let Err(ex) = f.write_all(seccomp_profile_data) {
        //                     tx.send(common::AppEvent::ConversionProgressEvent(printer.print(5, trans.gettext_fmt("Could not save security profile to {0}. {1}.", vec![&seccomp_profile_pathbuf.display().to_string(), &ex.to_string()]))))?;
        //                     return Err(ex.into());
        //                 }

        //                 if let Err(ex) = f.sync_all() {
        //                     tx.send(common::AppEvent::ConversionProgressEvent(printer.print(5, trans.gettext_fmt("Could not save security profile to {0}. {1}.", vec![&seccomp_profile_pathbuf.display().to_string(), &ex.to_string()]))))?;
        //                     return Err(ex.into());
        //                 }
        //             },
        //             Err(ex) => {
        //                 tx.send(common::AppEvent::ConversionProgressEvent(printer.print(5, trans.gettext_fmt("Could not save security profile to {0}. {1}.", vec![&seccomp_profile_pathbuf.display().to_string(), &ex.to_string()]))))?;
        //                 return Err(ex.into());
        //             }
        //         }
        //     }
        // }

        // TODO dynamic naming for couple of folders overall
        // This is needed for parallel conversion and not overwritting files among other things
        let mut dz_tmp_safe:PathBuf = dz_tmp.clone();
        dz_tmp_safe.push("safe");
        mkdirp(&dz_tmp_safe, trans.clone())?;

        // Mitigate volume permissions issues with Docker under Linux
        #[cfg(not(target_os = "windows"))] {
            use std::ffi::CString;
            let path_safe_string = dz_tmp_safe.display().to_string();
            let path_safe_cstring = CString::new(path_safe_string).unwrap();
            let path_safe = path_safe_cstring.as_bytes().as_ptr() as *mut std::os::raw::c_char;
            let _ = unsafe { libc::chmod (path_safe, 0o777) };
        }

        let safedir_volume = format!("{}:/safezone:Z", dz_tmp_safe.display());

        convert_args.append(&mut vec![
            "-v".to_string(), input_file_volume,
            "-v".to_string(), safedir_volume,
        ]);

        convert_args.append(&mut vec![
            "-e".to_string(), format!("LOG_FORMAT={}", convert_options.log_format),
            "-e".to_string(), format!("{}={}", l10n::ENV_VAR_ENTRUSTED_LANGID, trans.langid())
        ]);

        if let Some(ocr_language) = convert_options.opt_ocr_lang {
            convert_args.append(&mut vec![
                "-e".to_string(), format!("OCR_LANGUAGE={}", ocr_language.to_owned())
            ]);
        }

        if let Some(passwd) = convert_options.opt_passwd {
            if !passwd.is_empty() {
                convert_args.append(&mut vec![
                    "-e".to_string(), format!("{}={}", common::ENV_VAR_ENTRUSTED_DOC_PASSWD, passwd.to_owned())
                ]);
            }
        }

        convert_args.append(&mut vec![
            convert_options.container_image_name.to_owned(),
            common::CONTAINER_IMAGE_EXE.to_string()
        ]);

        if let Ok(_) = exec_crt_command(trans.gettext("Starting document processing"), container_rt, convert_args, tx.clone_box(), true, printer.clone_box(), trans.clone()) {
            if output_path.exists() {
                if let Err(ex) = fs::remove_file(output_path.clone()) {
                    eprintln!("{}", trans.gettext_fmt("Cannot remove output file: {0}. {1}.", vec![&output_path.display().to_string(), &ex.to_string()]));
                }
            }

            // In the case of a container crash, the output file will not be present...
            // This should be handled upstream by capturing proper exit codes of the sanitization process
            let mut container_output_file_path = dz_tmp_safe.clone();
            container_output_file_path.push("safe-output-compressed.pdf");

            if !container_output_file_path.exists() {
                let msg_info = trans.gettext("Potential sanitization process crash detected, the sanitized PDF result was not created.");
                err_msg.push_str(&msg_info);
            } else {
                let atime = FileTime::now();

                fs::copy(&container_output_file_path, &output_path)?;
                fs::remove_file(container_output_file_path)?;

                let output_file = fs::File::open(&output_path)?;

                // This seems to fail on Microsoft Windows with permission denied errors
                let _ = filetime::set_file_handle_times(&output_file, Some(atime), Some(atime));

                if let Err(ex) = cleanup_dir(&dz_tmp_safe) {
                    tx.send(common::AppEvent::ConversionProgressEvent(printer.print(100, trans.gettext_fmt("Failed to cleanup temporary folder: {0}. {1}.", vec![&dz_tmp.clone().display().to_string(), &ex.to_string()]))))?;
                }

                success = true;
            }
        } else {
            err_msg = trans.gettext("Conversion failed!");
        }
    } else {
        err_msg.push_str(&trans.gettext("No container runtime executable found!"));
        err_msg.push_str("\n");

        if cfg!(any(target_os="windows")) {
            err_msg.push_str(&trans.gettext("Please install Docker."));
        } else if cfg!(any(target_os="macos")) {
            err_msg.push_str(&trans.gettext("Please install Docker or Lima."));
        } else { // Linux and others
            err_msg.push_str(&trans.gettext("Please install Docker or Podman."));
        }

        tx.send(common::AppEvent::ConversionProgressEvent(printer.print(100, err_msg.clone())))?;
    }

    if success {
        Ok(success)
    } else {
        Err(err_msg.into())
    }
}
