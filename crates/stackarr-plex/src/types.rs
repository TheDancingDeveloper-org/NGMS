use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

// ── Plex API response types ────────────────────────────────────────────────

/// Top-level Plex XML/JSON container.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexMediaContainer<T> {
    #[serde(rename = "MediaContainer")]
    pub media_container: T,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexServerInfo {
    pub machine_identifier: Option<String>,
    pub friendly_name: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexLibrariesContainer {
    #[serde(default, rename = "Directory")]
    pub directory: Vec<PlexLibrarySection>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexLibrarySection {
    pub key: String,
    pub title: String,
    #[serde(rename = "type")]
    pub section_type: String, // "movie" or "show"
    pub agent: Option<String>,
    pub scanner: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexItemsContainer {
    #[serde(default)]
    pub total_size: Option<i64>,
    #[serde(default, rename = "Metadata")]
    pub metadata: Vec<PlexMetadataItem>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexMetadataItem {
    pub rating_key: String,
    #[serde(default)]
    pub parent_rating_key: Option<String>,
    #[serde(default)]
    pub grandparent_rating_key: Option<String>,
    pub title: String,
    #[serde(rename = "type")]
    pub item_type: String, // "movie", "show", "season", "episode"
    pub guid: Option<String>,
    #[serde(default, rename = "Guid")]
    pub guids: Vec<PlexGuid>,
    #[serde(default)]
    pub added_at: Option<i64>, // unix timestamp
    #[serde(default, rename = "Media")]
    pub media: Vec<PlexMediaInfo>,
    // Season/episode info
    #[serde(default)]
    pub leaf_count: Option<i32>,
    #[serde(default)]
    pub viewed_leaf_count: Option<i32>,
    #[serde(default, rename = "Children")]
    pub children: Option<PlexChildrenContainer>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexChildrenContainer {
    #[serde(default, rename = "Metadata")]
    pub metadata: Vec<PlexMetadataItem>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlexGuid {
    pub id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexMediaInfo {
    #[serde(default)]
    pub width: Option<i32>,
    #[serde(default)]
    pub height: Option<i32>,
    #[serde(default)]
    pub video_resolution: Option<String>,
}

// ── Plex.tv types ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlexTvUser {
    pub id: i64,
    pub uuid: String,
    pub username: String,
    pub email: Option<String>,
    pub thumb: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlexTvUserContainer {
    pub user: PlexTvUser,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexResource {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub client_identifier: String,
    #[serde(default)]
    pub provides: String,
    #[serde(default)]
    pub connections: Vec<PlexConnection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexConnection {
    #[serde(default)]
    pub uri: String,
    #[serde(default)]
    pub local: bool,
    #[serde(default)]
    pub protocol: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlexPinResponse {
    pub id: i64,
    pub code: String,
    #[serde(rename = "authToken")]
    pub auth_token: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexWatchlistContainer {
    #[serde(default, rename = "Metadata")]
    pub metadata: Vec<PlexWatchlistItem>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexWatchlistItem {
    pub rating_key: String,
    pub title: String,
    #[serde(rename = "type")]
    pub item_type: String, // "movie" or "show"
    pub guid: Option<String>,
    #[serde(default, rename = "Guid")]
    pub guids: Vec<PlexGuid>,
}

// ── Database models ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct PlexServer {
    pub id: i64,
    pub name: String,
    pub machine_id: Option<String>,
    pub ip: String,
    pub port: i32,
    pub use_ssl: bool,
    pub verify_tls: bool,
    pub auth_token: Option<String>,
    pub web_app_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct PlexLibrary {
    pub id: i64,
    pub plex_server_id: i64,
    pub section_id: String,
    pub name: String,
    pub enabled: bool,
    pub library_type: String, // "movie" or "show"
    pub last_scan: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct WatchlistEntry {
    pub id: i64,
    pub tmdb_id: i64,
    pub media_type: String,
    pub plex_rating_key: Option<String>,
    pub auto_requested: bool,
    pub created_at: DateTime<Utc>,
}

// ── Input types ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePlexServerInput {
    pub name: Option<String>,
    pub ip: String,
    pub port: Option<i32>,
    pub use_ssl: Option<bool>,
    pub verify_tls: Option<bool>,
    pub auth_token: String,
    pub web_app_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePlexServerInput {
    pub name: Option<String>,
    pub ip: Option<String>,
    pub port: Option<i32>,
    pub use_ssl: Option<bool>,
    pub verify_tls: Option<bool>,
    pub auth_token: Option<String>,
    pub web_app_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePlexLibraryInput {
    pub enabled: bool,
}

// ── Extracted IDs from Plex GUIDs ──────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ExtractedIds {
    pub tmdb_id: Option<i64>,
    pub imdb_id: Option<String>,
    pub tvdb_id: Option<i64>,
}

/// Whether a Plex item is 4K quality.
pub fn is_4k(media: &[PlexMediaInfo]) -> bool {
    media.iter().any(|m| m.width.unwrap_or(0) >= 2000)
}
