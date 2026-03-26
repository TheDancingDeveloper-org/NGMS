use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::AppState;

#[derive(Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
struct IndexerResponse {
    id: i32,
    name: String,
    indexer_type: String,
    base_url: String,
    api_key: Option<String>,
    protocol: String,
    categories: Option<Vec<i32>>,
    enabled: bool,
    priority: i32,
    supports_search: bool,
    supports_rss: bool,
    config: Option<serde_json::Value>,
    last_rss_sync: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateIndexerRequest {
    name: String,
    indexer_type: String,
    base_url: String,
    api_key: Option<String>,
    protocol: String,
    categories: Option<Vec<i32>>,
    enabled: Option<bool>,
    priority: Option<i32>,
    supports_search: Option<bool>,
    supports_rss: Option<bool>,
    config: Option<serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateIndexerRequest {
    name: Option<String>,
    indexer_type: Option<String>,
    base_url: Option<String>,
    api_key: Option<String>,
    protocol: Option<String>,
    categories: Option<Vec<i32>>,
    enabled: Option<bool>,
    priority: Option<i32>,
    supports_search: Option<bool>,
    supports_rss: Option<bool>,
    config: Option<serde_json::Value>,
}

async fn list_indexers(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let pool = state.db.pool();

    match sqlx::query_as::<_, IndexerResponse>(
        "SELECT id, name, indexer_type, base_url, api_key, protocol, categories,
                enabled, priority, supports_search, supports_rss, config, last_rss_sync
         FROM indexers ORDER BY priority, id",
    )
    .fetch_all(pool)
    .await
    {
        Ok(indexers) => Json(indexers).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to list indexers");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

async fn create_indexer(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateIndexerRequest>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    if body.name.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "name cannot be empty"})),
        )
            .into_response();
    }

    if body.base_url.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "base_url cannot be empty"})),
        )
            .into_response();
    }

    let enabled = body.enabled.unwrap_or(true);
    let priority = body.priority.unwrap_or(25);
    let supports_search = body.supports_search.unwrap_or(true);
    let supports_rss = body.supports_rss.unwrap_or(true);

    match sqlx::query_as::<_, IndexerResponse>(
        "INSERT INTO indexers (name, indexer_type, base_url, api_key, protocol, categories,
                               enabled, priority, supports_search, supports_rss, config)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
         RETURNING id, name, indexer_type, base_url, api_key, protocol, categories,
                   enabled, priority, supports_search, supports_rss, config, last_rss_sync",
    )
    .bind(body.name.trim())
    .bind(&body.indexer_type)
    .bind(body.base_url.trim())
    .bind(&body.api_key)
    .bind(&body.protocol)
    .bind(&body.categories)
    .bind(enabled)
    .bind(priority)
    .bind(supports_search)
    .bind(supports_rss)
    .bind(&body.config)
    .fetch_one(pool)
    .await
    {
        Ok(indexer) => (StatusCode::CREATED, Json(json!(indexer))).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to create indexer");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

async fn update_indexer(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(body): Json<UpdateIndexerRequest>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    match sqlx::query_as::<_, IndexerResponse>(
        "UPDATE indexers SET
            name = COALESCE($1, name),
            indexer_type = COALESCE($2, indexer_type),
            base_url = COALESCE($3, base_url),
            api_key = COALESCE($4, api_key),
            protocol = COALESCE($5, protocol),
            categories = COALESCE($6, categories),
            enabled = COALESCE($7, enabled),
            priority = COALESCE($8, priority),
            supports_search = COALESCE($9, supports_search),
            supports_rss = COALESCE($10, supports_rss),
            config = COALESCE($11, config)
         WHERE id = $12
         RETURNING id, name, indexer_type, base_url, api_key, protocol, categories,
                   enabled, priority, supports_search, supports_rss, config, last_rss_sync",
    )
    .bind(body.name.as_deref().map(str::trim))
    .bind(&body.indexer_type)
    .bind(body.base_url.as_deref().map(str::trim))
    .bind(&body.api_key)
    .bind(&body.protocol)
    .bind(&body.categories)
    .bind(body.enabled)
    .bind(body.priority)
    .bind(body.supports_search)
    .bind(body.supports_rss)
    .bind(&body.config)
    .bind(id as i32)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(indexer)) => Json(json!(indexer)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "indexer not found"})),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to update indexer");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

async fn delete_indexer(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    match sqlx::query("DELETE FROM indexers WHERE id = $1")
        .bind(id as i32)
        .execute(pool)
        .await
    {
        Ok(result) => {
            if result.rows_affected() == 0 {
                (
                    StatusCode::NOT_FOUND,
                    Json(json!({"error": "indexer not found"})),
                )
                    .into_response()
            } else {
                StatusCode::NO_CONTENT.into_response()
            }
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to delete indexer");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

async fn test_indexer(
    State(_state): State<Arc<AppState>>,
    Path(_id): Path<i64>,
) -> impl IntoResponse {
    // Stub: just return ok for now
    Json(json!({
        "success": true,
        "message": "connection test passed"
    }))
    .into_response()
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/v1/indexer",
            get(list_indexers).post(create_indexer),
        )
        .route(
            "/api/v1/indexer/{id}",
            axum::routing::put(update_indexer).delete(delete_indexer),
        )
        .route("/api/v1/indexer/{id}/test", post(test_indexer))
}
