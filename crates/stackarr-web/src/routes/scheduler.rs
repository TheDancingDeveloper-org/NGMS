use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::json;

use crate::AppState;
use crate::middleware::RequireAdmin;

/// List all scheduled tasks with their status, last run, and next run times.
#[utoipa::path(
    get,
    path = "/api/v1/scheduler/tasks",
    tag = "Scheduler",
    operation_id = "listSchedulerTasks",
    responses(
        (status = 200, description = "List of scheduled tasks"),
    ),
    security(("ApiKeyAuth" = []), ("BearerAuth" = [])),
)]
pub async fn list_tasks(
    _admin: RequireAdmin,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.scheduler_registry.load_full() {
        Some(registry) => {
            let mut tasks = registry.list_tasks();
            tasks.sort_by(|a, b| a.name.cmp(&b.name));
            Json(serde_json::to_value(&tasks).unwrap_or_default()).into_response()
        }
        None => Json(json!([])).into_response(),
    }
}

/// Manually trigger a scheduled task by name.
#[utoipa::path(
    post,
    path = "/api/v1/scheduler/tasks/{name}/trigger",
    tag = "Scheduler",
    operation_id = "triggerSchedulerTask",
    params(("name" = String, Path, description = "Task name (e.g. rss_sync, health_check, auto_search)")),
    responses(
        (status = 200, description = "Task triggered"),
        (status = 404, description = "Task not found"),
        (status = 503, description = "Scheduler not running"),
    ),
    security(("ApiKeyAuth" = []), ("BearerAuth" = [])),
)]
pub async fn trigger_task(
    _admin: RequireAdmin,
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match state.scheduler_registry.load_full() {
        Some(registry) => {
            if registry.trigger(&name) {
                Json(json!({"ok": true, "message": format!("task '{name}' triggered")}))
                    .into_response()
            } else {
                (
                    StatusCode::NOT_FOUND,
                    Json(json!({"error": format!("task '{name}' not found")})),
                )
                    .into_response()
            }
        }
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error": "scheduler not running"})),
        )
            .into_response(),
    }
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/scheduler/tasks", get(list_tasks))
        .route("/api/v1/scheduler/tasks/{name}/trigger", post(trigger_task))
}
