use serde::de::DeserializeOwned;
use std::error::Error;
use serde::{Serialize, Deserialize};
use std::fs;
use std::io::Write;
use dirs;

pub const PROGRAM_GROUP: &str = "com.rimerosolutions.entrusted.entrusted_client";
pub const CFG_FILENAME: &str = "config.toml";
pub const DEFAULT_FILE_SUFFIX: &str  ="entrusted";

#[derive(Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct AppConfig {
    #[serde(rename(serialize = "ocr-lang", deserialize = "ocr-lang"))]
    pub ocr_lang: Option<String>,
    #[serde(rename(serialize = "file-suffix", deserialize = "file-suffix"))]
    pub file_suffix: String,
    #[serde(rename(serialize = "container-image-name", deserialize = "container-image-name"))]
    pub container_image_name: Option<String>,
    #[serde(rename(serialize = "preview-result-appname", deserialize = "preview-result-appname"))]
    pub openwith_appname: Option<String>
}

pub fn default_container_image_name() -> String {
    let app_version = option_env!("CARGO_PKG_VERSION").unwrap_or("Unknown");

    format!("{}:{}", "docker.io/uycyjnzgntrn/entrusted_container", app_version)
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            ocr_lang: None,
            file_suffix: String::from(DEFAULT_FILE_SUFFIX),
            container_image_name: None,
            openwith_appname: None,
        }
    }
}

pub fn load_config <T> () -> Result<T, Box<dyn Error>> where T: Default + DeserializeOwned {
    if let Some(config_dir) = dirs::config_dir() {
        let config_dir_dgz = config_dir.join(PROGRAM_GROUP);

        if config_dir_dgz.exists() {
            let config_path = config_dir_dgz.join(CFG_FILENAME);

            if config_path.exists() {
                let ret = {
                    let config_data = fs::read(&config_path)?;
                    toml::from_slice(&config_data)
                };

                if let Ok(data) = ret {
                    return Ok(data);
                }
            }
        }
    }

    Ok(T::default())
}

// Only used in the GUI Desktop client
#[allow(dead_code)]
pub fn save_config <T> (config_instance: T) -> Result<(), Box<dyn Error>>
where T: Default + Serialize {
    if let Some(config_dir) = dirs::config_dir() {
        let config_dir_dgz = config_dir.join(PROGRAM_GROUP);

        if !config_dir_dgz.exists() {
            if let Err(ex) = fs::create_dir_all(&config_dir_dgz) {
                return Err(format!("Couldn't create configuration folder: {}. {}", config_dir_dgz.display(), ex.to_string()).into())
            }
        }

        let config_path = config_dir_dgz.join(CFG_FILENAME);
        let mut f = fs::OpenOptions::new().create(true).write(true).truncate(true).open(config_path.clone())?;
        let config_data = toml::to_vec(&config_instance)?;

        if let Err(e) = f.write(&config_data) {
            Err(format!("Could not save configuration! {}", e.to_string()).into())
        } else {
            Ok(())
        }
    } else {
        Err("Cannot determine configuration directory on this machine!".into())
    }
}
