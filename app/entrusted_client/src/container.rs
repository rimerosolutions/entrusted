use std::error::Error;
use std::fs;
use std::io;
use std::process::Child;
use filetime::FileTime;
use std::path::PathBuf;
use std::collections::HashMap;
use std::env;
use std::process::{Command, Stdio};
use std::io::{BufReader, BufRead, Read};
use std::thread::JoinHandle;
use std::thread;
use uuid::Uuid;

use entrusted_l10n as l10n;
use crate::common;

fn mkdirp(p: &PathBuf, trans: l10n::Translations) -> Result<(), Box<dyn Error>> {
    if !p.exists() {
        if let Err(ex) = fs::create_dir_all(p) {
            return Err(trans.gettext_fmt("Cannot create directory: {0}! Error: {1}", vec![&p.display().to_string(), &ex.to_string()]).into());
        }
    }

    Ok(())
}

fn cleanup_dir(dir: &PathBuf) -> Result<(), Box<dyn Error>> {
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

        fs::remove_dir(dir)?;
    }

    Ok(())
}

trait SanitizerRt {
    fn install(&self, convert_options: common::ConvertOptions, tx: Box<dyn common::EventSender>, printer: &dyn Fn(usize, String) -> String,  trans: l10n::Translations) -> Result<(), Box<dyn Error>>;
    fn process(&self, input_path: PathBuf, output_path: PathBuf, convert_options: common::ConvertOptions, tx: Box<dyn common::EventSender>, printer: &dyn Fn(usize, String) -> String,  trans: l10n::Translations) -> Result<(), Box<dyn Error>>;
}

struct ContainerizedSanitizerRt<'a>  {
    container_program: common::ContainerProgram<'a>
}

impl <'a>ContainerizedSanitizerRt<'a> {
    fn new(container_program: common::ContainerProgram<'a>) -> Self {
        Self {
            container_program
        }
    }
}

struct NativeSanitizerRt<'a>  {
    container_program: common::ContainerProgram<'a>
}

impl <'a>NativeSanitizerRt<'a> {
    fn new(container_program: common::ContainerProgram<'a>) -> Self {
        Self {
            container_program
        }
    }
}

impl <'a> SanitizerRt for NativeSanitizerRt<'a> {
    fn install(&self, _: common::ConvertOptions, _: Box<dyn common::EventSender>, _: &dyn Fn(usize, String) -> String, _: l10n::Translations) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn process(&self, input_path: PathBuf, output_path: PathBuf, convert_options: common::ConvertOptions, tx: Box<dyn common::EventSender>, printer: &dyn Fn(usize, String) -> String, trans: l10n::Translations) -> Result<(), Box<dyn Error>> {
        let mut success = false;
        let mut err_msg = String::new();
        let mut env_vars = HashMap::new();
        let mut convert_args = Vec::new();

        env_vars.insert(l10n::ENV_VAR_ENTRUSTED_LANGID.to_string(), trans.langid());

        if let Some(passwd) = convert_options.opt_passwd {
            env_vars.insert(common::ENV_VAR_ENTRUSTED_DOC_PASSWD.to_string(), passwd);
        }

        if let Some(ocr_language) = convert_options.opt_ocr_lang {
            convert_args.append(&mut vec![
                "--ocr-lang".to_string(), ocr_language
            ]);
        }

        convert_args.append(&mut vec![
            "--input-filename".to_string(), input_path.display().to_string(),
            "--output-filename".to_string(), output_path.display().to_string(),
            "--visual-quality".to_string(), convert_options.visual_quality,
            "--log-format".to_string(), convert_options.log_format,
        ]);

        if exec_crt_command(trans.gettext("Starting document processing"), self.container_program.clone(), env_vars, convert_args, tx.clone_box(), true, printer, trans.clone()).is_ok() {
            let atime = FileTime::now();
            let output_file = fs::File::open(&output_path)?;

            // This seems to fail on Microsoft Windows with permission denied errors
            let _ = filetime::set_file_handle_times(&output_file, Some(atime), Some(atime));

            success = true;
        } else {
            err_msg = trans.gettext("Conversion failed!");
        }

        if success {
            Ok(())
        } else {
            Err(err_msg.into())
        }
    }
}

