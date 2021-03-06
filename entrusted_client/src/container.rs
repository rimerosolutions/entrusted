use serde_json;
use std::error::Error;
use std::fs;
use std::io;
use filetime::FileTime;
use std::path::PathBuf;
use std::env;
use std::process::{Command, Stdio};
use std::io::{BufReader, BufRead, Read};
use std::thread::JoinHandle;
use std::sync::mpsc::Sender;
use std::thread;

use entrusted_l10n as l10n;
use crate::common;

fn read_output<R>(thread_name: &str, stream: R, tx: Sender<String>) -> Result<JoinHandle<Result<(), Box<dyn Error + Send + Sync>>>, io::Error>
where
    R: Read + Send + 'static {
    thread::Builder::new()
        .name(thread_name.to_string())
        .spawn(move || {
            let reader = BufReader::new(stream);
            reader.lines()
                .try_for_each(|line| {
                    match tx.send(line?) {
                        Ok(_)   => Ok(()),
                        Err(ex) => Err(ex.into())
                    }
                })
        })
}

fn exec_crt_command (container_program: common::ContainerProgram, args: Vec<&str>, tx: Sender<String>, capture_output: bool, printer: Box<dyn LogPrinter>, trans: l10n::Translations) -> Result<(), Box<dyn Error>> {
    let rt_path = container_program.exec_path;
    let sub_commands = container_program.sub_commands;
    let rt_executable: &str = &format!("{}", rt_path.display());

    let mut cmd = vec![];
    cmd.extend(sub_commands);
    cmd.extend(args.clone());

    let cmd_masked: Vec<&str> = cmd.clone()
        .iter()
        .map(|i| {
            if i.contains(common::ENV_VAR_ENTRUSTED_DOC_PASSWD) {
                "******"
            } else {
                i
            }
        }).collect();

    tx.send(printer.print(1, trans.gettext_fmt("Running command: {0}", vec![&format!("{} {}", rt_executable, cmd_masked.join(" "))])))?;

    let mut child = Command::new(rt_executable)
        .args(cmd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    if capture_output {
        let stdout_handles = vec![
            read_output("entrusted.stdout",  child.stdout.take().expect("!stdout"), tx.clone()),
            read_output("entrusted.stderr" , child.stderr.take().expect("!stderr"), tx.clone()),
        ];

        for stdout_handle in stdout_handles {
            if let Err(ex) = stdout_handle?.join() {
                return Err(format!("{} {:?}", trans.gettext("Command output capture failed!"), ex).into());
            }
        }
    }

    let exit_status = child.wait()?;

    match exit_status.success() {
        true => Ok(()),
        false => {
            // mitigate apparent 139 exit codes that don't happen with Debian bullseye
            // Sadly it's a much bigger image than with Alpine so we decide to mitigate the issue
            // Apparently a vsyscall=emulate argument needs to be added to /proc/cmdline depending on the kernel version
            // Essentially not mitigating the issue would be a problem in a non-controlled environment (i.e. Live CD ISO)
            if let Some(exit_code) = exit_status.code() {
                if exit_code == 139 {
                    return Ok(());
                }
            }

            Err(trans.gettext("Command failed!").into())
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

pub fn convert(input_path: PathBuf, output_path: PathBuf, convert_options: common::ConvertOptions, tx: Sender<String>, trans: l10n::Translations) -> Result<bool, Box<dyn Error>> {
    if !input_path.exists() {
        return Err(trans.gettext_fmt("The selected file does not exists: {0}!", vec![&input_path.display().to_string()]).into());
    }

    let printer: Box<dyn LogPrinter> = if convert_options.log_format == "plain".to_string() {
        Box::new(PlainLogPrinter)
    } else {
        Box::new(JsonLogPrinter)
    };

    tx.send(printer.print(1, format!("{} {}.", trans.gettext("Converting"), input_path.display())))?;

    let mut success = false;

    let ocr = if convert_options.opt_ocr_lang.is_some() {
        "1"
    } else {
        "0"
    };

    let ocr_language = match convert_options.opt_ocr_lang {
        Some(ocr_lang_val) => ocr_lang_val,
        None => String::new()
    };

    fn mkdirp(p: PathBuf, trans: l10n::Translations) -> Result<(), Box<dyn Error>> {
        if !p.exists() {
            let dir_created = fs::create_dir(&p);

            match dir_created {
                Err(ex) => {
                    Err(trans.gettext_fmt("Cannot create directory: {0}! Error: {1}", vec![&p.display().to_string(), &ex.to_string()]).into())
                },
                _ => Ok(())
            }
        } else {
            Ok(())
        }
    }

    pub fn cleanup_dir(dir: PathBuf) -> Result<(), Box<dyn Error>> {
        if dir.exists() && dir.is_dir() {
            let mut s = vec![dir];

            while let Some(f) = s.pop() {
                if f.is_file() && f.exists() {
                    fs::remove_file(f)?;
                } else {
                    for p in fs::read_dir(&f)? {
                        s.push(p?.path());
                    }
                }
            }
        }

        Ok(())
    }

    let run_args:Vec<&str> = vec![
        "run",
        "--network",
        "none",
        "--security-opt",
        "label=disable"
    ];

    let input_file_volume = &format!("{}:/tmp/input_file", input_path.display());
    let mut err_msg = String::new();

    // TODO for Lima we assume the default VM instance, that might not be true all the time...
    if let Some(container_rt) = common::container_runtime_path() {
        let mut ensure_image_args = vec!["inspect", &convert_options.container_image_name];

        tx.send(printer.print(1, trans.gettext("Checking if container image exists")))?;

        if let Err(ex) = exec_crt_command(container_rt.clone(), ensure_image_args, tx.clone(), false, printer.clone_box(), trans.clone()) {
            tx.send(printer.print(1, trans.gettext_fmt("The container image was not found. {0}", vec![&ex.to_string()])))?;
            ensure_image_args = vec!["pull", &convert_options.container_image_name];
            tx.send(printer.print(1, trans.gettext("Please wait, downloading image (roughly 600 MB).")))?;

            if let Err(exe) = exec_crt_command(container_rt.clone(), ensure_image_args, tx.clone(), false, printer.clone_box(), trans.clone()) {
                tx.send(printer.print(1, trans.gettext("Couldn't download container image!")))?;
                return Err(exe.into());
            }

            tx.send(printer.print(5, trans.gettext("Container image download completed...")))?;
        }

        let mut pixels_to_pdf_args = vec![];
        pixels_to_pdf_args.append(&mut run_args.clone());
        pixels_to_pdf_args.append(&mut container_rt.suggested_run_args.clone());

        // TODO potentially this needs to be configurable
        // i.e. for Lima with assume that /tmp/lima is the configured writable folder...
        let mut dz_tmp = match container_rt.suggested_tmp_dir {
            Some(ref suggested_dir) => suggested_dir.clone(),
            None => env::temp_dir(),
        };
        dz_tmp.push("entrusted");
        mkdirp(dz_tmp.clone(), trans.clone())?;
        cleanup_dir(dz_tmp.clone().to_path_buf())?;

        let mut dz_tmp_safe:PathBuf = dz_tmp.clone();
        dz_tmp_safe.push("safe");
        mkdirp(dz_tmp_safe.clone(), trans.clone())?;

        let mut dz_tmp_pixels:PathBuf = dz_tmp.clone();
        dz_tmp_pixels.push("pixels");
        mkdirp(dz_tmp_pixels.clone(), trans.clone())?;

        let safedir_volume = &format!("{}:/safezone", dz_tmp_safe.display());
        let ocr_env = &format!("OCR={}", ocr);
        let ocr_language_env = &format!("OCR_LANGUAGE={}", ocr_language);
        let locale_language_env = &format!("{}={}", l10n::ENV_VAR_ENTRUSTED_LANGID, trans.langid());
        let logformat_env = &format!("LOG_FORMAT={}", convert_options.log_format);
        let passwd_env = &format!("{}={}", common::ENV_VAR_ENTRUSTED_DOC_PASSWD, convert_options.opt_passwd.clone().unwrap_or_default());

        pixels_to_pdf_args.append(&mut vec![
            "-v", input_file_volume,
            "-v", safedir_volume,
        ]);

        pixels_to_pdf_args.append(&mut vec![
            "-e", ocr_env,
            "-e", logformat_env,
            "-e", ocr_language_env,
            "-e", locale_language_env
        ]);

        if let Some(passwd) = convert_options.opt_passwd {
            if !passwd.is_empty() {
                pixels_to_pdf_args.append(&mut vec![
                    "-e", passwd_env
                ]);
            }
        }

        pixels_to_pdf_args.append(&mut vec![
            &convert_options.container_image_name,
            common::CONTAINER_IMAGE_EXE
        ]);

        if let Ok(_) = exec_crt_command(container_rt, pixels_to_pdf_args, tx.clone(), true, printer.clone_box(), trans.clone()) {
            if output_path.exists() {
                fs::remove_file(output_path.clone())?;
            }

            let mut container_output_file_path = dz_tmp_safe.clone();
            container_output_file_path.push("safe-output-compressed.pdf");
            let atime = FileTime::now();
            let output_path_clone = output_path.clone();

            fs::copy(&container_output_file_path, output_path)?;
            fs::remove_file(container_output_file_path)?;

            let output_file = fs::File::open(output_path_clone)?;
            filetime::set_file_handle_times(&output_file, Some(atime), Some(atime))?;

            if let Err(ex) = cleanup_dir(dz_tmp.clone().to_path_buf()) {
                tx.send(printer.print(100, trans.gettext_fmt("Failed to cleanup temporary folder: {0}. {1}.", vec![&dz_tmp.clone().display().to_string(), &ex.to_string()])))?;
            }
            success = true;
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
    }

    match success {
        true => Ok(success),
        _ => Err(err_msg.into())
    }
}
