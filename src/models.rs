use chrono::{DateTime, Utc};
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Attachment {
    pub id: u32,
    pub saved_path: String,
    pub original_name: String,
    pub mime: String,
}
pub type Attachments = Vec<Attachment>;

#[derive(Debug, Serialize, Deserialize)]
pub struct Entry {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub content: String,
    pub created_at: chrono::DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Attachments>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AttachmentResponse {
    pub id: u32,
    pub mime: String,
    pub original_name: String,
    pub download_url: String,
}
pub type AttachmentsResponse = Vec<AttachmentResponse>;

#[derive(Debug, Serialize, Deserialize)]
pub struct EntryResponse {
    pub id: Option<String>, // For the frontend
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub attachments: AttachmentsResponse,
}

// For download files
#[derive(Debug, Clone)]
pub struct DownloadRequest {
    pub token: String,
    pub file_path: String,
    pub original_name: String,
    pub expires_at: DateTime<Utc>,
}
