// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::AppState;
use crate::middleware::{RequireAdmin, RequireUser};

// ── Types ────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateRequestBody {
    media_type: String,
    tmdb_id: i64,
    title: String,
    year: Option<i32>,
    poster_url: Option<String>,
    overview: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListRequestsQuery {
    status: Option<String>,
    mine: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AdminNoteBody {
    note: Option<String>,
}

// ── Handlers ─────────────────────────────────────────────────────────────────

async fn create_request(
    State(state): State<Arc<AppState>>,
    RequireUser(user): RequireUser,
    Json(body): Json<CreateRequestBody>,
) -> impl IntoResponse {
    // Validate media_type
    if body.media_type != "movie" && body.media_type != "series" {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "mediaType must be 'movie' or 'series'"})),
        )
            .into_response();
    }

    // Check if already in library (series by tmdb_id or movies by tmdb_id)
    let pool = state.db.pool();
    let in_library = if body.media_type == "series" {
        let row: Option<(i64,)> = sqlx::query_as("SELECT id FROM series WHERE tmdb_id = $1")
            .bind(body.tmdb_id)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();
        row.is_some()
    } else {
        let row: Option<(i64,)> = sqlx::query_as("SELECT id FROM movies WHERE tmdb_id = $1")
            .bind(body.tmdb_id)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();
        row.is_some()
    };

    if in_library {
        return (
            StatusCode::CONFLICT,
            Json(json!({"error": "This title is already in your library"})),
        )
            .into_response();
    }

    // Check for duplicate request
    if let Ok(Some(existing)) = state
        .db
        .check_request_exists(body.tmdb_id, &body.media_type)
        .await
    {
        return (
            StatusCode::CONFLICT,
            Json(json!({
                "error": "A request for this title already exists",
                "existingRequest": {
                    "id": existing.id,
                    "status": existing.status,
                }
            })),
        )
            .into_response();
    }

    match state
        .db
        .create_media_request(
            user.user_id,
            &body.media_type,
            body.tmdb_id,
            &body.title,
            body.year,
            body.poster_url.as_deref(),
            body.overview.as_deref(),
        )
        .await
    {
        Ok(request) => (StatusCode::CREATED, Json(json!(request))).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to create media request");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "failed to create request"})),
            )
                .into_response()
        }
    }
}

async fn list_requests(
    State(state): State<Arc<AppState>>,
    RequireUser(user): RequireUser,
    Query(query): Query<ListRequestsQuery>,
) -> impl IntoResponse {
    let is_admin = user.role == "admin";
    let user_filter = if is_admin && query.mine != Some(true) {
        None
    } else {
        Some(user.user_id)
    };

    match state
        .db
        .list_media_requests(query.status.as_deref(), user_filter)
        .await
    {
        Ok(requests) => Json(json!(requests)).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to list media requests");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "failed to list requests"})),
            )
                .into_response()
        }
    }
}

async fn get_request(
    State(state): State<Arc<AppState>>,
    RequireUser(user): RequireUser,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match state.db.get_media_request(id).await {
        Ok(Some(request)) => {
            // Non-admin users can only see their own requests
            if user.role != "admin" && request.user_id != user.user_id {
                return StatusCode::NOT_FOUND.into_response();
            }
            Json(json!(request)).into_response()
        }
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to get media request");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "failed to get request"})),
            )
                .into_response()
        }
    }
}

async fn delete_request(
    State(state): State<Arc<AppState>>,
    RequireAdmin(_user): RequireAdmin,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match state.db.delete_media_request(id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to delete media request");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "failed to delete request"})),
            )
                .into_response()
        }
    }
}

async fn approve_request(
    State(state): State<Arc<AppState>>,
    RequireAdmin(user): RequireAdmin,
    Path(id): Path<i64>,
    Json(body): Json<AdminNoteBody>,
) -> impl IntoResponse {
    match state
        .db
        .update_request_status(id, "approved", Some(user.user_id), body.note.as_deref())
        .await
    {
        Ok(true) => {
            if let Ok(Some(req)) = state.db.get_media_request(id).await {
                Json(json!(req)).into_response()
            } else {
                StatusCode::OK.into_response()
            }
        }
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to approve media request");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "failed to approve request"})),
            )
                .into_response()
        }
    }
}

async fn decline_request(
    State(state): State<Arc<AppState>>,
    RequireAdmin(user): RequireAdmin,
    Path(id): Path<i64>,
    Json(body): Json<AdminNoteBody>,
) -> impl IntoResponse {
    match state
        .db
        .update_request_status(id, "declined", Some(user.user_id), body.note.as_deref())
        .await
    {
        Ok(true) => {
            if let Ok(Some(req)) = state.db.get_media_request(id).await {
                Json(json!(req)).into_response()
            } else {
                StatusCode::OK.into_response()
            }
        }
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to decline media request");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "failed to decline request"})),
            )
                .into_response()
        }
    }
}

async fn count_pending(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.db.count_pending_requests().await {
        Ok(count) => Json(json!({"count": count})).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to count pending requests");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "failed to count requests"})),
            )
                .into_response()
        }
    }
}

// ── Router ───────────────────────────────────────────────────────────────────

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/requests", post(create_request).get(list_requests))
        .route(
            "/api/v1/requests/{id}",
            get(get_request).delete(delete_request),
        )
        .route("/api/v1/requests/{id}/approve", put(approve_request))
        .route("/api/v1/requests/{id}/decline", put(decline_request))
        .route("/api/v1/requests/pending/count", get(count_pending))
}
