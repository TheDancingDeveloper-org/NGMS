// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::json;

use crate::AppState;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn not_enabled() -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({"error": "Indexarr sidecar not enabled"})),
    )
        .into_response()
}

/// Forward a GET request to Indexarr, injecting the API key.
async fn proxy_get(
    base_url: &str,
    api_key: &str,
    path: &str,
    params: &HashMap<String, String>,
) -> Response {
    let client = reqwest::Client::new();
    let url = format!("{base_url}{path}");
    let req = client.get(&url).header("X-Api-Key", api_key).query(params);

    match req.send().await {
        Ok(resp) => {
            let status =
                StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
            let body = resp.text().await.unwrap_or_default();
            (status, [("Content-Type", "application/json")], body).into_response()
        }
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({"error": format!("failed to reach Indexarr: {e}")})),
        )
            .into_response(),
    }
}

/// Forward a POST request (JSON body) to Indexarr, injecting the API key.
async fn proxy_post_json(
    base_url: &str,
    api_key: &str,
    path: &str,
    body: serde_json::Value,
) -> Response {
    let client = reqwest::Client::new();
    let url = format!("{base_url}{path}");
    let req = client.post(&url).header("X-Api-Key", api_key).json(&body);

    match req.send().await {
        Ok(resp) => {
            let status =
                StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
            let body = resp.text().await.unwrap_or_default();
            (status, [("Content-Type", "application/json")], body).into_response()
        }
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({"error": format!("failed to reach Indexarr: {e}")})),
        )
            .into_response(),
    }
}

/// Extract Indexarr URL + API key from state, or return error response.
#[allow(clippy::result_large_err)]
fn indexarr_config(state: &AppState) -> Result<(String, String), Response> {
    let config = state.config.load();
    if !config.indexarr.enabled {
        return Err(not_enabled());
    }
    let api_key = config.indexarr.api_key.clone().unwrap_or_default();
    Ok((
        config.indexarr.url.trim_end_matches('/').to_string(),
        api_key,
    ))
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// GET /api/v1/indexarr/status — check if Indexarr is enabled and reachable.
async fn indexarr_status(State(state): State<Arc<AppState>>) -> Response {
    let (base_url, api_key) = match indexarr_config(&state) {
        Ok(v) => v,
        Err(_) => {
            return Json(json!({"enabled": false})).into_response();
        }
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap_or_default();

    let req = client
        .get(format!("{base_url}/api/v1/system/status"))
        .header("X-Api-Key", &api_key);

    match req.send().await {
        Ok(resp) if resp.status().is_success() => {
            let body: serde_json::Value = resp.json().await.unwrap_or(json!({}));
            Json(json!({
                "enabled": true,
                "reachable": true,
                "indexarr": body,
            }))
            .into_response()
        }
        Ok(resp) => Json(json!({
            "enabled": true,
            "reachable": false,
            "error": format!("Indexarr returned status {}", resp.status()),
        }))
        .into_response(),
        Err(e) => Json(json!({
            "enabled": true,
            "reachable": false,
            "error": format!("{e}"),
        }))
        .into_response(),
    }
}

/// GET /api/v1/indexarr/search — proxy to Indexarr search API.
async fn indexarr_search(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let (base_url, api_key) = match indexarr_config(&state) {
        Ok(v) => v,
        Err(r) => return r,
    };
    proxy_get(&base_url, &api_key, "/api/v1/search", &params).await
}

/// GET /api/v1/indexarr/recent — proxy to Indexarr recent torrents.
async fn indexarr_recent(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let (base_url, api_key) = match indexarr_config(&state) {
        Ok(v) => v,
        Err(r) => return r,
    };
    proxy_get(&base_url, &api_key, "/api/v1/recent", &params).await
}

/// GET /api/v1/indexarr/trending — proxy to Indexarr trending torrents.
async fn indexarr_trending(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let (base_url, api_key) = match indexarr_config(&state) {
        Ok(v) => v,
        Err(r) => return r,
    };
    proxy_get(&base_url, &api_key, "/api/v1/trending", &params).await
}

/// GET /api/v1/indexarr/torrent/{info_hash} — proxy to torrent detail.
async fn indexarr_torrent_detail(
    State(state): State<Arc<AppState>>,
    Path(info_hash): Path<String>,
) -> Response {
    let (base_url, api_key) = match indexarr_config(&state) {
        Ok(v) => v,
        Err(r) => return r,
    };
    proxy_get(
        &base_url,
        &api_key,
        &format!("/api/v1/torrent/{info_hash}"),
        &HashMap::new(),
    )
    .await
}

/// GET /api/v1/indexarr/identity/status — proxy to identity status.
async fn indexarr_identity_status(State(state): State<Arc<AppState>>) -> Response {
    let (base_url, api_key) = match indexarr_config(&state) {
        Ok(v) => v,
        Err(r) => return r,
    };
    proxy_get(
        &base_url,
        &api_key,
        "/api/v1/identity/status",
        &HashMap::new(),
    )
    .await
}

/// POST /api/v1/indexarr/identity/acknowledge — proxy to identity acknowledge.
async fn indexarr_identity_acknowledge(State(state): State<Arc<AppState>>) -> Response {
    let (base_url, api_key) = match indexarr_config(&state) {
        Ok(v) => v,
        Err(r) => return r,
    };
    proxy_post_json(
        &base_url,
        &api_key,
        "/api/v1/identity/acknowledge",
        json!({}),
    )
    .await
}

/// GET /api/v1/indexarr/sync/preferences — proxy to sync preferences.
async fn indexarr_sync_prefs_get(State(state): State<Arc<AppState>>) -> Response {
    let (base_url, api_key) = match indexarr_config(&state) {
        Ok(v) => v,
        Err(r) => return r,
    };
    proxy_get(
        &base_url,
        &api_key,
        "/api/v1/system/sync/preferences",
        &HashMap::new(),
    )
    .await
}

/// POST /api/v1/indexarr/sync/preferences — proxy to update sync preferences.
async fn indexarr_sync_prefs_set(
    State(state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> Response {
    let (base_url, api_key) = match indexarr_config(&state) {
        Ok(v) => v,
        Err(r) => return r,
    };
    proxy_post_json(&base_url, &api_key, "/api/v1/system/sync/preferences", body).await
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/indexarr/status", get(indexarr_status))
        .route("/api/v1/indexarr/search", get(indexarr_search))
        .route("/api/v1/indexarr/recent", get(indexarr_recent))
        .route("/api/v1/indexarr/trending", get(indexarr_trending))
        .route(
            "/api/v1/indexarr/torrent/{info_hash}",
            get(indexarr_torrent_detail),
        )
        .route(
            "/api/v1/indexarr/identity/status",
            get(indexarr_identity_status),
        )
        .route(
            "/api/v1/indexarr/identity/acknowledge",
            post(indexarr_identity_acknowledge),
        )
        .route(
            "/api/v1/indexarr/sync/preferences",
            get(indexarr_sync_prefs_get).post(indexarr_sync_prefs_set),
        )
}
