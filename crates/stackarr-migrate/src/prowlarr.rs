// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::{Connection, OpenFlags};
use serde::Deserialize;
use tracing::{debug, warn};

// ---------------------------------------------------------------------------
// Intermediate structs – mirror the Prowlarr SQLite schema
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ProwlarrIndexer {
    pub id: i64,
    pub name: String,
    pub implementation: String,
    pub settings: Option<String>,
    pub enable: bool,
    pub priority: i32,
    pub added: Option<String>,
    pub redirect: bool,
    pub app_profile_id: Option<i64>,
    pub tags: Option<String>,
    pub download_client_id: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct ProwlarrTag {
    pub id: i64,
    pub label: String,
}

/// All data extracted from a Prowlarr SQLite database.
#[derive(Debug, Clone)]
pub struct ProwlarrData {
    pub indexers: Vec<ProwlarrIndexer>,
    pub tags: Vec<ProwlarrTag>,
}

// ---------------------------------------------------------------------------
// Settings JSON helpers
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct ProwlarrSettings {
    #[serde(alias = "baseUrl", alias = "BaseUrl")]
    pub base_url: Option<String>,
    #[serde(alias = "apiKey", alias = "ApiKey")]
    pub api_key: Option<String>,
    #[serde(alias = "apiPath", alias = "ApiPath")]
    pub api_path: Option<String>,
    #[serde(alias = "categories", alias = "Categories")]
    pub categories: Option<Vec<i32>>,
    #[serde(alias = "definitionFile", alias = "DefinitionFile")]
    pub definition_file: Option<String>,
}

pub fn parse_prowlarr_settings(json: &str) -> ProwlarrSettings {
    serde_json::from_str(json).unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Protocol mapping
// ---------------------------------------------------------------------------

/// Map Prowlarr Implementation to StackArr protocol.
/// "Newznab" -> usenet, "Cardigann"/"Torznab" -> torrent
pub fn map_prowlarr_protocol(implementation: &str) -> &'static str {
    match implementation {
        "Newznab" => "usenet",
        "Cardigann" | "Torznab" => "torrent",
        _ => "torrent",
    }
}

/// Map Prowlarr Implementation to StackArr indexer_type.
/// "Cardigann" maps to "Torznab" since Cardigann is a torrent indexer framework.
pub fn map_prowlarr_indexer_type(implementation: &str) -> &'static str {
    match implementation {
        "Newznab" => "Newznab",
        "Cardigann" => "Torznab",
        "Torznab" => "Torznab",
        _ => "Torznab",
    }
}

// ---------------------------------------------------------------------------
// Read the entire Prowlarr database
// ---------------------------------------------------------------------------

pub fn read_prowlarr(path: &Path) -> Result<ProwlarrData> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| format!("failed to open Prowlarr DB at {}", path.display()))?;

    debug!("reading Prowlarr database from {}", path.display());

    let indexers = read_indexers(&conn)?;
    let tags = read_tags(&conn)?;

    debug!("Prowlarr: {} indexers, {} tags", indexers.len(), tags.len());

    Ok(ProwlarrData { indexers, tags })
}

fn read_indexers(conn: &Connection) -> Result<Vec<ProwlarrIndexer>> {
    let mut stmt = conn.prepare(
        "SELECT Id, Name, Implementation, Settings, Enable, Priority,
                Added, Redirect, AppProfileId, Tags, DownloadClientId
         FROM Indexers",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(ProwlarrIndexer {
            id: row.get(0)?,
            name: row.get(1)?,
            implementation: row.get(2)?,
            settings: row.get(3)?,
            enable: row.get::<_, i32>(4)? != 0,
            priority: row.get::<_, i32>(5).unwrap_or(25),
            added: row.get(6)?,
            redirect: row.get::<_, i32>(7).unwrap_or(0) != 0,
            app_profile_id: row.get(8)?,
            tags: row.get(9)?,
            download_client_id: row.get(10)?,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        match row {
            Ok(i) => result.push(i),
            Err(e) => warn!("skipping malformed Prowlarr indexer row: {e}"),
        }
    }
    Ok(result)
}

fn read_tags(conn: &Connection) -> Result<Vec<ProwlarrTag>> {
    let mut stmt = conn.prepare("SELECT Id, Label FROM Tags")?;
    let rows = stmt.query_map([], |row| {
        Ok(ProwlarrTag {
            id: row.get(0)?,
            label: row.get(1)?,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        match row {
            Ok(t) => result.push(t),
            Err(e) => warn!("skipping malformed Prowlarr tag row: {e}"),
        }
    }
    Ok(result)
}
