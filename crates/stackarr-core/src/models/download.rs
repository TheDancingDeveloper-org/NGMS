use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use super::media::{DownloadProtocol, MediaType};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "snake_case")]
#[serde(rename_all = "camelCase")]
pub enum DownloadStatus {
    Queued,
    Downloading,
    Paused,
    PostProcessing,
    Completed,
    Failed,
    Warning,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct QueueItem {
    pub id: i64,
    pub media_type: MediaType,
    pub media_id: i64,
    pub episode_id: Option<i64>,
    pub title: String,
    pub quality: serde_json::Value,
    pub languages: Option<serde_json::Value>,
    pub size: Option<i64>,
    pub status: DownloadStatus,
    pub download_id: String,
    pub download_client_id: Option<i32>,
    pub indexer_id: Option<i32>,
    pub protocol: DownloadProtocol,
    pub error_message: Option<String>,
    pub added_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct DownloadClientConfig {
    pub id: i64,
    pub name: String,
    pub client_type: String,
    pub protocol: DownloadProtocol,
    pub config: serde_json::Value,
    pub enabled: bool,
    pub priority: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct IndexerConfig {
    pub id: i64,
    pub name: String,
    pub indexer_type: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub protocol: DownloadProtocol,
    pub categories: Option<Vec<i32>>,
    pub enabled: bool,
    pub priority: i32,
    pub supports_search: bool,
    pub supports_rss: bool,
    pub config: Option<serde_json::Value>,
    pub last_rss_sync: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseInfo {
    pub guid: String,
    pub title: String,
    pub download_url: Option<String>,
    pub info_url: Option<String>,
    pub indexer_id: i64,
    pub indexer_name: String,
    pub protocol: DownloadProtocol,
    pub size: i64,
    pub age_days: i64,
    pub publish_date: DateTime<Utc>,
    // Torrent-specific
    pub info_hash: Option<String>,
    pub magnet_url: Option<String>,
    pub seeders: Option<i32>,
    pub leechers: Option<i32>,
    // Usenet-specific
    pub nzb_url: Option<String>,
    // External IDs from indexer
    pub tvdb_id: Option<i64>,
    pub imdb_id: Option<String>,
    pub tmdb_id: Option<i64>,
    // Categories
    pub categories: Vec<i32>,
    pub indexer_flags: Vec<String>,
    /// Priority of the source indexer (lower = higher priority).
    #[serde(default = "default_indexer_priority")]
    pub indexer_priority: i32,
}

fn default_indexer_priority() -> i32 {
    25
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Blocklist {
    pub id: i64,
    pub media_type: MediaType,
    pub media_id: i64,
    pub source_title: String,
    pub quality: serde_json::Value,
    pub languages: Option<serde_json::Value>,
    pub indexer_id: Option<i64>,
    pub info_hash: Option<String>,
    pub message: Option<String>,
    pub added_at: DateTime<Utc>,
}
