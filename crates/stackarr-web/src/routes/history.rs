use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use stackarr_core::models::HistoryEvent;

use crate::AppState;

#[derive(Debug, Deserialize)]
struct PaginationParams {
    #[serde(default = "default_page")]
    page: i64,
    #[serde(default = "default_page_size")]
    page_size: i64,
}

fn default_page() -> i64 {
    1
}

fn default_page_size() -> i64 {
    20
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PaginatedHistory {
    page: i64,
    page_size: i64,
    total_records: i64,
    records: Vec<HistoryEvent>,
}

async fn list_history(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PaginationParams>,
) -> impl IntoResponse {
    let offset = (params.page - 1) * params.page_size;

    // Get total count
    let count_result: Result<(i64,), _> =
        sqlx::query_as("SELECT COUNT(*) FROM history")
            .fetch_one(state.db.pool())
            .await;

    let total = match count_result {
        Ok((c,)) => c,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    let result = sqlx::query_as::<_, HistoryEvent>(
        "SELECT * FROM history ORDER BY occurred_at DESC LIMIT $1 OFFSET $2",
    )
    .bind(params.page_size)
    .bind(offset)
    .fetch_all(state.db.pool())
    .await;

    match result {
        Ok(records) => Json(PaginatedHistory {
            page: params.page,
            page_size: params.page_size,
            total_records: total,
            records,
        })
        .into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/api/v1/history", get(list_history))
}
