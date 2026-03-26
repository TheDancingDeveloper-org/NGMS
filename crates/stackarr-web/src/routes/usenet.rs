use std::sync::Arc;

use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::AppState;

// ---------------------------------------------------------------------------
// Request types
// ---------------------------------------------------------------------------

/// Will be used when the usenet engine is wired in.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddNzbRequest {
    url: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NntpServerRequest {
    name: Option<String>,
    host: Option<String>,
    port: Option<u16>,
    username: Option<String>,
    password: Option<String>,
    ssl: Option<bool>,
    connections: Option<u32>,
    priority: Option<i32>,
    enabled: Option<bool>,
}

// ---------------------------------------------------------------------------
// Stub helpers
// ---------------------------------------------------------------------------

fn engine_not_initialized() -> impl IntoResponse {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({
            "error": "usenet engine not initialized"
        })),
    )
}

// ---------------------------------------------------------------------------
// Status / queue handlers
// ---------------------------------------------------------------------------

/// GET /api/v1/usenet/status
async fn usenet_status() -> impl IntoResponse {
    Json(json!({
        "speed": 0,
        "queueSize": 0,
        "activeDownloads": 0,
        "paused": false,
        "enabled": false,
        "message": "Usenet engine not initialized. Configure NNTP servers in settings."
    }))
}

/// GET /api/v1/usenet/queue
async fn usenet_queue() -> impl IntoResponse {
    Json(json!({
        "jobs": []
    }))
}

/// POST /api/v1/usenet/add
async fn usenet_add() -> impl IntoResponse {
    engine_not_initialized()
}

/// POST /api/v1/usenet/queue/{id}/pause
async fn usenet_queue_pause(Path(_id): Path<String>) -> impl IntoResponse {
    engine_not_initialized()
}

/// POST /api/v1/usenet/queue/{id}/resume
async fn usenet_queue_resume(Path(_id): Path<String>) -> impl IntoResponse {
    engine_not_initialized()
}

/// POST /api/v1/usenet/queue/{id}/delete
async fn usenet_queue_delete(Path(_id): Path<String>) -> impl IntoResponse {
    engine_not_initialized()
}

// ---------------------------------------------------------------------------
// History handlers
// ---------------------------------------------------------------------------

/// GET /api/v1/usenet/history
async fn usenet_history() -> impl IntoResponse {
    Json(json!({
        "records": []
    }))
}

/// POST /api/v1/usenet/history/{id}/retry
async fn usenet_history_retry(Path(_id): Path<String>) -> impl IntoResponse {
    engine_not_initialized()
}

// ---------------------------------------------------------------------------
// NNTP server management handlers
// ---------------------------------------------------------------------------

/// GET /api/v1/usenet/servers
async fn usenet_servers_list() -> impl IntoResponse {
    Json(json!({
        "servers": []
    }))
}

/// POST /api/v1/usenet/servers
async fn usenet_servers_add(Json(_body): Json<NntpServerRequest>) -> impl IntoResponse {
    engine_not_initialized()
}

/// PUT /api/v1/usenet/servers/{id}
async fn usenet_servers_update(
    Path(_id): Path<String>,
    Json(_body): Json<NntpServerRequest>,
) -> impl IntoResponse {
    engine_not_initialized()
}

/// DELETE /api/v1/usenet/servers/{id}
async fn usenet_servers_delete(Path(_id): Path<String>) -> impl IntoResponse {
    engine_not_initialized()
}

/// POST /api/v1/usenet/servers/{id}/test
async fn usenet_servers_test(Path(_id): Path<String>) -> impl IntoResponse {
    engine_not_initialized()
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        // Status & queue
        .route("/api/v1/usenet/status", get(usenet_status))
        .route("/api/v1/usenet/queue", get(usenet_queue))
        .route("/api/v1/usenet/add", post(usenet_add))
        .route("/api/v1/usenet/queue/{id}/pause", post(usenet_queue_pause))
        .route(
            "/api/v1/usenet/queue/{id}/resume",
            post(usenet_queue_resume),
        )
        .route(
            "/api/v1/usenet/queue/{id}/delete",
            post(usenet_queue_delete),
        )
        // History
        .route("/api/v1/usenet/history", get(usenet_history))
        .route(
            "/api/v1/usenet/history/{id}/retry",
            post(usenet_history_retry),
        )
        // NNTP server management
        .route(
            "/api/v1/usenet/servers",
            get(usenet_servers_list).post(usenet_servers_add),
        )
        .route(
            "/api/v1/usenet/servers/{id}",
            put(usenet_servers_update).delete(usenet_servers_delete),
        )
        .route(
            "/api/v1/usenet/servers/{id}/test",
            post(usenet_servers_test),
        )
}
