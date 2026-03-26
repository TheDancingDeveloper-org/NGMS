use std::sync::Arc;

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use crate::AppState;

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
}

async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

async fn system_health(State(_state): State<Arc<AppState>>) -> Json<HealthResponse> {
    // TODO: check DB connectivity, disk space, etc.
    Json(HealthResponse { status: "ok" })
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/health", get(health_check))
        .route("/api/v1/system/health", get(system_health))
}
