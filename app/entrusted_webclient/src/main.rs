use clap::{Command, Arg, ArgAction, builder::PossibleValue};
use futures::stream::StreamExt;
use reqwest::{Body, Client};
use reqwest_eventsource::{Event, EventSource};
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use std::io::Write;
use indicatif::ProgressBar;
use rpassword;
use std::env;
use once_cell::sync::OnceCell;

use std::{
    fs,
    error::Error,
    path::PathBuf,
};

use serde::de::DeserializeOwned;

use dirs;

use entrusted_l10n as l10n;

const PROGRAM_GROUP: &str = "com.rimerosolutions.entrusted.entrusted_webclient";
const CFG_FILENAME: &str = "config.toml";

pub const IMAGE_QUALITY_CHOICES: [&str; 3] = ["low", "medium", "high"];
pub const IMAGE_QUALITY_DEFAULT_CHOICE_INDEX: usize = 1;

static INSTANCE_HOST: OnceCell<String> = OnceCell::new();
static INSTANCE_PORT: OnceCell<String> = OnceCell::new();
static INSTANCE_FILESUFFIX: OnceCell<String> = OnceCell::new();

fn host_to_str() -> &'static str {
    &INSTANCE_HOST.get().expect("host value not set!")
}

fn port_to_str() -> &'static str {
    &INSTANCE_PORT.get().expect("port value not set!")
}

fn filesuffix_to_str() -> &'static str {
    &INSTANCE_FILESUFFIX.get().expect("filesuffix value not set!")
}

macro_rules! incl_gettext_files {
    ( $( $x:expr ),* ) => {
        {
            let mut ret = HashMap::with_capacity(2);
            $(
                let data = include_bytes!(concat!("../translations/", $x, "/LC_MESSAGES/messages.mo")).as_slice();
                ret.insert($x, data);

            )*

                ret
        }
    };
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct AppConfig {
    #[serde(rename(serialize = "ocr-lang", deserialize = "ocr-lang"))]
    pub ocr_lang: Option<String>,
    pub host: String,
    pub port: u16,
    #[serde(rename(serialize = "file-suffix", deserialize = "file-suffix"))]
    pub file_suffix: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            ocr_lang: None,
            host: "localhost".to_string(),
            port: 13000,
            file_suffix: "entrusted".to_string()
        }
    }
}

fn load_config <T> () -> Result<T, Box<dyn Error>> where T: Default + DeserializeOwned {
    if let Some(config_dir) = dirs::config_dir() {
        let config_dir_dgz = config_dir.join(PROGRAM_GROUP);

        if config_dir_dgz.exists() {
            let config_path = config_dir_dgz.join(CFG_FILENAME);

            if config_path.exists() {
                let ret = {
                    let config_data = fs::read(&config_path)?;
                    toml::from_slice(&config_data)
                };

                if let Ok(data) = ret {
                    return Ok(data);
                }
            }
        }
    }

    Ok(T::default())
}

