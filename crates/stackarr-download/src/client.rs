use std::path::PathBuf;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

// ── Protocol enum (local copy to avoid circular dep with stackarr-core) ─────

/// Whether a release is fetched via Usenet or BitTorrent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DownloadProtocol {
    Usenet,
    Torrent,
}

impl std::fmt::Display for DownloadProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Usenet => write!(f, "usenet"),
            Self::Torrent => write!(f, "torrent"),
        }
    }
}

// ── Request / response types ────────────────────────────────────────────────

/// A request to add a download to a client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrabRequest {
    pub title: String,
    pub download_url: String,
    pub category: Option<String>,
    pub protocol: DownloadProtocol,
}

/// Represents a single item inside a download client's queue / history.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadItem {
    pub download_id: String,
    pub title: String,
    pub status: DownloadItemStatus,
    pub total_size: u64,
    pub remaining_size: u64,
    pub output_path: Option<PathBuf>,
    pub category: Option<String>,
    pub can_move_files: bool,
    pub can_be_removed: bool,
    pub protocol: DownloadProtocol,
    pub error_message: Option<String>,
}

/// Fine-grained status for a download item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DownloadItemStatus {
    Queued,
    Downloading,
    Paused,
    Completed,
    Failed,
    Warning,
    Seeding,
    Extracting,
    Verifying,
}

impl std::fmt::Display for DownloadItemStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Queued => "queued",
            Self::Downloading => "downloading",
            Self::Paused => "paused",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Warning => "warning",
            Self::Seeding => "seeding",
            Self::Extracting => "extracting",
            Self::Verifying => "verifying",
        };
        write!(f, "{s}")
    }
}

/// Status report returned by [`DownloadClient::status`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientStatus {
    pub name: String,
    pub protocol: DownloadProtocol,
    pub version: String,
    pub is_connected: bool,
}

// ── Trait ────────────────────────────────────────────────────────────────────

/// Abstraction over an external download client (qBittorrent, Transmission,
/// SABnzbd, NZBGet, ...).
#[async_trait]
pub trait DownloadClient: Send + Sync {
    /// Human-readable client name (e.g. "qBittorrent").
    fn name(&self) -> &str;

    /// Which protocol this client supports.
    fn protocol(&self) -> DownloadProtocol;

    /// Add a new download.
    async fn add(&self, request: &GrabRequest) -> anyhow::Result<String>;

    /// List all items currently tracked by the client.
    async fn get_items(&self) -> anyhow::Result<Vec<DownloadItem>>;

    /// Remove a download by its client-side ID.
    async fn remove(&self, id: &str, delete_data: bool) -> anyhow::Result<()>;

    /// Pause a download.
    async fn pause(&self, id: &str) -> anyhow::Result<()>;

    /// Resume a paused download.
    async fn resume(&self, id: &str) -> anyhow::Result<()>;

    /// Quick connectivity / auth test.
    async fn test(&self) -> anyhow::Result<()>;

    /// Retrieve version and connection info.
    async fn status(&self) -> anyhow::Result<ClientStatus>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_download_protocol_display() {
        assert_eq!(DownloadProtocol::Usenet.to_string(), "usenet");
        assert_eq!(DownloadProtocol::Torrent.to_string(), "torrent");
    }

    #[test]
    fn test_download_item_status_display() {
        assert_eq!(DownloadItemStatus::Queued.to_string(), "queued");
        assert_eq!(DownloadItemStatus::Downloading.to_string(), "downloading");
        assert_eq!(DownloadItemStatus::Paused.to_string(), "paused");
        assert_eq!(DownloadItemStatus::Completed.to_string(), "completed");
        assert_eq!(DownloadItemStatus::Failed.to_string(), "failed");
        assert_eq!(DownloadItemStatus::Warning.to_string(), "warning");
        assert_eq!(DownloadItemStatus::Seeding.to_string(), "seeding");
        assert_eq!(DownloadItemStatus::Extracting.to_string(), "extracting");
        assert_eq!(DownloadItemStatus::Verifying.to_string(), "verifying");
    }

    #[test]
    fn test_download_protocol_equality() {
        assert_eq!(DownloadProtocol::Usenet, DownloadProtocol::Usenet);
        assert_ne!(DownloadProtocol::Usenet, DownloadProtocol::Torrent);
    }

    #[test]
    fn test_grab_request_serialization() {
        let req = GrabRequest {
            title: "Test Release".into(),
            download_url: "http://example.com/dl".into(),
            category: Some("tv".into()),
            protocol: DownloadProtocol::Torrent,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("Test Release"));
        assert!(json.contains("torrent"));
    }

    #[test]
    fn test_grab_request_deserialization() {
        let json =
            r#"{"title":"Test","download_url":"http://x","category":null,"protocol":"usenet"}"#;
        let req: GrabRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.title, "Test");
        assert_eq!(req.protocol, DownloadProtocol::Usenet);
        assert!(req.category.is_none());
    }

    #[test]
    fn test_client_status_serialization() {
        let status = ClientStatus {
            name: "qBittorrent".into(),
            protocol: DownloadProtocol::Torrent,
            version: "4.6.0".into(),
            is_connected: true,
        };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("qBittorrent"));
        assert!(json.contains("4.6.0"));
    }

    #[test]
    fn test_download_item_serialization_roundtrip() {
        let item = DownloadItem {
            download_id: "abc123".into(),
            title: "Test File".into(),
            status: DownloadItemStatus::Downloading,
            total_size: 1_000_000,
            remaining_size: 500_000,
            output_path: Some(std::path::PathBuf::from("/downloads/test")),
            category: Some("tv".into()),
            can_move_files: true,
            can_be_removed: true,
            protocol: DownloadProtocol::Torrent,
            error_message: None,
        };
        let json = serde_json::to_string(&item).unwrap();
        let deserialized: DownloadItem = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.download_id, "abc123");
        assert_eq!(deserialized.status, DownloadItemStatus::Downloading);
        assert_eq!(deserialized.total_size, 1_000_000);
    }
}