impl <'a> SanitizerRt for ContainerizedSanitizerRt<'a>  {
    fn install(&self, convert_options: common::ConvertOptions, tx: Box<dyn common::EventSender>, printer: &dyn Fn(usize, String) -> String,  trans: l10n::Translations) -> Result<(), Box<dyn Error>> {
        let mut ensure_image_args = vec!["inspect".to_string(), convert_options.container_image_name.to_owned()];

        let env_vars = HashMap::new();

        if let Err(ex) = exec_crt_command(trans.gettext("Checking if container image exists"), self.container_program.clone(), env_vars.clone(), ensure_image_args, tx.clone_box(), false, printer, trans.clone()) {
            tx.send(common::AppEvent::ConversionProgressEvent(printer(1, trans.gettext_fmt("The container image was not found. {0}", vec![&ex.to_string()]))))?;
            ensure_image_args = vec!["pull".to_string(), convert_options.container_image_name];

            if let Err(exe) = exec_crt_command(trans.gettext("Please wait, downloading sandbox image (roughly 600 MB)"), self.container_program.clone(), env_vars, ensure_image_args, tx.clone_box(), false, printer, trans.clone()) {
                tx.send(common::AppEvent::ConversionProgressEvent(printer(100, trans.gettext("Couldn't download container image!"))))?;
                return Err(exe);
            }

            tx.send(common::AppEvent::ConversionProgressEvent(printer(5, trans.gettext("Container image download completed..."))))?;
        }

        Ok(())
    }

    fn process(&self, input_path: PathBuf, output_path: PathBuf, convert_options: common::ConvertOptions, tx: Box<dyn common::EventSender>, printer: &dyn Fn(usize, String) -> String,  trans: l10n::Translations) -> Result<(), Box<dyn Error>> {
        let mut success = false;
        let mut err_msg = String::new();
        let container_rt = self.container_program.clone();

        let mut run_args:Vec<String> = vec![
            "run".to_string(),
            "--rm".to_string(),
            "--network".to_string() , "none".to_string(),
            "--cap-drop".to_string(), "all".to_string()
        ];

        let mut convert_args = Vec::with_capacity(run_args.len() + container_rt.suggested_run_args.len());
        convert_args.append(&mut run_args);
        convert_args.append(&mut container_rt.suggested_run_args.iter().map(|i| i.to_string()).collect());

        // TODO potentially this needs to be configurable
        // i.e. for Lima with assume that /tmp/lima is the configured writable folder...
        let mut dz_tmp = match container_rt.suggested_tmp_dir {
            Some(ref suggested_dir) => suggested_dir.clone(),
            None => env::temp_dir(),
        };
        dz_tmp.push("entrusted");
        mkdirp(&dz_tmp, trans.clone())?;

        // seccomp file generation steps with podman (test with Lima, Docker and Podman):
        // 1. On podman (amd64), install a hook https://github.com/containers/oci-seccomp-bpf-hook, this is easier on Fedora (dependencies, enabled kernel modules, etc.).
        // 2. On arm64, write a brute-force program that allows all amd64 calls and then disable selectively all allowed syscalls (https://github.com/yveszoundi/seccompia
        // 3. Merge syscalls obtained from the oci hook (amd64) with the ones generated by the brute-force program on arm64
        // 4. Minify the seccomp JSON file with the 'jq' linux tool
        let seccomp_profile_data = include_bytes!("../seccomp-entrusted-profile.json");
        let seccomp_profile_filename = format!("seccomp-entrusted-profile-{}.json", option_env!("CARGO_PKG_VERSION").unwrap_or("Unknown"));

        if convert_options.seccomp_profile_enabled {
            use std::io::Write;
            let seccomp_profile_pathbuf = dz_tmp.join(seccomp_profile_filename);
            convert_args.push("--security-opt".to_string());
            convert_args.push(format!("seccomp={}", seccomp_profile_pathbuf.display()));

            if !seccomp_profile_pathbuf.exists() {
                let f_ret = fs::File::create(&seccomp_profile_pathbuf);

                match f_ret {
                    Ok(mut f) => {
                        if let Err(ex) = f.write_all(seccomp_profile_data) {
                            tx.send(common::AppEvent::ConversionProgressEvent(printer(5, trans.gettext_fmt("Could not save security profile to {0}. {1}.", vec![&seccomp_profile_pathbuf.display().to_string(), &ex.to_string()]))))?;
                            return Err(ex.into());
                        }

                        if let Err(ex) = f.sync_all() {
                            tx.send(common::AppEvent::ConversionProgressEvent(printer(5, trans.gettext_fmt("Could not save security profile to {0}. {1}.", vec![&seccomp_profile_pathbuf.display().to_string(), &ex.to_string()]))))?;
                            return Err(ex.into());
                        }
                    },
                    Err(ex) => {
                        tx.send(common::AppEvent::ConversionProgressEvent(printer(5, trans.gettext_fmt("Could not save security profile to {0}. {1}.", vec![&seccomp_profile_pathbuf.display().to_string(), &ex.to_string()]))))?;
                        return Err(ex.into());
                    }
                }
            }
        }

        // TODO dynamic naming for couple of folders overall
        // This is needed for parallel conversion and not overwritting files among other things
        let request_id = Uuid::new_v4().to_string();
        let dz_tmp_safe:PathBuf = dz_tmp.join("safe").join(request_id);
        mkdirp(&dz_tmp_safe, trans.clone())?;

        // Mitigate volume permissions issues with Docker under Linux
        #[cfg(not(target_os = "windows"))] {
            use std::ffi::CString;
            let path_safe_string = dz_tmp_safe.display().to_string();

            if let Ok(path_safe_cstring) = CString::new(path_safe_string) {
                let path_safe = path_safe_cstring.as_bytes().as_ptr() as *mut std::os::raw::c_char;
                let _ = unsafe { libc::chmod (path_safe, 0o777) };
            }
        }