#[derive(Clone, Debug)]
pub struct ConversionOptions {
    pub host: String,
    pub port: String,
    pub image_quality: String,
    pub opt_ocr_lang: Option<String>,
    pub opt_passwd: Option<String>,
    pub file_suffix: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct UploadResponse {
    pub request_id: String,
    pub tracking_uri: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct DownloadResponse {
    pub id: String,
    pub data: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct LogMessage {
    pub data: String,
    pub percent_complete: usize,
}

#[tokio::main]
async fn main() {
    l10n::load_translations(incl_gettext_files!("en", "fr"));

    if let Err(ex) = process_cli_args().await {
        eprintln!("{}", ex.to_string());
        std::process::exit(1);
    }
}

async fn process_cli_args() -> Result<(), Box<dyn Error + Send + Sync>> {
    let locale = if let Ok(selected_locale) = env::var(l10n::ENV_VAR_ENTRUSTED_LANGID) {
        selected_locale
    } else {
        l10n::sys_locale()
    };

    let trans = l10n::new_translations(locale);

    let appconfig_ret = load_config();
    let appconfig = appconfig_ret.unwrap_or(AppConfig::default());

    let help_host = trans.gettext("Server host or IP address");
    let help_port = trans.gettext("Server port number");
    let help_output_filename = trans.gettext("Output filename");
    let help_input_filename = trans.gettext("Input filename");
    let help_ocr_lang = trans.gettext("Optional language for OCR (i.e. 'eng' for English)");
    let help_file_suffix = trans.gettext("Default file suffix (entrusted)");
    let help_password_prompt = trans.gettext("Prompt for document password");
    let help_visual_quality = trans.gettext("PDF result visual quality");
    
    INSTANCE_HOST.set(appconfig.host.to_owned())?;
    INSTANCE_PORT.set(appconfig.port.to_string())?;
    INSTANCE_FILESUFFIX.set(appconfig.file_suffix.to_owned())?;
    
    let cmd_help_template = trans.gettext(&format!("{}\n{}\n{}\n\n{}\n\n{}\n{}", 
                                                  "{bin} {version}",
                                                  "{author}",
                                                  "{about}",
                                                  "Usage: {usage}",
                                                  "Options:",
                                                  "{options}"));
    
    let app = Command::new(option_env!("CARGO_PKG_NAME").unwrap_or("Unknown"))
        .version(option_env!("CARGO_PKG_VERSION").unwrap_or("Unknown"))
        .help_template(&cmd_help_template)
        .author(option_env!("CARGO_PKG_AUTHORS").unwrap_or("Unknown"))
        .about(option_env!("CARGO_PKG_DESCRIPTION").unwrap_or("Unknown"))
        .arg(
            Arg::new("host")
                .long("host")
                .help(&help_host)
                .required(false)
                .default_value(&host_to_str())
        )
        .arg(
            Arg::new("port")
                .long("port")
                .help(&help_port)
                .required(false)
                .default_value(&port_to_str())
        )
        .arg(
            Arg::new("ocr-lang")
                .long("ocr-lang")
                .help(&help_ocr_lang)
                .required(false)
        )
        .arg(
            Arg::new("input-filename")
                .long("input-filename")
                .help(&help_input_filename)
                .required(true)
        )
        .arg(
            Arg::new("output-filename")
                .long("output-filename")
                .help(&help_output_filename)
                .required(false)
        ).arg(
            Arg::new("file-suffix")
                .long("file-suffix")
                .help(&help_file_suffix)
                .default_value(&filesuffix_to_str())
                .required(false)
        ).arg(
            Arg::new("visual-quality")
                .long("visual-quality")
                .help(&help_visual_quality)
                .required(false)                
                .value_parser([
                    PossibleValue::new(IMAGE_QUALITY_CHOICES[0]),
                    PossibleValue::new(IMAGE_QUALITY_CHOICES[1]),
                    PossibleValue::new(IMAGE_QUALITY_CHOICES[2]),                    
                ])
                .default_value(IMAGE_QUALITY_CHOICES[IMAGE_QUALITY_DEFAULT_CHOICE_INDEX as usize])
        ).arg(
            Arg::new("passwd-prompt")
                .long("passwd-prompt")
                .help(&help_password_prompt)
                .required(false)
                .action(ArgAction::SetTrue)
        );

    let run_matches = app.to_owned().get_matches();

    let opt_ocr_lang = if let Some(proposed_ocr_lang) = run_matches.get_one::<String>("ocr-lang") {
        Some(proposed_ocr_lang.to_string())
    } else {
        appconfig.ocr_lang
    };

    let image_quality = if let Some(v) = &run_matches.get_one::<String>("visual-quality") {
        v
    } else {
        IMAGE_QUALITY_CHOICES[IMAGE_QUALITY_DEFAULT_CHOICE_INDEX]
    };
    
    if let Some(proposed_ocr_lang) = &opt_ocr_lang {
        let supported_ocr_languages = l10n::ocr_lang_key_by_name(&trans);
        let proposed_ocr_lang_str = proposed_ocr_lang.as_str();
        let selected_langcodes: Vec<&str> = proposed_ocr_lang_str.split("+").collect();

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
    }

    let output_path_opt = if let Some(proposed_output_filename) = run_matches.get_one::<String>("output-filename") {
        Some(PathBuf::from(proposed_output_filename))
    } else {
        None
    };

    let file_suffix = if let Some(proposed_file_suffix) = run_matches.get_one::<String>("file-suffix") {
        proposed_file_suffix.to_string()
    } else {
        appconfig.file_suffix.clone()
    };

    if let (Some(host), Some(port), Some(file)) = (
        run_matches.get_one::<String>("host"),
        run_matches.get_one::<String>("port"),
        run_matches.get_one::<String>("input-filename"),
    ) {
        let p = PathBuf::from(file);

        if let Err(e) = port.parse::<u16>() {
            return Err(format!("{}: {}! {}.", trans.gettext("Invalid port number"), port, e.to_string()).into());
        }

        if !p.exists() {
            return Err(trans.gettext("The input file doesn't exists!").into());
        }

        let opt_passwd = if run_matches.get_flag("passwd-prompt") {
            println!("{}", trans.gettext("Please enter the password for the document"));
            if let Ok(password) = rpassword::read_password() {
                Some(password)
            } else {
                return Err(trans.gettext("Failed to read password!").into());
            }
        } else {
            None
        };

        if let Some(output_dir) = p.parent() {
            let filename = p.file_name().unwrap().to_str().unwrap();

            let conversion_options = ConversionOptions {
                host: host.to_string(), 
                port: port.to_string(), 
                image_quality: image_quality.to_string(),
                opt_ocr_lang: opt_ocr_lang, 
                opt_passwd: opt_passwd, 
                file_suffix: file_suffix,
            };
            convert_file(conversion_options, output_dir.to_path_buf(), p.clone(), filename.to_string(), output_path_opt, trans.clone()).await
        } else {
            Err(trans.gettext("Could not determine input directory!").into())
        }
    } else {
        Err(trans.gettext("Runtime error, missing parameters!").into())
    }
}

async fn convert_file (
    conversion_options: ConversionOptions,
    output_dir: PathBuf,
    input_path: PathBuf,
    filename: String,
    output_path_opt: Option<PathBuf>,
    l10n: l10n::Translations
) -> Result<(), Box<dyn Error + Send + Sync>> {
    println!("{}: {}", l10n.gettext("Converting file"), input_path.display());

    let addr = format!("{}:{}", conversion_options.host, conversion_options.port.parse::<u16>()?);
    println!("{}: {}", l10n.gettext("Connecting to server at"), addr.clone());

    let stream = fs::read(&input_path)?;
    let stream_part = reqwest::multipart::Part::stream(Body::from(stream));

    let mut multipart_form = reqwest::multipart::Form::new()
        .text("filename", filename)
        .text("visualquality", conversion_options.image_quality.clone())
        .part("file", stream_part);

    if let Some(ocr_lang) = conversion_options.opt_ocr_lang {
        multipart_form = multipart_form.text("ocrlang", ocr_lang);
    }

    if let Some(passwd) = conversion_options.opt_passwd {
        multipart_form = multipart_form.text("docpasswd", passwd);
    }

    let client = Client::new();

    let resp = client
        .post(format!("http://{}/api/v1/upload", addr.clone()))
        .header("Accept-Language", l10n.langid())
        .multipart(multipart_form)
        .send()
        .await?
        .json::<UploadResponse>()
        .await?;

    let tracking_url = format!("http://{}{}", addr.clone(), resp.tracking_uri);
    let download_uri = process_notifications(tracking_url, &l10n).await?;

    let download_data = client
        .get(format!("http://{}{}", addr.clone(), download_uri))
        .send()
        .await?
        .bytes()
        .await?;

    let output_path = if let Some(output_path_value) = output_path_opt {
        output_path_value
    } else {
        if let Some(filename_noext) = input_path.file_stem().and_then(|i| i.to_str()) {
            output_dir.join([filename_noext.to_string(), "-".to_string(), conversion_options.file_suffix, ".pdf".to_string()].concat())
        } else {
            return Err(l10n.gettext("Could not determine input file base name!").into());
        }
    };

    let mut output_file = fs::File::create(output_path)?;
    output_file.write_all(&download_data)?;

    Ok(())
}

async fn process_notifications(tracking_url: String,
                               l10n: &l10n::Translations) -> Result<String, Box<dyn Error + Send + Sync>> {
    let mut es = EventSource::get(tracking_url);
    let pb = ProgressBar::new(100);

    let processing_status: Result<String, Box<dyn Error>> = {
        let mut download_uri = String::new();

        while let Some(event) = es.next().await {
            match event {
                Ok(Event::Open) => {
                    println!("{}", l10n.gettext("Connection open!"))
                },
                Ok(Event::Message(msg)) => {
                    if msg.event == "processing_update" {
                        let log_msg_ret: serde_json::Result<LogMessage> = serde_json::from_str(&msg.data);

                        if let Ok(log_msg) = log_msg_ret {
                            pb.set_position(log_msg.percent_complete as u64);
                            pb.println(&log_msg.data);
                        }
                    } else if msg.event == "processing_success" {
                        let log_msg_ret: serde_json::Result<LogMessage> = serde_json::from_str(&msg.data);

                        if let Ok(log_msg) = log_msg_ret {
                            download_uri = log_msg.data.clone();
                            pb.set_position(log_msg.percent_complete as u64);
                            println!("{}", l10n.gettext("Conversion completed successfully!"));
                        }

                        es.close();

                        return Ok(download_uri);
                    } else if msg.event == "processing_failure" {
                        let log_msg_ret: serde_json::Result<LogMessage> = serde_json::from_str(&msg.data);

                        if let Ok(log_msg) = log_msg_ret {
                            pb.set_position(log_msg.percent_complete as u64);
                        }

                        es.close();

                        return Err(l10n.gettext("Conversion failed!").into());
                    }
                },
                Err(err) => {
                    return Err(err.into());
                }
            }
        }

        Ok(download_uri)
    };

    pb.finish();

    match processing_status {
        Ok(download_uri) => Ok(download_uri),
        Err(ex)          => Err(ex.to_string().into()),
    }
}
