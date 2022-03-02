use std::error::Error;
use std::fs;
use filetime::FileTime;
use std::path::PathBuf;
use std::env;
use std::process::{Command, Stdio};
use std::io::{BufReader, BufRead, Read};
use std::thread::JoinHandle;
use std::sync::mpsc::Sender;
use std::thread;

use crate::common;

fn read_output<R>(thread_name: &str, stream: R, tx: Sender<String>) -> Result<JoinHandle<Result<(), Box<dyn Error + Send + Sync>>>, std::io::Error>
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

fn exec_crt_command(container_program: common::ContainerProgram, args: Vec<&str>, tx: Sender<String>, capture_output: bool) -> Result<(), Box<dyn Error>> {
    let rt_path = container_program.exec_path;
    let sub_commands = container_program.sub_commands;
    let rt_executable: &str = &format!("{}", rt_path.display());

    let mut cmd = vec![];
    cmd.extend(sub_commands);
    cmd.extend(args.clone());

    tx.send(format!("\n[CMD_LOCAL]: {} {}", rt_executable, cmd.join(" ")))?;

    let mut child = Command::new(rt_executable)
        .args(cmd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    if capture_output {
        let stdout_handles = vec![
            read_output("dangerzone.stdout",  child.stdout.take().expect("!stdout"), tx.clone()),
            read_output("dangerzone.stderr" , child.stderr.take().expect("!stderr"), tx.clone()),
        ];

        for stdout_handle in stdout_handles {
            if let Err(ex) = stdout_handle?.join() {
                return Err(format!("Failed to capture command output! {:?}", ex).into());
            }
        }
    }

    let exit_status = child.wait()?;

    match exit_status.success() {
        true => Ok(()),
        false => Err("Failed to run command!".into())
    }
}

pub fn convert(input_path: PathBuf, output_path: PathBuf, ci_name: Option<String>, ocr_lang: Option<String>, tx: Sender<String>) -> Result<bool, Box<dyn Error>> {
    if !input_path.exists() {
        return Err(format!("The selected file {} does not exits!", input_path.display()).into());
    }

    tx.send(format!("Converting {}.", input_path.display()))?;

    let mut success = false;

    let ocr = if ocr_lang.is_some() {
        "1"
    } else {
        "0"
    };

    let ocr_language = match ocr_lang {
        Some(ocr_lang_val) => ocr_lang_val,
        None => "".to_string()
    };

    let mut container_image_name = match ci_name {
        Some(image_name) => image_name,
        None => String::from(common::CONTAINER_IMAGE_NAME)
    };

    if container_image_name.trim().is_empty() {
        container_image_name = String::from(common::CONTAINER_IMAGE_NAME);
    }

    fn mkdirp(p: PathBuf) -> Result<(), Box<dyn Error>> {
        if !p.exists() {
            let dir_created = fs::create_dir(p.clone());

            match dir_created {
                Err(ex) => {
                    Err(format!("Cannot create directory: {:?}! Error: {}", p, ex.to_string()).into())
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
        "none"
    ];

    let input_file_volume = &format!("{}:/tmp/input_file", input_path.display());
    let mut err_msg = "".to_string();

    // TODO for Lima we assume the default VM instance, that might not be true all the time...
    if let Some(container_rt) = common::container_runtime_path() {
        let mut ensure_image_args = vec!["inspect", container_image_name.as_str()];
        tx.send("Checking if dangerzone container image exists".to_string())?;

        if let Err(ex) = exec_crt_command(container_rt.clone(), ensure_image_args, tx.clone(), false) {
            tx.send(format!("Cannot find dangerzone container image! {}", ex.to_string()))?;
            ensure_image_args = vec!["pull", container_image_name.as_str()];
            tx.send("Please wait, downloading image...".to_string())?;

            if let Err(exe) = exec_crt_command(container_rt.clone(), ensure_image_args, tx.clone(), false) {
                tx.send("Could not download dangerzone container image!".to_string())?;
                return Err(exe.into());
            }

            tx.send("Download completed...".to_string())?;
        }

        let mut pixels_to_pdf_args = vec![];
        pixels_to_pdf_args.append(&mut run_args.clone());

        // TODO potentially this needs to be configurable
        // i.e. for Lima with assume that /tmp/lima is the configured writable folder...
        let mut dz_tmp = match container_rt.suggested_tmp_dir {
            Some(ref suggested_dir) => suggested_dir.clone(),
            None => env::temp_dir(),
        };
        dz_tmp.push("dangerzone");
        mkdirp(dz_tmp.clone())?;
        cleanup_dir(dz_tmp.clone().to_path_buf())?;

        let mut dz_tmp_safe:PathBuf = dz_tmp.clone();
        dz_tmp_safe.push("safe");
        mkdirp(dz_tmp_safe.clone())?;

        let mut dz_tmp_pixels:PathBuf = dz_tmp.clone();
        dz_tmp_pixels.push("pixels");
        mkdirp(dz_tmp_pixels.clone())?;

        let safedir_volume = &format!("{}:/safezone", dz_tmp_safe.display());
        let ocr_env = &format!("OCR={}", ocr);
        let ocr_language_env = &format!("OCR_LANGUAGE={}", ocr_language);

        pixels_to_pdf_args.append(&mut vec![
            "-v",
            input_file_volume,
            "-v",
            safedir_volume,
            "-e",
            ocr_env,
            "-e",
            ocr_language_env,
            container_image_name.as_str(),
            common::CONTAINER_IMAGE_EXE
        ]);

        if let Ok(_) = exec_crt_command(container_rt, pixels_to_pdf_args, tx, true) {
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
                eprintln!("WARNING: Failed to cleanup temporary folder {}! {}", dz_tmp.clone().display(), ex.to_string());
            }
            success = true;
        } else {
            err_msg = "Conversion failed!".to_string();
        }
    } else {
        err_msg.push_str("Cannot find any container runtime executable!\n");

        if cfg!(any(target_os="windows")) {
            err_msg.push_str("Please install Docker.");
        } else if cfg!(any(target_os="macos")) {
            err_msg.push_str("Please install Docker or Lima.");
        } else {
            err_msg.push_str("Please install Podman.");
        }
    }

    match success {
        true => Ok(success),
        _ => Err(err_msg.into())
    }
}
