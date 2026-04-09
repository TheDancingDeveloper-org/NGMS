use std::path::PathBuf;

use anyhow::{Context, bail};
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::client::{
    ClientStatus, DownloadClient, DownloadItem, DownloadItemStatus, DownloadProtocol, GrabRequest,
};

/// NZBGet JSON-RPC client.
pub struct NzbgetClient {
    base_url: String,
    username: String,
    password: String,
    http: Client,
}

impl NzbgetClient {
    pub fn new(
        base_url: impl Into<String>,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            username: username.into(),
            password: password.into(),
            http: Client::new(),
        }
    }

    fn rpc_url(&self) -> String {
        format!("{}/jsonrpc", self.base_url)
    }

    async fn rpc(&self, method: &str, params: Vec<Value>) -> anyhow::Result<Value> {
        let body = json!({
            "method": method,
            "params": params,
        });
        let resp = self
            .http
            .post(self.rpc_url())
            .basic_auth(&self.username, Some(&self.password))
            .json(&body)
            .send()
            .await
            .context("NZBGet RPC request failed")?;

        if !resp.status().is_success() {
            bail!("NZBGet RPC returned HTTP {}", resp.status());
        }

        let result: Value = resp
            .json()
            .await
            .context("failed to parse NZBGet response")?;
        if let Some(err) = result.get("error")
            && !err.is_null()
        {
            bail!("NZBGet RPC error: {err}");
        }
        Ok(result["result"].clone())
    }
}

// ── Wire types ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct NzbGroup {
    #[serde(rename = "NZBID")]
    nzbid: i64,
    #[serde(rename = "NZBName")]
    nzb_name: String,
    status: String,
    #[serde(rename = "FileSizeLo")]
    file_size_lo: u64,
    #[serde(rename = "FileSizeHi")]
    file_size_hi: u64,
    #[serde(rename = "RemainingSizeLo")]
    remaining_size_lo: u64,
    #[serde(rename = "RemainingSizeHi")]
    remaining_size_hi: u64,
    #[serde(rename = "DestDir")]
    dest_dir: Option<String>,
    category: Option<String>,
}

impl NzbGroup {
    fn total_size(&self) -> u64 {
        self.file_size_lo | (self.file_size_hi << 32)
    }

    fn remaining_size(&self) -> u64 {
        self.remaining_size_lo | (self.remaining_size_hi << 32)
    }

