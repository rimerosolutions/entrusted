use once_cell::sync::Lazy;
use fluent_langneg::negotiate_languages;
use fluent_langneg::NegotiationStrategy;
use fluent_langneg::convert_vec_str_to_langids_lossy;
use unic_langid::LanguageIdentifier;
use std::collections::HashMap;
use std::io::Cursor;
use std::sync::Mutex;
use locale_config;
use gettext::Catalog;

pub const DEFAULT_LANGID: &str = "en";
pub const ENV_VAR_ENTRUSTED_LANGID: &str = "ENTRUSTED_LANGID";

static CATALOG_PER_LOCALE: Lazy<Mutex<HashMap<String, Catalog>>> = Lazy::new(|| {
    Mutex::new(HashMap::new())
});

pub fn sys_locale() -> String {
    let locale = locale_config::Locale::user_default();

    if let Some((_, language_range)) = locale.tags().next() {
        language_range.to_string()
    } else {
        String::from(DEFAULT_LANGID)
    }
}

pub fn load_translations(locale_data: HashMap<&str, &[u8]>) {
    if let Ok(mut ret) = CATALOG_PER_LOCALE.lock() {
        for (locale_id, locale_translation_bytes) in locale_data {
            let reader = Cursor::new(locale_translation_bytes);

            if let Ok(catalog) = gettext::Catalog::parse(reader) {
                ret.insert(locale_id.to_string(), catalog);
            }
        }
    }
}

#[derive(Clone)]
struct GettextTranslations {
    locale: String,
    catalog: Catalog,
}

pub trait Translations: Send + Sync {
    fn langid(&self, ) -> String;

    fn gettext(&self, msg: &str) -> String;

    fn gettext_fmt(&self, template: &str, params: Vec<&str>) -> String;

    fn ngettext(&self, msgid: &str, msgid_plural: &str, n: u64) -> String;

    fn ngettext_fmt(&self, msgid: &str, msgid_plural: &str, n: u64, params: Vec<&str>) -> String;

    fn clone_box(&self) -> Box<dyn Translations>;
}

impl Clone for Box<dyn Translations> {
    fn clone(&self) -> Box<dyn Translations> {
        self.clone_box()
    }
}

pub fn negotiate_langid(requested_locale: String, keys: Vec<String>) -> String {    
    let requested = convert_vec_str_to_langids_lossy(&[requested_locale]);
    let available = convert_vec_str_to_langids_lossy(&keys);
    let default: LanguageIdentifier = DEFAULT_LANGID.parse().expect("Parsing default language failed!");

    let supported = negotiate_languages(
        &requested,
        &available,
        Some(&default),
        NegotiationStrategy::Matching
    );

    let locale = supported[0].to_string();

    locale
}

pub fn new_translations(requested_locale: String) -> Box<dyn Translations + Send + Sync> {
    let catalog_per_langid = CATALOG_PER_LOCALE.lock().unwrap();
    let keys: Vec<String> = catalog_per_langid.keys().cloned().collect();
    let locale = negotiate_langid(requested_locale, keys);
    let catalog = catalog_per_langid[&locale].clone();

    Box::new(GettextTranslations { locale, catalog })
}

impl Translations for GettextTranslations {

    fn langid(&self) -> String {
        self.locale.clone()
    }

    fn clone_box(&self) -> Box<dyn Translations> {
        Box::new(self.clone())
    }

    fn gettext(&self, msg: &str) -> String {
        self.catalog.gettext(msg).to_string()
    }

    fn gettext_fmt(&self, template: &str, params: Vec<&str>) -> String {
        match params.len() {
            0 => self.gettext(template),
            1 => runtime_format!(self.gettext(template), params[0]),
            2 => crate::runtime_format!(self.gettext(template), params[0], params[1]),
            3 => crate::runtime_format!(self.gettext(template), params[0], params[1], params[2]),
            4 => crate::runtime_format!(self.gettext(template), params[0], params[1], params[2], params[3]),
            _ => format!("{}", "too-many-params-4-values-max")
        }
    }

    fn ngettext(&self, msgid: &str, msgid_plural: &str, n: u64) -> String {
        crate::runtime_format!(self.catalog.ngettext(msgid, msgid_plural, n).to_string(), n)
    }

    fn ngettext_fmt(&self, msgid: &str, msgid_plural: &str, n: u64, params: Vec<&str>) -> String {
        match params.len() {
            0 => crate::runtime_format!(self.catalog.ngettext(msgid, msgid_plural, n).to_string(), n),
            1 => crate::runtime_format!(self.catalog.ngettext(msgid, msgid_plural, n).to_string(), n, params[0]),
            2 => crate::runtime_format!(self.catalog.ngettext(msgid, msgid_plural, n).to_string(), n, params[0], params[1]),
            3 => crate::runtime_format!(self.catalog.ngettext(msgid, msgid_plural, n).to_string(), n, params[0], params[1], params[2]),
            4 => crate::runtime_format!(self.catalog.ngettext(msgid, msgid_plural, n).to_string(), n, params[0], params[1], params[2], params[3]),
            _ => format!("{}", "too-many-params-4-values-max")
        }
    }
}

