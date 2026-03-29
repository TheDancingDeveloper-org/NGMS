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
struct LogQuery {
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

/// GET /api/v1/log — return recent log entries from the in-memory buffer.
async fn get_logs(
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

/// GET /api/v1/log/file — list available log files on disk.
async fn list_log_files(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let config = state.config.load();
    let log_dir = config.general.data_dir.join("logs");

    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&log_dir) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_file() {
                    files.push(json!({
                        "name": entry.file_name().to_string_lossy(),
                        "size": meta.len(),
                    }));
                }
            }
        }
    }

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
