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
struct QueueResponse {
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
}

impl QueueResponse {
    fn from_item(item: QueueItem, client_name: Option<&str>) -> Self {
        let total = item.size.unwrap_or(0).max(0) as u64;
        // Calculate progress from download client items if available,
        // otherwise infer from status
        let progress = match &item.status {
            stackarr_core::models::DownloadStatus::Completed => 100.0,
            stackarr_core::models::DownloadStatus::Queued => 0.0,
            _ => {
                // We don't have remaining_size in the DB — show indeterminate
                // unless completed
                0.0
            }
        };
        let size_left = if matches!(item.status, stackarr_core::models::DownloadStatus::Completed) {
            0
        } else {
            total as i64
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
        }
    }
}

async fn list_queue(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    // Only return active items — not completed/failed/imported
    let result = sqlx::query_as::<_, QueueItem>(
        "SELECT * FROM queue \
         WHERE status IN ('queued', 'downloading', 'paused', 'post_processing', 'warning') \
         ORDER BY added_at DESC",
    )
    .fetch_all(state.db.pool())
    .await;

    match result {
        Ok(items) => {
            // Build a map of client_id → client_name for display
            let client_names: std::collections::HashMap<i32, String> = sqlx::query_as::<_, (i32, String)>(
                "SELECT id, name FROM download_clients",
            )
            .fetch_all(state.db.pool())
            .await
            .unwrap_or_default()
            .into_iter()
            .collect();

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
                    QueueResponse::from_item(item, name)
                })
                .collect();

            Json(responses).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn delete_queue_item(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let result = sqlx::query("DELETE FROM queue WHERE id = $1")
        .bind(id)
        .execute(state.db.pool())
        .await;
    match result {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/queue", get(list_queue))
        .route(
            "/api/v1/queue/{id}",
            axum::routing::delete(delete_queue_item),
        )
}
