use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use stackarr_core::models::QueueItem;

use crate::AppState;

/// Shape returned to the frontend — adds computed fields the UI needs.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueResponse {
    id: i64,
    title: String,
    status: String,
    progress: f64,
    size: i64,
    size_left: i64,
    estimated_completion_time: Option<String>,
    download_client: String,
    media_type: String,
    series_id: Option<i64>,
    movie_id: Option<i64>,
    episode_id: Option<i64>,
    quality: serde_json::Value,
    error_message: Option<String>,
    download_id: String,
    protocol: String,
}

impl QueueResponse {
    /// `live` is `(progress_pct, remaining_bytes)` from the download client if available.
    fn from_item(item: QueueItem, client_name: Option<&str>, live: Option<(f64, i64)>) -> Self {
        let download_id = item.download_id.clone();
        let protocol = match item.protocol {
            stackarr_core::models::DownloadProtocol::Usenet => "usenet",
            stackarr_core::models::DownloadProtocol::Torrent => "torrent",
        };
        let total = item.size.unwrap_or(0).max(0) as u64;

        // Use live progress from download client when available
        let (progress, size_left) = match &item.status {
            stackarr_core::models::DownloadStatus::Completed
            | stackarr_core::models::DownloadStatus::Importing => (100.0, 0i64),
            stackarr_core::models::DownloadStatus::Queued => (0.0, total as i64),
            _ => match live {
                Some((pct, remaining)) => (pct, remaining),
                None => (0.0, total as i64),
            },
        };

        let (media_type_str, series_id, movie_id) = match item.media_type {
            stackarr_core::models::MediaType::Series => ("series", Some(item.media_id), None),
            stackarr_core::models::MediaType::Movie => ("movie", None, Some(item.media_id)),
        };

        let status_str = match item.status {
            stackarr_core::models::DownloadStatus::Queued => "queued",
            stackarr_core::models::DownloadStatus::Downloading => "downloading",
            stackarr_core::models::DownloadStatus::Paused => "paused",
            stackarr_core::models::DownloadStatus::PostProcessing => "postProcessing",
            stackarr_core::models::DownloadStatus::Completed => "completed",
            stackarr_core::models::DownloadStatus::Importing => "importing",
            stackarr_core::models::DownloadStatus::Failed => "failed",
            stackarr_core::models::DownloadStatus::Warning => "warning",
        };

        Self {
            id: item.id,
            title: item.title,
            status: status_str.to_string(),
            progress,
            size: total as i64,
            size_left,
            estimated_completion_time: None,
            download_client: client_name.unwrap_or("Unknown").to_string(),
            media_type: media_type_str.to_string(),
            series_id,
            movie_id,
            episode_id: item.episode_id,
            quality: item.quality,
            error_message: item.error_message,
            download_id,
            protocol: protocol.to_string(),
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/queue",
    tag = "Queue",
    operation_id = "listQueue",
    responses(
        (status = 200, description = "Active and recently completed queue items"),
    ),
    security(("ApiKeyAuth" = []), ("BearerAuth" = [])),
)]
pub async fn list_queue(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    // Return active items plus completed/importing so users can see import progress
    let result = sqlx::query_as::<_, QueueItem>(
        "SELECT * FROM queue \
         WHERE status IN ('queued', 'downloading', 'paused', 'post_processing', 'warning', 'completed', 'importing') \
         ORDER BY added_at DESC",
    )
    .fetch_all(state.db.pool())
    .await;

    match result {
        Ok(items) => {
            // Build a map of client_id → client_name for display
            let client_names: std::collections::HashMap<i32, String> =
                sqlx::query_as::<_, (i32, String)>("SELECT id, name FROM download_clients")
                    .fetch_all(state.db.pool())
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .collect();

            // Fetch live progress from download clients so the UI shows
            // real-time percentages instead of always 0%.
            let live_items = {
                let dm = state.download_manager.read().await;
                dm.get_items_all().await
            };
            let mut progress_map: std::collections::HashMap<String, (f64, i64)> =
                std::collections::HashMap::new();
            for (_client_id, client_items) in &live_items {
                for di in client_items {
                    let total = di.total_size as f64;
                    let remaining = di.remaining_size as f64;
                    let pct = if total > 0.0 {
                        ((total - remaining) / total * 100.0).clamp(0.0, 100.0)
                    } else {
                        0.0
                    };
                    progress_map.insert(di.download_id.clone(), (pct, di.remaining_size as i64));
                }
            }

            let responses: Vec<QueueResponse> = items
                .into_iter()
                .map(|item| {
                    let name = item
                        .download_client_id
                        .and_then(|id| client_names.get(&id).map(|s| s.as_str()));
                    // Embedded usenet client has no DB row — detect by NULL client_id
                    let name = name.or_else(|| {
                        if item.download_client_id.is_none() {
                            Some("Embedded Usenet")
                        } else {
                            None
                        }
                    });
                    let live = progress_map.get(&item.download_id).copied();
                    QueueResponse::from_item(item, name, live)
                })
                .collect();

            Json(responses).into_response()
        }
        Err(e) => super::api_error(StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}

#[utoipa::path(
    delete,
    path = "/api/v1/queue/{id}",
    tag = "Queue",
    operation_id = "deleteQueueItem",
    params(("id" = i64, Path, description = "Queue item ID")),
    responses(
        (status = 204, description = "Queue item deleted"),
    ),
    security(("ApiKeyAuth" = []), ("BearerAuth" = [])),
)]
pub async fn delete_queue_item(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let result = sqlx::query("DELETE FROM queue WHERE id = $1")
        .bind(id)
        .execute(state.db.pool())
        .await;
    match result {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => super::api_error(StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/api/v1/queue", get(list_queue)).route(
        "/api/v1/queue/{id}",
        axum::routing::delete(delete_queue_item),
    )
}
