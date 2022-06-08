use clap::{App, Arg};
use futures::stream::StreamExt;
use reqwest::{Body, Client};
use reqwest_eventsource::{Event, EventSource};
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use indicatif::ProgressBar;

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

const PROGRAM_GROUP: &str = "dangerzone-httpclient";
const CFG_FILENAME: &str = "config.toml";

#[derive(Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct AppConfig {
    #[serde(rename(serialize = "ocr-lang", deserialize = "ocr-lang"))]
    pub ocr_lang: Option<String>,
    pub host: String,
    pub port: u16,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            ocr_lang: None,
            host: String::from("localhost"),
            port: 13000,
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
    let appconfig_ret = load_config();
    let appconfig = appconfig_ret.unwrap_or(AppConfig::default());
    let port_number_text = format!("{}", appconfig.port);
    
    let app = App::new(option_env!("CARGO_PKG_NAME").unwrap_or("Unknown"))
        .version(option_env!("CARGO_PKG_VERSION").unwrap_or("Unknown"))
        .author(option_env!("CARGO_PKG_AUTHORS").unwrap_or("Unknown"))
        .about(option_env!("CARGO_PKG_DESCRIPTION").unwrap_or("Unknown"))
        .arg(
            Arg::with_name("host")
                .long("host")
                .help("Server host")
                .required(true)
                .default_value(&appconfig.host)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("ocr-lang")
                .long("ocr-lang")
                .help("Optional language for OCR")
                .required(false)
                .takes_value(true)
        )
        .arg(
            Arg::with_name("port")
                .long("port")
                .help("Server port")
                .required(true)
                .default_value(&port_number_text)
                .takes_value(true),
        )
        .arg(
             Arg::with_name("input-filename")
                .long("input-filename")
                .help("Input filename")
                .required(true)
                .takes_value(true)
        )
        .arg(
            Arg::with_name("output-filename")
                .long("output-filename")
                .help("Optional output filename defaulting to <filename>-safe.pdf.")
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
        let supported_ocr_languages = ocr_lang_key_by_name();
        let proposed_ocr_lang_str = proposed_ocr_lang.as_str();
        
        if !supported_ocr_languages.contains_key(&proposed_ocr_lang_str) {
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

    let output_path_opt = if let Some(proposed_output_filename) = run_matches.value_of("output-filename") {
        Some(PathBuf::from(proposed_output_filename))
    } else {
        None
    };

    if let (Some(host), Some(port), Some(file)) = (
        run_matches.value_of("host"),
        run_matches.value_of("port"),
        run_matches.value_of("input-filename"),
    ) {
        let p = PathBuf::from(file);

        if let Err(e) = port.parse::<u16>() {
            return Err(format!("Invalid port number: {}! {}.", port, e.to_string()).into());
        }

        if !p.exists() {
            return Err("The input file doesn't exists!".into());
        }

        if let Some(output_dir) = p.parent() {
            let filename = p.file_name().unwrap().to_str().unwrap();
            convert_file(host, port, ocr_lang_opt, output_dir.to_path_buf(), p.clone(), filename.to_string(), output_path_opt).await
        } else {
            Err("Could not determine input directory!".into())
        }
    } else {
        Err("Missing parameters!".into())
    }
}

async fn convert_file (
    host: &str,
    port: &str,
    ocr_lang_opt: Option<String>,
    output_dir: PathBuf,
    input_path: PathBuf,
    filename: String,
    output_path_opt: Option<PathBuf>
) -> Result<(), Box<dyn Error + Send + Sync>> {
    println!("Converting file file: {}", input_path.display());

    let addr = format!("{}:{}", host, port.parse::<u16>()?);
    println!("Connecting to server at: {}", addr.clone());

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
        .multipart(multipart_form)
        .send()
        .await?
        .json::<UploadResponse>()
        .await?;

    let tracking_url = format!("http://{}{}", addr.clone(), resp.tracking_uri);
    let download_uri = process_notifications(tracking_url).await?;

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
            output_dir.join([filename_noext.to_string(), "-dgz.pdf".to_string()].concat())
        } else {
            return Err("Could not determine input file base name!".into());
        }
    };
    
    let mut output_file = File::create(output_path).await?;
    output_file.write_all(&download_data).await?;

    Ok(())
}

