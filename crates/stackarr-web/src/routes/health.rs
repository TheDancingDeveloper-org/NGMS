use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use serde_json::json;

use crate::AppState;

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
}

async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

async fn system_health(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match sqlx::query("SELECT 1").execute(state.db.pool()).await {
        Ok(_) => Json(json!({
            "status": "ok",
            "checks": { "database": "ok" }
        }))
        .into_response(),
        Err(e) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "status": "unhealthy",
                "checks": { "database": e.to_string() }
            })),
        )
            .into_response(),
    }
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/health", get(health_check))
        .route("/api/v1/system/health", get(system_health))
}
