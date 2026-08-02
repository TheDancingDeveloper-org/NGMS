use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::AppState;
use crate::middleware::RequireUser;

// ── Types ────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NotificationQuery {
    unread: Option<bool>,
    limit: Option<i64>,
    offset: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PushSubscriptionBody {
    endpoint: String,
    p256dh: String,
    auth: String,
    user_agent: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PushSubscriptionDeleteBody {
    endpoint: String,
}

// ── Notification routes ──────────────────────────────────────────────────────

async fn list_notifications(
    State(state): State<Arc<AppState>>,
    RequireUser(auth_user): RequireUser,
    Query(q): Query<NotificationQuery>,
) -> impl IntoResponse {
    let unread_only = q.unread.unwrap_or(false);
    let limit = q.limit.unwrap_or(50).min(200);
    let offset = q.offset.unwrap_or(0);

    match state
        .db
        .list_notifications(auth_user.user_id, unread_only, limit, offset)
        .await
    {
        Ok(notifications) => {
            Json(serde_json::to_value(notifications).unwrap_or_default()).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to list notifications");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

async fn unread_count(
    State(state): State<Arc<AppState>>,
    RequireUser(auth_user): RequireUser,
) -> impl IntoResponse {
    match state.db.unread_notification_count(auth_user.user_id).await {
        Ok(count) => Json(json!({ "count": count })).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to get unread count");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

async fn mark_read(
    State(state): State<Arc<AppState>>,
    RequireUser(auth_user): RequireUser,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match state.db.mark_notification_read(id, auth_user.user_id).await {
        Ok(true) => Json(json!({"ok": true})).into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "notification not found"})),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to mark notification read");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

async fn mark_all_read(
    State(state): State<Arc<AppState>>,
    RequireUser(auth_user): RequireUser,
) -> impl IntoResponse {
    match state
        .db
        .mark_all_notifications_read(auth_user.user_id)
        .await
    {
        Ok(count) => Json(json!({"marked": count})).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to mark all notifications read");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

// ── Push subscription routes ─────────────────────────────────────────────────

async fn save_push_subscription(
    State(state): State<Arc<AppState>>,
    RequireUser(auth_user): RequireUser,
    Json(body): Json<PushSubscriptionBody>,
) -> impl IntoResponse {
    if body.endpoint.is_empty() || body.p256dh.is_empty() || body.auth.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "endpoint, p256dh, and auth are required"})),
        )
            .into_response();
    }

    match state
        .db
        .save_push_subscription(
            auth_user.user_id,
            &body.endpoint,
            &body.p256dh,
            &body.auth,
            body.user_agent.as_deref(),
        )
        .await
    {
        Ok(sub) => Json(serde_json::to_value(sub).unwrap_or_default()).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to save push subscription");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

async fn remove_push_subscription(
    State(state): State<Arc<AppState>>,
    RequireUser(auth_user): RequireUser,
    Json(body): Json<PushSubscriptionDeleteBody>,
) -> impl IntoResponse {
    match state
        .db
        .delete_push_subscription(&body.endpoint, auth_user.user_id)
        .await
    {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "subscription not found"})),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to delete push subscription");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

// ── DELETE /api/v1/user/notifications — clear all notifications ─────────────

async fn clear_notifications(
    auth: RequireUser,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let pool = state.db.pool();
    match sqlx::query("DELETE FROM user_notifications WHERE user_id = ?")
        .bind(auth.0.user_id)
        .execute(pool)
        .await
    {
        Ok(r) => Json(json!({"deleted": r.rows_affected()})).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to clear notifications");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

// ── Router ───────────────────────────────────────────────────────────────────

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/v1/user/notifications",
            get(list_notifications).delete(clear_notifications),
        )
        .route("/api/v1/user/notifications/unread-count", get(unread_count))
        .route("/api/v1/user/notifications/{id}/read", put(mark_read))
        .route("/api/v1/user/notifications/read-all", put(mark_all_read))
        .route(
            "/api/v1/user/push-subscription",
            post(save_push_subscription).delete(remove_push_subscription),
        )
}
