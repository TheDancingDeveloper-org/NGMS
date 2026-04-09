//! DAV streaming routes — WebDAV endpoint + REST API for search/stream.

use std::sync::Arc;

use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{delete, get, post};
use serde::{Deserialize, Serialize};

use crate::AppState;

// ── REST API routes (use AppState) ─────────────────────────────────────────

/// Protected DAV API routes (behind auth middleware).
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/dav/stream", post(stream_handler))
        .route("/api/v1/dav/items", get(list_items))
        .route("/api/v1/dav/items/{id}", delete(delete_item))
        .route("/api/v1/dav/history", get(list_history))
        .route("/api/v1/dav/status", get(get_status))
}

// ── WebDAV mount (separate state: Arc<DatabaseStore>) ──────────────────────

/// Build the WebDAV router if the DAV module is enabled.
/// Returns `None` if the module is disabled or not initialized.
pub fn webdav_router(state: &Arc<AppState>) -> Option<Router> {
    let dav = state.dav_manager.load();
    let dav = dav.as_ref()?;
    let store = Arc::clone(&dav.store);
    Some(nzbdav_dav::dav_router(store))
}

// ── Stream handler (inline NZB processing) ─────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StreamRequest {
    nzb_url: String,
    name: String,
    category: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StreamResponse {
    dav_path: String,
    items_created: usize,
    job_dir_id: String,
}

async fn stream_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<StreamRequest>,
) -> Response {
    let dav = state.dav_manager.load();
    let Some(dav) = dav.as_ref() else {
        return (StatusCode::SERVICE_UNAVAILABLE, "DAV streaming not enabled").into_response();
    };

    // Fetch NZB from the indexer URL
    let nzb_data = match reqwest::get(&body.nzb_url).await {
        Ok(resp) => match resp.bytes().await {
            Ok(b) => b,
            Err(e) => {
                return (
                    StatusCode::BAD_GATEWAY,
                    format!("Failed to read NZB response: {e}"),
                )
                    .into_response()
            }
        },
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                format!("Failed to fetch NZB: {e}"),
            )
                .into_response()
        }
    };

    // Build a temporary QueueItem for the pipeline
    let queue_item = nzbdav_core::models::QueueItem {
        id: uuid::Uuid::new_v4(),
        created_at: chrono::Utc::now().naive_utc(),
        file_name: format!("{}.nzb", body.name),
        job_name: body.name,
        nzb_file_size: nzb_data.len() as i64,
        total_segment_bytes: 0,
        category: body.category.unwrap_or_default(),
        priority: 0,
        post_processing: -1,
        pause_until: None,
    };

    // Run the pipeline inline
    match dav.processor.process(&*dav.db, &queue_item, &nzb_data).await {
        Ok(result) => Json(StreamResponse {
            dav_path: format!("/content/{}/", queue_item.job_name),
            items_created: result.items_created,
            job_dir_id: result.job_dir_id.to_string(),
        })
        .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "DAV stream pipeline failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Pipeline failed: {e}"),
            )
                .into_response()
        }
    }
}

// ── List DAV items ─────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct ListItemsQuery {
    path: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DavItemResponse {
    id: String,
    name: String,
    path: String,
    file_size: Option<i64>,
    is_directory: bool,
    item_type: i32,
    sub_type: i32,
    created_at: String,
}

async fn list_items(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(query): axum::extract::Query<ListItemsQuery>,
) -> Response {
    let dav = state.dav_manager.load();
    let Some(dav) = dav.as_ref() else {
        return (StatusCode::SERVICE_UNAVAILABLE, "DAV streaming not enabled").into_response();
    };

    let path = query.path.as_deref().unwrap_or("/content/");
    match dav.db.get_dav_children_by_path(path).await {
        Ok(items) => {
            let response: Vec<DavItemResponse> = items
                .into_iter()
                .map(|item| DavItemResponse {
                    id: item.id.to_string(),
                    name: item.name,
                    path: item.path,
                    file_size: item.file_size,
                    is_directory: item.item_type == nzbdav_core::models::ItemType::Directory,
                    item_type: item.item_type as i32,
                    sub_type: item.sub_type as i32,
                    created_at: item.created_at.to_string(),
                })
                .collect();
            Json(response).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to list items: {e}"),
        )
            .into_response(),
    }
}

// ── Delete DAV item ────────────────────────────────────────────────────────

async fn delete_item(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
) -> Response {
    let dav = state.dav_manager.load();
    let Some(dav) = dav.as_ref() else {
        return (StatusCode::SERVICE_UNAVAILABLE, "DAV streaming not enabled").into_response();
    };

    match dav.db.delete_dav_item(id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to delete item: {e}"),
        )
            .into_response(),
    }
}

// ── DAV history ────────────────────────────────────────────────────────────

async fn list_history(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(pagination): axum::extract::Query<PaginationQuery>,
) -> Response {
    let dav = state.dav_manager.load();
    let Some(dav) = dav.as_ref() else {
        return (StatusCode::SERVICE_UNAVAILABLE, "DAV streaming not enabled").into_response();
    };

    let offset = pagination.offset.unwrap_or(0);
    let limit = pagination.limit.unwrap_or(50);

    match dav.db.list_history_items(offset, limit).await {
        Ok(items) => Json(items).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to list history: {e}"),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct PaginationQuery {
    offset: Option<i64>,
    limit: Option<i64>,
}

// ── DAV status ─────────────────────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DavStatusResponse {
    enabled: bool,
    provider_connections: usize,
    items_count: i64,
    queue_count: i64,
    history_count: i64,
}

async fn get_status(State(state): State<Arc<AppState>>) -> Response {
    let dav = state.dav_manager.load();
    let Some(dav) = dav.as_ref() else {
        return Json(DavStatusResponse {
            enabled: false,
            provider_connections: 0,
            items_count: 0,
            queue_count: 0,
            history_count: 0,
        })
        .into_response();
    };

    let queue_count = dav.db.count_queue_items().await.unwrap_or(0);
    let history_count = dav.db.count_history_items().await.unwrap_or(0);

    // Count content items (non-root directories and files)
    let items_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM dav_items WHERE sub_type NOT IN (102, 103, 104, 105, 106)",
    )
    .fetch_one(state.db.pool())
    .await
    .unwrap_or(0);

    Json(DavStatusResponse {
        enabled: true,
        provider_connections: dav.provider.total_connections(),
        items_count,
        queue_count,
        history_count,
    })
    .into_response()
}
