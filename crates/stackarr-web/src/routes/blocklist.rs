use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{delete, get};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::AppState;
use crate::middleware::RequireUser;

#[derive(Deserialize)]
struct PaginationParams {
    #[serde(default = "default_page")]
    page: i64,
    #[serde(default = "default_page_size", alias = "pageSize")]
    page_size: i64,
}

fn default_page() -> i64 {
    1
}
fn default_page_size() -> i64 {
    25
}

#[derive(Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
struct BlocklistEntry {
    id: i64,
    media_type: String,
    media_id: i64,
    source_title: String,
    quality: serde_json::Value,
    languages: Option<serde_json::Value>,
    indexer_id: Option<i32>,
    info_hash: Option<String>,
    message: Option<String>,
    added_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateBlocklistEntry {
    media_type: String,
    media_id: i64,
    source_title: String,
    quality: serde_json::Value,
    languages: Option<serde_json::Value>,
    indexer_id: Option<i32>,
    info_hash: Option<String>,
    message: Option<String>,
}

#[derive(Deserialize)]
struct BulkDeleteRequest {
    ids: Vec<i64>,
}

/// GET /api/v1/blocklist
async fn list_blocklist(
    _auth: RequireUser,
    State(state): State<Arc<AppState>>,
    Query(params): Query<PaginationParams>,
) -> impl IntoResponse {
    let pool = state.db.pool();
    let page = params.page.max(1);
    let page_size = params.page_size.clamp(1, 250);
    let offset = (page - 1) * page_size;

    let total: i64 = match sqlx::query_scalar("SELECT COUNT(*) FROM blocklist")
        .fetch_one(pool)
        .await
    {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(error = %e, "blocklist: failed to count entries");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "failed to retrieve blocklist"})),
            )
                .into_response();
        }
    };

    match sqlx::query_as::<_, BlocklistEntry>(
        "SELECT id, media_type, media_id, source_title, quality, languages,
                indexer_id, info_hash, message, added_at
         FROM blocklist ORDER BY added_at DESC LIMIT ? OFFSET ?",
    )
    .bind(page_size)
    .bind(offset)
    .fetch_all(pool)
    .await
    {
        Ok(records) => Json(json!({
            "page": page,
            "pageSize": page_size,
            "totalRecords": total,
            "records": records,
        }))
        .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "blocklist: failed to fetch entries");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "failed to retrieve blocklist"})),
            )
                .into_response()
        }
    }
}

/// POST /api/v1/blocklist
async fn add_blocklist_entry(
    _auth: RequireUser,
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateBlocklistEntry>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    match sqlx::query(
        "INSERT INTO blocklist (media_type, media_id, source_title, quality, languages, indexer_id, info_hash, message)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&body.media_type)
    .bind(body.media_id)
    .bind(&body.source_title)
    .bind(&body.quality)
    .bind(&body.languages)
    .bind(body.indexer_id)
    .bind(&body.info_hash)
    .bind(&body.message)
    .execute(pool)
    .await
    {
        Ok(result) => match sqlx::query_as::<_, BlocklistEntry>(
            "SELECT id, media_type, media_id, source_title, quality, languages, indexer_id, info_hash, message, added_at FROM blocklist WHERE id = ?",
        )
        .bind(result.last_insert_id() as i64)
        .fetch_one(pool)
        .await
        {
            Ok(entry) => (StatusCode::CREATED, Json(json!(entry))).into_response(),
            Err(e) => {
                tracing::error!(error = %e, "blocklist: failed to load created entry");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        },
        Err(e) => {
            tracing::error!(error = %e, "blocklist: failed to add entry");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "failed to add blocklist entry"})),
            )
                .into_response()
        }
    }
}

/// DELETE /api/v1/blocklist/{id}
async fn delete_blocklist_entry(
    _auth: RequireUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    match sqlx::query("DELETE FROM blocklist WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
    {
        Ok(r) if r.rows_affected() == 0 => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "blocklist entry not found"})),
        )
            .into_response(),
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => {
            tracing::error!(error = %e, "blocklist: failed to delete entry");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "failed to delete blocklist entry"})),
            )
                .into_response()
        }
    }
}

/// DELETE /api/v1/blocklist/bulk
async fn bulk_delete_blocklist(
    _auth: RequireUser,
    State(state): State<Arc<AppState>>,
    Json(body): Json<BulkDeleteRequest>,
) -> impl IntoResponse {
    if body.ids.is_empty() {
        return Json(json!({"deleted": 0})).into_response();
    }

    let pool = state.db.pool();
    let mut query = sqlx::QueryBuilder::new("DELETE FROM blocklist WHERE id IN (");
    let mut ids = query.separated(", ");
    for id in &body.ids {
        ids.push_bind(id);
    }
    ids.push_unseparated(")");

    match query.build().execute(pool).await {
        Ok(r) => Json(json!({"deleted": r.rows_affected()})).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "blocklist: failed to bulk delete");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "failed to delete blocklist entries"})),
            )
                .into_response()
        }
    }
}

/// DELETE /api/v1/blocklist/clear
async fn clear_blocklist(
    _auth: RequireUser,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let pool = state.db.pool();
    match sqlx::query("DELETE FROM blocklist").execute(pool).await {
        Ok(r) => Json(json!({"deleted": r.rows_affected()})).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "blocklist: failed to clear all");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "failed to clear blocklist"})),
            )
                .into_response()
        }
    }
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/v1/blocklist",
            get(list_blocklist).post(add_blocklist_entry),
        )
        .route("/api/v1/blocklist/clear", delete(clear_blocklist))
        .route("/api/v1/blocklist/{id}", delete(delete_blocklist_entry))
        .route("/api/v1/blocklist/bulk", delete(bulk_delete_blocklist))
}
