use std::sync::Arc;

use axum::extract::{Query, State};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::middleware::RequireApiKey;
use crate::AppState;

#[derive(Deserialize)]
struct LogQuery {
    level: Option<String>,
    limit: Option<usize>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LogResponse {
    entries: Vec<serde_json::Value>,
    message: String,
}

/// GET /api/v1/log — return recent log entries.
///
/// Full WebSocket streaming is a future enhancement; for now this returns
/// a placeholder directing users to check container logs.
async fn get_logs(
    _auth: RequireApiKey,
    Query(params): Query<LogQuery>,
) -> impl IntoResponse {
    let _level = params.level.unwrap_or_else(|| "info".to_string());
    let _limit = params.limit.unwrap_or(100);

    // Log files are managed by tracing-subscriber / container runtime.
    // This endpoint returns a message directing to container logs.
    Json(LogResponse {
        entries: Vec::new(),
        message: "Use 'docker logs ngms' or the container runtime to view full logs. WebSocket streaming will be added in a future release.".to_string(),
    })
}

/// GET /api/v1/log/file — list available log files.
async fn list_log_files(
    _auth: RequireApiKey,
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
