use std::sync::Arc;

use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::AppState;

// ---------------------------------------------------------------------------
// Request / query types
// ---------------------------------------------------------------------------

/// Will be used when the torrent engine is wired in.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct AddTorrentRequest {
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteTorrentQuery {
    #[serde(default)]
    #[allow(dead_code)]
    delete_files: bool,
}

// ---------------------------------------------------------------------------
// Stub helpers
// ---------------------------------------------------------------------------

fn engine_not_initialized() -> impl IntoResponse {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({
            "error": "torrent engine not initialized"
        })),
    )
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// GET /api/v1/torrent/status
async fn torrent_status() -> impl IntoResponse {
    Json(json!({
        "active": 0,
        "paused": 0,
        "downloadSpeed": 0,
        "uploadSpeed": 0,
        "sessionUptime": 0,
        "enabled": false,
        "message": "Torrent engine not initialized. Enable in settings."
    }))
}

/// GET /api/v1/torrent/list
async fn torrent_list() -> impl IntoResponse {
    Json(json!({
        "torrents": []
    }))
}

/// POST /api/v1/torrent/add
async fn torrent_add() -> impl IntoResponse {
    engine_not_initialized()
}

/// POST /api/v1/torrent/{id}/pause
async fn torrent_pause(Path(_id): Path<String>) -> impl IntoResponse {
    engine_not_initialized()
}

/// POST /api/v1/torrent/{id}/resume
async fn torrent_resume(Path(_id): Path<String>) -> impl IntoResponse {
    engine_not_initialized()
}

/// POST /api/v1/torrent/{id}/delete
async fn torrent_delete(
    Path(_id): Path<String>,
    Query(_params): Query<DeleteTorrentQuery>,
) -> impl IntoResponse {
    engine_not_initialized()
}

/// GET /api/v1/torrent/{id}
async fn torrent_details(Path(id): Path<String>) -> impl IntoResponse {
    Json(json!({
        "id": id,
        "name": "",
        "infoHash": "",
        "state": "unknown",
        "progress": 0.0,
        "totalBytes": 0,
        "downloadedBytes": 0,
        "uploadedBytes": 0,
        "downloadSpeed": 0,
        "uploadSpeed": 0,
        "peers": 0,
        "seeds": 0,
        "eta": 0,
        "files": [],
        "trackers": [],
        "category": ""
    }))
}

/// GET /api/v1/torrent/{id}/stats
async fn torrent_stats(Path(id): Path<String>) -> impl IntoResponse {
    Json(json!({
        "id": id,
        "downloadSpeed": 0,
        "uploadSpeed": 0,
        "peers": 0,
        "seeds": 0,
        "progress": 0.0,
        "downloadedBytes": 0,
        "uploadedBytes": 0,
        "wastedBytes": 0,
        "eta": 0
    }))
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/torrent/status", get(torrent_status))
        .route("/api/v1/torrent/list", get(torrent_list))
        .route("/api/v1/torrent/add", post(torrent_add))
        .route("/api/v1/torrent/{id}", get(torrent_details))
        .route("/api/v1/torrent/{id}/stats", get(torrent_stats))
        .route("/api/v1/torrent/{id}/pause", post(torrent_pause))
        .route("/api/v1/torrent/{id}/resume", post(torrent_resume))
        .route("/api/v1/torrent/{id}/delete", post(torrent_delete))
}
