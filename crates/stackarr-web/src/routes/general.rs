// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

use std::sync::Arc;

use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::AppState;

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct GeneralConfig {
    instance_name: String,
    auth_method: String,
    grab_strategy: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateGeneralConfig {
    instance_name: Option<String>,
    auth_method: Option<String>,
    grab_strategy: Option<String>,
}

async fn get_general(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let rows = sqlx::query_as::<_, (String, serde_json::Value)>(
        "SELECT key, value FROM app_config WHERE key IN ('instance_name', 'auth_method', 'grab_strategy')",
    )
    .fetch_all(state.db.pool())
    .await
    .unwrap_or_default();

    let mut config = GeneralConfig {
        instance_name: String::new(),
        auth_method: "none".to_string(),
        grab_strategy: "best_quality".to_string(),
    };

    for (key, value) in rows {
        let s = value
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| value.to_string().trim_matches('"').to_string());
        match key.as_str() {
            "instance_name" => config.instance_name = s,
            "auth_method" => config.auth_method = s,
            "grab_strategy" => config.grab_strategy = s,
            _ => {}
        }
    }

    Json(config)
}

async fn put_general(
    State(state): State<Arc<AppState>>,
    Json(body): Json<UpdateGeneralConfig>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    if let Some(name) = &body.instance_name {
        let _ = sqlx::query(
            "INSERT INTO app_config (key, value) VALUES ('instance_name', $1::jsonb)
             ON CONFLICT (key) DO UPDATE SET value = $1::jsonb",
        )
        .bind(serde_json::json!(name))
        .execute(pool)
        .await;
    }

    if let Some(method) = &body.auth_method {
        let _ = sqlx::query(
            "INSERT INTO app_config (key, value) VALUES ('auth_method', $1::jsonb)
             ON CONFLICT (key) DO UPDATE SET value = $1::jsonb",
        )
        .bind(serde_json::json!(method))
        .execute(pool)
        .await;
        state.set_cached_auth_method(method.clone());
    }

    if let Some(strategy) = &body.grab_strategy {
        let _ = sqlx::query(
            "INSERT INTO app_config (key, value) VALUES ('grab_strategy', $1::jsonb)
             ON CONFLICT (key) DO UPDATE SET value = $1::jsonb",
        )
        .bind(serde_json::json!(strategy))
        .execute(pool)
        .await;
    }

    Json(serde_json::json!({"success": true}))
}

