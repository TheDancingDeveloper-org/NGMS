// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use serde_json::json;

use crate::AppState;

#[derive(Serialize, utoipa::ToSchema)]
pub struct HealthResponse {
    status: &'static str,
}

/// Basic health check — returns 200 if the service is running.
#[utoipa::path(
    get,
    path = "/health",
    tag = "Health",
    operation_id = "healthCheck",
    responses(
        (status = 200, description = "Service is healthy", body = HealthResponse),
    ),
)]
pub async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

/// Comprehensive health check with component status (DB, disk, engines, indexers).
#[utoipa::path(
    get,
    path = "/api/v1/system/health",
    tag = "Health",
    operation_id = "systemHealth",
    responses(
        (status = 200, description = "System is healthy"),
        (status = 503, description = "System is unhealthy"),
    ),
)]
pub async fn system_health(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let mut checks = serde_json::Map::new();
    let mut overall = "ok";

    // 1. Database connectivity
    match sqlx::query("SELECT 1").execute(state.db.pool()).await {
        Ok(_) => {
            checks.insert("database".into(), json!("ok"));
        }
        Err(_) => {
            checks.insert("database".into(), json!("unavailable"));
            overall = "unhealthy";
        }
    }

    // 2. Disk space on media library folders
    let folder_rows: Vec<(String,)> = sqlx::query_as("SELECT path FROM media_library_folders")
        .fetch_all(state.db.pool())
        .await
        .unwrap_or_default();

    let disk_issues = tokio::task::spawn_blocking(move || {
        let mut issues = Vec::new();
        for (path,) in &folder_rows {
            let scan_path = std::path::Path::new(path);
            if !scan_path.exists() {
                issues.push(format!("{path}: path does not exist"));
            } else if let Ok(output) = std::process::Command::new("df")
                .args(["--output=avail", "-B1", path])
                .output()
                && output.status.success()
            {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if let Some(avail_str) = stdout.lines().nth(1)
                    && let Ok(avail) = avail_str.trim().parse::<u64>()
                {
                    // Warn if less than 1 GB free
                    if avail < 1_073_741_824 {
                        issues.push(format!(
                            "{path}: low disk space ({} MB free)",
                            avail / 1_048_576
                        ));
                    }
                }
            }
        }
        issues
    })
    .await
    .unwrap_or_default();

    if disk_issues.is_empty() {
        checks.insert("diskSpace".into(), json!("ok"));
    } else {
        checks.insert("diskSpace".into(), json!({"warning": disk_issues}));
        if overall == "ok" {
            overall = "warning";
        }
    }

    // 3. Embedded torrent engine
    let modules = state.db.load_enabled_modules().await.unwrap_or_default();
    if modules.torrent_embedded {
        if state.torrent_session.load().is_some() {
            checks.insert("torrentEngine".into(), json!("ok"));
        } else {
            checks.insert("torrentEngine".into(), json!("not running"));
            if overall == "ok" {
                overall = "warning";
            }
        }
    }

    // 4. Embedded usenet engine
    if modules.usenet_embedded {
        if state.usenet_queue.load().is_some() {
            checks.insert("usenetEngine".into(), json!("ok"));
        } else {
            checks.insert("usenetEngine".into(), json!("not running"));
            if overall == "ok" {
                overall = "warning";
            }
        }
    }

    // 5. Indexarr sidecar
    if modules.indexarr_sidecar {
        if let Some(ref client) = state.indexarr_client {
            match client.health_check().await {
                Ok(()) => {
                    checks.insert("indexarr".into(), json!("ok"));
                }
                Err(_) => {
                    checks.insert("indexarr".into(), json!("unreachable"));
                    if overall == "ok" {
                        overall = "warning";
                    }
                }
            }
        } else {
            checks.insert("indexarr".into(), json!("not configured"));
        }
    }

    // 6. Indexer count
    let indexer_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM indexers WHERE enabled = true")
            .fetch_one(state.db.pool())
            .await
            .unwrap_or(0);
    checks.insert("indexers".into(), json!({"enabled": indexer_count}));

    // 7. Download client count
    let client_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM download_clients WHERE enabled = true")
            .fetch_one(state.db.pool())
            .await
            .unwrap_or(0);
    checks.insert("downloadClients".into(), json!({"enabled": client_count}));

    let status = match overall {
        "unhealthy" => StatusCode::SERVICE_UNAVAILABLE,
        _ => StatusCode::OK,
    };

    (
        status,
        Json(json!({
            "status": overall,
            "checks": serde_json::Value::Object(checks),
        })),
    )
        .into_response()
}

