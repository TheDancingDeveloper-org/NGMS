use std::path::PathBuf;

use anyhow::{Context, bail};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::RwLock;
use tracing::debug;

use crate::client::{
    ClientStatus, DownloadClient, DownloadItem, DownloadItemStatus, DownloadProtocol, GrabRequest,
};

/// Transmission RPC client.
pub struct TransmissionClient {
    base_url: String,
    username: Option<String>,
    password: Option<String>,
    session_id: RwLock<Option<String>>,
    http: Client,
}

impl TransmissionClient {
    pub fn new(
        base_url: impl Into<String>,
        username: Option<String>,
        password: Option<String>,
    ) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            username,
            password,
            session_id: RwLock::new(None),
            http: Client::new(),
        }
    }

    fn rpc_url(&self) -> String {
        format!("{}/transmission/rpc", self.base_url)
    }

    /// Send a JSON-RPC request, automatically handling 409 session-id refresh.
    async fn rpc(&self, method: &str, arguments: Option<Value>) -> anyhow::Result<Value> {
        let body = RpcRequest {
            method: method.to_string(),
            arguments,
        };

        // First attempt
        let resp = self.send_rpc(&body).await?;
        if resp.status() == reqwest::StatusCode::CONFLICT {
            // Extract session id from header and retry
            if let Some(sid) = resp.headers().get("X-Transmission-Session-Id") {
                let sid = sid.to_str().unwrap_or_default().to_string();
                debug!("Transmission 409 — new session id acquired");
                *self.session_id.write().await = Some(sid);
            }
            let resp = self.send_rpc(&body).await?;
            return self.parse_response(resp).await;
        }
        self.parse_response(resp).await
    }

    async fn send_rpc(&self, body: &RpcRequest) -> anyhow::Result<reqwest::Response> {
        let mut req = self.http.post(self.rpc_url()).json(body);

        if let Some(sid) = self.session_id.read().await.as_deref() {
            req = req.header("X-Transmission-Session-Id", sid);
        }
        if let (Some(u), Some(p)) = (&self.username, &self.password) {
            req = req.basic_auth(u, Some(p));
        }

        req.send().await.context("Transmission RPC request failed")
    }

    async fn parse_response(&self, resp: reqwest::Response) -> anyhow::Result<Value> {
        let status = resp.status();
        let body: Value = resp.json().await.context("failed to parse Transmission response")?;
        let result_str = body["result"].as_str().unwrap_or("");
        if !status.is_success() || result_str != "success" {
            bail!(
                "Transmission RPC error (HTTP {status}): {}",
                body["result"].as_str().unwrap_or("unknown")
            );
        }
        Ok(body["arguments"].clone())
    }
}

// ── Wire types ──────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct RpcRequest {
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    arguments: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct TransTorrent {
    #[serde(rename = "hashString")]
    hash_string: String,
    name: String,
    status: i64,
    #[serde(rename = "totalSize")]
    total_size: u64,
    #[serde(rename = "leftUntilDone")]
    left_until_done: u64,
    #[serde(rename = "downloadDir")]
    download_dir: Option<String>,
}

impl TransTorrent {
    fn to_item(&self) -> DownloadItem {
        DownloadItem {
            download_id: self.hash_string.clone(),
            title: self.name.clone(),
            status: map_transmission_status(self.status),
            total_size: self.total_size,
            remaining_size: self.left_until_done,
            output_path: self.download_dir.as_ref().map(PathBuf::from),
            category: None,
            can_move_files: true,
            can_be_removed: true,
            protocol: DownloadProtocol::Torrent,
        }
    }
}

/// Transmission status codes:
/// 0=stopped, 1=check-wait, 2=check, 3=dl-wait, 4=downloading, 5=seed-wait, 6=seeding
fn map_transmission_status(status: i64) -> DownloadItemStatus {
    match status {
        0 => DownloadItemStatus::Paused,
        1 | 3 => DownloadItemStatus::Queued,
        2 => DownloadItemStatus::Verifying,
        4 => DownloadItemStatus::Downloading,
        5 | 6 => DownloadItemStatus::Seeding,
        _ => DownloadItemStatus::Queued,
    }
}

// ── DownloadClient impl ─────────────────────────────────────────────────────

#[async_trait]
impl DownloadClient for TransmissionClient {
    fn name(&self) -> &str {
        "Transmission"
    }

    fn protocol(&self) -> DownloadProtocol {
        DownloadProtocol::Torrent
    }

    async fn add(&self, request: &GrabRequest) -> anyhow::Result<String> {
        let args = json!({ "filename": request.download_url });
        let result = self.rpc("torrent-add", Some(args)).await?;
        // The response contains either torrent-added or torrent-duplicate
        let hash = result["torrent-added"]["hashString"]
            .as_str()
            .or_else(|| result["torrent-duplicate"]["hashString"].as_str())
            .unwrap_or("unknown")
            .to_string();
        Ok(hash)
    }

    async fn get_items(&self) -> anyhow::Result<Vec<DownloadItem>> {
        let args = json!({
            "fields": ["hashString", "name", "status", "totalSize", "leftUntilDone", "downloadDir"]
        });
        let result = self.rpc("torrent-get", Some(args)).await?;
        let torrents: Vec<TransTorrent> =
            serde_json::from_value(result["torrents"].clone()).unwrap_or_default();
        Ok(torrents.iter().map(TransTorrent::to_item).collect())
    }

    async fn remove(&self, id: &str, delete_data: bool) -> anyhow::Result<()> {
        let args = json!({
            "ids": [id],
            "delete-local-data": delete_data,
        });
        self.rpc("torrent-remove", Some(args)).await?;
        Ok(())
    }

    async fn pause(&self, id: &str) -> anyhow::Result<()> {
        let args = json!({ "ids": [id] });
        self.rpc("torrent-stop", Some(args)).await?;
        Ok(())
    }

    async fn resume(&self, id: &str) -> anyhow::Result<()> {
        let args = json!({ "ids": [id] });
        self.rpc("torrent-start", Some(args)).await?;
        Ok(())
    }

    async fn test(&self) -> anyhow::Result<()> {
        self.rpc("session-get", None).await?;
        Ok(())
    }

    async fn status(&self) -> anyhow::Result<ClientStatus> {
        let result = self.rpc("session-get", None).await?;
        let version = result["version"].as_str().unwrap_or("unknown").to_string();
        Ok(ClientStatus {
            name: "Transmission".to_string(),
            protocol: DownloadProtocol::Torrent,
            version,
            is_connected: true,
        })
    }
}
