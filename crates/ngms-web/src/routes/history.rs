use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use ngms_core::models::HistoryEvent;

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
    records: Vec<HistoryResponse>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HistoryResponse {
    id: i64,
    date: DateTime<Utc>,
    event_type: String,
    source_title: String,
    quality: serde_json::Value,
    indexer: Option<String>,
    media_type: String,
    series_id: Option<i64>,
    movie_id: Option<i64>,
    episode_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
}

use super::resolve_quality;

fn to_response(
    event: HistoryEvent,
    indexer_names: &HashMap<i64, String>,
) -> HistoryResponse {
    let indexer = event
        .indexer_id
        .and_then(|id| indexer_names.get(&id).cloned());
    let media_type_str = format!("{:?}", event.media_type).to_lowercase();
    let event_type_str = format!("{:?}", event.event_type);
    // Convert PascalCase enum variant to camelCase for frontend
    let event_type_camel = {
        let mut chars = event_type_str.chars();
        match chars.next() {
            None => String::new(),
            Some(c) => c.to_lowercase().to_string() + chars.as_str(),
        }
    };

    let is_series = media_type_str == "series";
    let is_movie = media_type_str == "movie";

    HistoryResponse {
        id: event.id,
        date: event.occurred_at,
        event_type: event_type_camel,
        source_title: event.source_title,
        quality: resolve_quality(&event.quality),
        indexer,
        media_type: media_type_str,
        series_id: if is_series { Some(event.media_id) } else { None },
        movie_id: if is_movie { Some(event.media_id) } else { None },
        episode_id: event.episode_id,
        data: event.data,
    }
}

#[derive(sqlx::FromRow)]
struct IndexerRow {
    id: i64,
    name: String,
}

async fn list_history(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PaginationParams>,
) -> impl IntoResponse {
    let pool = state.db.pool();
    let offset = (params.page - 1) * params.page_size;

    // Get total count
    let count_result: Result<(i64,), _> =
        sqlx::query_as("SELECT COUNT(*) FROM history")
            .fetch_one(pool)
            .await;

    let total = match count_result {
        Ok((c,)) => c,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    let (events_result, indexers_result) = tokio::join!(
        sqlx::query_as::<_, HistoryEvent>(
            "SELECT * FROM history ORDER BY occurred_at DESC LIMIT $1 OFFSET $2",
        )
        .bind(params.page_size)
        .bind(offset)
        .fetch_all(pool),
        sqlx::query_as::<_, IndexerRow>("SELECT id, name FROM indexers")
            .fetch_all(pool),
    );

    let indexer_names: HashMap<i64, String> = indexers_result
        .unwrap_or_default()
        .into_iter()
        .map(|r| (r.id, r.name))
        .collect();

    match events_result {
        Ok(records) => {
            let responses: Vec<HistoryResponse> = records
                .into_iter()
                .map(|e| to_response(e, &indexer_names))
                .collect();
            Json(PaginatedHistory {
                page: params.page,
                page_size: params.page_size,
                total_records: total,
                records: responses,
            })
            .into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// Recent events stream — returns the last N history events (default 30).
/// Designed for the activity popup to poll at a short interval.
async fn recent_events(
    State(state): State<Arc<AppState>>,
    Query(params): Query<RecentParams>,
) -> impl IntoResponse {
    let pool = state.db.pool();
    let limit = params.limit.unwrap_or(30).min(100);

    let (events_result, indexers_result) = tokio::join!(
        sqlx::query_as::<_, HistoryEvent>(
            "SELECT * FROM history ORDER BY occurred_at DESC LIMIT $1",
        )
        .bind(limit)
        .fetch_all(pool),
        sqlx::query_as::<_, IndexerRow>("SELECT id, name FROM indexers")
            .fetch_all(pool),
    );

    let indexer_names: HashMap<i64, String> = indexers_result
        .unwrap_or_default()
        .into_iter()
        .map(|r| (r.id, r.name))
        .collect();

    match events_result {
        Ok(records) => {
            let responses: Vec<HistoryResponse> = records
                .into_iter()
                .map(|e| to_response(e, &indexer_names))
                .collect();
            Json(responses).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(Debug, Deserialize)]
struct RecentParams {
    limit: Option<i64>,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/history", get(list_history))
        .route("/api/v1/history/stream", get(recent_events))
}
