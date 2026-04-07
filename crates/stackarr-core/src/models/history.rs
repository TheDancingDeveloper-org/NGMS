use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use super::media::MediaType;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "snake_case")]
#[serde(rename_all = "camelCase")]
pub enum HistoryEventType {
    Grabbed,
    Imported,
    DownloadImported,
    ImportStarted,
    DownloadFailed,
    FileDeleted,
    FileRenamed,
    DownloadIgnored,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEvent {
    pub id: i64,
    pub media_type: MediaType,
    pub media_id: i64,
    pub episode_id: Option<i64>,
    pub event_type: HistoryEventType,
    pub quality: serde_json::Value,
    pub languages: Option<serde_json::Value>,
    pub source_title: String,
    pub download_id: Option<String>,
    pub indexer_id: Option<i32>,
    pub download_client: Option<String>,
    pub data: Option<serde_json::Value>,
    pub occurred_at: DateTime<Utc>,
}
