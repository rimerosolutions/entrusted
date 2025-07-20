use serde::de::DeserializeOwned;
use std::error::Error;
use serde::{Serialize, Deserialize};
use std::fs;
use std::io::Write;

use crate::common;
use crate::error;

pub const PROGRAM_GROUP: &str = "com.rimerosolutions.Entrusted";
pub const CFG_FILENAME: &str = "config.toml";

#[derive(Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct AppConfig {
    #[serde(rename(serialize = "ocr-lang", deserialize = "ocr-lang"))]
    pub ocr_lang: Option<String>,
    #[serde(rename(serialize = "file-suffix", deserialize = "file-suffix"))]
    pub file_suffix: Option<String>,
    #[serde(rename(serialize = "visual-quality", deserialize = "visual-quality"))]
    pub visual_quality: Option<String>,
    #[serde(rename(serialize = "output-folder", deserialize = "output-folder"))]
    pub output_folder: Option<String>,
    #[serde(rename(serialize = "ui-theme", deserialize = "ui-theme"))]
    pub ui_theme: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            ocr_lang       : None,
            file_suffix    : Some(common::DEFAULT_FILE_SUFFIX.to_string()),
            visual_quality : None,
            output_folder  : None,
            ui_theme       : None,
        }
    }
}

pub fn load_config <T> () -> Result<T, error::Failure> where T: Default + DeserializeOwned {
    if let Some(config_dir) = dirs::config_dir() {
        let config_appdir = config_dir.join(PROGRAM_GROUP);

        if config_appdir.exists() {
            let config_appfile = config_appdir.join(CFG_FILENAME);

            if config_appfile.exists() {
                let ret: Result<T, Box<dyn Error>> = {
                    let config_appdata = fs::read_to_string(&config_appfile)?;

                    match toml::from_str(&config_appdata) {
                        Ok(v)   => Ok(v),
                        Err(ex) => Err(ex.into())
                    }
                };

                if let Ok(data) = ret {
                    return Ok(data);
                }
            }
        }
    }

    Ok(T::default())
}

pub fn save_config (config_instance: AppConfig) -> Result<(), error::Failure> {
    if let Some(config_dir) = dirs::config_dir() {
        let config_appdir = config_dir.join(PROGRAM_GROUP);

        if !config_appdir.exists() {
            if let Err(ex) = fs::create_dir_all(&config_appdir) {
                return Err(ex.into());
            }
        }

        let config_appfile = config_appdir.join(CFG_FILENAME);
        let mut f = fs::OpenOptions::new().create(true).write(true).truncate(true).open(config_appfile)?;
        let config_appdata = toml::to_string(&config_instance)?;

        if let Err(e) = f.write(config_appdata.as_bytes()) {
            Err(e.into())
        } else {
            Ok(())
        }
    } else {
        Err(error::Failure::RuntimeError("Cannot determine configuration directory on this machine!".to_string()))
    }
}
