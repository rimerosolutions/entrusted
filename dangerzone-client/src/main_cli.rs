use clap::{App, Arg};
use std::error::Error;
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::fs;
use std::thread;

mod common;
mod container;

fn main() -> Result<(), Box<dyn Error>> {
    let app = App::new(option_env!("CARGO_PKG_NAME").unwrap_or("Unknown"))
        .version(option_env!("CARGO_PKG_VERSION").unwrap_or("Unknown"))
        .author(option_env!("CARGO_PKG_AUTHORS").unwrap_or("Unknown"))
        .about(option_env!("CARGO_PKG_DESCRIPTION").unwrap_or("Unknown"))
        .arg(
            Arg::with_name("output-filename")
                .long("output-filename")
                .help("Optional output filename defaulting to <filename>-safe.pdf.")
                .required(false)
                .takes_value(true)
        ).arg(
            Arg::with_name("ocr-lang")
                .long("ocr-lang")
                .help("Optional language for OCR")
                .required(false)
                .takes_value(true)
        ).arg(
            Arg::with_name("input-filename")
                .long("input-filename")
                .help("Input filename")
                .takes_value(true)
                .required(true)
        ).arg(
            Arg::with_name("container-image-name")
                .long("container-image-name")
                .help("Optional custom Docker or Podman image name")
                .required(false)
                .takes_value(true)
        );

    let run_matches= app.to_owned().get_matches();

    let mut input_filename = "";
    let mut output_filename = PathBuf::from("");

    if let Some(proposed_input_filename) = run_matches.value_of("input-filename") {
        input_filename = proposed_input_filename;
    }

    if let Some(proposed_output_filename) = run_matches.value_of("output-filename") {
        output_filename = PathBuf::from(proposed_output_filename);
    }

    let mut ocr_lang = None;

    if let Some(proposed_ocr_lang) = &run_matches.value_of("ocr-lang") {
        let supported_ocr_languages = common::ocr_lang_key_by_name();

        if supported_ocr_languages.contains_key(proposed_ocr_lang) {
            ocr_lang = Some(format!("{}", proposed_ocr_lang));
        } else {
            let mut ocr_lang_err = "".to_string();
            ocr_lang_err.push_str(&format!("Unsupported language code for the ocr-lang parameter: {}. Hint: Try 'eng' for English. => ", proposed_ocr_lang));
            let mut prev = false;

            for (lang_code, language) in supported_ocr_languages {
                if !prev {
                    ocr_lang_err.push_str(&format!("{} ({})", lang_code, language));
                    prev = true;
                } else {
                    ocr_lang_err.push_str(&format!(", {} ({})", lang_code, language));
                }
            }

            return Err(ocr_lang_err.into());
        }
    }

    let src_path = fs::canonicalize(input_filename);
    let src_path_copy = fs::canonicalize(input_filename)?;
    let abs_output_filename = common::default_output_path(src_path?)?;

    if output_filename.file_name().is_none() {
        output_filename = abs_output_filename;
    }

    let container_image_name = match &run_matches.value_of("container-image-name") {
        Some(img) => Some(format!("{}", img)),
        None => None
    };

    let (tx, rx) = channel();

    let exec_handle = thread::spawn(move || {
        match container::convert(src_path_copy, output_filename, container_image_name, ocr_lang, tx) {
            Ok(_) => None,
            Err(ex) => Some(format!("{}", ex))
        }
    });

    for line in rx {
        println!("{}", line);
    }

    match exec_handle.join() {
        Ok(exec_result) => match exec_result {
            None => Ok(()),
            Some(msg) => Err(msg.into())
        },
        _ => Err("Conversion failed!".into())
    }
}