/// Prometheus-compatible metrics in text exposition format.
#[utoipa::path(
    get,
    path = "/metrics",
    tag = "Health",
    operation_id = "prometheusMetrics",
    responses(
        (status = 200, description = "Prometheus metrics", content_type = "text/plain"),
    ),
)]
pub async fn prometheus_metrics(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let pool = state.db.pool();

    let series_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM series")
        .fetch_one(pool)
        .await
        .unwrap_or(0);
    let movies_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM movies")
        .fetch_one(pool)
        .await
        .unwrap_or(0);
    let episodes_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM episodes")
        .fetch_one(pool)
        .await
        .unwrap_or(0);
    let queue_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM queue")
        .fetch_one(pool)
        .await
        .unwrap_or(0);
    let history_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM history")
        .fetch_one(pool)
        .await
        .unwrap_or(0);
    let indexers_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM indexers WHERE enabled = true")
            .fetch_one(pool)
            .await
            .unwrap_or(0);
    let clients_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM download_clients WHERE enabled = true")
            .fetch_one(pool)
            .await
            .unwrap_or(0);
    let blocklist_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM blocklist")
        .fetch_one(pool)
        .await
        .unwrap_or(0);
    let media_files_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM media_files")
        .fetch_one(pool)
        .await
        .unwrap_or(0);
    let monitored_series: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM series WHERE monitored = true")
            .fetch_one(pool)
            .await
            .unwrap_or(0);
    let monitored_movies: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM movies WHERE monitored = true")
            .fetch_one(pool)
            .await
            .unwrap_or(0);

    let body = format!(
        "# HELP stackarr_series_total Total number of series\n\
         # TYPE stackarr_series_total gauge\n\
         stackarr_series_total {series_count}\n\
         # HELP stackarr_series_monitored Number of monitored series\n\
         # TYPE stackarr_series_monitored gauge\n\
         stackarr_series_monitored {monitored_series}\n\
         # HELP stackarr_movies_total Total number of movies\n\
         # TYPE stackarr_movies_total gauge\n\
         stackarr_movies_total {movies_count}\n\
         # HELP stackarr_movies_monitored Number of monitored movies\n\
         # TYPE stackarr_movies_monitored gauge\n\
         stackarr_movies_monitored {monitored_movies}\n\
         # HELP stackarr_episodes_total Total number of episodes\n\
         # TYPE stackarr_episodes_total gauge\n\
         stackarr_episodes_total {episodes_count}\n\
         # HELP stackarr_media_files_total Total number of media files on disk\n\
         # TYPE stackarr_media_files_total gauge\n\
         stackarr_media_files_total {media_files_count}\n\
         # HELP stackarr_queue_total Items currently in download queue\n\
         # TYPE stackarr_queue_total gauge\n\
         stackarr_queue_total {queue_count}\n\
         # HELP stackarr_history_total Total history events\n\
         # TYPE stackarr_history_total gauge\n\
         stackarr_history_total {history_count}\n\
         # HELP stackarr_blocklist_total Blocklisted releases\n\
         # TYPE stackarr_blocklist_total gauge\n\
         stackarr_blocklist_total {blocklist_count}\n\
         # HELP stackarr_indexers_enabled Enabled indexers\n\
         # TYPE stackarr_indexers_enabled gauge\n\
         stackarr_indexers_enabled {indexers_count}\n\
         # HELP stackarr_download_clients_enabled Enabled download clients\n\
         # TYPE stackarr_download_clients_enabled gauge\n\
         stackarr_download_clients_enabled {clients_count}\n"
    );

    (
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        body,
    )
}