pub fn ocr_lang_key_by_name(trans: Box<dyn Translations>) -> HashMap<&'static str, String> {
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
    ]
        .map( |(k, v)| (v, trans.gettext(k).to_owned()))
        .iter().cloned().collect()
}

// copy paste a subset of https://github.com/woboq/tr/
// Need dynamic evaluation for translation placeholder arguments
mod runtime_format {
    pub struct FormatArg<'a> {
        #[doc(hidden)]
        pub format_str: &'a str,
        #[doc(hidden)]
        pub args: &'a [(&'static str, &'a dyn (::std::fmt::Display))],
    }

    impl<'a> ::std::fmt::Display for FormatArg<'a> {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            let mut arg_idx = 0;
            let mut pos = 0;
            while let Some(mut p) = self.format_str[pos..].find(|x| x == '{' || x == '}') {
                if self.format_str.len() - pos < p + 1 {
                    break;
                }
                p += pos;

                // Skip escaped }
                if self.format_str.get(p..=p) == Some("}") {
                    self.format_str[pos..=p].fmt(f)?;
                    if self.format_str.get(p + 1..=p + 1) == Some("}") {
                        pos = p + 2;
                    } else {
                        // FIXME! this is an error, it should be reported  ('}' must be escaped)
                        pos = p + 1;
                    }
                    continue;
                }

                // Skip escaped {
                if self.format_str.get(p + 1..=p + 1) == Some("{") {
                    self.format_str[pos..=p].fmt(f)?;
                    pos = p + 2;
                    continue;
                }

                // Find the argument
                let end = if let Some(end) = self.format_str[p..].find('}') {
                    end + p
                } else {
                    // FIXME! this is an error, it should be reported
                    self.format_str[pos..=p].fmt(f)?;
                    pos = p + 1;
                    continue;
                };
                let argument = self.format_str[p + 1..end].trim();
                let pa = if p == end - 1 {
                    arg_idx += 1;
                    arg_idx - 1
                } else if let Ok(n) = argument.parse::<usize>() {
                    n
                } else if let Some(p) = self.args.iter().position(|x| x.0 == argument) {
                    p
                } else {
                    // FIXME! this is an error, it should be reported
                    self.format_str[pos..end].fmt(f)?;
                    pos = end;
                    continue;
                };

                // format the part before the '{'
                self.format_str[pos..p].fmt(f)?;
                if let Some(a) = self.args.get(pa) {
                    a.1.fmt(f)?;
                } else {
                    // FIXME! this is an error, it should be reported
                    self.format_str[p..=end].fmt(f)?;
                }
                pos = end + 1;
            }
            self.format_str[pos..].fmt(f)
        }
    }

    #[macro_export]
    macro_rules! runtime_format {
        ($fmt:expr) => {{
            // TODO! check if 'fmt' does not have {}
            format!("{}", $fmt)
        }};
        ($fmt:expr,  $($tail:tt)* ) => {{
            let format_str = $fmt;
            let fa = runtime_format::FormatArg {
                format_str: AsRef::as_ref(&format_str),
                //args: &[ $( $crate::runtime_format!(@parse_arg $e) ),* ],
                args: crate::runtime_format!(@parse_args [] $($tail)*)
            };
            format!("{}", fa)
        }};

        (@parse_args [$($args:tt)*]) => { &[ $( $args ),* ]  };
        (@parse_args [$($args:tt)*] $name:ident) => {
            $crate::runtime_format!(@parse_args [$($args)* (stringify!($name) , &$name)])
        };
        (@parse_args [$($args:tt)*] $name:ident, $($tail:tt)*) => {
            $crate::runtime_format!(@parse_args [$($args)* (stringify!($name) , &$name)] $($tail)*)
        };
        (@parse_args [$($args:tt)*] $name:ident = $e:expr) => {
            $crate::runtime_format!(@parse_args [$($args)* (stringify!($name) , &$e)])
        };
        (@parse_args [$($args:tt)*] $name:ident = $e:expr, $($tail:tt)*) => {
            $crate::runtime_format!(@parse_args [$($args)* (stringify!($name) , &$e)] $($tail)*)
        };
        (@parse_args [$($args:tt)*] $e:expr) => {
            $crate::runtime_format!(@parse_args [$($args)* ("" , &$e)])
        };
        (@parse_args [$($args:tt)*] $e:expr, $($tail:tt)*) => {
            $crate::runtime_format!(@parse_args [$($args)* ("" , &$e)] $($tail)*)
        };
    }
}
