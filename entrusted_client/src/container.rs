use serde_json;
use std::error::Error;
use std::fs;
use std::io;
use std::process::Child;
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

fn read_output<R>(thread_name: &str, stream: R, tx: Sender<common::AppEvent>) -> Result<JoinHandle<Result<(), Box<dyn Error + Send + Sync>>>, io::Error>
where
    R: Read + Send + 'static {
    thread::Builder::new()
        .name(thread_name.to_string())
        .spawn(move || {
            let reader = BufReader::new(stream);
            reader.lines()
                .try_for_each(|line| {
                    match tx.send(common::AppEvent::ConversionProgressEvent(line?)) {
                        Ok(())  => Ok(()),
                        Err(ex) => Err(ex.into())
                    }
                })
        })
}

fn read_output_no_capture<R>(thread_name: &str, stream: R, _: Sender<common::AppEvent>) -> Result<JoinHandle<Result<(), Box<dyn Error + Send + Sync>>>, io::Error>
where
    R: Read + Send + 'static {
    thread::Builder::new()
        .name(thread_name.to_string())
        .spawn(move || {
            let reader = BufReader::new(stream);
            reader.lines()
                .try_for_each(|_| {
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

fn exec_crt_command (container_program: common::ContainerProgram, args: Vec<String>, tx: Sender<common::AppEvent>, capture_output: bool, printer: Box<dyn LogPrinter>, trans: l10n::Translations) -> Result<(), Box<dyn Error>> {
    let rt_path = container_program.exec_path;
    let sub_commands = container_program.sub_commands.iter().map(|i| i.to_string());
    let rt_executable: &str = &format!("{}", rt_path.display());

    let mut cmd = Vec::with_capacity(sub_commands.len() + args.len());
    cmd.extend(sub_commands);
    cmd.extend(args.clone());

    let cmd_masked: Vec<&str> = cmd
        .iter()
        .map(|i| {
            if i.contains(common::ENV_VAR_ENTRUSTED_DOC_PASSWD) {
                "***"
            } else {
                i
            }
        }).collect();

    let masked_cmd = cmd_masked.join(" ");

    tx.send(common::AppEvent::ConversionProgressEvent(printer.print(1, trans.gettext_fmt("Running command: {0}", vec![&format!("{} {}", rt_executable, masked_cmd)]))))?;

    let mut cmd = spawn_command(rt_executable, cmd)?;

    
    let stdout_handles = if capture_output {
        vec![
            read_output("entrusted.stdout",  cmd.stdout.take().expect("!stdout"), tx.clone()),
            read_output("entrusted.stderr" , cmd.stderr.take().expect("!stderr"), tx.clone()),
        ]
    } else {
        vec![
            read_output_no_capture("entrusted.stdout",  cmd.stdout.take().expect("!stdout"), tx.clone()),
            read_output_no_capture("entrusted.stderr" , cmd.stderr.take().expect("!stderr"), tx.clone()),
        ]        
    };

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
                    return Err(trans.gettext("Command failed!").into());
                }
            },
            Ok(None) => {
                thread::yield_now();
            },
            Err(_) => {
                return Err(trans.gettext("Command failed!").into());
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

pub fn convert(input_path: PathBuf, output_path: PathBuf, convert_options: common::ConvertOptions, tx: Sender<common::AppEvent>, trans: l10n::Translations) -> Result<bool, Box<dyn Error>> {
    if !input_path.exists() {
        return Err(trans.gettext_fmt("The selected file does not exists: {0}!", vec![&input_path.display().to_string()]).into());
    }

    let printer: Box<dyn LogPrinter> = if convert_options.log_format == "plain".to_string() {
        Box::new(PlainLogPrinter)
    } else {
        Box::new(JsonLogPrinter)
    };

    tx.send(common::AppEvent::ConversionProgressEvent(printer.print(1, format!("{} {}.", trans.gettext("Converting"), input_path.display()))))?;

    let mut success = false;

    fn mkdirp(p: PathBuf, trans: l10n::Translations) -> Result<(), Box<dyn Error>> {
        if !p.exists() {
            if let Err(ex) = fs::create_dir(&p) {
                return Err(trans.gettext_fmt("Cannot create directory: {0}! Error: {1}", vec![&p.display().to_string(), &ex.to_string()]).into());
            }
        }

        Ok(())
    }

    pub fn cleanup_dir(dir: PathBuf) -> Result<(), Box<dyn Error>> {
        if dir.exists() && dir.is_dir() {
            let mut files = vec![dir];

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
        "--network".to_string(),
        "none".to_string(),
        "--security-opt".to_string(),
        "label=disable".to_string()
    ];

    let input_file_volume = format!("{}:/tmp/input_file", input_path.display());
    let mut err_msg = String::new();

    // TODO for Lima we assume the default VM instance, that might not be true all the time...
    if let Some(container_rt) = common::container_runtime_path() {
        let mut ensure_image_args = vec!["inspect".to_string(), convert_options.container_image_name.to_owned()];

        tx.send(common::AppEvent::ConversionProgressEvent(printer.print(1, trans.gettext("Checking if container image exists"))))?;

        if let Err(ex) = exec_crt_command(container_rt.clone(), ensure_image_args, tx.clone(), false, printer.clone_box(), trans.clone()) {
            tx.send(common::AppEvent::ConversionProgressEvent(printer.print(1, trans.gettext_fmt("The container image was not found. {0}", vec![&ex.to_string()]))))?;
            ensure_image_args = vec!["pull".to_string(), convert_options.container_image_name.to_owned()];
            tx.send(common::AppEvent::ConversionProgressEvent(printer.print(1, trans.gettext("Please wait, downloading image (roughly 600 MB)."))))?;

            if let Err(exe) = exec_crt_command(container_rt.clone(), ensure_image_args, tx.clone(), false, printer.clone_box(), trans.clone()) {
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
        mkdirp(dz_tmp.clone(), trans.clone())?;
        cleanup_dir(dz_tmp.clone().to_path_buf())?;

        let mut dz_tmp_safe:PathBuf = dz_tmp.clone();
        dz_tmp_safe.push("safe");
        mkdirp(dz_tmp_safe.clone(), trans.clone())?;

        let safedir_volume = format!("{}:/safezone", dz_tmp_safe.display());

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

        if let Ok(_) = exec_crt_command(container_rt, convert_args, tx.clone(), true, printer.clone_box(), trans.clone()) {
            if output_path.exists() {
                fs::remove_file(output_path.clone())?;
            }

            let mut container_output_file_path = dz_tmp_safe.clone();
            container_output_file_path.push("safe-output-compressed.pdf");
            let atime = FileTime::now();
            let output_path_clone = output_path.clone();

            fs::copy(&container_output_file_path, output_path)?;
            fs::remove_file(container_output_file_path)?;

            let output_file = fs::File::create(output_path_clone)?;
            filetime::set_file_handle_times(&output_file, Some(atime), Some(atime))?;

            if let Err(ex) = cleanup_dir(dz_tmp.clone().to_path_buf()) {
                tx.send(common::AppEvent::ConversionProgressEvent(printer.print(100, trans.gettext_fmt("Failed to cleanup temporary folder: {0}. {1}.", vec![&dz_tmp.clone().display().to_string(), &ex.to_string()]))))?;
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

    if success {
        Ok(success)
    } else {
        tx.send(common::AppEvent::ConversionProgressEvent(printer.print(100, err_msg.clone())))?;
        Err(err_msg.into())
    }
}
