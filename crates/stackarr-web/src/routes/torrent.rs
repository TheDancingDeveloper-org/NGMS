use std::num::NonZeroU32;
use std::sync::Arc;

use axum::extract::{Multipart, Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
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
    let torrent_api = state.torrent_api.load_full();
    match &torrent_api {
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
    let torrent_api = state.torrent_api.load_full();
    match &torrent_api {
        Some(api) => {
            let list = api.api_torrent_list_ext(librtbit::api::ApiTorrentListOpts {
                with_stats: true,
                ..Default::default()
            });
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
    let torrent_api = state.torrent_api.load_full();
    let Some(api) = &torrent_api else {
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

/// POST /api/v1/torrent/add/upload — accepts multipart .torrent file uploads.
async fn torrent_add_upload(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let torrent_api = state.torrent_api.load_full();
    let Some(api) = &torrent_api else {
        return engine_not_initialized().into_response();
    };

    let mut file_bytes: Option<Vec<u8>> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        if name == "file" {
            match field.bytes().await {
                Ok(b) => file_bytes = Some(b.to_vec()),
                Err(e) => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(json!({ "error": format!("failed to read uploaded file: {e}") })),
                    )
                        .into_response();
                }
            }
        }
    }

    let file_bytes = match file_bytes {
        Some(b) if !b.is_empty() => b,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "no torrent file uploaded" })),
            )
                .into_response();
        }
    };

    let add = librtbit::AddTorrent::from_bytes(file_bytes);
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
    let torrent_api = state.torrent_api.load_full();
    let Some(api) = &torrent_api else {
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
    let torrent_api = state.torrent_api.load_full();
    let Some(api) = &torrent_api else {
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
    let torrent_api = state.torrent_api.load_full();
    let Some(api) = &torrent_api else {
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
    let torrent_api = state.torrent_api.load_full();
    let Some(api) = &torrent_api else {
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
    let torrent_api = state.torrent_api.load_full();
    let Some(api) = &torrent_api else {
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
// Settings types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TorrentSettings {
    download_folder: String,
    completed_folder: Option<String>,
    upload_limit_bps: u32,
    download_limit_bps: u32,
    peer_limit: usize,
    concurrent_init_limit: usize,
    dht_enabled: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateTorrentSettings {
    download_folder: Option<String>,
    completed_folder: Option<Option<String>>,
    upload_limit_bps: Option<u32>,
    download_limit_bps: Option<u32>,
    peer_limit: Option<usize>,
    concurrent_init_limit: Option<usize>,
}

// ---------------------------------------------------------------------------
// Settings handlers
// ---------------------------------------------------------------------------

/// GET /api/v1/torrent/settings
async fn torrent_settings_get(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let torrent_api = state.torrent_api.load_full();
    let Some(api) = &torrent_api else {
        return engine_not_initialized().into_response();
    };

    let session = api.session();
    let settings = TorrentSettings {
        download_folder: api.api_output_folder(),
        completed_folder: api.api_completed_folder(),
        upload_limit_bps: session
            .ratelimits
            .get_upload_bps()
            .map(|v| v.get())
            .unwrap_or(0),
        download_limit_bps: session
            .ratelimits
            .get_download_bps()
            .map(|v| v.get())
            .unwrap_or(0),
        peer_limit: session.get_peer_limit(),
        concurrent_init_limit: session.get_concurrent_init_limit(),
        dht_enabled: session.get_dht().is_some(),
    };

    Json(settings).into_response()
}

/// PUT /api/v1/torrent/settings
async fn torrent_settings_update(
    State(state): State<Arc<AppState>>,
    Json(body): Json<UpdateTorrentSettings>,
) -> impl IntoResponse {
    let torrent_api = state.torrent_api.load_full();
    let Some(api) = &torrent_api else {
        return engine_not_initialized().into_response();
    };

    let session = api.session();

    if let Some(ref folder) = body.download_folder {
        let _ = std::fs::create_dir_all(folder);
        api.api_set_output_folder(folder.clone());
        let _ = sqlx::query(
            "INSERT INTO app_config (key, value) VALUES ('torrent_download_dir', $1::jsonb) \
             ON CONFLICT (key) DO UPDATE SET value = $1::jsonb",
        )
        .bind(serde_json::json!(folder))
        .execute(state.db.pool())
        .await;
    }
    if let Some(ref folder) = body.completed_folder {
        api.api_set_completed_folder(folder.clone());
        if let Some(f) = folder.as_ref() {
            let _ = std::fs::create_dir_all(f);
        }
        let _ = sqlx::query(
            "INSERT INTO app_config (key, value) VALUES ('torrent_complete_dir', $1::jsonb) \
             ON CONFLICT (key) DO UPDATE SET value = $1::jsonb",
        )
        .bind(serde_json::json!(folder))
        .execute(state.db.pool())
        .await;
    }
    if let Some(bps) = body.upload_limit_bps {
        session.ratelimits.set_upload_bps(NonZeroU32::new(bps));
    }
    if let Some(bps) = body.download_limit_bps {
        session.ratelimits.set_download_bps(NonZeroU32::new(bps));
    }
    if let Some(limit) = body.peer_limit {
        session.set_peer_limit(limit);
    }
    if let Some(limit) = body.concurrent_init_limit {
        session.set_concurrent_init_limit(limit);
    }

    // Return the updated settings
    let settings = TorrentSettings {
        download_folder: api.api_output_folder(),
        completed_folder: api.api_completed_folder(),
        upload_limit_bps: session
            .ratelimits
            .get_upload_bps()
            .map(|v| v.get())
            .unwrap_or(0),
        download_limit_bps: session
            .ratelimits
            .get_download_bps()
            .map(|v| v.get())
            .unwrap_or(0),
        peer_limit: session.get_peer_limit(),
        concurrent_init_limit: session.get_concurrent_init_limit(),
        dht_enabled: session.get_dht().is_some(),
    };

    Json(settings).into_response()
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/torrent/status", get(torrent_status))
        .route("/api/v1/torrent/list", get(torrent_list))
        .route(
            "/api/v1/torrent/settings",
            get(torrent_settings_get).put(torrent_settings_update),
        )
        .route("/api/v1/torrent/add", post(torrent_add))
        .route("/api/v1/torrent/add/upload", post(torrent_add_upload))
        .route("/api/v1/torrent/{id}", get(torrent_details))
        .route("/api/v1/torrent/{id}/stats", get(torrent_stats))
        .route("/api/v1/torrent/{id}/pause", post(torrent_pause))
        .route("/api/v1/torrent/{id}/resume", post(torrent_resume))
        .route("/api/v1/torrent/{id}/delete", post(torrent_delete))
}
