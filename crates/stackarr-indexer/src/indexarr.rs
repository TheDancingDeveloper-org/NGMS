// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

use std::collections::HashMap;

use anyhow::{Context, bail};
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::newznab::{self, Protocol, ReleaseInfo};

/// Client for the Indexarr sidecar / distributed indexer (indexarr-rs).
///
/// Indexarr exposes both a Torznab-compatible XML endpoint (`/api/torznab`)
/// and a richer REST JSON API (`/api/v1/*`).  This client wraps both.
pub struct IndexarrClient {
    base_url: String,
    api_key: String,
    http: Client,
}

/// Status returned by indexarr-rs `/api/v1/system/status`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct IndexarrStatus {
    pub version: String,
    pub total_hashes: i64,
    pub resolved_hashes: i64,
    pub workers: Vec<String>,
    pub uptime_seconds: f64,
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

/// A single result from the indexarr-rs REST search API.
#[derive(Debug, Deserialize)]
struct IndexarrSearchResult {
    info_hash: String,
    name: Option<String>,
    size: Option<i64>,
    content_type: Option<String>,
    resolution: Option<String>,
    seed_count: i32,
    peer_count: i32,
    discovered_at: Option<DateTime<Utc>>,
    resolved_at: Option<DateTime<Utc>>,
    #[allow(dead_code)]
    tags: Vec<String>,
}

/// Wrapper for the indexarr-rs search response.
#[derive(Debug, Deserialize)]
struct IndexarrSearchResponse {
    results: Vec<IndexarrSearchResult>,
    #[allow(dead_code)]
    total: i64,
}

/// Compute a Torznab category from content_type + resolution.
fn content_to_category(content_type: Option<&str>, resolution: Option<&str>) -> i32 {
    match content_type {
        Some("movie") => match resolution {
            Some("2160p" | "4320p" | "1440p") => 2045,
            Some("1080p" | "720p") => 2040,
            Some("480p" | "360p" | "576p") => 2030,
            _ => 2000,
        },
        Some("tv_show") => match resolution {
            Some("2160p" | "4320p" | "1440p") => 5045,
            Some("1080p" | "720p") => 5040,
            Some("480p" | "360p" | "576p") => 5030,
            _ => 5000,
        },
        Some("music") => 3000,
        Some("audiobook") => 3030,
        Some("game") => 4050,
        Some("software") => 4010,
        Some("ebook") => 7010,
        Some("comic") => 7020,
        Some("xxx") => 6000,
        _ => 8000,
    }
}

impl IndexarrClient {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key: api_key.into(),
            http: Client::new(),
        }
    }

    /// Perform a Torznab-compatible XML search via indexarr-rs `/api/torznab`.
    pub async fn torznab_search(
        &self,
        params: &HashMap<String, String>,
    ) -> anyhow::Result<Vec<ReleaseInfo>> {
        let mut url = format!("{}/api/torznab?apikey={}", self.base_url, self.api_key);
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

        // Re-use the newznab XML parser.  indexarr-rs speaks standard torznab.
        newznab::parse_newznab_xml_public(&body, 0, "Indexarr", Protocol::Torrent)
    }

    /// Perform a richer REST/JSON search against indexarr-rs `/api/v1/search`.
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

        let search_resp: IndexarrSearchResponse = resp
            .json()
            .await
            .context("failed to parse indexarr REST response")?;

        let now = Utc::now();
        let mut releases: Vec<ReleaseInfo> = search_resp
            .results
            .into_iter()
            .map(|r| {
                let title = r.name.unwrap_or_else(|| r.info_hash.clone());
                let publish_date = r.resolved_at.or(r.discovered_at).unwrap_or(now);
                let age_days = (now - publish_date).num_days();
                let magnet = format!(
                    "magnet:?xt=urn:btih:{}&dn={}",
                    r.info_hash,
                    urlencoding::encode(&title)
                );
                let category =
                    content_to_category(r.content_type.as_deref(), r.resolution.as_deref());

                ReleaseInfo {
                    guid: r.info_hash.clone(),
                    title,
                    download_url: Some(magnet.clone()),
                    info_url: None,
                    indexer_id: -1,
                    indexer_name: "Indexarr".to_string(),
                    protocol: Protocol::Torrent,
                    size: r.size.unwrap_or(0),
                    age_days,
                    publish_date,
                    info_hash: Some(r.info_hash),
                    magnet_url: Some(magnet),
                    seeders: Some(r.seed_count),
                    leechers: Some(r.peer_count),
                    nzb_url: None,
                    tvdb_id: None,
                    imdb_id: None,
                    tmdb_id: None,
                    categories: vec![category],
                    indexer_flags: Vec::new(),
                    indexer_priority: 25,
                    password: None,
                }
            })
            .collect();

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

    /// Fetch the Indexarr service status from `/api/v1/system/status`.
    pub async fn status(&self) -> anyhow::Result<IndexarrStatus> {
        let url = format!("{}/api/v1/system/status", self.base_url);
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

    /// Quick health check — returns `Ok(())` if Indexarr is reachable via
    /// `/health`.
    pub async fn health_check(&self) -> anyhow::Result<()> {
        let url = format!("{}/health", self.base_url);
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .context("indexarr health check request failed")?;

        if resp.status().is_success() {
            Ok(())
        } else {
            bail!("indexarr health check returned HTTP {}", resp.status());
        }
    }
}
