use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::AppState;

#[derive(Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
struct TagResponse {
    id: i32,
    label: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateTagRequest {
    label: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateTagRequest {
    label: String,
}

async fn list_tags(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let pool = state.db.pool();

    match sqlx::query_as::<_, TagResponse>("SELECT id, label FROM tags ORDER BY id")
        .fetch_all(pool)
        .await
    {
        Ok(tags) => Json(tags).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to list tags");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

async fn create_tag(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateTagRequest>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    if body.label.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "label cannot be empty"})),
        )
            .into_response();
    }

    match sqlx::query_as::<_, TagResponse>(
        "INSERT INTO tags (label) VALUES ($1) RETURNING id, label",
    )
    .bind(body.label.trim())
    .fetch_one(pool)
    .await
    {
        Ok(tag) => (StatusCode::CREATED, Json(json!(tag))).into_response(),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("duplicate key") || msg.contains("unique") {
                (
                    StatusCode::CONFLICT,
                    Json(json!({"error": "tag with this label already exists"})),
                )
                    .into_response()
            } else {
                tracing::error!(error = %e, "failed to create tag");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "internal server error"})),
                )
                    .into_response()
            }
        }
    }
}

async fn update_tag(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(body): Json<UpdateTagRequest>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    if body.label.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "label cannot be empty"})),
        )
            .into_response();
    }

    match sqlx::query_as::<_, TagResponse>(
        "UPDATE tags SET label = $1 WHERE id = $2 RETURNING id, label",
    )
    .bind(body.label.trim())
    .bind(id as i32)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(tag)) => Json(json!(tag)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "tag not found"})),
        )
            .into_response(),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("duplicate key") || msg.contains("unique") {
                (
                    StatusCode::CONFLICT,
                    Json(json!({"error": "tag with this label already exists"})),
                )
                    .into_response()
            } else {
                tracing::error!(error = %e, "failed to update tag");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "internal server error"})),
                )
                    .into_response()
            }
        }
    }
}

async fn delete_tag(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> impl IntoResponse {
    let pool = state.db.pool();

    match sqlx::query("DELETE FROM tags WHERE id = $1")
        .bind(id as i32)
        .execute(pool)
        .await
    {
        Ok(result) => {
            if result.rows_affected() == 0 {
                (
                    StatusCode::NOT_FOUND,
                    Json(json!({"error": "tag not found"})),
                )
                    .into_response()
            } else {
                StatusCode::NO_CONTENT.into_response()
            }
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to delete tag");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/tag", get(list_tags).post(create_tag))
        .route(
            "/api/v1/tag/{id}",
            axum::routing::put(update_tag).delete(delete_tag),
        )
}