    fn to_item(&self) -> DownloadItem {
        DownloadItem {
            download_id: self.nzbid.to_string(),
            title: self.nzb_name.clone(),
            status: map_nzbget_status(&self.status),
            total_size: self.total_size(),
            remaining_size: self.remaining_size(),
            output_path: self.dest_dir.as_ref().map(PathBuf::from),
            category: self.category.clone(),
            can_move_files: true,
            can_be_removed: true,
            protocol: DownloadProtocol::Usenet,
            error_message: None,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct NzbHistoryItem {
    #[serde(rename = "NZBID")]
    nzbid: i64,
    name: String,
    status: String,
    #[serde(rename = "FileSizeLo")]
    file_size_lo: u64,
    #[serde(rename = "FileSizeHi")]
    file_size_hi: u64,
    #[serde(rename = "DestDir")]
    dest_dir: Option<String>,
    category: Option<String>,
}

impl NzbHistoryItem {
    fn total_size(&self) -> u64 {
        self.file_size_lo | (self.file_size_hi << 32)
    }

    fn to_item(&self) -> DownloadItem {
        let status = if self.status.starts_with("SUCCESS") {
            DownloadItemStatus::Completed
        } else {
            DownloadItemStatus::Failed
        };
        DownloadItem {
            download_id: self.nzbid.to_string(),
            title: self.name.clone(),
            status,
            total_size: self.total_size(),
            remaining_size: 0,
            output_path: self.dest_dir.as_ref().map(PathBuf::from),
            category: self.category.clone(),
            can_move_files: true,
            can_be_removed: true,
            protocol: DownloadProtocol::Usenet,
            error_message: None,
        }
    }
}

fn map_nzbget_status(status: &str) -> DownloadItemStatus {
    match status {
        "DOWNLOADING" => DownloadItemStatus::Downloading,
        "PAUSED" | "QUEUED_PAUSED" => DownloadItemStatus::Paused,
        "QUEUED" => DownloadItemStatus::Queued,
        "PP_QUEUED" | "LOADING_PARS" | "VERIFYING_SOURCES" | "REPAIRING" | "VERIFYING_REPAIRED"
        | "RENAMING" | "UNPACKING" | "MOVING" | "EXECUTING_SCRIPT" => {
            DownloadItemStatus::Extracting
        }
        _ => DownloadItemStatus::Queued,
    }
}

// ── DownloadClient impl ─────────────────────────────────────────────────────

#[async_trait]
impl DownloadClient for NzbgetClient {
    fn name(&self) -> &str {
        "NZBGet"
    }

    fn protocol(&self) -> DownloadProtocol {
        DownloadProtocol::Usenet
    }

    async fn add(&self, request: &GrabRequest) -> anyhow::Result<String> {
        // append(NZBFilename, URL, Category, Priority, AddToTop, AddPaused,
        //        DupeKey, DupeScore, DupeMode, PPParameters)
        let category = request.category.as_deref().unwrap_or("");
        let params = vec![
            json!(""),                   // NZBFilename
            json!(request.download_url), // URL
            json!(category),             // Category
            json!(0),                    // Priority (normal)
            json!(false),                // AddToTop
            json!(false),                // AddPaused
            json!(""),                   // DupeKey
            json!(0),                    // DupeScore
            json!("score"),              // DupeMode
            json!([]),                   // PPParameters
        ];
        let result = self.rpc("append", params).await?;
        let id = result.as_i64().unwrap_or(0);
        if id <= 0 {
            bail!("NZBGet append returned invalid ID: {result}");
        }
        Ok(id.to_string())
    }

    async fn get_items(&self) -> anyhow::Result<Vec<DownloadItem>> {
        let mut items = Vec::new();

        // Active queue
        let groups_val = self.rpc("listgroups", vec![]).await?;
        if let Ok(groups) = serde_json::from_value::<Vec<NzbGroup>>(groups_val) {
            for g in &groups {
                items.push(g.to_item());
            }
        }

        // History
        let hist_val = self.rpc("history", vec![json!(false)]).await?;
        if let Ok(history) = serde_json::from_value::<Vec<NzbHistoryItem>>(hist_val) {
            for h in &history {
                items.push(h.to_item());
            }
        }

        Ok(items)
    }

    async fn remove(&self, id: &str, _delete_data: bool) -> anyhow::Result<()> {
        let nzbid: i64 = id.parse().unwrap_or(0);
        // editqueue(Command, Offset, EditText, IDs)
        self.rpc(
            "editqueue",
            vec![json!("GroupDelete"), json!(0), json!(""), json!([nzbid])],
        )
        .await?;
        Ok(())
    }

    async fn pause(&self, id: &str) -> anyhow::Result<()> {
        let nzbid: i64 = id.parse().unwrap_or(0);
        self.rpc(
            "editqueue",
            vec![json!("GroupPause"), json!(0), json!(""), json!([nzbid])],
        )
        .await?;
        Ok(())
    }

    async fn resume(&self, id: &str) -> anyhow::Result<()> {
        let nzbid: i64 = id.parse().unwrap_or(0);
        self.rpc(
            "editqueue",
            vec![json!("GroupResume"), json!(0), json!(""), json!([nzbid])],
        )
        .await?;
        Ok(())
    }

    async fn test(&self) -> anyhow::Result<()> {
        self.rpc("version", vec![]).await?;
        Ok(())
    }

    async fn status(&self) -> anyhow::Result<ClientStatus> {
        let result = self.rpc("version", vec![]).await?;
        let version = result.as_str().unwrap_or("unknown").to_string();
        Ok(ClientStatus {
            name: "NZBGet".to_string(),
            protocol: DownloadProtocol::Usenet,
            version,
            is_connected: true,
        })
    }
}
