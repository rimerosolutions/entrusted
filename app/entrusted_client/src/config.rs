use serde::de::DeserializeOwned;
use std::error::Error;
use serde::{Serialize, Deserialize};
use std::fs;
use std::io::Write;

pub const PROGRAM_GROUP: &str = "com.rimerosolutions.entrusted.entrusted_client";
pub const CFG_FILENAME: &str = "config.toml";
pub const DEFAULT_FILE_SUFFIX: &str  ="entrusted";

#[derive(Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct AppConfig {
    #[serde(rename(serialize = "ocr-lang", deserialize = "ocr-lang"))]
    pub ocr_lang: Option<String>,
    #[serde(rename(serialize = "file-suffix", deserialize = "file-suffix"))]
    pub file_suffix: Option<String>,
    #[serde(rename(serialize = "container-image-name", deserialize = "container-image-name"))]
    pub container_image_name: Option<String>,
    #[serde(rename(serialize = "preview-result-appname", deserialize = "preview-result-appname"))]
    pub openwith_appname: Option<String>,
    #[serde(rename(serialize = "visual-quality", deserialize = "visual-quality"))]
    pub visual_quality: Option<String>,
    #[serde(rename(serialize = "seccomp-profile-disabled", deserialize = "seccomp-profile-disabled"))]
    pub seccomp_profile_disabled: Option<bool>,
}

pub fn default_container_image_name() -> String {
    let app_version = option_env!("CARGO_PKG_VERSION").unwrap_or("Unknown");

    format!("{}:{}", "docker.io/uycyjnzgntrn/entrusted_container", app_version)
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            ocr_lang: None,
            file_suffix: Some(DEFAULT_FILE_SUFFIX.to_string()),
            container_image_name: None,
            openwith_appname: None,
            visual_quality: None,
            seccomp_profile_disabled: None,
        }
    }
}

pub fn load_config <T> () -> Result<T, Box<dyn Error>> where T: Default + DeserializeOwned {
    if let Some(config_dir) = dirs::config_dir() {
        let config_appdir = config_dir.join(PROGRAM_GROUP);

        if config_appdir.exists() {
            let config_appfile = config_appdir.join(CFG_FILENAME);

            if config_appfile.exists() {
                let ret = {
                    let config_appdata = fs::read(&config_appfile)?;
                    toml::from_slice(&config_appdata)
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
        let config_appdir = config_dir.join(PROGRAM_GROUP);

        if !config_appdir.exists() {
            if let Err(ex) = fs::create_dir_all(&config_appdir) {
                return Err(format!("Couldn't create configuration folder: {}. {}", config_appdir.display(), ex).into())
            }
        }

        let config_appfile = config_appdir.join(CFG_FILENAME);
        let mut f = fs::OpenOptions::new().create(true).write(true).truncate(true).open(config_appfile)?;
        let config_appdata = toml::to_vec(&config_instance)?;

        if let Err(e) = f.write(&config_appdata) {
            Err(format!("Could not save configuration! {}", e).into())
        } else {
            Ok(())
        }
    } else {
        Err("Cannot determine configuration directory on this machine!".into())
    }
}
