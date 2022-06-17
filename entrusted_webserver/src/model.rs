use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct Notification {
    pub event: String,
    pub id: String,
    pub data: String,
}

impl Notification {
    pub fn new(event: String, id: String, data: String) -> Self {
        Self { event, id, data }
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct CompletionMessage {    
    pub percent_complete: usize,
    pub data: String
}

impl CompletionMessage {
    pub fn new(new_data: String) -> Self {
        Self { data: new_data, percent_complete: 100 }
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct UploadResponse {
    pub id: String,
    pub tracking_uri: String
}

impl UploadResponse {
    pub fn new(id: String, tracking_uri: String) -> Self {
        Self { id, tracking_uri }
    }
}
