use std::sync::Arc;

use axum::extract::{Query, State};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::AppState;

// ── Types ────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ActivityQuery {
    limit: Option<i64>,
    include_completed: Option<bool>,
}

// ── GET /api/v1/activities ──────────────────────────────────────────────────

async fn list_activities(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ActivityQuery>,
) -> impl IntoResponse {
    let limit = q.limit.unwrap_or(20).min(100);
    let include_completed = q.include_completed.unwrap_or(true);

    match state.db.list_activities(limit, include_completed).await {
        Ok(activities) => {
            Json(serde_json::to_value(activities).unwrap_or_default()).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to list activities");
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

// ── GET /api/v1/activities/running ──────────────────────────────────────────

async fn running_count(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.db.get_running_activity_count().await {
        Ok(count) => Json(json!({"count": count})).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to get running activity count");
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

// ── DELETE /api/v1/activities — clear completed/failed activities ────────────

async fn clear_activities(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.db.delete_old_activities(0).await {
        Ok(deleted) => Json(json!({"deleted": deleted})).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to clear activities");
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

// ── Router ──────────────────────────────────────────────────────────────────

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/v1/activities",
            get(list_activities).delete(clear_activities),
        )
        .route("/api/v1/activities/running", get(running_count))
}