        let safedir_volume = format!("{}:/safezone:Z", dz_tmp_safe.display());

        // TODO make Lima handling more explicit but less annoying, abstractions?
        // Need to ensure that we use /tmp/lima for Lima as other folders are not mounted by default...
        // Note that this also applies to the default instance and we assume no expert user customization...
        let mut tmp_input_loc = String::new();

        let input_file_volume = match container_rt.suggested_tmp_dir {
            Some(ref suggested_dir) => {
                // This would be /tmp/lima/entrusted/requests
                let input_dir = suggested_dir.clone().join("entrusted").join("requests");

                if let Err(ex) = fs::create_dir_all(&input_dir) {
                    return Err(trans.gettext_fmt("Cannot create directory: {0}! Error: {1}", vec![&input_dir.display().to_string(), &ex.to_string()]).into());
                }

                let filename = input_path.file_name().unwrap().to_str().unwrap();
                let tmp_input_path = input_dir.join(filename);
                fs::copy(&input_path, &tmp_input_path)?;
                tmp_input_loc.push_str(&tmp_input_path.display().to_string());

                format!("{}:/tmp/input_file:Z", tmp_input_loc)
            },
            None => {
                format!("{}:/tmp/input_file:Z", input_path.display())
            }
        };

        convert_args.append(&mut vec![
            "-v".to_string(), input_file_volume,
            "-v".to_string(), safedir_volume,
        ]);

        convert_args.append(&mut vec![
            "-e".to_string(), format!("{}={}", l10n::ENV_VAR_ENTRUSTED_LANGID, trans.langid())
        ]);

        if let Some(passwd) = convert_options.opt_passwd {
            if !passwd.is_empty() {
                convert_args.append(&mut vec![
                    "-e".to_string(), format!("{}={}", common::ENV_VAR_ENTRUSTED_DOC_PASSWD, passwd)
                ]);
            }
        }

