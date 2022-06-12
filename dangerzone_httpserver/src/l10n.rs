use once_cell::sync::Lazy;
use std::io::Cursor;
use fluent_langneg::negotiate_languages;
use fluent_langneg::NegotiationStrategy;
use fluent_langneg::convert_vec_str_to_langids_lossy;
use unic_langid::LanguageIdentifier;
use gettext::Catalog;
use std::collections::HashMap;
use std::sync::Mutex;

pub const DEFAULT_LANGID: &str = "en";
pub const ENV_VAR_DANGERZONE_LANGID: &str = "DANGERZONE_LANGID";

static UITRANSLATIONS_PER_LOCALE: Lazy<Mutex<HashMap<String, Vec<u8>>>> = Lazy::new(|| {
    Mutex::new(load_ui_translations())
});

static CATALOG_PER_LOCALE: Lazy<Mutex<HashMap<String, Catalog>>> = Lazy::new(|| {
    Mutex::new(load_translation_catalog())
});

macro_rules! incl_ui_json_files {
    ( $( $x:expr ),* ) => {
        {
            let mut profs = Vec::new();
            $(
                let data = include_bytes!(concat!("../translations/", $x, "/", $x, ".json")).as_slice();
                profs.push(($x, data));
            )*

                profs
        }
    };
}

macro_rules! incl_gettext_files {
    ( $( $x:expr ),* ) => {
        {
            let mut profs = Vec::new();
            $(
                let data = include_bytes!(concat!("../translations/", $x, "/", $x, ".mo")).as_slice();
                profs.push(($x, data));
            )*

                profs
        }
    };
}

fn load_ui_translations() -> HashMap::<String, Vec<u8>> {
    let mut ret = HashMap::new();

    let locale_data = incl_ui_json_files!("en", "fr");

    for (locale_id, locale_translation_bytes)  in locale_data {
        ret.insert(locale_id.to_string(), locale_translation_bytes.to_vec());
    }

    ret
}

fn load_translation_catalog() -> HashMap::<String, Catalog> {
    let mut ret = HashMap::new();

    let locale_data = incl_gettext_files!("en", "fr");

    for (locale_id, locale_translation_bytes)  in locale_data {
        let reader = Cursor::new(locale_translation_bytes);
        if let Ok(catalog) = gettext::Catalog::parse(reader) {
            ret.insert(locale_id.to_string(), catalog);
        }
    }

    ret
}

pub fn ui_translation_for(locale: String) -> Vec<u8> {
    let catalog_per_langid = UITRANSLATIONS_PER_LOCALE.lock().unwrap();
    let keys: Vec<String> = catalog_per_langid.keys().cloned().collect();

    let requested = convert_vec_str_to_langids_lossy(&[locale]);
    let available = convert_vec_str_to_langids_lossy(&keys);
    let default: LanguageIdentifier = DEFAULT_LANGID.parse().expect("Parsing default language failed!");

    let supported = negotiate_languages(
        &requested,
        &available,
        Some(&default),
        NegotiationStrategy::Matching
    );

    let langid = supported[0].to_string();
    let translations = catalog_per_langid[&langid].clone();

    translations
}

#[derive(Clone)]
pub struct Messages {
    locale: String,
    catalog: Catalog,
}

impl Messages {
    pub fn new(requested_locale: String) -> Self {
        let catalog_per_langid = CATALOG_PER_LOCALE.lock().unwrap();
        let keys: Vec<String> = catalog_per_langid.keys().cloned().collect();

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
        let catalog = catalog_per_langid[&locale].clone();

        Self { locale, catalog }
    }

    pub fn langid(&self) -> String {
        self.locale.clone()
    }

    pub fn get_message(&self, key: &str) -> String {
        String::from(self.catalog.gettext(key))
    }
}
