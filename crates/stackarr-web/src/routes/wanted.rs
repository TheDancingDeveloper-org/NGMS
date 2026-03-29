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
    season_number: Option<i32>,
    episode_number: Option<i32>,
    episode_title: Option<String>,
    quality_profile: Option<String>,
    air_date: Option<String>,
    monitored: bool,
    /// Current file quality name (cutoff tab only)
    #[serde(skip_serializing_if = "Option::is_none")]
    current_quality: Option<String>,
    /// Wanted cutoff quality name (cutoff tab only)
    #[serde(skip_serializing_if = "Option::is_none")]
    cutoff_quality: Option<String>,
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
         AND e.season_number > 0
         AND (e.air_date IS NULL OR e.air_date <= CURRENT_DATE)",
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
                s.title, e.season_number, e.episode_number,
                e.title as episode_title,
                qp.name as quality_profile,
                e.air_date::text as air_date, e.monitored,
                NULL::text as current_quality, NULL::text as cutoff_quality
         FROM episodes e
         JOIN series s ON e.series_id = s.id
         LEFT JOIN quality_profiles qp ON s.quality_profile_id = qp.id
         WHERE e.monitored = true AND s.monitored = true
         AND e.episode_file_id IS NULL
         AND e.season_number > 0
         AND (e.air_date IS NULL OR e.air_date <= CURRENT_DATE)
         ORDER BY e.air_date DESC NULLS LAST",
    )
    .fetch_all(pool)
    .await;

    let movies = sqlx::query_as::<_, WantedRecord>(
        "SELECT m.id, 'movie' as media_type, m.id as media_id,
                m.title, NULL::int as season_number, NULL::int as episode_number,
                NULL::text as episode_title,
                qp.name as quality_profile,
                NULL::text as air_date, m.monitored,
                NULL::text as current_quality, NULL::text as cutoff_quality
         FROM movies m
         LEFT JOIN quality_profiles qp ON m.quality_profile_id = qp.id
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
/// Returns episodes/movies that have files below the quality profile's cutoff.
async fn get_cutoff(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PaginationParams>,
) -> impl IntoResponse {
    let pool = state.db.pool();
    let limit = params.page_size;
    let offset = (params.page - 1) * params.page_size;

    // Count episodes below cutoff
    let ep_count: i64 = match sqlx::query_scalar(
        "SELECT COUNT(*) FROM episodes e
         JOIN series s ON e.series_id = s.id
         JOIN media_files mf ON e.episode_file_id = mf.id
         JOIN quality_profiles qp ON s.quality_profile_id = qp.id
         WHERE e.monitored = true AND s.monitored = true
         AND e.episode_file_id IS NOT NULL
         AND (mf.quality->>'quality')::int < qp.cutoff",
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

    // Count movies below cutoff
    let movie_count: i64 = match sqlx::query_scalar(
        "SELECT COUNT(*) FROM movies m
         JOIN media_files mf ON m.movie_file_id = mf.id
         JOIN quality_profiles qp ON m.quality_profile_id = qp.id
         WHERE m.monitored = true AND m.movie_file_id IS NOT NULL
         AND (mf.quality->>'quality')::int < qp.cutoff",
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

    // Fetch episodes below cutoff with current + wanted quality
    let episodes = sqlx::query_as::<_, WantedRecord>(
        "SELECT e.id, 'series' as media_type, e.series_id as media_id,
                s.title, e.season_number, e.episode_number,
                e.title as episode_title,
                qp.name as quality_profile,
                e.air_date::text as air_date, e.monitored,
                (mf.quality->>'quality') as current_quality,
                qp.cutoff::text as cutoff_quality
         FROM episodes e
         JOIN series s ON e.series_id = s.id
         JOIN media_files mf ON e.episode_file_id = mf.id
         JOIN quality_profiles qp ON s.quality_profile_id = qp.id
         WHERE e.monitored = true AND s.monitored = true
         AND e.episode_file_id IS NOT NULL
         AND (mf.quality->>'quality')::int < qp.cutoff
         ORDER BY e.air_date DESC NULLS LAST",
    )
    .fetch_all(pool)
    .await;

    let movies = sqlx::query_as::<_, WantedRecord>(
        "SELECT m.id, 'movie' as media_type, m.id as media_id,
                m.title, NULL::int as season_number, NULL::int as episode_number,
                NULL::text as episode_title,
                qp.name as quality_profile,
                NULL::text as air_date, m.monitored,
                (mf.quality->>'quality') as current_quality,
                qp.cutoff::text as cutoff_quality
         FROM movies m
         JOIN media_files mf ON m.movie_file_id = mf.id
         JOIN quality_profiles qp ON m.quality_profile_id = qp.id
         WHERE m.monitored = true AND m.movie_file_id IS NOT NULL
         AND (mf.quality->>'quality')::int < qp.cutoff
         ORDER BY m.title",
    )
    .fetch_all(pool)
    .await;

    match (episodes, movies) {
        (Ok(mut ep_records), Ok(movie_records)) => {
            ep_records.extend(movie_records);

            // Resolve quality IDs to human-readable names
            let paginated: Vec<WantedRecord> = ep_records
                .into_iter()
                .skip(offset as usize)
                .take(limit as usize)
                .map(|mut r| {
                    r.current_quality = r.current_quality.map(|q| {
                        q.parse::<i32>()
                            .map(|n| stackarr_quality::quality_name(n).to_string())
                            .unwrap_or(q)
                    });
                    r.cutoff_quality = r.cutoff_quality.map(|q| {
                        q.parse::<i32>()
                            .map(|n| stackarr_quality::quality_name(n).to_string())
                            .unwrap_or(q)
                    });
                    r
                })
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