        convert_args.append(&mut vec![
            convert_options.container_image_name.to_owned()
        ]);
        
        convert_args.append(&mut vec![
            common::CONTAINER_IMAGE_EXE.to_string()
        ]);

        if let Some(ocr_language) = convert_options.opt_ocr_lang {
            convert_args.append(&mut vec![
                "--ocr-lang".to_string(), ocr_language
            ]);
        }

        convert_args.append(&mut vec![
            "--visual-quality".to_string(), convert_options.visual_quality,
            "--log-format".to_string(), convert_options.log_format,
        ]);

        let env_vars = HashMap::new();

        if exec_crt_command(trans.gettext("Starting document processing"), self.container_program.clone(), env_vars, convert_args, tx.clone_box(), true, printer, trans.clone()).is_ok() {
            // Delete file temporarily copied to a "well-known" mounted path for Lima
            if !tmp_input_loc.is_empty() {
                let _ = fs::remove_file(tmp_input_loc);
            }

            if output_path.exists() {
                if let Err(ex) = fs::remove_file(&output_path) {
                    eprintln!("{}", trans.gettext_fmt("Cannot remove output file: {0}. {1}.", vec![&output_path.display().to_string(), &ex.to_string()]));
                }
            }

            // In the case of a container crash, the output file will not be present...
            // This should be handled upstream by capturing proper exit codes of the sanitization process
            let container_output_file_path = dz_tmp_safe.join("safe-output-compressed.pdf");

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
                    tx.send(common::AppEvent::ConversionProgressEvent(printer(100, trans.gettext_fmt("Failed to cleanup temporary folder: {0}. {1}.", vec![&dz_tmp.clone().display().to_string(), &ex.to_string()]))))?;
                }

                success = true;
            }
        } else {
            // Delete file temporarily copied to a visible mount for Lima
            if !tmp_input_loc.is_empty() {
                let _ = fs::remove_file(tmp_input_loc);
            }

            err_msg = trans.gettext("Conversion failed!");
        }

        if fs::metadata(&dz_tmp_safe).is_ok() {
            let _ = cleanup_dir(&dz_tmp_safe); // ensure that we cleanup after ourselves....
        }

        if success {
            Ok(())
        } else {
            Err(err_msg.into())
        }
    }
}

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

