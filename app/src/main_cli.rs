use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::path::PathBuf;
use std::fs;
use std::sync::{atomic, mpsc, Arc};
use std::thread;
use std::str::FromStr;

use clap::{Command, Arg, ArgAction, ArgMatches};
use indicatif::ProgressBar;
use uuid::Uuid;
use mimalloc::MiMalloc;

mod l10n;
mod common;
mod error;
mod config;
mod processing;
mod platform;
mod sanitizer;

use crate::common::VisualQuality;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[derive(Clone)]
struct CliEventSender {
    tx: mpsc::Sender<common::AppEvent>
}

impl common::EventSender for CliEventSender {
    fn send(&self, evt: crate::common::AppEvent) -> Result<(), mpsc::SendError<crate::common::AppEvent>> {
        self.tx.send(evt)
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    let exe_path = PathBuf::from(&args[0]);

    l10n::load_translations(incl_gettext_files!("en", "fr"));
    let locale = env::var(l10n::ENV_VAR_ENTRUSTED_LANGID).unwrap_or(l10n::sys_locale());
    let trans = l10n::new_translations(locale);

    let app_config_ret = config::load_config();
    let app_config: config::AppConfig = app_config_ret.unwrap_or_default();

    let run_matches = cli_build_args(&app_config, &trans);

    let (src_path, output_folder, filename_suffix, visual_quality, ocr_lang_code, password_decrypt, password_encrypt) = cli_parse_args(run_matches, &app_config,  &trans)?;

    let sanitizer = sanitizer::Sanitizer::new(platform::resolve_sanitizer_settings(exe_path));

    let (exec_handle, rx) = {
        let (tx, rx) = mpsc::channel::<common::AppEvent>();

        let exec_handle = thread::spawn({
            move || {
                let convert_options = common::ConvertOptions::new(output_folder, filename_suffix, visual_quality, ocr_lang_code, password_decrypt, password_encrypt);
                let eventer = Box::new(CliEventSender {
                    tx
                });

                let stop_signal = Arc::new(atomic::AtomicBool::new(false));

                if let Err(ex) = sanitizer.sanitize(Uuid::new_v4(), src_path.clone(), convert_options, eventer, trans, stop_signal) {
                    Some(ex.to_string())
                } else {
                    None
                }
            }
        });
        (exec_handle, rx)
    };

    let pb = ProgressBar::new(100);

    for line in rx {
        if let common::AppEvent::ConversionProgressed(_, increment, msg) = line {
            pb.set_position(increment as u64);
            pb.println(&msg);
        }
    }

    let exit_code = if let Ok(exec_result) = exec_handle.join() {
        i32::from(exec_result.is_some())
    } else {
        1
    };

    std::process::exit(exit_code);
}

fn cli_build_args(app_config: &config::AppConfig, trans: &l10n::Translations) -> ArgMatches {
    let help_output_filename = trans.gettext("Optional output filename defaulting to <filename>-entrusted.pdf.");
    let help_ocr_lang = trans.gettext("Optional language for OCR (i.e. 'eng' for English)");
    let help_input_filename = trans.gettext("Input filename");
    let help_visual_quality = trans.gettext("PDF result visual quality");
    let help_file_suffix = trans.gettext("Default file suffix (entrusted)");
    let help_password_prompt = trans.gettext("Prompt for document password");
    
    let cmd_help_template = trans.gettext(&format!("{}\n{}\n{}\n\n{}\n\n{}\n{}",
                                                   "{bin} {version}",
                                                   "{author}",
                                                   "{about}",
                                                   "Usage: {usage}",
                                                   "Options:",
                                                   "{options}"));

    let default_file_suffix = app_config.file_suffix.clone().unwrap();

    let app = Command::new(option_env!("CARGO_PKG_NAME").unwrap_or("Unknown"))
        .version(option_env!("CARGO_PKG_VERSION").unwrap_or("Unknown"))
        .help_template(cmd_help_template)
        .author(option_env!("CARGO_PKG_AUTHORS").unwrap_or("Unknown"))
        .about(option_env!("CARGO_PKG_DESCRIPTION").unwrap_or("Unknown"))
        .arg(
            Arg::new("output-folder")
                .long("output-folder")
                .help(help_output_filename)
                .required(false)
        ).arg(
            Arg::new("ocr-lang")
                .long("ocr-lang")
                .help(help_ocr_lang)
                .required(false)
        ).arg(
            Arg::new("input-filename")
                .long("input-filename")
                .help(help_input_filename)
                .required(true)
        ).arg(
            Arg::new("file-suffix")
                .long("file-suffix")
                .help(help_file_suffix)
                .default_value(default_file_suffix)
                .required(false)
        ).arg(
            Arg::new("visual-quality")
                .long("visual-quality")
                .help(help_visual_quality)
                .required(false)
                .value_parser([
                    common::VisualQuality::Low.to_string().to_lowercase(),
                    common::VisualQuality::Medium.to_string().to_lowercase(),
                    common::VisualQuality::High.to_string().to_lowercase(),
                ])
                .default_value(common::VisualQuality::default_value().to_string().to_lowercase())
        ).arg(
            Arg::new("decryption-password")
                .long("decryption-password")
                .help(&help_password_prompt)
                .required(false)
                .action(ArgAction::SetTrue)
        ).arg(
            Arg::new("encryption-password")
                .long("encryption-password")
                .help(&help_password_prompt)
                .required(false)
                .action(ArgAction::SetTrue)
        );

    app.get_matches()
}

fn cli_parse_args(run_matches: ArgMatches, app_config: &config::AppConfig, trans: &l10n::Translations) -> Result<(PathBuf, Option<PathBuf>, String, VisualQuality, Option<String>, Option<String>, Option<String>), Box<dyn Error>> {
    let mut input_filename = "";
    let mut output_folder_opt = None;

    if let Some(proposed_input_filename) = run_matches.get_one::<String>("input-filename") {
        input_filename = proposed_input_filename;
    }

    if let Some(proposed_output_folder) = run_matches.get_one::<String>("output-folder") {
        output_folder_opt = Some(PathBuf::from(proposed_output_folder));
    }

    if fs::metadata(input_filename).is_err() {
        return Err(trans.gettext_fmt("The input file does not exists! {0}", vec![input_filename]).into());
    }

    let ocr_lang = {
        if let Some(proposed_ocr_lang) = &run_matches.get_one::<String>("ocr-lang") {
            let supported_ocr_languages = l10n::ocr_lang_key_by_name(trans);
            let selected_langcodes: Vec<&str> = proposed_ocr_lang.split('+').collect();

            for selected_langcode in selected_langcodes {
                if !supported_ocr_languages.contains_key(&selected_langcode) {
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

            Some(proposed_ocr_lang.to_string())
        } else {
            None
        }
    };

    // Deal transparently with Windows UNC path returned by fs::canonicalize
    // std::fs::canonicalize returns UNC paths on Windows, and a lot of software doesn't support UNC paths
    // This is problematic with Docker and mapped volumes for this application
    // See https://github.com/rust-lang/rust/issues/42869
    let src_path = {
        #[cfg(not(target_os = "windows"))] {
            std::fs::canonicalize(input_filename)?
        }
        #[cfg(target_os = "windows")] {
            dunce::canonicalize(input_filename)?
        }
    };

    let file_suffix = if let Some(proposed_file_suffix) = &run_matches.get_one::<String>("file-suffix") {
        proposed_file_suffix.to_string()
    } else {
        app_config.file_suffix.clone().unwrap_or_else(|| common::DEFAULT_FILE_SUFFIX.to_string())
    };

    let output_folder = output_folder_opt.clone();

    let image_quality = {
        let ret = if let Some(v) = &run_matches.get_one::<String>("visual-quality") {
            v.to_string()
        } else {
            common::VisualQuality::default_value().to_string().to_lowercase()
        };
        VisualQuality::from_str(&ret).unwrap()
    };

    let password_decrypt = if run_matches.get_flag("decryption-password") {
        // Simplification for password entry from the Web interface and similar non-TTY use-cases
        if let Ok(env_passwd) = env::var("ENTRUSTED_AUTOMATED_PASSWORD_ENTRY") {
            Some(env_passwd)
        } else {
            println!("{}", trans.gettext("Please enter the password for decrypting the input file"));

            if let Ok(passwd) = rpassword::read_password() {
                Some(passwd)
            } else {
                return Err(trans.gettext("Failed to read password!").into());
            }
        }
    }  else {
        None
    };

    let password_encrypt = if run_matches.get_flag("encryption-password") {        
        println!("{}", trans.gettext("Please enter the password for encrypting the output file"));

        if let Ok(passwd) = rpassword::read_password() {
            Some(passwd)
        } else {
            return Err(trans.gettext("Failed to read password!").into());
        }
    }  else {
        None
    };

    Ok((src_path, output_folder, file_suffix, image_quality, ocr_lang, password_decrypt, password_encrypt))
}
