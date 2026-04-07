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
use crate::middleware::RequireAdmin;

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

async fn fetch_episode_counts_for_series(
    pool: &sqlx::PgPool,
    series_id: i64,
) -> Result<HashMap<i64, EpisodeCounts>, sqlx::Error> {
    let row = sqlx::query_as::<_, EpisodeCounts>(
        "SELECT series_id,
                COUNT(*) FILTER (WHERE season_number > 0) as episode_count,
                COUNT(*) FILTER (WHERE episode_file_id IS NOT NULL AND season_number > 0) as episode_file_count,
                COUNT(*) as total_episode_count,
                COUNT(DISTINCT season_number) FILTER (WHERE season_number > 0) as season_count
         FROM episodes
         WHERE series_id = $1
         GROUP BY series_id",
    )
    .bind(series_id)
    .fetch_optional(pool)
    .await?;

    let mut map = HashMap::new();
    if let Some(r) = row {
        map.insert(r.series_id, r);
    }
    Ok(map)
}

async fn list_series(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let pool = state.db.pool();
    let svc = SeriesService::new(pool.clone());
    let (series_result, counts_result) = tokio::join!(svc.list(), fetch_episode_counts(pool));

    match (series_result, counts_result) {
        (Ok(list), Ok(counts)) => {
            let responses: Vec<SeriesResponse> = list
                .into_iter()
                .map(|s| enrich_series(s, &counts))
                .collect();
            Json(responses).into_response()
        }
        (Err(e), _) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        (_, Err(e)) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn get_series(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> impl IntoResponse {
    let pool = state.db.pool();
    let svc = SeriesService::new(pool.clone());
    let (series_result, counts_result) =
        tokio::join!(svc.get(id), fetch_episode_counts_for_series(pool, id));

    match (series_result, counts_result) {
        (Ok(s), Ok(counts)) => Json(enrich_series(s, &counts)).into_response(),
        (Err(_), _) => StatusCode::NOT_FOUND.into_response(),
        (_, Err(e)) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn create_series(
    State(state): State<Arc<AppState>>,
    Json(mut input): Json<CreateSeriesInput>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    // Auto-fill path from first TV media library folder if empty
    if input.path.is_empty() {
        let root: Option<(String,)> = sqlx::query_as(
            "SELECT path FROM media_library_folders WHERE media_type IN ('series', 'tv') ORDER BY id LIMIT 1",
        )
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();
        let root_path = root.map(|r| r.0).unwrap_or_else(|| "/tv".to_string());
        input.path = format!("{}/{}", root_path.trim_end_matches('/'), input.title);
    }

    // Auto-fill quality profile from first available if zero
    if input.quality_profile_id == 0 {
        let qp: Option<(i32,)> =
            sqlx::query_as("SELECT id FROM quality_profiles ORDER BY id LIMIT 1")
                .fetch_optional(pool)
                .await
                .ok()
                .flatten();
        input.quality_profile_id = qp.map(|r| r.0).unwrap_or(1);
    }

    let tmdb_id = input.tmdb_id;
    let svc = SeriesService::new(pool.clone());
    let series = match svc.create(input).await {
        Ok(s) => s,
        Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    };

    // If we have a TMDB ID, fetch full metadata and populate episodes inline
    if let Some(tmdb_id) = tmdb_id {
        let client = match &state.tmdb_client {
            Some(c) => Some(Arc::clone(c)),
            None => resolve_tmdb_api_key(pool)
                .await
                .map(|key| Arc::new(TmdbClient::new(key))),
        };
        if let Some(client) = client
            && let Ok(detail) = client.get_series(tmdb_id).await
        {
            // Build images JSONB
            let mut images = Vec::new();
            if let Some(ref p) = detail.poster_path {
                images.push(json!({
                    "coverType": "poster",
                    "remoteUrl": format!("https://image.tmdb.org/t/p/w342{p}")
                }));
            }
            if let Some(ref b) = detail.backdrop_path {
                images.push(json!({
                    "coverType": "fanart",
                    "remoteUrl": format!("https://image.tmdb.org/t/p/original{b}")
                }));
            }
            let images_json = serde_json::Value::Array(images);

            // Map TMDB status to our SeriesStatus
            let status_str = match detail.status.as_deref() {
                Some("Returning Series") | Some("In Production") => "continuing",
                Some("Ended") | Some("Canceled") | Some("Cancelled") => "ended",
                Some("Planned") => "upcoming",
                _ => "continuing",
            };

            let network = detail
                .networks
                .first()
                .map(|n| n.name.as_str())
                .unwrap_or("");
            let genres: Vec<String> = detail.genres.iter().map(|g| g.name.clone()).collect();
            let year = detail
                .first_air_date
                .as_deref()
                .and_then(|d| d.get(..4))
                .and_then(|y| y.parse::<i32>().ok());
            let runtime = detail.episode_run_time.first().copied();

            // External IDs from TMDB
            let tvdb_id = detail.external_ids.as_ref().and_then(|e| e.tvdb_id);
            let imdb_id = detail.external_ids.as_ref().and_then(|e| e.imdb_id.clone());

            // Update series with full metadata
            let _ = sqlx::query(
                    "UPDATE series SET overview = $1, status = $2::text::series_status, network = $3,
                     images = $4, genres = $5, year = $6, runtime = $7, tvdb_id = COALESCE($8, tvdb_id),
                     imdb_id = COALESCE($9, imdb_id), last_info_sync = NOW()
                     WHERE id = $10",
                )
                .bind(&detail.overview)
                .bind(status_str)
                .bind(network)
                .bind(&images_json)
                .bind(&genres)
                .bind(year)
                .bind(runtime)
                .bind(tvdb_id)
                .bind(&imdb_id)
                .bind(series.id)
                .execute(pool)
                .await;

            // Fetch all seasons and insert episodes
            let num_seasons = detail.number_of_seasons.unwrap_or(0);
            for season_num in 0..=num_seasons {
                if let Ok(season) = client.get_season(tmdb_id, season_num).await {
                    for ep in &season.episodes {
                        let _ = sqlx::query(
                                "INSERT INTO episodes (series_id, season_number, episode_number, title, overview, air_date, runtime, monitored)
                                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                                 ON CONFLICT (series_id, season_number, episode_number) DO NOTHING",
                            )
                            .bind(series.id)
                            .bind(ep.season_number)
                            .bind(ep.episode_number)
                            .bind(&ep.name)
                            .bind(&ep.overview)
                            .bind(ep.air_date)
                            .bind(ep.runtime)
                            .bind(season_num > 0) // specials unmonitored by default
                            .execute(pool)
                            .await;
                    }
                }
            }
        }
    }

    // Re-fetch the series with updated metadata
    let svc = SeriesService::new(pool.clone());
    match svc.get(series.id).await {
        Ok(updated) => {
            let counts = fetch_episode_counts_for_series(pool, series.id)
                .await
                .unwrap_or_default();
            (StatusCode::CREATED, Json(enrich_series(updated, &counts))).into_response()
        }
        Err(_) => {
            // Fallback to original if re-fetch fails
            let counts = HashMap::new();
            (StatusCode::CREATED, Json(enrich_series(series, &counts))).into_response()
        }
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
            let counts = fetch_episode_counts_for_series(pool, id)
                .await
                .unwrap_or_default();
            Json(enrich_series(s, &counts)).into_response()
        }
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

async fn delete_series(
    State(state): State<Arc<AppState>>,
    RequireAdmin(_admin): RequireAdmin,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let svc = SeriesService::new(state.db.pool().clone());
    match svc.delete(id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

// ── TMDB helpers ────────────────────────────────────────────────────────────

/// Resolve TMDB API key from env or database.
async fn resolve_tmdb_api_key(pool: &sqlx::PgPool) -> Option<String> {
    if let Ok(key) = std::env::var("STACKARR_TMDB_API_KEY")
        && !key.is_empty()
    {
        return Some(key);
    }
    let val: serde_json::Value =
        sqlx::query_scalar("SELECT value FROM app_config WHERE key = 'tmdb_api_key'")
            .fetch_optional(pool)
            .await
            .ok()
            .flatten()?;
    val.as_str()
        .filter(|k| !k.is_empty())
        .map(|k| k.to_string())
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
    let pool = state.db.pool();
    let client = match &state.tmdb_client {
        Some(c) => Arc::clone(c),
        None => match resolve_tmdb_api_key(pool).await {
            Some(key) => Arc::new(TmdbClient::new(key)),
            None => {
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(json!({"error": "TMDB API key not configured. Set STACKARR_TMDB_API_KEY or configure via settings."})),
                )
                    .into_response();
            }
        },
    };

    match client.search_series(&query.term, None).await {
        Ok(results) => {
            let transformed: Vec<serde_json::Value> = results
                .results
                .iter()
                .map(|s| {
                    let year = s
                        .first_air_date
                        .as_deref()
                        .and_then(|d| d.get(..4))
                        .and_then(|y| y.parse::<i32>().ok())
                        .unwrap_or(0);
                    let poster_url = s.poster_path.as_deref().map(|p| {
                        super::proxy_image_url(&format!("https://image.tmdb.org/t/p/w342{p}"))
                    });
                    json!({
                        "title": s.name,
                        "year": year,
                        "overview": s.overview,
                        "network": "",
                        "tmdbId": s.id,
                        "posterUrl": poster_url,
                        "seasonCount": 0,
                    })
                })
                .collect();
            Json(json!(transformed)).into_response()
        }
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
