use entrusted_l10n as l10n;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Mutex;

static UITRANSLATIONS_PER_LOCALE: Lazy<Mutex<HashMap<String, Vec<u8>>>> =
    Lazy::new(|| Mutex::new(load_ui_translations()));

macro_rules! incl_ui_json_files {
    ( $( $x:expr ),* ) => {
        {
            let mut profs = Vec::with_capacity(2);
            $(
                let data = include_bytes!(concat!("../translations/", $x, "/messages.json")).as_slice();
                profs.push(($x, data));
            )*

                profs
        }
    };
}

fn load_ui_translations() -> HashMap<String, Vec<u8>> {
    let mut ret = HashMap::new();
    let locale_data = incl_ui_json_files!("en", "fr");

    for (locale_id, locale_translation_bytes) in locale_data {
        ret.insert(locale_id.to_string(), locale_translation_bytes.to_vec());
    }

    ret
}

pub fn ui_translation_for(requested_locale: String) -> Vec<u8> {
    let catalog_per_langid = UITRANSLATIONS_PER_LOCALE.lock().unwrap();
    let keys: Vec<String> = catalog_per_langid.keys().cloned().collect();
    let langid = l10n::negotiate_langid(requested_locale, keys);
    let translations = catalog_per_langid[&langid].clone();

    translations
}
