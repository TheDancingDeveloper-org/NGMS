use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;

use ngms_core::models::media::{MediaFile, Movie};
use ngms_media::{CreateMovieInput, MovieService, UpdateMovieInput};
use ngms_metadata::TmdbClient;

use super::{extract_image_url, resolve_media_file_quality};
use crate::middleware::RequireAdmin;
use crate::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MovieResponse {
    #[serde(flatten)]
    movie: Movie,
    poster_url: Option<String>,
    fanart_url: Option<String>,
    has_file: bool,
    movie_file: Option<MediaFile>,
}

fn enrich_movie(movie: Movie, files: &HashMap<i64, MediaFile>) -> MovieResponse {
    let poster_url = extract_image_url(&movie.images, "poster");
    let fanart_url = extract_image_url(&movie.images, "fanart");
    let has_file = movie.movie_file_id.is_some();
    let movie_file = movie
        .movie_file_id
        .and_then(|fid| files.get(&fid).cloned())
        .map(resolve_media_file_quality);
    MovieResponse {
        movie,
        poster_url,
        fanart_url,
        has_file,
        movie_file,
    }
}

async fn fetch_media_files(
    pool: &sqlx::PgPool,
    file_ids: &[i64],
) -> Result<HashMap<i64, MediaFile>, sqlx::Error> {
    if file_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows = sqlx::query_as::<_, MediaFile>(
        "SELECT * FROM media_files WHERE id = ANY($1)",
    )
    .bind(file_ids)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|f| (f.id, f)).collect())
}

async fn list_movies(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let pool = state.db.pool();
    let svc = MovieService::new(pool.clone());
    match svc.list().await {
        Ok(list) => {
            let file_ids: Vec<i64> = list.iter().filter_map(|m| m.movie_file_id).collect();
            let files = fetch_media_files(pool, &file_ids)
                .await
                .unwrap_or_default();
            let responses: Vec<MovieResponse> = list
                .into_iter()
                .map(|m| enrich_movie(m, &files))
                .collect();
            Json(responses).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn get_movie(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let pool = state.db.pool();
    let svc = MovieService::new(pool.clone());
    match svc.get(id).await {
        Ok(m) => {
            let file_ids: Vec<i64> = m.movie_file_id.into_iter().collect();
            let files = fetch_media_files(pool, &file_ids)
                .await
                .unwrap_or_default();
            Json(enrich_movie(m, &files)).into_response()
        }
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn create_movie(
    State(state): State<Arc<AppState>>,
    Json(input): Json<CreateMovieInput>,
) -> impl IntoResponse {
    let svc = MovieService::new(state.db.pool().clone());
    match svc.create(input).await {
        Ok(m) => {
            let files = HashMap::new();
            (StatusCode::CREATED, Json(enrich_movie(m, &files))).into_response()
        }
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

async fn update_movie(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(input): Json<UpdateMovieInput>,
) -> impl IntoResponse {
    let pool = state.db.pool();
    let svc = MovieService::new(pool.clone());
    match svc.update(id, input).await {
        Ok(m) => {
            let file_ids: Vec<i64> = m.movie_file_id.into_iter().collect();
            let files = fetch_media_files(pool, &file_ids)
                .await
                .unwrap_or_default();
            Json(enrich_movie(m, &files)).into_response()
        }
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

async fn delete_movie(
    State(state): State<Arc<AppState>>,
    RequireAdmin(_admin): RequireAdmin,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let svc = MovieService::new(state.db.pool().clone());
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

async fn lookup_movie(
    State(state): State<Arc<AppState>>,
    Query(query): Query<LookupQuery>,
) -> impl IntoResponse {
    // Get TMDB API key from env or DB
    let api_key = std::env::var("NGMS_TMDB_API_KEY").ok();

    let api_key = match api_key {
        Some(key) if !key.is_empty() => key,
        _ => {
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
                            Json(json!({"error": "TMDB API key not configured. Set NGMS_TMDB_API_KEY or configure via settings."})),
                        )
                            .into_response();
                    }
                },
                _ => {
                    return (
                        StatusCode::SERVICE_UNAVAILABLE,
                        Json(json!({"error": "TMDB API key not configured. Set NGMS_TMDB_API_KEY or configure via settings."})),
                    )
                        .into_response();
                }
            }
        }
    };

    let client = TmdbClient::new(api_key);
    match client.search_movie(&query.term, None).await {
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
        .route("/api/v1/movies", get(list_movies).post(create_movie))
        .route(
            "/api/v1/movies/{id}",
            get(get_movie).put(update_movie).delete(delete_movie),
        )
        .route("/api/v1/movies/lookup", get(lookup_movie))
}
