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
    pub id: i32,
    pub name: String,
    pub machine_id: Option<String>,
    pub ip: String,
    pub port: i32,
    pub use_ssl: bool,
    pub verify_tls: bool,
    pub auth_token: Option<String>,
    pub web_app_url: Option<String>,
    pub webhook_secret: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct PlexLibrary {
    pub id: i32,
    pub plex_server_id: i32,
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

// ── Plex active sessions (from /status/sessions) ─────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexSessionsContainer {
    #[serde(default, rename = "Metadata")]
    pub metadata: Vec<PlexSession>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexSession {
    #[serde(default)]
    pub rating_key: String,
    #[serde(default)]
    pub title: String,
    #[serde(rename = "type")]
    pub media_type: Option<String>,
    pub grandparent_title: Option<String>,
    pub parent_title: Option<String>,
    pub view_offset: Option<i64>,
    pub duration: Option<i64>,
    #[serde(rename = "User")]
    pub user: Option<PlexSessionUser>,
    #[serde(rename = "Player")]
    pub player: Option<PlexSessionPlayer>,
    #[serde(rename = "TranscodeSession")]
    pub transcode_session: Option<PlexTranscodeSession>,
    #[serde(default, rename = "Media")]
    pub media: Vec<PlexSessionMedia>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlexSessionUser {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub title: String,
    pub thumb: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexSessionPlayer {
    #[serde(default)]
    pub title: String,
    pub machine_identifier: Option<String>,
    pub state: Option<String>,
    pub local: Option<bool>,
    pub platform: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexTranscodeSession {
    pub key: Option<String>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub video_decision: Option<String>,
    pub audio_decision: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub speed: Option<f64>,
    pub progress: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexSessionMedia {
    pub video_resolution: Option<String>,
    pub bitrate: Option<i64>,
    #[serde(default, rename = "Part")]
    pub parts: Vec<PlexSessionPart>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexSessionPart {
    pub decision: Option<String>,
    #[serde(default, rename = "Stream")]
    pub streams: Vec<PlexSessionStream>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexSessionStream {
    pub stream_type: Option<i32>,
    pub codec: Option<String>,
    pub display_title: Option<String>,
    pub decision: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub bitrate: Option<i64>,
}

// ── Plex webhook event types ─────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexWebhookPayload {
    pub event: String,
    #[serde(default)]
    pub user: bool,
    #[serde(default)]
    pub owner: bool,
    #[serde(rename = "Account")]
    pub account: Option<PlexWebhookAccount>,
    #[serde(rename = "Server")]
    pub server: Option<PlexWebhookServer>,
    #[serde(rename = "Player")]
    pub player: Option<PlexWebhookPlayer>,
    #[serde(rename = "Metadata")]
    pub metadata: Option<PlexWebhookMetadata>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlexWebhookAccount {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub title: String,
    pub thumb: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexWebhookServer {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub uuid: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexWebhookPlayer {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub uuid: String,
    #[serde(default)]
    pub local: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexWebhookMetadata {
    #[serde(default)]
    pub title: String,
    pub rating_key: Option<String>,
    #[serde(rename = "type")]
    pub media_type: Option<String>,
    pub year: Option<i32>,
    pub grandparent_title: Option<String>,
    pub parent_title: Option<String>,
}

// ── Plex event DB model ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct PlexEvent {
    pub id: i64,
    pub event_type: String,
    pub plex_server_id: Option<i32>,
    pub user_name: Option<String>,
    pub title: Option<String>,
    pub rating_key: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub thumb_url: Option<String>,
    pub received_at: DateTime<Utc>,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn media(width: Option<i32>) -> PlexMediaInfo {
        PlexMediaInfo {
            width,
            height: None,
            video_resolution: None,
        }
    }

    #[test]
    fn is_4k_empty_media() {
        assert!(!is_4k(&[]));
    }

    #[test]
    fn is_4k_below_threshold() {
        assert!(!is_4k(&[media(Some(1920))]));
    }

    #[test]
    fn is_4k_at_threshold() {
        assert!(is_4k(&[media(Some(2000))]));
    }

    #[test]
    fn is_4k_above_threshold() {
        assert!(is_4k(&[media(Some(3840))]));
    }

    #[test]
    fn is_4k_none_width() {
        assert!(!is_4k(&[media(None)]));
    }

    #[test]
    fn is_4k_mixed_media() {
        assert!(is_4k(&[media(Some(1920)), media(Some(3840))]));
    }

    #[test]
    fn is_4k_all_low_res() {
        assert!(!is_4k(&[media(Some(720)), media(Some(1080))]));
    }
}
