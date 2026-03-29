use std::path::PathBuf;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

// ── Protocol enum (local copy to avoid circular dep with ngms-core) ─────

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
