use std::sync::Arc;

use axum::extract::{Path, Query, State};
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

#[derive(Debug, Deserialize)]
struct AddTorrentRequest {
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteTorrentQuery {
    #[serde(default)]
    delete_files: bool,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn engine_not_initialized() -> impl IntoResponse {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({
            "error": "torrent engine not initialized"
        })),
    )
}

fn parse_torrent_id(id: &str) -> Result<librtbit::api::TorrentIdOrHash, impl IntoResponse> {
    librtbit::api::TorrentIdOrHash::parse(id).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": format!("invalid torrent id: {e}") })),
        )
    })
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// GET /api/v1/torrent/status
async fn torrent_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match &state.torrent_api {
        Some(api) => {
            let stats = api.api_session_stats();
            Json(json!({
                "enabled": true,
                "downloadSpeed": stats.download_speed.as_bytes(),
                "uploadSpeed": stats.upload_speed.as_bytes(),
                "sessionUptime": stats.uptime_seconds,
                "peers": {
                    "connecting": stats.peers.connecting,
                    "liveTcp": stats.peers.live_tcp,
                    "liveUtp": stats.peers.live_utp,
                    "dead": stats.peers.dead,
                    "queued": stats.peers.queued,
                    "seen": stats.peers.seen,
                },
                "counters": {
                    "fetchedBytes": stats.counters.fetched_bytes,
                    "uploadedBytes": stats.counters.uploaded_bytes,
                },
            }))
            .into_response()
        }
        None => Json(json!({
            "enabled": false,
            "message": "Torrent engine not enabled"
        }))
        .into_response(),
    }
}

/// GET /api/v1/torrent/list
async fn torrent_list(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match &state.torrent_api {
        Some(api) => {
            let list = api.api_torrent_list();
            Json(json!({
                "torrents": list.torrents,
                "total": list.total,
            }))
            .into_response()
        }
        None => Json(json!({
            "torrents": [],
            "total": 0,
        }))
        .into_response(),
    }
}

/// POST /api/v1/torrent/add
async fn torrent_add(
    State(state): State<Arc<AppState>>,
    Json(body): Json<AddTorrentRequest>,
) -> impl IntoResponse {
    let Some(api) = &state.torrent_api else {
        return engine_not_initialized().into_response();
    };

    let url = match body.url {
        Some(u) => u,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "missing 'url' field" })),
            )
                .into_response();
        }
    };

    let add = librtbit::AddTorrent::from_url(url);
    let opts = librtbit::AddTorrentOptions {
        overwrite: true,
        ..Default::default()
    };

    match api.api_add_torrent(add, Some(opts)).await {
        Ok(resp) => Json(json!({
            "id": resp.id,
            "details": resp.details,
            "outputFolder": resp.output_folder,
        }))
        .into_response(),
        Err(e) => e.into_response(),
    }
}

/// POST /api/v1/torrent/{id}/pause
async fn torrent_pause(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Some(api) = &state.torrent_api else {
        return engine_not_initialized().into_response();
    };

    let idx = match parse_torrent_id(&id) {
        Ok(idx) => idx,
        Err(e) => return e.into_response(),
    };

    match api.api_torrent_action_pause(idx).await {
        Ok(_) => Json(json!({ "success": true })).into_response(),
        Err(e) => e.into_response(),
    }
}

/// POST /api/v1/torrent/{id}/resume
async fn torrent_resume(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Some(api) = &state.torrent_api else {
        return engine_not_initialized().into_response();
    };

    let idx = match parse_torrent_id(&id) {
        Ok(idx) => idx,
        Err(e) => return e.into_response(),
    };

    match api.api_torrent_action_start(idx).await {
        Ok(_) => Json(json!({ "success": true })).into_response(),
        Err(e) => e.into_response(),
    }
}

/// POST /api/v1/torrent/{id}/delete
async fn torrent_delete(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(params): Query<DeleteTorrentQuery>,
) -> impl IntoResponse {
    let Some(api) = &state.torrent_api else {
        return engine_not_initialized().into_response();
    };

    let idx = match parse_torrent_id(&id) {
        Ok(idx) => idx,
        Err(e) => return e.into_response(),
    };

    let result = if params.delete_files {
        api.api_torrent_action_delete(idx).await
    } else {
        api.api_torrent_action_forget(idx).await
    };

    match result {
        Ok(_) => Json(json!({ "success": true })).into_response(),
        Err(e) => e.into_response(),
    }
}

/// GET /api/v1/torrent/{id}
async fn torrent_details(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Some(api) = &state.torrent_api else {
        return engine_not_initialized().into_response();
    };

    let idx = match parse_torrent_id(&id) {
        Ok(idx) => idx,
        Err(e) => return e.into_response(),
    };

    match api.api_torrent_details(idx) {
        Ok(details) => Json(json!(details)).into_response(),
        Err(e) => e.into_response(),
    }
}

/// GET /api/v1/torrent/{id}/stats
async fn torrent_stats(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Some(api) = &state.torrent_api else {
        return engine_not_initialized().into_response();
    };

    let idx = match parse_torrent_id(&id) {
        Ok(idx) => idx,
        Err(e) => return e.into_response(),
    };

    match api.api_stats_v1(idx) {
        Ok(stats) => Json(json!(stats)).into_response(),
        Err(e) => e.into_response(),
    }
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