async fn process_notifications(tracking_url: String) -> Result<String, Box<dyn Error + Send + Sync>> {
    let mut es = EventSource::get(format!("{}", tracking_url));
    let pb = ProgressBar::new(100);
    
    let processing_status: Result<String, Box<dyn Error>> = {
        let mut download_uri = String::new();

        while let Some(event) = es.next().await {
            match event {
                Ok(Event::Open) => println!("Connection open!"),
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
                            println!("Conversion completed successfully!");                            
                            es.close();
                            
                            return Ok(download_uri);
                        } else if msg.event == "processing_failure" {
                            println!("Conversion failed!");
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

pub fn ocr_lang_key_by_name() -> HashMap<&'static str, &'static str> {
    [
        ("Afrikaans", "ar"),
        ("Albanian", "sqi"),
        ("Amharic", "amh"),
        ("Arabic", "ara"),
        ("Arabic script", "Arabic"),
        ("Armenian", "hye"),
        ("Armenian script", "Armenian"),
        ("Assamese", "asm"),
        ("Azerbaijani", "aze"),
        ("Azerbaijani (Cyrillic)", "aze_cyrl"),
        ("Basque", "eus"),
        ("Belarusian", "bel"),
        ("Bengali", "ben"),
        ("Bengali script", "Bengali"),
        ("Bosnian", "bos"),
        ("Breton", "bre"),
        ("Bulgarian", "bul"),
        ("Burmese", "mya"),
        ("Canadian Aboriginal script", "Canadian_Aboriginal"),
        ("Catalan", "cat"),
        ("Cebuano", "ceb"),
        ("Cherokee", "chr"),
        ("Cherokee script", "Cherokee"),
        ("Chinese - Simplified", "chi_sim"),
        ("Chinese - Simplified (vertical)", "chi_sim_vert"),
        ("Chinese - Traditional", "chi_tra"),
        ("Chinese - Traditional (vertical)", "chi_tra_vert"),
        ("Corsican", "cos"),
        ("Croatian", "hrv"),
        ("Cyrillic script", "Cyrillic"),
        ("Czech", "ces"),
        ("Danish", "dan"),
        ("Devanagari script", "Devanagari"),
        ("Divehi", "div"),
        ("Dutch", "nld"),
        ("Dzongkha", "dzo"),
        ("English", "eng"),
        ("English, Middle (1100-1500)", "enm"),
        ("Esperanto", "epo"),
        ("Estonian", "est"),
        ("Ethiopic script", "Ethiopic"),
        ("Faroese", "fao"),
        ("Filipino", "fil"),
        ("Finnish", "fin"),
        ("Fraktur script", "Fraktur"),
        ("Frankish", "frk"),
        ("French", "fra"),
        ("French, Middle (ca.1400-1600)", "frm"),
        ("Frisian (Western)", "fry"),
        ("Gaelic (Scots)", "gla"),
        ("Galician", "glg"),
        ("Georgian", "kat"),
        ("Georgian script", "Georgian"),
        ("German", "deu"),
        ("Greek", "ell"),
        ("Greek script", "Greek"),
        ("Gujarati", "guj"),
        ("Gujarati script", "Gujarati"),
        ("Gurmukhi script", "Gurmukhi"),
        ("Hangul script", "Hangul"),
        ("Hangul (vertical) script", "Hangul_vert"),
        ("Han - Simplified script", "HanS"),
        ("Han - Simplified (vertical) script", "HanS_vert"),
        ("Han - Traditional script", "HanT"),
        ("Han - Traditional (vertical) script", "HanT_vert"),
        ("Hatian", "hat"),
        ("Hebrew", "heb"),
        ("Hebrew script", "Hebrew"),
        ("Hindi", "hin"),
        ("Hungarian", "hun"),
        ("Icelandic", "isl"),
        ("Indonesian", "ind"),
        ("Inuktitut", "iku"),
        ("Irish", "gle"),
        ("Italian", "ita"),
        ("Italian - Old", "ita_old"),
        ("Japanese", "jpn"),
        ("Japanese script", "Japanese"),
        ("Japanese (vertical)", "jpn_vert"),
        ("Japanese (vertical) script", "Japanese_vert"),
        ("Javanese", "jav"),
        ("Kannada", "kan"),
        ("Kannada script", "Kannada"),
        ("Kazakh", "kaz"),
        ("Khmer", "khm"),
        ("Khmer script", "Khmer"),
        ("Korean", "kor"),
        ("Korean (vertical)", "kor_vert"),
        ("Kurdish (Arabic)", "kur_ara"),
        ("Kyrgyz", "kir"),
        ("Lao", "lao"),
        ("Lao script", "Lao"),
        ("Latin", "lat"),
        ("Latin script", "Latin"),
        ("Latvian", "lav"),
        ("Lithuanian", "lit"),
        ("Luxembourgish", "ltz"),
        ("Macedonian", "mkd"),
        ("Malayalam", "mal"),
        ("Malayalam script", "Malayalam"),
        ("Malay", "msa"),
        ("Maltese", "mlt"),
        ("Maori", "mri"),
        ("Marathi", "mar"),
        ("Mongolian", "mon"),
        ("Myanmar script", "Myanmar"),
        ("Nepali", "nep"),
        ("Norwegian", "nor"),
        ("Occitan (post 1500)", "oci"),
        ("Old Georgian", "kat_old"),
        ("Oriya (Odia) script", "Oriya"),
        ("Oriya", "ori"),
        ("Pashto", "pus"),
        ("Persian", "fas"),
        ("Polish", "pol"),
        ("Portuguese", "por"),
        ("Punjabi", "pan"),
        ("Quechua", "que"),
        ("Romanian", "ron"),
        ("Russian", "rus"),
        ("Sanskrit", "san"),
        ("script and orientation", "osd"),
        ("Serbian (Latin)", "srp_latn"),
        ("Serbian", "srp"),
        ("Sindhi", "snd"),
        ("Sinhala script", "Sinhala"),
        ("Sinhala", "sin"),
        ("Slovakian", "slk"),
        ("Slovenian", "slv"),
        ("Spanish, Castilian - Old", "spa_old"),
        ("Spanish", "spa"),
        ("Sundanese", "sun"),
        ("Swahili", "swa"),
        ("Swedish", "swe"),
        ("Syriac script", "Syriac"),
        ("Syriac", "syr"),
        ("Tajik", "tgk"),
        ("Tamil script", "Tamil"),
        ("Tamil", "tam"),
        ("Tatar", "tat"),
        ("Telugu script", "Telugu"),
        ("Telugu", "tel"),
        ("Thaana script", "Thaana"),
        ("Thai script", "Thai"),
        ("Thai", "tha"),
        ("Tibetan script", "Tibetan"),
        ("Tibetan Standard", "bod"),
        ("Tigrinya", "tir"),
        ("Tonga", "ton"),
        ("Turkish", "tur"),
        ("Ukrainian", "ukr"),
        ("Urdu", "urd"),
        ("Uyghur", "uig"),
        ("Uzbek (Cyrillic)", "uzb_cyrl"),
        ("Uzbek", "uzb"),
        ("Vietnamese script", "Vietnamese"),
        ("Vietnamese", "vie"),
        ("Welsh", "cym"),
        ("Yiddish", "yid"),
        ("Yoruba", "yor"),
    ].map( |(k, v)| (v, k)).iter().cloned().collect()
}
