use clap::{Command, Arg, ArgAction, builder::PossibleValue};
use std::env;
use std::error::Error;
use std::path::PathBuf;
use std::fs;
use std::sync::mpsc;
use std::thread;

use indicatif::ProgressBar;

use std::collections::HashMap;
use once_cell::sync::OnceCell;
use entrusted_l10n as l10n;

mod common;
mod config;
mod container;

const LOG_FORMAT_PLAIN: &str = "plain";

static INSTANCE_DEFAULT_IMAGE: OnceCell<String> = OnceCell::new();
static INSTANCE_FILESUFFIX: OnceCell<String> = OnceCell::new();

#[derive(Clone)]
struct CliEventSender {
    tx: mpsc::Sender<common::AppEvent>
}

impl common::EventSender for CliEventSender {
    fn send(&self, evt: crate::common::AppEvent) -> Result<(), mpsc::SendError<crate::common::AppEvent>> {
        self.tx.send(evt)
    }

    fn clone_box(&self) -> Box<dyn common::EventSender> {
        Box::new(self.clone())
    }
}

fn default_container_image_to_str() -> &'static str {
    INSTANCE_DEFAULT_IMAGE.get().expect("Image value not set!")
}

fn filesuffix_to_str() -> &'static str {
    INSTANCE_FILESUFFIX.get().expect("filesuffix value not set!")
}

