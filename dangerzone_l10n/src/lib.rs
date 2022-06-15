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
pub const ENV_VAR_DANGERZONE_LANGID: &str = "DANGERZONE_LANGID";

static CATALOG_PER_LOCALE: Lazy<Mutex<HashMap<String, Catalog>>> = Lazy::new(|| {
    Mutex::new(HashMap::new())
});

pub fn sys_locale() -> String {
    let locale = locale_config::Locale::user_default();

    if let Some(ll) = locale.tags().next() {
        ll.1.to_string()
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

pub trait Translations {
    fn langid(&self, ) -> String;

    fn gettext(&self, msg: &str) -> String;

    fn gettext_fmt(&self, template: &str, params: Vec<&str>) -> String;

    fn ngettext(&self, msgid: &str, msgid_plural: &str, n: u64) -> String;

    fn ngettext_fmt(&self, msgid: &str, msgid_plural: &str, n: u64, params: Vec<&str>) -> String;

    fn clone_box(&self) -> Box<dyn Translations + Send>;
}

impl Clone for Box<dyn Translations> {
    fn clone(&self) -> Box<dyn Translations> {
        self.clone_box()
    }
}

pub fn new_translations(requested_locale: String) -> Box<dyn Translations + Send + Sync> {
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

    Box::new(GettextTranslations { locale, catalog })
}

impl Translations for GettextTranslations {

    fn langid(&self) -> String {
        self.locale.clone()
    }

    fn clone_box(&self) -> Box<dyn Translations + Send> {
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