/// Detailed system diagnostics for debugging — DB pool, uptime, memory, storage.
#[utoipa::path(
    get,
    path = "/api/v1/system/diagnostics",
    tag = "System",
    operation_id = "systemDiagnostics",
    responses(
        (status = 200, description = "System diagnostics"),
    ),
    security(("ApiKeyAuth" = []), ("BearerAuth" = [])),
)]
pub async fn system_diagnostics(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let pool = state.db.pool();

    // DB connection pool stats
    let pool_size = pool.size();
    let pool_idle = pool.num_idle();

    // Process uptime
    let uptime_secs = state.start_time.elapsed().as_secs();
    let uptime_human = format!(
        "{}d {}h {}m {}s",
        uptime_secs / 86400,
        (uptime_secs % 86400) / 3600,
        (uptime_secs % 3600) / 60,
        uptime_secs % 60,
    );

    // Process memory (Linux /proc/self/status)
    let memory = tokio::task::spawn_blocking(|| {
        let mut rss_kb: Option<u64> = None;
        let mut vm_kb: Option<u64> = None;
        if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
            for line in status.lines() {
                if let Some(val) = line.strip_prefix("VmRSS:") {
                    rss_kb = val
                        .trim()
                        .strip_suffix("kB")
                        .and_then(|v| v.trim().parse().ok());
                } else if let Some(val) = line.strip_prefix("VmSize:") {
                    vm_kb = val
                        .trim()
                        .strip_suffix("kB")
                        .and_then(|v| v.trim().parse().ok());
                }
            }
        }
        json!({
            "residentMb": rss_kb.map(|k| k / 1024),
            "virtualMb": vm_kb.map(|k| k / 1024),
        })
    })
    .await
    .unwrap_or(json!(null));

    // Storage per media library folder
    let folder_rows: Vec<(String,)> = sqlx::query_as("SELECT path FROM media_library_folders")
        .fetch_all(pool)
        .await
        .unwrap_or_default();

    let storage = tokio::task::spawn_blocking(move || {
        let mut folders = Vec::new();
        for (path,) in &folder_rows {
            let mut info = json!({"path": path});
            if let Ok(output) = std::process::Command::new("df")
                .args(["--output=size,used,avail,pcent", "-B1", path])
                .output()
                && output.status.success()
            {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if let Some(data_line) = stdout.lines().nth(1) {
                    let parts: Vec<&str> = data_line.split_whitespace().collect();
                    if parts.len() >= 4 {
                        info["totalBytes"] = json!(parts[0].parse::<u64>().ok());
                        info["usedBytes"] = json!(parts[1].parse::<u64>().ok());
                        info["availableBytes"] = json!(parts[2].parse::<u64>().ok());
                        info["usedPercent"] = json!(parts[3].trim_end_matches('%'));
                    }
                }
            }
            folders.push(info);
        }
        folders
    })
    .await
    .unwrap_or_default();

    // DB table row counts for major tables
    let db_stats = {
        let tables = [
            "series",
            "movies",
            "episodes",
            "media_files",
            "queue",
            "history",
            "indexers",
            "download_clients",
            "notification_providers",
            "blocklist",
        ];
        let mut map = serde_json::Map::new();
        for table in tables {
            let count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
                .fetch_one(pool)
                .await
                .unwrap_or(0);
            map.insert(table.to_string(), json!(count));
        }
        serde_json::Value::Object(map)
    };

    Json(json!({
        "uptime": {
            "seconds": uptime_secs,
            "human": uptime_human,
        },
        "database": {
            "poolSize": pool_size,
            "poolIdle": pool_idle,
            "poolActive": pool_size - pool_idle as u32,
            "tableCounts": db_stats,
        },
        "memory": memory,
        "storage": storage,
    }))
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/health", get(health_check))
        .route("/api/v1/system/health", get(system_health))
        .route("/api/v1/system/diagnostics", get(system_diagnostics))
        .route("/metrics", get(prometheus_metrics))
}
