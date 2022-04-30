use clap::{App, Arg};
use futures::stream::StreamExt;
use reqwest::{Body, Client};
use reqwest_eventsource::{Event, EventSource};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use std::{
    error::Error,
    path::PathBuf,
};
use tokio::{fs::File, io::AsyncWriteExt};
use tokio_util::codec::{BytesCodec, FramedRead};

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct UploadResponse {
    pub id: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct DownloadResponse {
    pub id: String,
    pub data: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let app = App::new(option_env!("CARGO_PKG_NAME").unwrap_or("Unknown"))
        .version(option_env!("CARGO_PKG_VERSION").unwrap_or("Unknown"))
        .author(option_env!("CARGO_PKG_AUTHORS").unwrap_or("Unknown"))
        .about(option_env!("CARGO_PKG_DESCRIPTION").unwrap_or("Unknown"))
        .arg(
            Arg::with_name("host")
                .long("host")
                .help("Server host")
                .required(true)
                .default_value("localhost")
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
                .default_value("13000")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("file")
                .long("file")
                .help("Input file")
                .required(true)
                .takes_value(true),
        );

    let run_matches = app.to_owned().get_matches();

    let mut ocr_lang_opt = None;

    if let Some(proposed_ocr_lang) = run_matches.value_of("ocr-lang") {
        let supported_ocr_languages = ocr_lang_key_by_name();

        if supported_ocr_languages.contains_key(proposed_ocr_lang) {
            ocr_lang_opt = Some(proposed_ocr_lang.to_string());
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

    if let (Some(host), Some(port), Some(file)) = (
        run_matches.value_of("host"),
        run_matches.value_of("port"),
        run_matches.value_of("file"),
    ) {
        let p = PathBuf::from(file);

        if !p.exists() {
            return Err("The input file doesn't exists!".into());
        }

        if let Some(output_dir) = p.parent() {
            let filename = p.file_name().unwrap().to_str().unwrap();
            convert_file(host, port, ocr_lang_opt, output_dir.to_path_buf(), p.clone(), filename.to_string()).await
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

    process_notifications(addr.clone(), resp.id.clone()).await?;

    let download_data = client
        .get(format!("http://{}/downloads/{}", addr.clone(), resp.id))
        .send()
        .await?
        .bytes()
        .await?;

    let filename_noext_opt = input_path.file_stem().and_then(|i| i.to_str());

    if let Some(filename_noext) = filename_noext_opt {
        let output_path = output_dir.join([filename_noext.to_string(), "-safe.pdf".to_string()].concat());
        let mut output_file = File::create(output_path).await?;
        output_file.write_all(&download_data).await?;
    } else {
        return Err("Could not determine input file base name".into());
    }

    Ok(())
}

async fn process_notifications(addr: String, request_id: String) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut es = EventSource::get(format!("http://{}/events/{}", addr, request_id));

    let processing_status: Result<(), Box<dyn Error>> = {
        while let Some(event) = es.next().await {
            match event {
                Ok(Event::Open) => println!("Connection open!"),
                Ok(Event::Message(msg)) => {
                    if msg.event == "processing_update" {                        
                            println!("{}", msg.data);
                    } else {
                        if msg.event == "processing_success" {
                            println!("Conversion completed!");
                            es.close();
                            return Ok(());
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

        Ok(())
    };

    match processing_status {
        Ok(_) => Ok(()),
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
