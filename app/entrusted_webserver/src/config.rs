use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs;

pub const PROGRAM_GROUP: &str = "com.rimerosolutions.entrusted.entrusted_webserver";
pub const DEFAULT_FILE_SUFFIX: &str = "entrusted";

#[derive(Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    #[serde(rename(
        serialize = "container-image-name",
        deserialize = "container-image-name"
    ))]
    pub container_image_name: String,
}

pub fn default_container_image_name() -> String {
    let app_version = option_env!("CARGO_PKG_VERSION").unwrap_or("Unknown");

    format!(
        "{}:{}",
        "docker.io/uycyjnzgntrn/entrusted_container", app_version
    )
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 13000,
            container_image_name: default_container_image_name(),
        }
    }
}

pub fn load_config<T>() -> Result<T, Box<dyn Error>>
where
    T: Default + DeserializeOwned,
{
    if let Some(config_dir) = dirs::config_dir() {
        let config_dir_dgz = config_dir.join(PROGRAM_GROUP);

        if config_dir_dgz.exists() {
            let config_path = config_dir_dgz.join("config.toml");

            if config_path.exists() {
                let ret: Result<T, Box<dyn Error>> = {
                    let config_appdata = fs::read_to_string(&config_path)?;
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
