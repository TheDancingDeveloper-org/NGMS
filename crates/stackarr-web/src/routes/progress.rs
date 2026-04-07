use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{delete, get, put};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::AppState;
use crate::middleware::RequireUser;

// ── Query params ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContinueWatchingQuery {
    limit: Option<i64>,
}

// ── Upsert body ──────────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpsertProgressRequest {
    position_secs: f32,
    duration_secs: f32,
}

// ── GET /api/v1/user/progress/continue ───────────────────────────────────────

async fn get_continue_watching(
    State(state): State<Arc<AppState>>,
    RequireUser(auth_user): RequireUser,
    Query(params): Query<ContinueWatchingQuery>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(20).min(100);

    match state
        .db
        .get_continue_watching_enriched(auth_user.user_id, limit)
        .await
    {
        Ok(items) => Json(serde_json::to_value(items).unwrap_or_default()).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to get continue watching");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

// ── GET /api/v1/user/progress/{mediaFileId} ──────────────────────────────────

async fn get_progress(
    State(state): State<Arc<AppState>>,
    RequireUser(auth_user): RequireUser,
    Path(media_file_id): Path<i64>,
) -> impl IntoResponse {
    match state
        .db
        .get_watch_progress(auth_user.user_id, media_file_id)
        .await
    {
        Ok(Some(progress)) => {
            Json(serde_json::to_value(progress).unwrap_or_default()).into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "no progress found"})),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to get watch progress");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

// ── PUT /api/v1/user/progress/{mediaFileId} ──────────────────────────────────

async fn upsert_progress(
    State(state): State<Arc<AppState>>,
    RequireUser(auth_user): RequireUser,
    Path(media_file_id): Path<i64>,
    Json(body): Json<UpsertProgressRequest>,
) -> impl IntoResponse {
    // Resolve media_file -> media_type, media_id, episode_id
    let resolved = match state.db.resolve_media_file(media_file_id).await {
        Ok(Some(r)) => r,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "media file not found"})),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to resolve media file");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response();
        }
    };

    let (media_type, media_id, episode_id) = resolved;

    // Auto-complete at >90% progress
    let completed = body.duration_secs > 0.0 && (body.position_secs / body.duration_secs) > 0.9;

    match state
        .db
        .upsert_watch_progress(
            auth_user.user_id,
            media_file_id,
            &media_type,
            media_id,
            episode_id,
            body.position_secs,
            body.duration_secs,
            completed,
        )
        .await
    {
        Ok(progress) => Json(serde_json::to_value(progress).unwrap_or_default()).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to upsert watch progress");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

// ── DELETE /api/v1/user/progress/{mediaFileId} ───────────────────────────────

async fn delete_progress(
    State(state): State<Arc<AppState>>,
    RequireUser(auth_user): RequireUser,
    Path(media_file_id): Path<i64>,
) -> impl IntoResponse {
    match state
        .db
        .delete_watch_progress(auth_user.user_id, media_file_id)
        .await
    {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "no progress found"})),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to delete watch progress");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

// ── GET /api/v1/user/progress/series/{seriesId} ─────────────────────────────

async fn get_series_progress(
    State(state): State<Arc<AppState>>,
    RequireUser(auth_user): RequireUser,
    Path(series_id): Path<i64>,
) -> impl IntoResponse {
    match state
        .db
        .get_series_progress(auth_user.user_id, series_id)
        .await
    {
        Ok(rows) => Json(serde_json::to_value(rows).unwrap_or_default()).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to get series progress");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

// ── GET /api/v1/user/progress/movie/{movieId} ───────────────────────────────

async fn get_movie_progress(
    State(state): State<Arc<AppState>>,
    RequireUser(auth_user): RequireUser,
    Path(movie_id): Path<i64>,
) -> impl IntoResponse {
    match state
        .db
        .get_movie_progress(auth_user.user_id, movie_id)
        .await
    {
        Ok(Some(progress)) => {
            Json(serde_json::to_value(progress).unwrap_or_default()).into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "no progress found"})),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to get movie progress");
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
        .route("/api/v1/user/progress/continue", get(get_continue_watching))
        .route(
            "/api/v1/user/progress/series/{seriesId}",
            get(get_series_progress),
        )
        .route(
            "/api/v1/user/progress/movie/{movieId}",
            get(get_movie_progress),
        )
        .route(
            "/api/v1/user/progress/{mediaFileId}",
            get(get_progress)
                .put(upsert_progress)
                .delete(delete_progress),
        )
}
