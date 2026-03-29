use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};

use ngms_core::models::QueueItem;

use crate::AppState;

async fn list_queue(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let result = sqlx::query_as::<_, QueueItem>("SELECT * FROM queue ORDER BY added_at DESC")
        .fetch_all(state.db.pool())
        .await;
    match result {
        Ok(items) => Json(items).into_response(),
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
        .route("/api/v1/queue/{id}", axum::routing::delete(delete_queue_item))
}
