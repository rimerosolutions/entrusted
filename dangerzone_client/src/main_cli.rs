use clap::{App, Arg};
use std::env;
use std::error::Error;
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::fs;
use std::thread;
use serde_json;
use indicatif::ProgressBar;
use std::collections::HashMap;
use dangerzone_l10n as l10n;

mod common;
mod config;
mod container;

const LOG_FORMAT_PLAIN: &str = "plain";
const LOG_FORMAT_JSON: &str  = "json";

fn main() -> Result<(), Box<dyn Error>> {
    l10n::load_translations(incl_gettext_files!("en", "fr"));

    let locale = match env::var(l10n::ENV_VAR_DANGERZONE_LANGID) {
        Ok(selected_locale) => selected_locale,
        Err(_) => l10n::sys_locale()
    };
    let trans = l10n::new_translations(locale);

    let app_config_ret = config::load_config();
    let app_config = app_config_ret.unwrap_or(config::AppConfig::default());

    let default_container_image_name = if let Some(img_name) = app_config.container_image_name.clone() {
        img_name
    } else {
        config::default_container_image_name()
    };

    let help_output_filename = trans.gettext("Optional output filename defaulting to <filename>-dgz.pdf.");
    let help_ocr_lang = trans.gettext("Optional language for OCR (i.e. 'eng' for English)");
    let help_input_filename = trans.gettext("Input filename");
    let help_container_image_name = trans.gettext("Optional custom Docker or Podman image name");
    let help_log_format = trans.gettext("Log format (json or plain)");
    let help_file_suffix = trans.gettext("Default file suffix (dgz)");
    
    let app = App::new(option_env!("CARGO_PKG_NAME").unwrap_or("Unknown"))
        .version(option_env!("CARGO_PKG_VERSION").unwrap_or("Unknown"))
        .author(option_env!("CARGO_PKG_AUTHORS").unwrap_or("Unknown"))
        .about(option_env!("CARGO_PKG_DESCRIPTION").unwrap_or("Unknown"))
        .arg(
            Arg::with_name("output-filename")
                .long("output-filename")
                .help(&help_output_filename)
                .required(false)
                .takes_value(true)
        ).arg(
            Arg::with_name("ocr-lang")
                .long("ocr-lang")
                .help(&help_ocr_lang)
                .required(false)
                .takes_value(true)
        ).arg(
            Arg::with_name("input-filename")
                .long("input-filename")
                .help(&help_input_filename)
                .takes_value(true)
                .required(true)
        ).arg(
            Arg::with_name("container-image-name")
                .long("container-image-name")
                .help(&help_container_image_name)
                .default_value(&default_container_image_name)
                .required(false)
                .takes_value(true)
        ).arg(
            Arg::with_name("log-format")
                .long("log-format")
                .help(&help_log_format)
                .possible_values(&[LOG_FORMAT_JSON, LOG_FORMAT_PLAIN])
                .default_value(LOG_FORMAT_PLAIN)
                .required(false)
                .takes_value(true)
        ).arg(
            Arg::with_name("file-suffix")
                .long("file-suffix")
                .help(&help_file_suffix)
                .default_value(&app_config.file_suffix)
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

    if !PathBuf::from(input_filename).exists() {
        return Err(trans.gettext_fmt("The selected file does not exists! {0}", vec![input_filename]).into());
    }

    let mut ocr_lang = None;

    if let Some(proposed_ocr_lang) = &run_matches.value_of("ocr-lang") {
        let supported_ocr_languages = common::ocr_lang_key_by_name(trans.clone_box());

        if supported_ocr_languages.contains_key(proposed_ocr_lang) {
            ocr_lang = Some(format!("{}", proposed_ocr_lang));
        } else {
            let mut ocr_lang_err = String::new();
            ocr_lang_err.push_str(&trans.gettext_fmt("Unknown language code for the ocr-lang parameter: {0}. Hint: Try 'eng' for English.", vec![proposed_ocr_lang]));

            ocr_lang_err.push_str(" => ");
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
    let file_suffix = if let Some(proposed_file_suffix) = &run_matches.value_of("file-suffix") {
        String::from(proposed_file_suffix.clone())
    } else {
        String::from(app_config.file_suffix.clone())
    };
    
    let abs_output_filename = common::default_output_path(src_path?, file_suffix)?;

    if output_filename.file_name().is_none() {
        output_filename = abs_output_filename;
    }

    let container_image_name = match &run_matches.value_of("container-image-name") {
        Some(img) => format!("{}", img),
        None => if let Some(container_image_name_saved) = app_config.container_image_name {
            container_image_name_saved.clone()
        } else {
            config::default_container_image_name()
        }
    };

    let log_format = match &run_matches.value_of("log-format") {
        Some(fmt) => fmt.to_string(),
        None => LOG_FORMAT_PLAIN.to_string()
    };

    let (tx, rx) = channel();

    let exec_handle = thread::spawn(move || {
        match container::convert(src_path_copy, output_filename, container_image_name, String::from(LOG_FORMAT_JSON), ocr_lang, tx, trans.clone_box()) {
            Ok(_) => None,
            Err(ex) => Some(format!("{}", ex))
        }
    });

    // Rendering a progressbar in plain mode
    if log_format == LOG_FORMAT_PLAIN.to_string() {
        let pb = ProgressBar::new(100);
        for line in rx {
            let log_msg_ret: serde_json::Result<common::LogMessage> = serde_json::from_slice(line.as_bytes());

            if let Ok(log_msg) = log_msg_ret {
                pb.set_position(log_msg.percent_complete as u64);
                pb.println(&log_msg.data);
            }
        }
    } else {
        for line in rx {
            println!("{}", line);
        }
    }

    let exit_code = {
        match exec_handle.join() {
            Ok(exec_result) => match exec_result {
                None => 0,
                Some(_) => 1
            },
            _ => 1
        }
    };

    std::process::exit(exit_code);
}
