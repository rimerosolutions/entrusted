use std::collections::HashMap;
use std::error::Error;
use std::path::PathBuf;
use which;
use serde::{Deserialize, Serialize};

use crate::l10n;

pub const CONTAINER_IMAGE_EXE: &str = "/usr/local/bin/dangerzone-container";

pub fn ocr_lang_key_by_name(l10n: l10n::Messages) -> HashMap<&'static str, String> {
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