type JoinHandleResult = Result<JoinHandle<Result<(), Box<dyn Error + Send + Sync>>>, io::Error>;
fn read_cmd_output<R>(thread_name: &str, stream: R, tx: Box<dyn common::EventSender>) -> JoinHandleResult
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
fn spawn_command(cmd: &str, env_vars: HashMap<String, String>, cmd_args: Vec<String>) -> std::io::Result<Child> {
    Command::new(cmd)
        .envs(env_vars)
        .args(cmd_args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
}

#[cfg(target_os = "windows")]
fn spawn_command(cmd: &str, env_vars: HashMap<String, String>, cmd_args: Vec<String>) -> std::io::Result<Child> {
    use std::os::windows::process::CommandExt;
    Command::new(cmd)
        .envs(env_vars)
        .args(cmd_args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(0x08000000)
        .spawn()
}

fn exec_crt_command (cmd_desc: String, container_program: common::ContainerProgram, env_vars: HashMap<String, String>, args: Vec<String>, tx: Box<dyn common::EventSender>, capture_output: bool, printer: &dyn Fn(usize, String) -> String, trans: l10n::Translations) -> Result<(), Box<dyn Error>> {
    let rt_path = container_program.exec_path;
    let sub_commands = container_program.sub_commands.iter().map(|i| i.to_string());
    let rt_executable: &str = &rt_path.display().to_string();

    let mut cmd = Vec::with_capacity(sub_commands.len() + args.len());
    cmd.extend(sub_commands);
    cmd.extend(args);

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

    tx.send(common::AppEvent::ConversionProgressEvent(printer(1, trans.gettext_fmt("Running command: {0}", vec![&format!("{} {}", rt_executable, masked_cmd)]))))?;
    tx.send(common::AppEvent::ConversionProgressEvent(printer(1, cmd_desc)))?;

    let mut cmd = spawn_command(rt_executable, env_vars, cmd)?;

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

    match cmd.wait() {
        Ok(exit_status) => {
            if exit_status.success() {
                Ok(())
            } else {
                if let Some(exit_code) = exit_status.code() {
                    // https://www.containiq.com/post/exit-code-137
                    if exit_code == 139 || exit_code == 137 {
                        let mut explanation = trans.gettext("Container process terminated abruptly potentially due to memory usage. Are PDF pages too big? Try increasing the container engine memory allocation?");

                        // See https://www.containiq.com/post/sigsegv-segmentation-fault-linux-containers-exit-code-139
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

                Err(trans.gettext("Command failed!").into())
            }
        },
        Err(ex) => {
            Err(format!("{} {}", trans.gettext("Command failed!"), ex).into())
        }
    }

}

fn print_progress_plain(percent_complete: usize, data: String) -> String {
        format!("{}% {}", percent_complete, data)
}

fn print_progress_json(percent_complete: usize, data: String) -> String {
        let log_msg = &common::LogMessage {
            percent_complete, data
        };
        serde_json::to_string(log_msg).unwrap()
}

// TODO abstractions: Runtime Mode: InProc(NATIVE or CONTAINERIZED) or Remoting
//
// For the 'remoting' mode, this could be the Desktop front-end for the functionality implemented in entrusted-webclient
// - This could be talking to a load-balanced instance of entrusted-webserver somewhere (cloud, local network, etc.)
// - This would make synchronous or asynchronous network calls to sanitize documents
pub fn convert(input_path: PathBuf, output_path: PathBuf, convert_options: common::ConvertOptions, tx: Box<dyn common::EventSender>,  trans: l10n::Translations) -> Result<bool, Box<dyn Error>> {
    if !input_path.exists() {
        return Err(trans.gettext_fmt("The selected file does not exists: {0}!", vec![&input_path.display().to_string()]).into());
    }

    let printer: &dyn Fn(usize, String) -> String = if convert_options.log_format == *"plain" {
        &print_progress_plain
    } else {
        &print_progress_json
    };

    tx.send(common::AppEvent::ConversionProgressEvent(printer(1, format!("{} {}", trans.gettext("Converting"), input_path.display()))))?;

    let mut success = false;
    let mut err_msg = String::new();

    // TODO for Lima we assume the default VM instance, that might not be true all the time...
    if let Some(container_rt) = common::container_runtime_path() {        
        // TODO improve Flatpak detection and read relevant documentation
        let rt: Box<dyn SanitizerRt> = if env::var("FLATPAK_ID").is_err() {
            Box::new(ContainerizedSanitizerRt::new(container_rt.clone()))
        } else {
            Box::new(NativeSanitizerRt::new(container_rt.clone()))
        };

        // Check if we're ready to sanitize any document
        rt.install(convert_options.clone(), tx.clone_box(), printer, trans.clone())?;

        if let Err(ex) = rt.process(input_path, output_path, convert_options, tx.clone_box(), printer, trans) {
            err_msg.push_str(&ex.to_string());
            tx.send(common::AppEvent::ConversionProgressEvent(printer(100, err_msg.clone())))?;
        } else {
            success = true;
        }
    } else {
        err_msg.push_str(&trans.gettext("No container runtime executable found!"));
        err_msg.push('\n');

        if cfg!(any(target_os = "windows")) {
            err_msg.push_str(&trans.gettext("Please install Docker and make sure that it's running."));
        } else if cfg!(any(target_os = "macos")) {
            err_msg.push_str(&trans.gettext("Please install Docker Desktop and make sure that it's running."));
        } else { // Linux and others
            err_msg.push_str(&trans.gettext("Please install Docker or Podman, and make sure that it's running."));
        }

        tx.send(common::AppEvent::ConversionProgressEvent(printer(100, err_msg.clone())))?;
    }

    if success {
        Ok(success)
    } else {
        Err(err_msg.into())
    }
}
