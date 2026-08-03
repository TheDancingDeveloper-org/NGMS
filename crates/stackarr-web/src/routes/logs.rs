// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::AppState;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogQuery {
    level: Option<String>,
    limit: Option<usize>,
    after_seq: Option<u64>,
    target: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LogResponse {
    entries: Vec<stackarr_core::log_buffer::LogEntry>,
    latest_seq: u64,
}

/// Return recent log entries from the in-memory buffer.
#[utoipa::path(
    get,
    path = "/api/v1/log",
    tag = "Logs",
    operation_id = "getLogs",
    params(
        ("level" = Option<String>, Query, description = "Filter by log level (trace, debug, info, warn, error)"),
        ("limit" = Option<usize>, Query, description = "Max entries to return (default 500, max 5000)"),
        ("after_seq" = Option<u64>, Query, description = "Only return entries after this sequence number"),
        ("target" = Option<String>, Query, description = "Filter by log target module"),
    ),
    responses(
        (status = 200, description = "Log entries"),
    ),
    security(("ApiKeyAuth" = []), ("BearerAuth" = [])),
)]
pub async fn get_logs(
    State(state): State<Arc<AppState>>,
    Query(params): Query<LogQuery>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(500).min(5000);
    let entries = state.log_buffer.get_entries(
        params.after_seq,
        params.level.as_deref(),
        params.target.as_deref(),
        limit,
    );
    let latest_seq = state.log_buffer.latest_seq();

    Json(LogResponse {
        entries,
        latest_seq,
    })
}

/// List available log files on disk.
#[utoipa::path(
    get,
    path = "/api/v1/log/file",
    tag = "Logs",
    operation_id = "listLogFiles",
    responses(
        (status = 200, description = "Available log files"),
    ),
    security(("ApiKeyAuth" = []), ("BearerAuth" = [])),
)]
pub async fn list_log_files(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let config = state.config.load();
    let log_dir = config.general.data_dir.join("logs");

    let scan_dir = log_dir.clone();
    let files = tokio::task::spawn_blocking(move || {
        let mut files = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&scan_dir) {
            for entry in entries.flatten() {
                if let Ok(meta) = entry.metadata()
                    && meta.is_file()
                {
                    files.push(json!({
                        "name": entry.file_name().to_string_lossy(),
                        "size": meta.len(),
                    }));
                }
            }
        }
        files
    })
    .await
    .unwrap_or_default();

    Json(json!({
        "logDir": log_dir.to_string_lossy(),
        "files": files,
    }))
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/log", get(get_logs))
        .route("/api/v1/log/file", get(list_log_files))
}
