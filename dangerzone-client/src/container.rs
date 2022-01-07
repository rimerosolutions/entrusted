use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::env;
use crate::common;
use std::process::{Command, Stdio};

fn exec_container(args: Vec<&str>, _stdout_cb: fn(String)) -> Result<bool, Box<dyn Error>> {
    if let Some(rt_path) = common::container_runtime_path() {
        let rt_executable: &str = &format!("{}", rt_path.display());
        let mut cmd = vec![rt_executable];
        cmd.extend(args.clone());

        println!("\n[CMD_LOCAL]: {}", cmd.join(" "));

        let exit_status = Command::new(rt_executable)
            .args(args)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .output()
            .expect("Cannot run command!");

        if exit_status.status.success() {
            return Ok(true);
        }

        return Err("Cannot run command inside container!".into());
    }
    
    Err("Cannot find container runtime executable!".into())
}

pub fn convert(input_path: PathBuf, output_path: PathBuf, ci_name: Option<&str>, ocr_lang: Option<&str>, stdout_cb: fn(String)) -> Result<bool, Box<dyn Error>> {
    println!("Converting {}", input_path.display());

    let mut success = false;

    let ocr = if ocr_lang.is_some() {
        "1"
    } else {
        "0"
    };

    let ocr_language = match ocr_lang {
        Some(ocr_lang_val) => ocr_lang_val,
        None => ""
    };

    let container_image_name = match ci_name {
        Some(image_name) => image_name,
        None => common::CONTAINER_IMAGE_NAME
    };

    fn mkdirp(p: PathBuf) -> Result<(), Box<dyn Error>> {
        if !p.exists() {
            println!("Creating folder: {:?}", p.clone());
            let dir_created = fs::create_dir(p.clone());

            match dir_created {
                Err(ex) => {
                    let msg = format!("Cannot create directory: {:?}! Error: {}", p, ex.to_string());
                    Err(msg.into())
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

    let mut dz_tmp = env::temp_dir();
    dz_tmp.push("dangerzone");
    mkdirp(dz_tmp.clone())?;

    cleanup_dir(dz_tmp.clone().to_path_buf())?;

    let mut dz_tmp_pixels:PathBuf = dz_tmp.clone();
    dz_tmp_pixels.push("pixels");
    mkdirp(dz_tmp_pixels.clone())?;

    let mut dz_tmp_safe:PathBuf = dz_tmp.clone();
    dz_tmp_safe.push("safe");
    mkdirp(dz_tmp_safe.clone())?;

    let platform_args = if matches!(common::container_runtime_tech(), common::ContainerRt::DOCKER) {
        vec!["--platform", "--linux/amd64"]
    } else {
        vec![]
    };

    let run_args:Vec<&str> = vec![
        "run",
        "--network",
        "none"
    ];

    let mut cr_args:Vec<&str> = vec![];
    cr_args.append(&mut run_args.clone());
    cr_args.append(&mut platform_args.clone());
    let input_file_volume = &format!("{}:/tmp/input_file", input_path.display());
    let pixels_volume = &format!("{}:/dangerzone", dz_tmp_pixels.display());
    cr_args.append(&mut vec![
        "-v",
        input_file_volume,
        "-v",
        pixels_volume,
        container_image_name,
        common::CONTAINER_IMAGE_EXE,
        "document-to-pixels"
    ]);

    let mut ret = exec_container(cr_args, stdout_cb);
    let mut err_msg = "document-to-pixels failed!".to_string();

    if let Ok(true) = ret {
        let mut pixels_to_pdf_args = vec![];
        pixels_to_pdf_args.append(&mut run_args.clone());
        pixels_to_pdf_args.append(&mut platform_args.clone());
        let dangerzone_volume = &format!("{}:/dangerzone", dz_tmp_pixels.display());
        let safedir_volume = &format!("{}:/safezone", dz_tmp_safe.display());
        let ocr_env = &format!("OCR={}", ocr);
        let ocr_language_env = &format!("OCR_LANGUAGE={}", ocr_language);

        pixels_to_pdf_args.append(&mut vec![
            "-v",
            dangerzone_volume,
            "-v",
            safedir_volume,
            "-e",
            ocr_env,
            "-e",
            ocr_language_env,
            container_image_name,
            common::CONTAINER_IMAGE_EXE,
            "pixels-to-pdf"
        ]);

        ret = exec_container(pixels_to_pdf_args, stdout_cb);

        match ret {
            Ok(false) => {
                err_msg = "pixels-to-pdf failed!".to_string();
            },
            Ok(true) => {
                success = true;
            },
            Err(ee) => {
                err_msg = ee.to_string();
            }
        }

        if success {
            println!("Removing file: {}", output_path.display());

            if output_path.exists() {
                fs::remove_file(output_path.clone())?;
            }

            let mut container_output_file_path = dz_tmp_safe.clone();
            container_output_file_path.push("safe-output-compressed.pdf");

            let output_src_filename = format!("{}", container_output_file_path.as_path().display());
            println!("Moving compressed file from {} to {}", output_src_filename, output_path.display());

            // TODO change created/modified times (rust 'lifetime' crate)
            move_file(container_output_file_path, output_path)?;
        }
    }

    fn move_file(src: PathBuf, dest: PathBuf) -> Result<(), Box<dyn Error>> {
        // We don't use rename because of couple of issues (across different filesystems)
        // For example moving file across mounts with different filesystems (regular fs, overlayfs, etc.)
        fs::copy(&src, dest)?;
        fs::remove_file(src)?;

        Ok(())
    }

    cleanup_dir(dz_tmp.clone().to_path_buf())?;

    match success {
        true => Ok(success),
        _ => Err(err_msg.into())
    }
}
