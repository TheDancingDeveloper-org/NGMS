use std::path::PathBuf;

use anyhow::{Context, bail};
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;

use crate::client::{
    ClientStatus, DownloadClient, DownloadItem, DownloadItemStatus, DownloadProtocol, GrabRequest,
};

/// SABnzbd API client.
pub struct SabnzbdClient {
    base_url: String,
    api_key: String,
    http: Client,
}

impl SabnzbdClient {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key: api_key.into(),
            http: Client::new(),
        }
    }

    /// Build a URL with the standard SABnzbd query params.
    fn api_url(&self, mode: &str) -> String {
        format!(
            "{}/api?mode={}&apikey={}&output=json",
            self.base_url, mode, self.api_key
        )
    }

    async fn api_get(&self, mode: &str, extra: &[(&str, &str)]) -> anyhow::Result<Value> {
        let mut url = self.api_url(mode);
        for (k, v) in extra {
            url.push_str(&format!("&{k}={v}"));
        }
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .context("SABnzbd API request failed")?;
        if !resp.status().is_success() {
            bail!("SABnzbd API returned HTTP {}", resp.status());
        }
        resp.json().await.context("failed to parse SABnzbd response")
    }
}

// ── API response shapes ─────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct SabQueueSlot {
    nzo_id: String,
    filename: String,
    status: String,
    #[serde(deserialize_with = "deserialize_sab_size")]
    mb: f64,
    #[serde(deserialize_with = "deserialize_sab_size")]
    mbleft: f64,
    #[serde(default)]
    cat: Option<String>,
    #[serde(default)]
    storage: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SabHistorySlot {
    nzo_id: String,
    name: String,
    status: String,
    bytes: u64,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    storage: Option<String>,
}

/// SABnzbd sometimes returns sizes as strings, sometimes as numbers.
fn deserialize_sab_size<'de, D: serde::Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
    let v: Value = Deserialize::deserialize(d)?;
    match v {
        Value::Number(n) => Ok(n.as_f64().unwrap_or(0.0)),
        Value::String(s) => s.parse::<f64>().map_err(serde::de::Error::custom),
        _ => Ok(0.0),
    }
}

fn mb_to_bytes(mb: f64) -> u64 {
    (mb * 1_048_576.0) as u64
}

fn map_sab_status(status: &str) -> DownloadItemStatus {
    match status {
        "Downloading" => DownloadItemStatus::Downloading,
        "Paused" => DownloadItemStatus::Paused,
        "Queued" | "Idle" | "Fetching" | "Propagating" => DownloadItemStatus::Queued,
        "Extracting" | "Repairing" | "Verifying" | "Running" => DownloadItemStatus::Extracting,
        "Completed" => DownloadItemStatus::Completed,
        "Failed" => DownloadItemStatus::Failed,
        _ => DownloadItemStatus::Queued,
    }
}

fn map_sab_history_status(status: &str) -> DownloadItemStatus {
    match status {
        "Completed" => DownloadItemStatus::Completed,
        "Failed" => DownloadItemStatus::Failed,
        "Extracting" | "Repairing" | "Verifying" | "Running" => DownloadItemStatus::Extracting,
        _ => DownloadItemStatus::Completed,
    }
}

// ── DownloadClient impl ─────────────────────────────────────────────────────

#[async_trait]
impl DownloadClient for SabnzbdClient {
    fn name(&self) -> &str {
        "SABnzbd"
    }

    fn protocol(&self) -> DownloadProtocol {
        DownloadProtocol::Usenet
    }

    async fn add(&self, request: &GrabRequest) -> anyhow::Result<String> {
        let mut extra: Vec<(&str, &str)> = vec![("name", &request.download_url)];
        let cat_val;
        if let Some(cat) = &request.category {
            cat_val = cat.clone();
            extra.push(("cat", &cat_val));
        }
        let result = self.api_get("addurl", &extra).await?;
        let nzo_ids = result["nzo_ids"]
            .as_array()
            .and_then(|a| a.first())
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        Ok(nzo_ids)
    }

    async fn get_items(&self) -> anyhow::Result<Vec<DownloadItem>> {
        let mut items = Vec::new();

        // Active queue
        let queue_resp = self.api_get("queue", &[]).await?;
        if let Some(slots) = queue_resp["queue"]["slots"].as_array() {
            for slot_val in slots {
                if let Ok(slot) = serde_json::from_value::<SabQueueSlot>(slot_val.clone()) {
                    items.push(DownloadItem {
                        download_id: slot.nzo_id,
                        title: slot.filename,
                        status: map_sab_status(&slot.status),
                        total_size: mb_to_bytes(slot.mb),
                        remaining_size: mb_to_bytes(slot.mbleft),
                        output_path: slot.storage.map(PathBuf::from),
                        category: slot.cat,
                        can_move_files: true,
                        can_be_removed: true,
                        protocol: DownloadProtocol::Usenet,
                    });
                }
            }
        }

        // History (completed / failed)
        let hist_resp = self.api_get("history", &[]).await?;
        if let Some(slots) = hist_resp["history"]["slots"].as_array() {
            for slot_val in slots {
                if let Ok(slot) = serde_json::from_value::<SabHistorySlot>(slot_val.clone()) {
                    items.push(DownloadItem {
                        download_id: slot.nzo_id,
                        title: slot.name,
                        status: map_sab_history_status(&slot.status),
                        total_size: slot.bytes,
                        remaining_size: 0,
                        output_path: slot.storage.map(PathBuf::from),
                        category: slot.category,
                        can_move_files: true,
                        can_be_removed: true,
                        protocol: DownloadProtocol::Usenet,
                    });
                }
            }
        }

        Ok(items)
    }

    async fn remove(&self, id: &str, _delete_data: bool) -> anyhow::Result<()> {
        // Try removing from queue first, then history
        let _ = self.api_get("queue", &[("name", "delete"), ("value", id)]).await;
        let _ = self.api_get("history", &[("name", "delete"), ("value", id)]).await;
        Ok(())
    }

    async fn pause(&self, id: &str) -> anyhow::Result<()> {
        self.api_get("pause", &[("value", id)]).await?;
        Ok(())
    }

    async fn resume(&self, id: &str) -> anyhow::Result<()> {
        self.api_get("resume", &[("value", id)]).await?;
        Ok(())
    }

    async fn test(&self) -> anyhow::Result<()> {
        let result = self.api_get("version", &[]).await?;
        if result["version"].is_null() {
            bail!("SABnzbd test failed — no version in response");
        }
        Ok(())
    }

    async fn status(&self) -> anyhow::Result<ClientStatus> {
        let result = self.api_get("version", &[]).await?;
        let version = result["version"].as_str().unwrap_or("unknown").to_string();
        Ok(ClientStatus {
            name: "SABnzbd".to_string(),
            protocol: DownloadProtocol::Usenet,
            version,
            is_connected: true,
        })
    }
}
