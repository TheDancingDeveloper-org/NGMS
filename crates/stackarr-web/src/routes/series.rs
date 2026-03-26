use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;

use stackarr_core::models::media::Series;
use stackarr_media::{CreateSeriesInput, SeriesService, UpdateSeriesInput};
use stackarr_metadata::TmdbClient;

use super::extract_image_url;
use crate::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SeriesResponse {
    #[serde(flatten)]
    series: Series,
    poster_url: Option<String>,
    fanart_url: Option<String>,
    episode_count: i64,
    episode_file_count: i64,
    total_episode_count: i64,
    season_count: i64,
}

#[derive(sqlx::FromRow)]
struct EpisodeCounts {
    series_id: i64,
    episode_count: Option<i64>,
    episode_file_count: Option<i64>,
    total_episode_count: Option<i64>,
    season_count: Option<i64>,
}

fn enrich_series(series: Series, counts: &HashMap<i64, EpisodeCounts>) -> SeriesResponse {
    let poster_url = extract_image_url(&series.images, "poster");
    let fanart_url = extract_image_url(&series.images, "fanart");
    let c = counts.get(&series.id);
    SeriesResponse {
        episode_count: c.and_then(|c| c.episode_count).unwrap_or(0),
        episode_file_count: c.and_then(|c| c.episode_file_count).unwrap_or(0),
        total_episode_count: c.and_then(|c| c.total_episode_count).unwrap_or(0),
        season_count: c.and_then(|c| c.season_count).unwrap_or(0),
        series,
        poster_url,
        fanart_url,
    }
}

async fn fetch_episode_counts(
    pool: &sqlx::PgPool,
) -> Result<HashMap<i64, EpisodeCounts>, sqlx::Error> {
    let rows = sqlx::query_as::<_, EpisodeCounts>(
        "SELECT series_id,
                COUNT(*) FILTER (WHERE season_number > 0) as episode_count,
                COUNT(*) FILTER (WHERE episode_file_id IS NOT NULL AND season_number > 0) as episode_file_count,
                COUNT(*) as total_episode_count,
                COUNT(DISTINCT season_number) FILTER (WHERE season_number > 0) as season_count
         FROM episodes
         GROUP BY series_id",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| (r.series_id, r)).collect())
}

async fn list_series(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let pool = state.db.pool();
    let svc = SeriesService::new(pool.clone());
    let (series_result, counts_result) =
        tokio::join!(svc.list(), fetch_episode_counts(pool));

    match (series_result, counts_result) {
        (Ok(list), Ok(counts)) => {
            let responses: Vec<SeriesResponse> = list
                .into_iter()
                .map(|s| enrich_series(s, &counts))
                .collect();
            Json(responses).into_response()
        }
        (Err(e), _) => {
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
        (_, Err(e)) => {
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

async fn get_series(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let pool = state.db.pool();
    let svc = SeriesService::new(pool.clone());
    let (series_result, counts_result) =
        tokio::join!(svc.get(id), fetch_episode_counts(pool));

    match (series_result, counts_result) {
        (Ok(s), Ok(counts)) => Json(enrich_series(s, &counts)).into_response(),
        (Err(_), _) => StatusCode::NOT_FOUND.into_response(),
        (_, Err(e)) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn create_series(
    State(state): State<Arc<AppState>>,
    Json(input): Json<CreateSeriesInput>,
) -> impl IntoResponse {
    let svc = SeriesService::new(state.db.pool().clone());
    match svc.create(input).await {
        Ok(s) => {
            let counts = HashMap::new();
            (StatusCode::CREATED, Json(enrich_series(s, &counts))).into_response()
        }
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

async fn update_series(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(input): Json<UpdateSeriesInput>,
) -> impl IntoResponse {
    let pool = state.db.pool();
    let svc = SeriesService::new(pool.clone());
    match svc.update(id, input).await {
        Ok(s) => {
            let counts = fetch_episode_counts(pool).await.unwrap_or_default();
            Json(enrich_series(s, &counts)).into_response()
        }
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

async fn delete_series(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let svc = SeriesService::new(state.db.pool().clone());
    match svc.delete(id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

// ── TMDB Lookup ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct LookupQuery {
    term: String,
}

async fn lookup_series(
    State(state): State<Arc<AppState>>,
    Query(query): Query<LookupQuery>,
) -> impl IntoResponse {
    // Get TMDB API key from config or environment
    let api_key = std::env::var("STACKARR_TMDB_API_KEY")
        .ok()
        .or_else(|| {
            // Also try app_config table (not blocking here since we need async)
            None
        });

    let api_key = match api_key {
        Some(key) if !key.is_empty() => key,
        _ => {
            // Try loading from DB synchronously via a quick query
            let pool = state.db.pool();
            match sqlx::query_scalar::<_, serde_json::Value>(
                "SELECT value FROM app_config WHERE key = 'tmdb_api_key'",
            )
            .fetch_optional(pool)
            .await
            {
                Ok(Some(val)) => match val.as_str() {
                    Some(k) if !k.is_empty() => k.to_string(),
                    _ => {
                        return (
                            StatusCode::SERVICE_UNAVAILABLE,
                            Json(json!({"error": "TMDB API key not configured. Set STACKARR_TMDB_API_KEY or configure via settings."})),
                        )
                            .into_response();
                    }
                },
                _ => {
                    return (
                        StatusCode::SERVICE_UNAVAILABLE,
                        Json(json!({"error": "TMDB API key not configured. Set STACKARR_TMDB_API_KEY or configure via settings."})),
                    )
                        .into_response();
                }
            }
        }
    };

    let client = TmdbClient::new(api_key);
    match client.search_series(&query.term, None).await {
        Ok(results) => Json(json!(results)).into_response(),
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({"error": format!("TMDB search failed: {e}")})),
        )
            .into_response(),
    }
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/series", get(list_series).post(create_series))
        .route(
            "/api/v1/series/{id}",
            get(get_series).put(update_series).delete(delete_series),
        )
        .route("/api/v1/series/lookup", get(lookup_series))
}
