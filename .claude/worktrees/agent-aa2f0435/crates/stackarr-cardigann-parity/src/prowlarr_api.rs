//! Prowlarr REST API client for the parity test harness.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

/// Client for interacting with Prowlarr's v1 REST API.
#[derive(Debug, Clone)]
pub struct ProwlarrClient {
    base_url: String,
    api_key: String,
    http: reqwest::Client,
}

/// An indexer as returned by Prowlarr's API.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProwlarrIndexer {
    pub id: i64,
    pub name: String,
    pub implementation: String,
    pub implementation_name: Option<String>,
    pub enable: bool,
    pub protocol: String,
    pub fields: Vec<ProwlarrField>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProwlarrField {
    pub name: String,
    #[serde(default)]
    pub value: Option<serde_json::Value>,
}

/// A search result from Prowlarr.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProwlarrRelease {
    pub guid: Option<String>,
    pub title: Option<String>,
    pub download_url: Option<String>,
    pub info_url: Option<String>,
    pub size: Option<i64>,
    pub publish_date: Option<String>,
    pub seeders: Option<i32>,
    pub leechers: Option<i32>,
    pub indexer_id: Option<i64>,
    pub indexer: Option<String>,
    pub categories: Option<Vec<ProwlarrCategory>>,
    pub info_hash: Option<String>,
    pub imdb_id: Option<i64>,
    pub tmdb_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProwlarrCategory {
    pub id: Option<i32>,
    pub name: Option<String>,
}

/// Payload for creating an indexer in Prowlarr.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateIndexerPayload {
    pub name: String,
    pub implementation: String,
    pub implementation_name: String,
    pub config_contract: String,
    pub fields: Vec<ProwlarrField>,
    pub enable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_profile_id: Option<i64>,
}

impl ProwlarrClient {
    pub fn new(base_url: &str, api_key: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_owned(),
            api_key: api_key.to_owned(),
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("failed to build HTTP client"),
        }
    }

    /// Wait for Prowlarr to become ready.
    pub async fn wait_ready(&self, timeout: Duration) -> Result<()> {
        let start = std::time::Instant::now();
        loop {
            match self.ping().await {
                Ok(_) => return Ok(()),
                Err(_) if start.elapsed() < timeout => {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
                Err(e) => bail!("Prowlarr not ready after {timeout:?}: {e}"),
            }
        }
    }

    async fn ping(&self) -> Result<()> {
        let url = format!("{}/ping", self.base_url);
        self.http.get(&url).send().await?.error_for_status()?;
        Ok(())
    }

    /// List all configured indexers.
    pub async fn list_indexers(&self) -> Result<Vec<ProwlarrIndexer>> {
        let url = format!("{}/api/v1/indexer", self.base_url);
        let resp = self
            .http
            .get(&url)
            .header("X-Api-Key", &self.api_key)
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }

    /// Add a new indexer.
    pub async fn add_indexer(&self, payload: &CreateIndexerPayload) -> Result<ProwlarrIndexer> {
        let url = format!("{}/api/v1/indexer", self.base_url);
        let resp = self
            .http
            .post(&url)
            .header("X-Api-Key", &self.api_key)
            .json(payload)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("failed to add indexer '{}': {} - {}", payload.name, status, body);
        }

        Ok(resp.json().await?)
    }

    /// Search across indexers.
    pub async fn search(
        &self,
        query: &str,
        indexer_ids: Option<&[i64]>,
    ) -> Result<Vec<ProwlarrRelease>> {
        let mut url = format!(
            "{}/api/v1/search?query={}&type=search&limit=100",
            self.base_url,
            urlencoding::encode(query)
        );

        if let Some(ids) = indexer_ids {
            for id in ids {
                url.push_str(&format!("&indexerIds={id}"));
            }
        }

        let resp = self
            .http
            .get(&url)
            .header("X-Api-Key", &self.api_key)
            .timeout(Duration::from_secs(60))
            .send()
            .await?
            .error_for_status()?;

        Ok(resp.json().await?)
    }

    /// Search a specific indexer.
    pub async fn search_indexer(
        &self,
        indexer_id: i64,
        query: &str,
    ) -> Result<Vec<ProwlarrRelease>> {
        self.search(query, Some(&[indexer_id])).await
    }
}

/// Auto-detect the Prowlarr API key from a Docker container.
pub async fn detect_api_key(container_name: &str) -> Result<String> {
    let output = tokio::process::Command::new("docker")
        .args(["exec", container_name, "cat", "/config/config.xml"])
        .output()
        .await
        .context("failed to exec docker command")?;

    let config = String::from_utf8_lossy(&output.stdout);
    let re = regex::Regex::new(r"<ApiKey>([^<]+)</ApiKey>")?;
    let caps = re
        .captures(&config)
        .ok_or_else(|| anyhow::anyhow!("could not find ApiKey in Prowlarr config.xml"))?;

    Ok(caps[1].to_owned())
}
