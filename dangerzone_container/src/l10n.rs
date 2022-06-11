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

static CATALOG_PER_LOCALE: Lazy<Mutex<HashMap<String, Catalog>>> = Lazy::new(|| {
    Mutex::new(get_translation_catalog())
});

macro_rules! incl_profiles {
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

fn get_translation_catalog() -> HashMap::<String, Catalog> {
    let mut ret = HashMap::new();

    let locale_data = incl_profiles!("en", "fr");

    for (locale_id, locale_translation_bytes)  in locale_data {
        let reader = Cursor::new(locale_translation_bytes);
        if let Ok(catalog) = gettext::Catalog::parse(reader) {
            ret.insert(locale_id.to_string(), catalog);
        }
    }

    ret
}

#[derive(Clone)]
pub struct Messages {
    catalog: Catalog,
}

impl Messages {
    pub fn new(locale: String) -> Self {
        let catalog_per_langid = CATALOG_PER_LOCALE.lock().unwrap();
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
        let catalog = catalog_per_langid[&langid].clone();

        Self { catalog }
    }

    pub fn get_message(&self, key: &str) -> String {
        String::from(self.catalog.gettext(key))
    }
}
