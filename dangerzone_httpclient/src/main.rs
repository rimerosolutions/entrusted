use clap::{App, Arg};
use futures::stream::StreamExt;
use reqwest::{Body, Client};
use reqwest_eventsource::{Event, EventSource};
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use indicatif::ProgressBar;

use std::env;

use std::{
    error::Error,
    path::PathBuf,
};
use tokio::{fs::File, io::AsyncWriteExt};
use tokio_util::codec::{BytesCodec, FramedRead};

use serde::de::DeserializeOwned;

use std::fs;
use std::io::Read;
use dirs;

mod l10n;

const PROGRAM_GROUP: &str = "dangerzone-httpclient";
const CFG_FILENAME: &str = "config.toml";

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
            host: String::from("localhost"),
            port: 13000,
            file_suffix: String::from("dgz")
        }
    }
}

fn load_config <T> () -> Result<T, Box<dyn Error>>
where T: Default + DeserializeOwned {
    let config_dir_opt = dirs::config_dir();

    if let Some(config_dir) = config_dir_opt {
        let config_dir_dgz = config_dir.join(PROGRAM_GROUP);

        if config_dir_dgz.exists() {
            let config_path = config_dir_dgz.join(CFG_FILENAME);

            if config_path.exists() {
                let mut f = fs::File::open(config_path.clone())?;
                let mut config_data = Vec::new();

                let ret = {
                    f.read_to_end(&mut config_data)?;
                    toml::from_slice(&config_data)
                };

                if let Ok(data) = ret {
                    return Ok(data);
                } else {
                    return Ok(T::default());
                }
            }
        }
    }

    Ok(T::default())
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct UploadResponse {
    pub id: String,
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
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let locale = match env::var(l10n::ENV_VAR_DANGERZONE_LANGID) {
        Ok(selected_locale) => selected_locale,
        Err(_) => l10n::sys_locale()
    };
    let l10n = l10n::Messages::new(locale);

    let appconfig_ret = load_config();
    let appconfig = appconfig_ret.unwrap_or(AppConfig::default());
    let port_number_text = format!("{}", appconfig.port);
    
    let help_host = l10n.get_message("cmdline-help-host");
    let help_port = l10n.get_message("cmdline-help-port");
    let help_output_filename = l10n.get_message("cmdline-help-output-filename");
    let help_input_filename = l10n.get_message("cmdline-help-input-filename");
    let help_ocr_lang = l10n.get_message("cmdline-help-ocr-lang");
    let help_file_suffix = l10n.get_message("cmdline-help-file-suffix");
    
    let app = App::new(option_env!("CARGO_PKG_NAME").unwrap_or("Unknown"))
        .version(option_env!("CARGO_PKG_VERSION").unwrap_or("Unknown"))
        .author(option_env!("CARGO_PKG_AUTHORS").unwrap_or("Unknown"))
        .about(option_env!("CARGO_PKG_DESCRIPTION").unwrap_or("Unknown"))
        .arg(
            Arg::with_name("host")
                .long("host")                
                .help(&help_host)
                .required(true)
                .default_value(&appconfig.host)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("port")
                .long("port")
                .help(&help_port)
                .required(true)
                .default_value(&port_number_text)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("ocr-lang")
                .long("ocr-lang")                
                .help(&help_ocr_lang)
                .required(false)
                .takes_value(true)
        )
        .arg(
             Arg::with_name("input-filename")
                .long("input-filename")
                .help(&help_input_filename)
                .required(true)
                .takes_value(true)
        )
        .arg(
            Arg::with_name("output-filename")
                .long("output-filename")
                .help(&help_output_filename)
                .required(false)
                .takes_value(true)
        ).arg(
            Arg::with_name("file-suffix")
                .long("file-suffix")
                .help(&help_file_suffix)
                .default_value(&appconfig.file_suffix)
                .required(false)
                .takes_value(true)
        );

    let run_matches = app.to_owned().get_matches();

    let ocr_lang_opt = if let Some(proposed_ocr_lang) = run_matches.value_of("ocr-lang") {
        Some(String::from(proposed_ocr_lang))
    } else {
        appconfig.ocr_lang
    };

    if let Some(proposed_ocr_lang) = &ocr_lang_opt {
        let supported_ocr_languages = ocr_lang_key_by_name(&l10n);
        let proposed_ocr_lang_str = proposed_ocr_lang.as_str();
        
        if !supported_ocr_languages.contains_key(&proposed_ocr_lang_str) {
            let mut ocr_lang_err = "".to_string();
             ocr_lang_err.push_str(&format!("{} {}. {}",
                                            l10n.get_message("msg-ocrlang-msg-unsupported"),
                                            proposed_ocr_lang,
                                            l10n.get_message("msg-ocrlang-msg-hint")));
            
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

    let output_path_opt = if let Some(proposed_output_filename) = run_matches.value_of("output-filename") {
        Some(PathBuf::from(proposed_output_filename))
    } else {
        None
    };

    let file_suffix = if let Some(proposed_file_suffix) = run_matches.value_of("file-suffix") {
        String::from(proposed_file_suffix)
    } else {
        appconfig.file_suffix.clone()
    };

    if let (Some(host), Some(port), Some(file)) = (
        run_matches.value_of("host"),
        run_matches.value_of("port"),
        run_matches.value_of("input-filename"),
    ) {
        let p = PathBuf::from(file);

        if let Err(e) = port.parse::<u16>() {
            return Err(format!("{}: {}! {}.", l10n.get_message("msg-invalid-port-number"), port, e.to_string()).into());
        }

        if !p.exists() {
            return Err(l10n.get_message("msg-input-file-doesnt-exists").into());
        }

        if let Some(output_dir) = p.parent() {
            let filename = p.file_name().unwrap().to_str().unwrap();
            convert_file(host, port, ocr_lang_opt, output_dir.to_path_buf(), p.clone(), filename.to_string(), output_path_opt, file_suffix, &l10n).await
        } else {
            Err(l10n.get_message("msg-couldnt-determine-input-directory").into())
        }
    } else {
        Err(l10n.get_message("msg-missing-parameters").into())
    }
}

async fn convert_file (
    host: &str,
    port: &str,
    ocr_lang_opt: Option<String>,
    output_dir: PathBuf,
    input_path: PathBuf,
    filename: String,
    output_path_opt: Option<PathBuf>,
    file_suffix: String,
    l10n: &l10n::Messages
) -> Result<(), Box<dyn Error + Send + Sync>> {
    println!("{}: {}", l10n.get_message("msg-converting-file"), input_path.display());

    let addr = format!("{}:{}", host, port.parse::<u16>()?);
    println!("{}: {}", l10n.get_message("cli-msg-connecting-to"), addr.clone());

    let file = File::open(input_path.clone()).await?;
    let stream = FramedRead::new(file, BytesCodec::new());
    let stream_part = reqwest::multipart::Part::stream(Body::wrap_stream(stream));

    let mut multipart_form = reqwest::multipart::Form::new()
        .text("filename", filename)
        .part("file", stream_part);

    if let Some(ocr_lang) = ocr_lang_opt {
        multipart_form = multipart_form.text("ocrlang", ocr_lang);
    }

    let client = Client::new();
    
    let resp = client
        .post(format!("http://{}/upload", addr.clone()))
        .header("Accept-Language", l10n.langid())
        .multipart(multipart_form)
        .send()
        .await?
        .json::<UploadResponse>()
        .await?;

    let tracking_url = format!("http://{}{}", addr.clone(), resp.tracking_uri);
    let download_uri = process_notifications(tracking_url, l10n).await?;

    let download_data = client
        .get(format!("http://{}{}", addr.clone(), download_uri))
        .send()
        .await?
        .bytes()
        .await?;

    let output_path = if let Some(output_path_value) = output_path_opt {
        output_path_value
    } else {
        let filename_noext_opt = input_path.file_stem().and_then(|i| i.to_str());

        if let Some(filename_noext) = filename_noext_opt {
            output_dir.join([filename_noext.to_string(), "-".to_string(), file_suffix, ".pdf".to_string()].concat())
        } else {
            return Err(l10n.get_message("msg-couldnt-find-input-basename").into());
        }
    };
    
    let mut output_file = File::create(output_path).await?;
    output_file.write_all(&download_data).await?;

    Ok(())
}

async fn process_notifications(tracking_url: String,
    l10n: &l10n::Messages) -> Result<String, Box<dyn Error + Send + Sync>> {
    let mut es = EventSource::get(format!("{}", tracking_url));
    let pb = ProgressBar::new(100);
    
    let processing_status: Result<String, Box<dyn Error>> = {
        let mut download_uri = String::new();

        while let Some(event) = es.next().await {
            match event {
                Ok(Event::Open) => println!("{}", l10n.get_message("msg-notif-connection-open")),
                Ok(Event::Message(msg)) => {
                    if msg.event == "processing_update" {
                        let log_msg_ret: serde_json::Result<LogMessage> = serde_json::from_str(&msg.data);

                        if let Ok(log_msg) = log_msg_ret {
                            pb.set_position(log_msg.percent_complete as u64);
                            pb.println(&log_msg.data);
                        }
                    } else {
                        if msg.event == "processing_success" {
                            let log_msg_ret: LogMessage = serde_json::from_str(&msg.data).unwrap();                            
                            download_uri = log_msg_ret.data.clone();
                            println!("{}", l10n.get_message("msg-notif-processing-success"));
                            es.close();
                            
                            return Ok(download_uri);
                        } else if msg.event == "processing_failure" {
                            println!("{}", l10n.get_message("msg-notif-processing-failure"));
                            es.close();
                            return Err(msg.data.into());
                        }
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
        Err(ex) => Err(ex.to_string().into()),
    }
}

pub fn ocr_lang_key_by_name(l10n: &l10n::Messages) -> HashMap<&'static str, String> {
    [
        ("ocrlang-afrikaans", "ar"),
        ("ocrlang-albanian", "sqi"),
        ("ocrlang-amharic", "amh"),
        ("ocrlang-arabic", "ara"),
        ("ocrlang-arabic-script", "Arabic"),
        ("ocrlang-armenian", "hye"),
        ("ocrlang-armenian-script", "Armenian"),
        ("ocrlang-assamese", "asm"),
        ("ocrlang-azerbaijani", "aze"),
        ("ocrlang-azerbaijani-cyrillic", "aze_cyrl"),
        ("ocrlang-basque", "eus"),
        ("ocrlang-belarusian", "bel"),
        ("ocrlang-bengali", "ben"),
        ("ocrlang-bengali-script", "Bengali"),
        ("ocrlang-bosnian", "bos"),
        ("ocrlang-breton", "bre"),
        ("ocrlang-bulgarian", "bul"),
        ("ocrlang-burmese", "mya"),
        ("ocrlang-canadian-aboriginal-script", "Canadian_Aboriginal"),
        ("ocrlang-catalan", "cat"),
        ("ocrlang-cebuano", "ceb"),
        ("ocrlang-cherokee", "chr"),
        ("ocrlang-cherokee-script", "Cherokee"),
        ("ocrlang-chinese-simplified", "chi_sim"),
        ("ocrlang-chinese-simplified-vertical", "chi_sim_vert"),
        ("ocrlang-chinese-traditional", "chi_tra"),
        ("ocrlang-chinese-traditional-vertical", "chi_tra_vert"),
        ("ocrlang-corsican", "cos"),
        ("ocrlang-croatian", "hrv"),
        ("ocrlang-cyrillic-script", "Cyrillic"),
        ("ocrlang-czech", "ces"),
        ("ocrlang-danish", "dan"),
        ("ocrlang-devanagari-script", "Devanagari"),
        ("ocrlang-divehi", "div"),
        ("ocrlang-dutch", "nld"),
        ("ocrlang-dzongkha", "dzo"),
        ("ocrlang-english", "eng"),
        ("ocrlang-english-middle-1100-1500", "enm"),
        ("ocrlang-esperanto", "epo"),
        ("ocrlang-estonian", "est"),
        ("ocrlang-ethiopic-script", "Ethiopic"),
        ("ocrlang-faroese", "fao"),
        ("ocrlang-filipino", "fil"),
        ("ocrlang-finnish", "fin"),
        ("ocrlang-fraktur-script", "Fraktur"),
        ("ocrlang-frankish", "frk"),
        ("ocrlang-french", "fra"),
        ("ocrlang-french-middle-ca-1400-1600", "frm"),
        ("ocrlang-frisian-western", "fry"),
        ("ocrlang-gaelic-scots", "gla"),
        ("ocrlang-galician", "glg"),
        ("ocrlang-georgian", "kat"),
        ("ocrlang-georgian-script", "Georgian"),
        ("ocrlang-german", "deu"),
        ("ocrlang-greek", "ell"),
        ("ocrlang-greek-script", "Greek"),
        ("ocrlang-gujarati", "guj"),
        ("ocrlang-gujarati-script", "Gujarati"),
        ("ocrlang-gurmukhi-script", "Gurmukhi"),
        ("ocrlang-hangul-script", "Hangul"),
        ("ocrlang-hangul-vertical-script", "Hangul_vert"),
        ("ocrlang-han-simplified-script", "HanS"),
        ("ocrlang-han-simplified-vertical-script", "HanS_vert"),
        ("ocrlang-han-traditional-script", "HanT"),
        ("ocrlang-han-traditional-vertical-script", "HanT_vert"),
        ("ocrlang-hatian", "hat"),
        ("ocrlang-hebrew", "heb"),
        ("ocrlang-hebrew-script", "Hebrew"),
        ("ocrlang-hindi", "hin"),
        ("ocrlang-hungarian", "hun"),
        ("ocrlang-icelandic", "isl"),
        ("ocrlang-indonesian", "ind"),
        ("ocrlang-inuktitut", "iku"),
        ("ocrlang-irish", "gle"),
        ("ocrlang-italian", "ita"),
        ("ocrlang-italian-old", "ita_old"),
        ("ocrlang-japanese", "jpn"),
        ("ocrlang-japanese-script", "Japanese"),
        ("ocrlang-japanese-vertical", "jpn_vert"),
        ("ocrlang-japanese-vertical-script", "Japanese_vert"),
        ("ocrlang-javanese", "jav"),
        ("ocrlang-kannada", "kan"),
        ("ocrlang-kannada-script", "Kannada"),
        ("ocrlang-kazakh", "kaz"),
        ("ocrlang-khmer", "khm"),
        ("ocrlang-khmer-script", "Khmer"),
        ("ocrlang-korean", "kor"),
        ("ocrlang-korean-vertical", "kor_vert"),
        ("ocrlang-kurdish-arabic", "kur_ara"),
        ("ocrlang-kyrgyz", "kir"),
        ("ocrlang-lao", "lao"),
        ("ocrlang-lao-script", "Lao"),
        ("ocrlang-latin", "lat"),
        ("ocrlang-latin-script", "Latin"),
        ("ocrlang-latvian", "lav"),
        ("ocrlang-lithuanian", "lit"),
        ("ocrlang-luxembourgish", "ltz"),
        ("ocrlang-macedonian", "mkd"),
        ("ocrlang-malayalam", "mal"),
        ("ocrlang-malayalam-script", "Malayalam"),
        ("ocrlang-malay", "msa"),
        ("ocrlang-maltese", "mlt"),
        ("ocrlang-maori", "mri"),
        ("ocrlang-marathi", "mar"),
        ("ocrlang-mongolian", "mon"),
        ("ocrlang-myanmar-script", "Myanmar"),
        ("ocrlang-nepali", "nep"),
        ("ocrlang-norwegian", "nor"),
        ("ocrlang-occitan-post-1500", "oci"),
        ("ocrlang-old-georgian", "kat_old"),
        ("ocrlang-oriya-odia-script", "Oriya"),
        ("ocrlang-oriya", "ori"),
        ("ocrlang-pashto", "pus"),
        ("ocrlang-persian", "fas"),
        ("ocrlang-polish", "pol"),
        ("ocrlang-portuguese", "por"),
        ("ocrlang-punjabi", "pan"),
        ("ocrlang-quechua", "que"),
        ("ocrlang-romanian", "ron"),
        ("ocrlang-russian", "rus"),
        ("ocrlang-sanskrit", "san"),
        ("ocrlang-script-and-orientation", "osd"),
        ("ocrlang-serbian-latin", "srp_latn"),
        ("ocrlang-serbian", "srp"),
        ("ocrlang-sindhi", "snd"),
        ("ocrlang-sinhala-script", "Sinhala"),
        ("ocrlang-sinhala", "sin"),
        ("ocrlang-slovakian", "slk"),
        ("ocrlang-slovenian", "slv"),
        ("ocrlang-spanish-castilian-old", "spa_old"),
        ("ocrlang-spanish", "spa"),
        ("ocrlang-sundanese", "sun"),
        ("ocrlang-swahili", "swa"),
        ("ocrlang-swedish", "swe"),
        ("ocrlang-syriac-script", "Syriac"),
        ("ocrlang-syriac", "syr"),
        ("ocrlang-tajik", "tgk"),
        ("ocrlang-tamil-script", "Tamil"),
        ("ocrlang-tamil", "tam"),
        ("ocrlang-tatar", "tat"),
        ("ocrlang-telugu-script", "Telugu"),
        ("ocrlang-telugu", "tel"),
        ("ocrlang-thaana-script", "Thaana"),
        ("ocrlang-thai-script", "Thai"),
        ("ocrlang-thai", "tha"),
        ("ocrlang-tibetan-script", "Tibetan"),
        ("ocrlang-tibetan-standard", "bod"),
        ("ocrlang-tigrinya", "tir"),
        ("ocrlang-tonga", "ton"),
        ("ocrlang-turkish", "tur"),
        ("ocrlang-ukrainian", "ukr"),
        ("ocrlang-urdu", "urd"),
        ("ocrlang-uyghur", "uig"),
        ("ocrlang-uzbek-cyrillic", "uzb_cyrl"),
        ("ocrlang-uzbek", "uzb"),
        ("ocrlang-vietnamese-script", "Vietnamese"),
        ("ocrlang-vietnamese", "vie"),
        ("ocrlang-welsh", "cym"),
        ("ocrlang-yiddish", "yid"),
        ("ocrlang-yoruba", "yor"),
    ].map( |(k, v)| {
        let msg = l10n.get_message(k).to_owned();
        (v, msg)
    }).iter().cloned().collect()
}

