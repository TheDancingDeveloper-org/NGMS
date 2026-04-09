use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, bail};
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, warn};

use crate::client::{
    ClientStatus, DownloadClient, DownloadItem, DownloadItemStatus, DownloadProtocol, GrabRequest,
};

/// qBittorrent WebUI API v2 client.
pub struct QBittorrentClient {
    base_url: String,
    username: String,
    password: String,
    http: Arc<Client>,
}

impl QBittorrentClient {
    /// Create a new client.  The underlying `reqwest::Client` is built with a
    /// cookie store so the SID cookie persists across requests.
    pub fn new(
        base_url: impl Into<String>,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        let http = Client::builder()
            .cookie_store(true)
            .build()
            .expect("failed to build reqwest client");
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            username: username.into(),
            password: password.into(),
            http: Arc::new(http),
        }
    }

    /// Authenticate and obtain a session cookie.
    async fn login(&self) -> anyhow::Result<()> {
        let url = format!("{}/api/v2/auth/login", self.base_url);
        let resp = self
            .http
            .post(&url)
            .form(&[("username", &self.username), ("password", &self.password)])
            .send()
            .await
            .context("qBittorrent login request failed")?;

        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if status.is_success() && body.contains("Ok") {
            debug!("qBittorrent login successful");
            Ok(())
        } else {
            bail!("qBittorrent login failed (HTTP {status}): {body}");
        }
    }
}

// ── API response types ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct QBTorrent {
    hash: String,
    name: String,
    state: String,
    size: u64,
    #[serde(default)]
    amount_left: u64,
    save_path: Option<String>,
    category: Option<String>,
}

impl QBTorrent {
    fn to_item(&self) -> DownloadItem {
        DownloadItem {
            download_id: self.hash.clone(),
            title: self.name.clone(),
            status: map_qb_state(&self.state),
            total_size: self.size,
            remaining_size: self.amount_left,
            output_path: self.save_path.as_ref().map(PathBuf::from),
            category: self.category.clone(),
            can_move_files: true,
            can_be_removed: true,
            protocol: DownloadProtocol::Torrent,
            error_message: None,
        }
    }
}

fn map_qb_state(state: &str) -> DownloadItemStatus {
    match state {
        "error" | "missingFiles" => DownloadItemStatus::Failed,
        "uploading" | "stalledUP" | "forcedUP" | "queuedUP" => DownloadItemStatus::Seeding,
        "pausedDL" | "pausedUP" => DownloadItemStatus::Paused,
        "queuedDL" | "metaDL" | "allocating" => DownloadItemStatus::Queued,
        "downloading" | "stalledDL" | "forcedDL" => DownloadItemStatus::Downloading,
        "checkingDL" | "checkingUP" | "checkingResumeData" => DownloadItemStatus::Verifying,
        "moving" => DownloadItemStatus::Extracting,
        _ => DownloadItemStatus::Queued,
    }
}

// ── DownloadClient impl ─────────────────────────────────────────────────────

#[async_trait]
impl DownloadClient for QBittorrentClient {
    fn name(&self) -> &str {
        "qBittorrent"
    }

    fn protocol(&self) -> DownloadProtocol {
        DownloadProtocol::Torrent
    }

    async fn add(&self, request: &GrabRequest) -> anyhow::Result<String> {
        self.login().await?;
        let url = format!("{}/api/v2/torrents/add", self.base_url);

        let mut form = reqwest::multipart::Form::new().text("urls", request.download_url.clone());
        if let Some(cat) = &request.category {
            form = form.text("category", cat.clone());
        }

        let resp = self
            .http
            .post(&url)
            .multipart(form)
            .send()
            .await
            .context("qBittorrent add torrent request failed")?;

        if resp.status().is_success() {
            Ok(String::from("ok"))
        } else {
            let body = resp.text().await.unwrap_or_default();
            bail!("qBittorrent add failed: {body}");
        }
    }

    async fn get_items(&self) -> anyhow::Result<Vec<DownloadItem>> {
        self.login().await?;
        let url = format!("{}/api/v2/torrents/info", self.base_url);
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .context("qBittorrent torrents/info request failed")?;

        let torrents: Vec<QBTorrent> = resp
            .json()
            .await
            .context("failed to parse qBittorrent torrent list")?;
        Ok(torrents.iter().map(QBTorrent::to_item).collect())
    }

    async fn remove(&self, id: &str, delete_data: bool) -> anyhow::Result<()> {
        self.login().await?;
        let url = format!("{}/api/v2/torrents/delete", self.base_url);
        self.http
            .post(&url)
            .form(&[
                ("hashes", id),
                ("deleteFiles", if delete_data { "true" } else { "false" }),
            ])
            .send()
            .await
            .context("qBittorrent delete request failed")?;
        Ok(())
    }

    async fn pause(&self, id: &str) -> anyhow::Result<()> {
        self.login().await?;
        let url = format!("{}/api/v2/torrents/pause", self.base_url);
        self.http
            .post(&url)
            .form(&[("hashes", id)])
            .send()
            .await
            .context("qBittorrent pause request failed")?;
        Ok(())
    }

    async fn resume(&self, id: &str) -> anyhow::Result<()> {
        self.login().await?;
        let url = format!("{}/api/v2/torrents/resume", self.base_url);
        self.http
            .post(&url)
            .form(&[("hashes", id)])
            .send()
            .await
            .context("qBittorrent resume request failed")?;
        Ok(())
    }

    async fn test(&self) -> anyhow::Result<()> {
        self.login().await?;
        let url = format!("{}/api/v2/app/version", self.base_url);
        let resp = self.http.get(&url).send().await?;
        if resp.status().is_success() {
            Ok(())
        } else {
            bail!("qBittorrent test failed: HTTP {}", resp.status());
        }
    }

    async fn status(&self) -> anyhow::Result<ClientStatus> {
        self.login().await?;
        let url = format!("{}/api/v2/app/version", self.base_url);
        let resp = self.http.get(&url).send().await?;
        let version = if resp.status().is_success() {
            resp.text().await.unwrap_or_default()
        } else {
            warn!("qBittorrent version check failed");
            "unknown".to_string()
        };
        Ok(ClientStatus {
            name: "qBittorrent".to_string(),
            protocol: DownloadProtocol::Torrent,
            version,
            is_connected: true,
        })
    }
}
