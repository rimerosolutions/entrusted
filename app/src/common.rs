use std::path::PathBuf;
use std::sync::mpsc::SendError;
use std::str::FromStr;
use std::collections::HashMap;

use uuid::Uuid;

pub const DEFAULT_FILE_SUFFIX: &str = "entrusted";
pub const NAMESPACE_APP: &str = "com.rimerosolutions.entrusted";

#[derive(Debug, Clone, PartialEq)]
pub enum VisualQuality {
    Low, Medium, High
}

impl VisualQuality {
    pub const fn default_value() -> Self {
        Self::Medium
    }

    pub const fn image_max_size(&self) -> (f32, f32) {
        match self {
            VisualQuality::Low    => (794.0  , 1123.0),
            VisualQuality::Medium => (1240.0 , 1754.0),
            VisualQuality::High   => (4961.0 , 7016.0),
        }
    }
}

impl std::fmt::Display for VisualQuality {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VisualQuality::Low    => f.write_str("Low"),
            VisualQuality::Medium => f.write_str("Medium"),
            VisualQuality::High   => f.write_str("High")
        }
    }
}

impl FromStr for VisualQuality {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "low"    => Ok(VisualQuality::Low),
            "medium" => Ok(VisualQuality::Medium),
            "high"   => Ok(VisualQuality::High),
            _        => Err(format!("Invalid visual quality: {}", s)),
        }
    }
}

pub trait EventSender: Send {
    fn send(&self, evt: crate::common::AppEvent) -> Result<(), SendError<crate::common::AppEvent>>;
}

#[derive(Clone, Debug)]
pub enum AppEvent {
    // row_index
    ConversionStarted(usize),
    // doc_id, progress_value, progress_message
    ConversionProgressed(Uuid, usize, String),
    // row_index
    ConversionFailed(usize),
    // File row index and output path
    ConversionFinished(usize, Option<PathBuf>),
    // tasks_completed, tasks_failed, tasks_count
    AllConversionEnded(usize, usize, usize)
}

#[derive(Debug, Clone)]
pub struct ConvertOptions {
    pub output_folder: Option<PathBuf>,
    pub filename_suffix: String,
    pub visual_quality: VisualQuality,
    pub ocr_lang_code: Option<String>,
    pub password_decrypt: Option<String>,
    pub password_encrypt: Option<String>,
}

impl ConvertOptions {
    pub fn new(output_folder: Option<PathBuf>,
               filename_suffix: String,
               visual_quality: VisualQuality,
               ocr_lang_code: Option<String>,
               password_decrypt: Option<String>,
               password_encrypt: Option<String>,
    ) -> Self {
        Self {
            output_folder, filename_suffix, visual_quality, ocr_lang_code, password_decrypt, password_encrypt
        }
    }
}
