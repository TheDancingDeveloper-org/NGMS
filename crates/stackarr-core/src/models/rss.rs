// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use super::media::DownloadProtocol;

// ── Database models ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct RssFeed {
    pub id: i64,
    pub name: String,
    pub url: String,
    pub protocol: DownloadProtocol,
    pub poll_interval_secs: i32,
    pub category: Option<String>,
    pub filter_regex: Option<String>,
    pub enabled: bool,
    pub auto_download: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct RssItem {
    pub id: String,
    pub feed_id: i64,
    pub title: String,
    pub url: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
    pub first_seen_at: DateTime<Utc>,
    pub downloaded: bool,
    pub downloaded_at: Option<DateTime<Utc>>,
    pub category: Option<String>,
    pub size_bytes: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct RssRule {
    pub id: i64,
    pub name: String,
    pub feed_ids: Vec<i64>,
    pub category: Option<String>,
    pub priority: i32,
    pub match_regex: String,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
}

// ── Input types ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRssFeed {
    pub name: String,
    pub url: String,
    pub protocol: DownloadProtocol,
    pub poll_interval_secs: Option<i32>,
    pub category: Option<String>,
    pub filter_regex: Option<String>,
    pub enabled: Option<bool>,
    pub auto_download: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRssFeed {
    pub name: Option<String>,
    pub url: Option<String>,
    pub protocol: Option<DownloadProtocol>,
    pub poll_interval_secs: Option<i32>,
    pub category: Option<String>,
    pub filter_regex: Option<String>,
    pub enabled: Option<bool>,
    pub auto_download: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRssRule {
    pub name: String,
    pub feed_ids: Vec<i64>,
    pub category: Option<String>,
    pub priority: Option<i32>,
    pub match_regex: String,
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRssRule {
    pub name: Option<String>,
    pub feed_ids: Option<Vec<i64>>,
    pub category: Option<String>,
    pub priority: Option<i32>,
    pub match_regex: Option<String>,
    pub enabled: Option<bool>,
}