fn main() -> Result<(), Box<dyn Error>> {
    l10n::load_translations(incl_gettext_files!("en", "fr"));

    let locale = if let Ok(selected_locale) = env::var(l10n::ENV_VAR_ENTRUSTED_LANGID) {
        selected_locale
    } else {
        l10n::sys_locale()
    };

    let trans = l10n::new_translations(locale);
    let app_config_ret = config::load_config();
    let app_config: config::AppConfig = app_config_ret.unwrap_or_default();

    let default_container_image_name = if let Some(img_name) = app_config.container_image_name.clone() {
        img_name
    } else {
        config::default_container_image_name()
    };

    let help_output_filename = trans.gettext("Optional output filename defaulting to <filename>-entrusted.pdf.");
    let help_ocr_lang = trans.gettext("Optional language for OCR (i.e. 'eng' for English)");
    let help_input_filename = trans.gettext("Input filename");
    let help_visual_quality = trans.gettext("PDF result visual quality");
    let help_container_image_name = trans.gettext("Optional custom Docker or Podman image name");
    let help_log_format = trans.gettext("Log format (json or plain)");
    let help_file_suffix = trans.gettext("Default file suffix (entrusted)");
    let help_password_prompt = trans.gettext("Prompt for document password");
    let help_update_checks = trans.gettext("Check for updates");
    
    let cmd_help_template = trans.gettext(&format!("{}\n{}\n{}\n\n{}\n\n{}\n{}", 
                                                  "{bin} {version}",
                                                  "{author}",
                                                  "{about}",
                                                  "Usage: {usage}",
                                                  "Options:",
                                                  "{options}"));
    
    INSTANCE_DEFAULT_IMAGE.set(default_container_image_name)?;
    INSTANCE_FILESUFFIX.set(app_config.file_suffix.to_string())?;
    
    let app = Command::new(option_env!("CARGO_PKG_NAME").unwrap_or("Unknown"))
        .version(option_env!("CARGO_PKG_VERSION").unwrap_or("Unknown"))
        .help_template(&cmd_help_template)
        .author(option_env!("CARGO_PKG_AUTHORS").unwrap_or("Unknown"))
        .about(option_env!("CARGO_PKG_DESCRIPTION").unwrap_or("Unknown"))
        .arg(
            Arg::new("output-filename")
                .long("output-filename")
                .help(&help_output_filename)
                .required(false)
        ).arg(
            Arg::new("ocr-lang")
                .long("ocr-lang")
                .help(&help_ocr_lang)
                .required(false)
        ).arg(
            Arg::new("update-checks")
                .long("update-checks")
                .help(&help_update_checks)
                .required(false)
                .action(ArgAction::SetTrue)
        ).arg(
            Arg::new("input-filename")
                .long("input-filename")
                .help(&help_input_filename)
                .required_unless_present("update-checks")
        ).arg(
            Arg::new("container-image-name")
                .long("container-image-name")
                .help(&help_container_image_name)
                .default_value(default_container_image_to_str())
                .required(false)
        ).arg(
            Arg::new("log-format")
                .long("log-format")
                .help(&help_log_format)
                .value_parser([
                    PossibleValue::new(common::LOG_FORMAT_JSON), 
                    PossibleValue::new(LOG_FORMAT_PLAIN)                
                ])
                .default_value(LOG_FORMAT_PLAIN)
                .required(false)
        ).arg(
            Arg::new("file-suffix")
                .long("file-suffix")
                .help(&help_file_suffix)
                .default_value(filesuffix_to_str())
                .required(false)
        ).arg(
            Arg::new("visual-quality")
                .long("visual-quality")
                .help(&help_visual_quality)
                .required(false)                
                .value_parser([
                    PossibleValue::new(common::IMAGE_QUALITY_CHOICES[0]),
                    PossibleValue::new(common::IMAGE_QUALITY_CHOICES[1]),
                    PossibleValue::new(common::IMAGE_QUALITY_CHOICES[2]),                    
                ])
                .default_value(common::IMAGE_QUALITY_CHOICES[common::IMAGE_QUALITY_DEFAULT_CHOICE_INDEX as usize])
        ).arg(
            Arg::new("passwd-prompt")
                .long("passwd-prompt")
                .help(&help_password_prompt)
                .required(false)
                .action(ArgAction::SetTrue)
        );

    let run_matches= app.get_matches();

    if run_matches.get_flag("update-checks") {
        match common::update_check(&trans) {
            Ok(opt_new_release) => {
                if let Some(new_release) = opt_new_release {
                    println!("{}", trans.gettext_fmt("Version {0} is out!\nPlease visit {1}", vec![&new_release.tag_name, &new_release.html_url]));
                } else {
                    println!("{}", trans.gettext("No updates available at this time!"));
                }
            },
            Err(ex) => {
                let err_text = trans.gettext_fmt("Could not check for updates, please try later.\n{0}", vec![&ex.to_string()]);
                eprintln!("{}", err_text);
            }
        }

        return Ok(());
    }

    let mut input_filename = "";
    let mut output_filename = PathBuf::from("");

    if let Some(proposed_input_filename) = run_matches.get_one::<String>("input-filename") {
        input_filename = proposed_input_filename;
    }

    if let Some(proposed_output_filename) = run_matches.get_one::<String>("output-filename") {
        output_filename = PathBuf::from(proposed_output_filename);
    }

    if fs::metadata(input_filename).is_err() {
        return Err(trans.gettext_fmt("The selected file does not exists! {0}", vec![input_filename]).into());
    }

    let mut ocr_lang = None;

    if let Some(proposed_ocr_lang) = &run_matches.get_one::<String>("ocr-lang") {
        let supported_ocr_languages = l10n::ocr_lang_key_by_name(&trans);
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
        
        ocr_lang = Some(proposed_ocr_lang.to_string());
    }

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
        app_config.file_suffix.to_string()
    };

    let abs_output_filename = common::default_output_path(src_path.clone(), file_suffix)?;

    if output_filename.file_name().is_none() {
        output_filename = abs_output_filename;
    }

    let container_image_name = match &run_matches.get_one::<String>("container-image-name") {
        Some(img) => img.to_string(),
        None      => if let Some(container_image_name_saved) = app_config.container_image_name {
            container_image_name_saved
        } else {
            config::default_container_image_name()
        }
    };

    let log_format = if let Some(v) = &run_matches.get_one::<String>("log-format") {
        v
    } else {
        LOG_FORMAT_PLAIN
    };
    
    let image_quality = if let Some(v) = &run_matches.get_one::<String>("visual-quality") {
        v.to_string()
    } else {
        app_config.visual_quality
    };

    let opt_passwd = if run_matches.get_flag("passwd-prompt") {
        // Simplification for password entry from the Web interface and similar non-TTY use-cases
        if let Ok(env_passwd) = env::var("ENTRUSTED_AUTOMATED_PASSWORD_ENTRY") {
            Some(env_passwd)
        } else {
            println!("{}", trans.gettext("Please enter the password for the document"));

            if let Ok(passwd) = rpassword::read_password() {
                Some(passwd)
            } else {
                return Err(trans.gettext("Failed to read password!").into());
            }
        }
    }  else {
        None
    };

    let (exec_handle, rx) = {
        let (tx, rx) = mpsc::channel::<common::AppEvent>();

        let exec_handle = thread::spawn({
            move || {
                let convert_options = common::ConvertOptions::new(container_image_name, common::LOG_FORMAT_JSON.to_string(), image_quality, ocr_lang, opt_passwd);
                let eventer = Box::new(CliEventSender {
                    tx
                });

                if let Err(ex) = container::convert(src_path.clone(), output_filename, convert_options, eventer, trans) {
                    Some(ex.to_string())
                } else {
                    None
                }
            }
        });
        (exec_handle, rx)
    };

    // Rendering a progressbar in plain mode
    if log_format == LOG_FORMAT_PLAIN {
        let pb = ProgressBar::new(100);

        for line in rx {
            if let common::AppEvent::ConversionProgressEvent(msg) = line {
                if let Ok(log_msg) = serde_json::from_slice::<common::LogMessage>(msg.as_bytes()) {
                    pb.set_position(log_msg.percent_complete as u64);
                    pb.println(&log_msg.data);
                }
            }
        }
    } else {
        for line in rx {
            if let common::AppEvent::ConversionProgressEvent(msg) = line {
                println!("{}", msg);
            }
        }
    }

    let exit_code = {
        if let Ok(exec_result) = exec_handle.join() {
            if exec_result.is_none() {
                0
            } else {
                1
            }
        } else {
            1
        }
    };

    std::process::exit(exit_code);
}