// ---------------------------------------------------------------------------
// Bootstrap config
// ---------------------------------------------------------------------------

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct BootstrapConfigResponse {
    enabled: bool,
    url: String,
    token: String,
    advertise_port: Option<u16>,
    upnp_enabled: bool,
    discovery_name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateBootstrapConfig {
    enabled: Option<bool>,
    url: Option<String>,
    token: Option<String>,
    advertise_port: Option<Option<u16>>,
    upnp_enabled: Option<bool>,
    discovery_name: Option<String>,
}

async fn get_bootstrap_config(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let rows = sqlx::query_as::<_, (String, serde_json::Value)>(
        "SELECT key, value FROM app_config WHERE key IN ('bootstrap_enabled', 'bootstrap_url', 'bootstrap_token', 'bootstrap_advertise_port', 'bootstrap_upnp_enabled', 'discovery_name')",
    )
    .fetch_all(state.db.pool())
    .await
    .unwrap_or_default();

    // Fall back to TOML config values
    let toml_config = state.config.load();
    let mut config = BootstrapConfigResponse {
        enabled: toml_config.bootstrap.enabled,
        url: toml_config.bootstrap.url.clone().unwrap_or_default(),
        token: toml_config.bootstrap.token.clone().unwrap_or_default(),
        advertise_port: toml_config.bootstrap.advertise_port,
        upnp_enabled: toml_config.bootstrap.upnp_enabled,
        discovery_name: String::new(),
    };

    // Override with DB values where present
    for (key, value) in rows {
        match key.as_str() {
            "bootstrap_enabled" => {
                if let Some(b) = value.as_bool() {
                    config.enabled = b;
                }
            }
            "bootstrap_url" => {
                if let Some(s) = value.as_str() {
                    config.url = s.to_string();
                }
            }
            "bootstrap_token" => {
                if let Some(s) = value.as_str() {
                    config.token = s.to_string();
                }
            }
            "bootstrap_advertise_port" => {
                if value.is_null() {
                    config.advertise_port = None;
                } else if let Some(n) = value.as_u64() {
                    config.advertise_port = Some(n as u16);
                }
            }
            "bootstrap_upnp_enabled" => {
                if let Some(b) = value.as_bool() {
                    config.upnp_enabled = b;
                }
            }
            "discovery_name" => {
                if let Some(s) = value.as_str() {
                    config.discovery_name = s.to_string();
                }
            }
            _ => {}
        }
    }

    Json(config)
}

async fn put_bootstrap_config(
    State(state): State<Arc<AppState>>,
    Json(body): Json<UpdateBootstrapConfig>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    if let Some(enabled) = body.enabled {
        let _ = sqlx::query(
            "INSERT INTO app_config (key, value) VALUES ('bootstrap_enabled', $1::jsonb)
             ON CONFLICT (key) DO UPDATE SET value = $1::jsonb",
        )
        .bind(serde_json::json!(enabled))
        .execute(pool)
        .await;
    }

    if let Some(url) = &body.url {
        let _ = sqlx::query(
            "INSERT INTO app_config (key, value) VALUES ('bootstrap_url', $1::jsonb)
             ON CONFLICT (key) DO UPDATE SET value = $1::jsonb",
        )
        .bind(serde_json::json!(url))
        .execute(pool)
        .await;
    }

    if let Some(token) = &body.token {
        let _ = sqlx::query(
            "INSERT INTO app_config (key, value) VALUES ('bootstrap_token', $1::jsonb)
             ON CONFLICT (key) DO UPDATE SET value = $1::jsonb",
        )
        .bind(serde_json::json!(token))
        .execute(pool)
        .await;
    }

    if let Some(port) = body.advertise_port {
        let _ = sqlx::query(
            "INSERT INTO app_config (key, value) VALUES ('bootstrap_advertise_port', $1::jsonb)
             ON CONFLICT (key) DO UPDATE SET value = $1::jsonb",
        )
        .bind(serde_json::json!(port))
        .execute(pool)
        .await;
    }

    if let Some(upnp) = body.upnp_enabled {
        let _ = sqlx::query(
            "INSERT INTO app_config (key, value) VALUES ('bootstrap_upnp_enabled', $1::jsonb)
             ON CONFLICT (key) DO UPDATE SET value = $1::jsonb",
        )
        .bind(serde_json::json!(upnp))
        .execute(pool)
        .await;
    }

    if let Some(name) = &body.discovery_name {
        let _ = sqlx::query(
            "INSERT INTO app_config (key, value) VALUES ('discovery_name', $1::jsonb)
             ON CONFLICT (key) DO UPDATE SET value = $1::jsonb",
        )
        .bind(serde_json::json!(name))
        .execute(pool)
        .await;
    }

    Json(serde_json::json!({"success": true}))
}

// ---------------------------------------------------------------------------
// Storage / archive config
// ---------------------------------------------------------------------------
//
// Read-path: merge the TOML-loaded `storage.archive` config with any DB
// overrides in `app_config`. DB keys are:
//   archive_enabled, archive_torrent_dir, archive_nzb_dir,
//   archive_nzb_failed_dir, archive_max_torrent_files,
//   archive_max_nzb_files, archive_max_failed_nzb_files,
//   archive_cleanup_interval_hours
//
// Write-path: persist the submitted values to DB only. The currently-running
// download clients and scheduler task hold snapshots of the config at
// construction, so changes require a restart to take effect — the UI
// surfaces that caveat.

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct StorageArchiveConfig {
    enabled: bool,
    torrent_dir: String,
    nzb_dir: String,
    nzb_failed_dir: String,
    resolved_torrent_dir: String,
    resolved_nzb_dir: String,
    resolved_nzb_failed_dir: String,
    max_torrent_files: usize,
    max_nzb_files: usize,
    max_failed_nzb_files: usize,
    cleanup_interval_hours: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateStorageArchiveConfig {
    enabled: Option<bool>,
    torrent_dir: Option<String>,
    nzb_dir: Option<String>,
    nzb_failed_dir: Option<String>,
    max_torrent_files: Option<usize>,
    max_nzb_files: Option<usize>,
    max_failed_nzb_files: Option<usize>,
    cleanup_interval_hours: Option<u64>,
}

async fn get_storage_config(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let toml = state.config.load();
    let data_dir = toml.general.data_dir.clone();
    let mut config = StorageArchiveConfig {
        enabled: toml.storage.archive.enabled,
        torrent_dir: toml
            .storage
            .archive
            .torrent_dir
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
        nzb_dir: toml
            .storage
            .archive
            .nzb_dir
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
        nzb_failed_dir: toml
            .storage
            .archive
            .nzb_failed_dir
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
        resolved_torrent_dir: toml
            .storage
            .archive
            .resolved_torrent_dir(&data_dir)
            .display()
            .to_string(),
        resolved_nzb_dir: toml
            .storage
            .archive
            .resolved_nzb_dir(&data_dir)
            .display()
            .to_string(),
        resolved_nzb_failed_dir: toml
            .storage
            .archive
            .resolved_nzb_failed_dir(&data_dir)
            .display()
            .to_string(),
        max_torrent_files: toml.storage.archive.max_torrent_files,
        max_nzb_files: toml.storage.archive.max_nzb_files,
        max_failed_nzb_files: toml.storage.archive.max_failed_nzb_files,
        cleanup_interval_hours: toml.storage.archive.cleanup_interval_hours,
    };

    let rows = sqlx::query_as::<_, (String, serde_json::Value)>(
        "SELECT key, value FROM app_config WHERE key IN (
            'archive_enabled', 'archive_torrent_dir', 'archive_nzb_dir',
            'archive_nzb_failed_dir', 'archive_max_torrent_files',
            'archive_max_nzb_files', 'archive_max_failed_nzb_files',
            'archive_cleanup_interval_hours'
        )",
    )
    .fetch_all(state.db.pool())
    .await
    .unwrap_or_default();

    for (key, value) in rows {
        match key.as_str() {
            "archive_enabled" => {
                if let Some(b) = value.as_bool() {
                    config.enabled = b;
                }
            }
            "archive_torrent_dir" => {
                if let Some(s) = value.as_str() {
                    config.torrent_dir = s.to_string();
                }
            }
            "archive_nzb_dir" => {
                if let Some(s) = value.as_str() {
                    config.nzb_dir = s.to_string();
                }
            }
            "archive_nzb_failed_dir" => {
                if let Some(s) = value.as_str() {
                    config.nzb_failed_dir = s.to_string();
                }
            }
            "archive_max_torrent_files" => {
                if let Some(n) = value.as_u64() {
                    config.max_torrent_files = n as usize;
                }
            }
            "archive_max_nzb_files" => {
                if let Some(n) = value.as_u64() {
                    config.max_nzb_files = n as usize;
                }
            }
            "archive_max_failed_nzb_files" => {
                if let Some(n) = value.as_u64() {
                    config.max_failed_nzb_files = n as usize;
                }
            }
            "archive_cleanup_interval_hours" => {
                if let Some(n) = value.as_u64() {
                    config.cleanup_interval_hours = n;
                }
            }
            _ => {}
        }
    }

    Json(config)
}

async fn put_storage_config(
    State(state): State<Arc<AppState>>,
    Json(body): Json<UpdateStorageArchiveConfig>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    async fn set_json(
        pool: &sqlx::PgPool,
        key: &str,
        value: serde_json::Value,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO app_config (key, value) VALUES ($1, $2::jsonb)
             ON CONFLICT (key) DO UPDATE SET value = $2::jsonb",
        )
        .bind(key)
        .bind(value)
        .execute(pool)
        .await?;
        Ok(())
    }

    if let Some(v) = body.enabled {
        let _ = set_json(pool, "archive_enabled", serde_json::json!(v)).await;
    }
    if let Some(v) = body.torrent_dir {
        let _ = set_json(pool, "archive_torrent_dir", serde_json::json!(v)).await;
    }
    if let Some(v) = body.nzb_dir {
        let _ = set_json(pool, "archive_nzb_dir", serde_json::json!(v)).await;
    }
    if let Some(v) = body.nzb_failed_dir {
        let _ = set_json(pool, "archive_nzb_failed_dir", serde_json::json!(v)).await;
    }
    if let Some(v) = body.max_torrent_files {
        let _ = set_json(pool, "archive_max_torrent_files", serde_json::json!(v)).await;
    }
    if let Some(v) = body.max_nzb_files {
        let _ = set_json(pool, "archive_max_nzb_files", serde_json::json!(v)).await;
    }
    if let Some(v) = body.max_failed_nzb_files {
        let _ = set_json(pool, "archive_max_failed_nzb_files", serde_json::json!(v)).await;
    }
    if let Some(v) = body.cleanup_interval_hours {
        let _ = set_json(pool, "archive_cleanup_interval_hours", serde_json::json!(v)).await;
    }

    Json(serde_json::json!({
        "success": true,
        "restartRequired": true,
    }))
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/config/general", get(get_general).put(put_general))
        .route(
            "/api/v1/config/bootstrap",
            get(get_bootstrap_config).put(put_bootstrap_config),
        )
        .route(
            "/api/v1/config/storage",
            get(get_storage_config).put(put_storage_config),
        )
}
