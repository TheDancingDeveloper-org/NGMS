use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;

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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WantedResponse {
    page: i64,
    page_size: i64,
    total_records: i64,
    records: Vec<WantedRecord>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
struct WantedRecord {
    id: i64,
    media_type: String,
    media_id: i64,
    title: String,
    episode_info: Option<String>,
    quality_profile_id: Option<i32>,
    air_date: Option<String>,
    monitored: bool,
}

/// GET /api/v1/wanted/missing
///
/// Returns monitored episodes without files (already aired) and monitored movies without files.
async fn get_missing(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PaginationParams>,
) -> impl IntoResponse {
    let pool = state.db.pool();
    let limit = params.page_size;
    let offset = (params.page - 1) * params.page_size;

    // Count missing episodes
    let ep_count: i64 = match sqlx::query_scalar(
        "SELECT COUNT(*) FROM episodes e
         JOIN series s ON e.series_id = s.id
         WHERE e.monitored = true AND s.monitored = true
         AND e.episode_file_id IS NULL
         AND e.air_date_utc < NOW()",
    )
    .fetch_one(pool)
    .await
    {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    // Count missing movies
    let movie_count: i64 = match sqlx::query_scalar(
        "SELECT COUNT(*) FROM movies
         WHERE monitored = true AND movie_file_id IS NULL",
    )
    .fetch_one(pool)
    .await
    {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    let total = ep_count + movie_count;

    // Fetch missing episodes
    let episodes = sqlx::query_as::<_, WantedRecord>(
        "SELECT e.id, 'series' as media_type, e.series_id as media_id,
                s.title,
                'S' || LPAD(e.season_number::text, 2, '0') || 'E' || LPAD(e.episode_number::text, 2, '0') as episode_info,
                s.quality_profile_id, e.air_date::text as air_date, e.monitored
         FROM episodes e
         JOIN series s ON e.series_id = s.id
         WHERE e.monitored = true AND s.monitored = true
         AND e.episode_file_id IS NULL
         AND e.air_date_utc < NOW()
         ORDER BY e.air_date_utc DESC",
    )
    .fetch_all(pool)
    .await;

    let movies = sqlx::query_as::<_, WantedRecord>(
        "SELECT m.id, 'movie' as media_type, m.id as media_id,
                m.title, NULL as episode_info,
                m.quality_profile_id, NULL as air_date, m.monitored
         FROM movies m
         WHERE m.monitored = true AND m.movie_file_id IS NULL
         ORDER BY m.title",
    )
    .fetch_all(pool)
    .await;

    match (episodes, movies) {
        (Ok(mut ep_records), Ok(movie_records)) => {
            ep_records.extend(movie_records);
            // Apply pagination to the combined list
            let paginated: Vec<WantedRecord> = ep_records
                .into_iter()
                .skip(offset as usize)
                .take(limit as usize)
                .collect();

            Json(WantedResponse {
                page: params.page,
                page_size: params.page_size,
                total_records: total,
                records: paginated,
            })
            .into_response()
        }
        (Err(e), _) | (_, Err(e)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// GET /api/v1/wanted/cutoff
///
/// Returns episodes/movies that have files but may need quality upgrades.
/// Real cutoff comparison requires the decision engine (Phase 4); for now returns
/// items that have a file assigned.
async fn get_cutoff(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PaginationParams>,
) -> impl IntoResponse {
    let pool = state.db.pool();
    let limit = params.page_size;
    let offset = (params.page - 1) * params.page_size;

    // Count episodes with files (potential cutoff unmet)
    let ep_count: i64 = match sqlx::query_scalar(
        "SELECT COUNT(*) FROM episodes e
         JOIN series s ON e.series_id = s.id
         WHERE e.monitored = true AND s.monitored = true
         AND e.episode_file_id IS NOT NULL",
    )
    .fetch_one(pool)
    .await
    {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    // Count movies with files
    let movie_count: i64 = match sqlx::query_scalar(
        "SELECT COUNT(*) FROM movies
         WHERE monitored = true AND movie_file_id IS NOT NULL",
    )
    .fetch_one(pool)
    .await
    {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    let total = ep_count + movie_count;

    let episodes = sqlx::query_as::<_, WantedRecord>(
        "SELECT e.id, 'series' as media_type, e.series_id as media_id,
                s.title,
                'S' || LPAD(e.season_number::text, 2, '0') || 'E' || LPAD(e.episode_number::text, 2, '0') as episode_info,
                s.quality_profile_id, e.air_date::text as air_date, e.monitored
         FROM episodes e
         JOIN series s ON e.series_id = s.id
         WHERE e.monitored = true AND s.monitored = true
         AND e.episode_file_id IS NOT NULL
         ORDER BY e.air_date_utc DESC",
    )
    .fetch_all(pool)
    .await;

    let movies = sqlx::query_as::<_, WantedRecord>(
        "SELECT m.id, 'movie' as media_type, m.id as media_id,
                m.title, NULL as episode_info,
                m.quality_profile_id, NULL as air_date, m.monitored
         FROM movies m
         WHERE m.monitored = true AND m.movie_file_id IS NOT NULL
         ORDER BY m.title",
    )
    .fetch_all(pool)
    .await;

    match (episodes, movies) {
        (Ok(mut ep_records), Ok(movie_records)) => {
            ep_records.extend(movie_records);
            let paginated: Vec<WantedRecord> = ep_records
                .into_iter()
                .skip(offset as usize)
                .take(limit as usize)
                .collect();

            Json(WantedResponse {
                page: params.page,
                page_size: params.page_size,
                total_records: total,
                records: paginated,
            })
            .into_response()
        }
        (Err(e), _) | (_, Err(e)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/wanted/missing", get(get_missing))
        .route("/api/v1/wanted/cutoff", get(get_cutoff))
}
