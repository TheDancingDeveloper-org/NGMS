use std::collections::HashMap;

use anyhow::{Context, bail};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::newznab::{self, Protocol, ReleaseInfo};

/// Client for the Indexarr sidecar / distributed indexer.
///
/// Indexarr exposes both a Torznab-compatible XML endpoint and a richer REST
/// JSON API.  This client wraps both.
pub struct IndexarrClient {
    base_url: String,
    api_key: String,
    http: Client,
}

/// Health status returned by Indexarr's `/api/v1/health` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexarrStatus {
    pub version: String,
    pub peer_count: Option<u64>,
    pub index_count: Option<u64>,
    pub healthy: bool,
}

/// Filters for the Indexarr REST search endpoint.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestSearchFilters {
    pub categories: Option<Vec<i32>>,
    pub min_size: Option<i64>,
    pub max_size: Option<i64>,
    pub min_seeders: Option<i32>,
    pub max_age_days: Option<i64>,
}

impl IndexarrClient {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key: api_key.into(),
            http: Client::new(),
        }
    }

    /// Perform a Torznab-compatible XML search (delegates to [`NewznabClient`]
    /// XML parsing under the hood).
    pub async fn torznab_search(
        &self,
        params: &HashMap<String, String>,
    ) -> anyhow::Result<Vec<ReleaseInfo>> {
        let mut url = format!("{}/api?apikey={}&o=xml", self.base_url, self.api_key);
        for (k, v) in params {
            url.push_str(&format!("&{k}={v}"));
        }

        debug!(url = %url, "indexarr torznab search");
        let body = self
            .http
            .get(&url)
            .send()
            .await
            .context("indexarr torznab request failed")?
            .text()
            .await
            .context("indexarr torznab response body")?;

        // Re-use the newznab XML parser.  Indexarr always speaks torznab.
        newznab::parse_newznab_xml_public(&body, 0, "Indexarr", Protocol::Torrent)
    }

    /// Perform a richer REST/JSON search against Indexarr's own API.
    pub async fn rest_search(
        &self,
        query: &str,
        filters: &RestSearchFilters,
    ) -> anyhow::Result<Vec<ReleaseInfo>> {
        let url = format!("{}/api/v1/search", self.base_url);
        debug!(url = %url, query = %query, "indexarr REST search");

        let resp = self
            .http
            .get(&url)
            .header("X-Api-Key", &self.api_key)
            .query(&[("q", query)])
            .send()
            .await
            .context("indexarr REST search request failed")?;

        if !resp.status().is_success() {
            bail!("indexarr REST search returned HTTP {}", resp.status());
        }

        let mut releases: Vec<ReleaseInfo> = resp
            .json()
            .await
            .context("failed to parse indexarr REST response")?;

        // Apply local filters that the API might not enforce.
        if let Some(ref cats) = filters.categories {
            releases.retain(|r| r.categories.iter().any(|c| cats.contains(c)));
        }
        if let Some(min) = filters.min_size {
            releases.retain(|r| r.size >= min);
        }
        if let Some(max) = filters.max_size {
            releases.retain(|r| r.size <= max);
        }
        if let Some(min_s) = filters.min_seeders {
            releases.retain(|r| r.seeders.unwrap_or(0) >= min_s);
        }
        if let Some(max_age) = filters.max_age_days {
            releases.retain(|r| r.age_days <= max_age);
        }

        Ok(releases)
    }

    /// Fetch the Indexarr service status.
    pub async fn status(&self) -> anyhow::Result<IndexarrStatus> {
        let url = format!("{}/api/v1/status", self.base_url);
        let resp = self
            .http
            .get(&url)
            .header("X-Api-Key", &self.api_key)
            .send()
            .await
            .context("indexarr status request failed")?;

        if !resp.status().is_success() {
            bail!("indexarr status returned HTTP {}", resp.status());
        }
        resp.json()
            .await
            .context("failed to parse indexarr status response")
    }

    /// Quick health check — returns `Ok(())` if Indexarr is reachable and
    /// reports healthy.
    pub async fn health_check(&self) -> anyhow::Result<()> {
        let status = self.status().await?;
        if status.healthy {
            Ok(())
        } else {
            bail!("indexarr reports unhealthy");
        }
    }
}
