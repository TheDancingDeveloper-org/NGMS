use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, put};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::AppState;

#[derive(Debug, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
struct Episode {
    id: i64,
    series_id: i64,
    season_number: i32,
    episode_number: i32,
    absolute_number: Option<i32>,
    scene_season_number: Option<i32>,
    scene_episode_number: Option<i32>,
    scene_absolute_number: Option<i32>,
    title: Option<String>,
    overview: Option<String>,
    air_date: Option<chrono::NaiveDate>,
    air_date_utc: Option<chrono::DateTime<chrono::Utc>>,
    runtime: Option<i32>,
    monitored: bool,
    episode_file_id: Option<i64>,
    last_search_time: Option<chrono::DateTime<chrono::Utc>>,
}

/// GET /api/v1/series/{seriesId}/episodes
async fn list_episodes_for_series(
    State(state): State<Arc<AppState>>,
    Path(series_id): Path<i64>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    let rows = sqlx::query_as::<_, Episode>(
        "SELECT id, series_id, season_number, episode_number, absolute_number,
                scene_season_number, scene_episode_number, scene_absolute_number,
                title, overview, air_date, air_date_utc, runtime, monitored,
                episode_file_id, last_search_time
         FROM episodes
         WHERE series_id = $1
         ORDER BY season_number, episode_number",
    )
    .bind(series_id)
    .fetch_all(pool)
    .await;

    match rows {
        Ok(episodes) => Json(episodes).into_response(),
        Err(e) => {
            tracing::error!(error = %e, series_id, "failed to list episodes for series");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

/// GET /api/v1/episode/{id}
async fn get_episode(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    let row = sqlx::query_as::<_, Episode>(
        "SELECT id, series_id, season_number, episode_number, absolute_number,
                scene_season_number, scene_episode_number, scene_absolute_number,
                title, overview, air_date, air_date_utc, runtime, monitored,
                episode_file_id, last_search_time
         FROM episodes
         WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await;

    match row {
        Ok(Some(ep)) => Json(ep).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "episode not found"})),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, id, "failed to fetch episode");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateEpisodeRequest {
    monitored: Option<bool>,
}

/// PUT /api/v1/episode/{id}
async fn update_episode(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(body): Json<UpdateEpisodeRequest>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    if let Some(monitored) = body.monitored {
        let result = sqlx::query(
            "UPDATE episodes SET monitored = $1 WHERE id = $2",
        )
        .bind(monitored)
        .bind(id)
        .execute(pool)
        .await;

        match result {
            Ok(r) if r.rows_affected() == 0 => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(json!({"error": "episode not found"})),
                )
                    .into_response();
            }
            Ok(_) => {}
            Err(e) => {
                tracing::error!(error = %e, id, "failed to update episode");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "internal server error"})),
                )
                    .into_response();
            }
        }
    }

    // Return the updated episode
    let row = sqlx::query_as::<_, Episode>(
        "SELECT id, series_id, season_number, episode_number, absolute_number,
                scene_season_number, scene_episode_number, scene_absolute_number,
                title, overview, air_date, air_date_utc, runtime, monitored,
                episode_file_id, last_search_time
         FROM episodes
         WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await;

    match row {
        Ok(Some(ep)) => Json(ep).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "episode not found"})),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, id, "failed to fetch updated episode");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BulkMonitorRequest {
    episode_ids: Vec<i64>,
    monitored: bool,
}

/// PUT /api/v1/episode/monitor
async fn bulk_monitor(
    State(state): State<Arc<AppState>>,
    Json(body): Json<BulkMonitorRequest>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    if body.episode_ids.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "episode_ids must not be empty"})),
        )
            .into_response();
    }

    // Build a parameterized query for the IN clause
    // sqlx doesn't natively support binding Vec<i64> to IN(...) in raw queries,
    // so we use the ANY($1) pattern with a slice
    let result = sqlx::query(
        "UPDATE episodes SET monitored = $1 WHERE id = ANY($2)",
    )
    .bind(body.monitored)
    .bind(&body.episode_ids)
    .execute(pool)
    .await;

    match result {
        Ok(r) => Json(json!({
            "updated": r.rows_affected(),
            "monitored": body.monitored,
        }))
        .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to bulk update episode monitoring");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/v1/series/{seriesId}/episodes",
            get(list_episodes_for_series),
        )
        .route(
            "/api/v1/episode/{id}",
            get(get_episode).put(update_episode),
        )
        .route("/api/v1/episode/monitor", put(bulk_monitor))
}
