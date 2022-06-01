use std::collections::HashMap;
use std::error::Error;
use std::path::PathBuf;
use which;
use serde::{Deserialize, Serialize};

pub const CONTAINER_IMAGE_EXE: &str = "/usr/local/bin/dangerzone-container";
pub const DEFAULT_FILE_SUFFIX: &str = "dgz";
pub fn container_image_name() -> String {
    let app_version = option_env!("CARGO_PKG_VERSION").unwrap_or("Unknown");

    format!("{}:{}", "docker.io/uycyjnzgntrn/dangerzone-converter", app_version)
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

pub fn executable_find(exe_name: &str) -> Option<PathBuf> {
    match which::which(exe_name) {
        Err(_) => None,
        Ok(path_location) => Some(path_location)
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct LogMessage {
    pub data: String,
    pub percent_complete: usize,
}

#[derive(Clone)]
pub struct ContainerProgram<'a>{
    pub exec_path: PathBuf,
    pub sub_commands: Vec<&'a str>,
    pub suggested_run_args: Vec<&'a str>,
    pub suggested_tmp_dir: Option<PathBuf>,
}

impl<'a> ContainerProgram<'a> {
    pub fn new(exec_path: PathBuf, sub_commands: Vec<&'a str>, suggested_run_args: Vec<&'a str>, suggested_tmp_dir: Option<PathBuf>) -> Self {
        Self {
            exec_path,    
            sub_commands,
            suggested_run_args,
            suggested_tmp_dir
        }
    }
}

enum ContainerProgramStub<'a> {
    Docker(&'a str, Vec<&'a str>, Vec<&'a str>, Option<&'a str>),
    Podman(&'a str, Vec<&'a str>, Vec<&'a str>, Option<&'a str>),
    Lima(&'a str, Vec<&'a str>, Vec<&'a str>, Option<&'a str>)
}

// TODO this is not good enough, ideally subcommands should be captured at a higher level
// Especially for Lima and similar tooling, to avoid further downstream conditional blocks
pub fn container_runtime_path<'a>() -> Option<ContainerProgram<'a>> {
    let container_program_stubs = [
        ContainerProgramStub::Docker("docker", vec![], vec![], None),
        ContainerProgramStub::Podman("podman", vec![], vec!["--userns", "keep-id"], None),
        ContainerProgramStub::Lima("lima", vec!["nerdctl"], vec![], Some("/tmp/lima")),
    ];

    for i in 0..container_program_stubs.len() {
        match &container_program_stubs[i] {
            ContainerProgramStub::Docker(cmd, sub_cmd_args, cmd_args, tmp_dir_opt) |
            ContainerProgramStub::Podman(cmd, sub_cmd_args, cmd_args, tmp_dir_opt) |
            ContainerProgramStub::Lima(cmd, sub_cmd_args, cmd_args, tmp_dir_opt) => {
                if let Some(path_container_exe) = executable_find(cmd) {
                    let suggested_tmp_dir = match tmp_dir_opt {
                        None => None,
                        Some(tmp_dir) => Some(PathBuf::from(tmp_dir))
                    };
                    return Some(ContainerProgram::new(path_container_exe, sub_cmd_args.clone(), cmd_args.clone(), suggested_tmp_dir));
                }
            }
        }
    }

    None
}

pub fn default_output_path(input: PathBuf, file_suffix: String) -> Result<PathBuf, Box<dyn Error>> {
    let input_name_opt = input.file_stem().map(|i| i.to_str()).and_then(|v| v);
    let output_filename_opt = input.parent().map(|i| i.to_path_buf());

    if let (Some(input_name), Some(mut output_filename)) = (input_name_opt, output_filename_opt) {
        let filename = format!("{}-{}.pdf", input_name.to_owned(), file_suffix);
        output_filename.push(filename);
        Ok(output_filename)
    } else {
        Err("Cannot determine resulting PDF output path based on selected input document location!".into())
    }
}
