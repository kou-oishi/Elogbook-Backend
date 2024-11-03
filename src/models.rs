use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use mongodb::bson::oid::ObjectId;

#[derive(Debug, Serialize, Deserialize)]
pub struct Attachment {
    pub id: u32,  // シリアル番号
    pub saved_path: String,  // 実際にディレクトリに保存されたパス
    pub original_name: String,  // オリジナルの名前
}
pub type Attachments = Vec<Attachment>;

#[derive(Debug, Serialize, Deserialize)]
pub struct Entry {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub content: String,
    pub created_at: chrono::DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]  // attachmentsが空の場合はスキップ
    pub attachments: Option<Attachments>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EntryResponse {
    pub id: Option<String>,  // For the frontend
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub attachments: Option<Attachments>,
}
