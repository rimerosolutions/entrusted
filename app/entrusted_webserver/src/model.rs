use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct Notification {
    pub event: String,
    pub id: String,
    pub data: String,
}

#[derive(Debug, Clone)]
pub struct ConversionOptions {
    pub ci_image_name: String,
    pub opt_ocr_lang: Option<String>,
    pub opt_passwd: Option<String>,
    pub visualquality: String
}

impl ConversionOptions {
    pub fn new(
        ci_image_name: String,
        opt_ocr_lang: Option<String>,
        opt_passwd: Option<String>,
        visualquality: String
    ) -> Self {
        Self {
            ci_image_name,
            opt_ocr_lang,
            opt_passwd,
            visualquality
        }
    }
}

impl Notification {
    pub fn new(event: String, id: String, data: String) -> Self {
        Self { event, id, data }
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct CompletionMessage {
    pub percent_complete: usize,
    pub data: String,
}

impl CompletionMessage {
    pub fn new(new_data: String) -> Self {
        Self {
            data: new_data,
            percent_complete: 100,
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct UploadResponse {
    pub request_id: String,
    pub tracking_uri: String,
}

impl UploadResponse {
    pub fn new(request_id: String, tracking_uri: String) -> Self {
        Self { request_id, tracking_uri }
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct TranslationResponse {
    pub id: String,
    pub tracking_uri: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct UploadedFile {
    pub id: String,
    pub docpassword: String,
    pub location: String,
    pub ocrlang: String,
    pub fileext: String,
    pub visualquality: String
}
