use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, put};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;

use stackarr_core::models::media::MediaFile;
use stackarr_media::{EpisodeService, MonitorStrategy};

use super::resolve_media_file_quality;
use crate::AppState;

#[derive(Debug, sqlx::FromRow)]
#[expect(dead_code)]
struct EpisodeRow {
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EpisodeResponse {
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
    has_file: bool,
    episode_file: Option<MediaFile>,
}

fn enrich_episode(ep: EpisodeRow, files: &HashMap<i64, MediaFile>) -> EpisodeResponse {
    let has_file = ep.episode_file_id.is_some();
    let episode_file = ep
        .episode_file_id
        .and_then(|fid| files.get(&fid).cloned())
        .map(resolve_media_file_quality);
    EpisodeResponse {
        id: ep.id,
        series_id: ep.series_id,
        season_number: ep.season_number,
        episode_number: ep.episode_number,
        absolute_number: ep.absolute_number,
        scene_season_number: ep.scene_season_number,
        scene_episode_number: ep.scene_episode_number,
        scene_absolute_number: ep.scene_absolute_number,
        title: ep.title,
        overview: ep.overview,
        air_date: ep.air_date,
        air_date_utc: ep.air_date_utc,
        runtime: ep.runtime,
        monitored: ep.monitored,
        has_file,
        episode_file,
    }
}

async fn fetch_media_files(pool: &sqlx::MySqlPool, file_ids: &[i64]) -> HashMap<i64, MediaFile> {
    if file_ids.is_empty() {
        return HashMap::new();
    }
    let mut query = sqlx::QueryBuilder::new("SELECT * FROM media_files WHERE id IN (");
    let mut ids = query.separated(", ");
    for id in file_ids {
        ids.push_bind(id);
    }
    ids.push_unseparated(")");
    query
        .build_query_as::<MediaFile>()
        .fetch_all(pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|f| (f.id, f))
        .collect()
}

/// GET /api/v1/series/{seriesId}/episodes
async fn list_episodes_for_series(
    State(state): State<Arc<AppState>>,
    Path(series_id): Path<i64>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    let rows = sqlx::query_as::<_, EpisodeRow>(
        "SELECT id, series_id, season_number, episode_number, absolute_number,
                scene_season_number, scene_episode_number, scene_absolute_number,
                title, overview, air_date, air_date_utc, runtime, monitored,
                episode_file_id, last_search_time
         FROM episodes
         WHERE series_id = ?
         ORDER BY season_number, episode_number",
    )
    .bind(series_id)
    .fetch_all(pool)
    .await;

    match rows {
        Ok(episodes) => {
            let file_ids: Vec<i64> = episodes.iter().filter_map(|e| e.episode_file_id).collect();
            let files = fetch_media_files(pool, &file_ids).await;
            let responses: Vec<EpisodeResponse> = episodes
                .into_iter()
                .map(|e| enrich_episode(e, &files))
                .collect();
            Json(responses).into_response()
        }
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
async fn get_episode(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> impl IntoResponse {
    let pool = state.db.pool();

    let row = sqlx::query_as::<_, EpisodeRow>(
        "SELECT id, series_id, season_number, episode_number, absolute_number,
                scene_season_number, scene_episode_number, scene_absolute_number,
                title, overview, air_date, air_date_utc, runtime, monitored,
                episode_file_id, last_search_time
         FROM episodes
         WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await;

    match row {
        Ok(Some(ep)) => {
            let file_ids: Vec<i64> = ep.episode_file_id.into_iter().collect();
            let files = fetch_media_files(pool, &file_ids).await;
            Json(enrich_episode(ep, &files)).into_response()
        }
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
        let result = sqlx::query("UPDATE episodes SET monitored = ? WHERE id = ?")
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
    let row = sqlx::query_as::<_, EpisodeRow>(
        "SELECT id, series_id, season_number, episode_number, absolute_number,
                scene_season_number, scene_episode_number, scene_absolute_number,
                title, overview, air_date, air_date_utc, runtime, monitored,
                episode_file_id, last_search_time
         FROM episodes
         WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await;

    match row {
        Ok(Some(ep)) => {
            let file_ids: Vec<i64> = ep.episode_file_id.into_iter().collect();
            let files = fetch_media_files(pool, &file_ids).await;
            Json(enrich_episode(ep, &files)).into_response()
        }
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

    let mut query = sqlx::QueryBuilder::new("UPDATE episodes SET monitored = ");
    query.push_bind(body.monitored).push(" WHERE id IN (");
    let mut ids = query.separated(", ");
    for id in &body.episode_ids {
        ids.push_bind(id);
    }
    ids.push_unseparated(")");
    let result = query.build().execute(pool).await;

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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SeasonMonitorRequest {
    monitored: bool,
}

/// PUT /api/v1/series/{seriesId}/seasons/{seasonNumber}/monitor
async fn set_season_monitor(
    State(state): State<Arc<AppState>>,
    Path((series_id, season_number)): Path<(i64, i32)>,
    Json(body): Json<SeasonMonitorRequest>,
) -> impl IntoResponse {
    let svc = EpisodeService::new(state.db.pool().clone());
    match svc
        .set_season_monitored(series_id, season_number, body.monitored)
        .await
    {
        Ok(()) => Json(json!({
            "seriesId": series_id,
            "seasonNumber": season_number,
            "monitored": body.monitored,
        }))
        .into_response(),
        Err(e) => {
            tracing::error!(error = %e, series_id, season_number, "failed to set season monitored");
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
struct MonitorStrategyRequest {
    monitor_strategy: MonitorStrategy,
}

/// PUT /api/v1/series/{seriesId}/monitor
async fn apply_monitor_strategy(
    State(state): State<Arc<AppState>>,
    Path(series_id): Path<i64>,
    Json(body): Json<MonitorStrategyRequest>,
) -> impl IntoResponse {
    let svc = EpisodeService::new(state.db.pool().clone());
    match svc
        .apply_monitor_strategy(series_id, body.monitor_strategy)
        .await
    {
        Ok(()) => Json(json!({
            "seriesId": series_id,
            "monitorStrategy": body.monitor_strategy,
        }))
        .into_response(),
        Err(e) => {
            tracing::error!(error = %e, series_id, "failed to apply monitor strategy");
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
        .route("/api/v1/episode/{id}", get(get_episode).put(update_episode))
        .route("/api/v1/episode/monitor", put(bulk_monitor))
        .route(
            "/api/v1/series/{seriesId}/seasons/{seasonNumber}/monitor",
            put(set_season_monitor),
        )
        .route(
            "/api/v1/series/{seriesId}/monitor",
            put(apply_monitor_strategy),
        )
}
