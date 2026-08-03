// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, put};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;

use stackarr_core::models::media::{MediaFile, Movie};
use stackarr_media::{CreateMovieInput, MovieService, UpdateMovieInput};
use stackarr_metadata::TmdbClient;

use super::{extract_image_url, resolve_media_file_quality};
use crate::AppState;
use crate::middleware::RequireAdmin;

#[derive(Deserialize)]
pub struct PaginationParams {
    limit: Option<i64>,
    offset: Option<i64>,
}

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
    let rows = sqlx::query_as::<_, MediaFile>("SELECT * FROM media_files WHERE id = ANY($1)")
        .bind(file_ids)
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(|f| (f.id, f)).collect())
}

/// List all movies.
#[utoipa::path(
    get,
    path = "/api/v1/movies",
    tag = "Movies",
    operation_id = "listMovies",
    responses(
        (status = 200, description = "List of movies"),
    ),
    security(("ApiKeyAuth" = []), ("BearerAuth" = [])),
)]
pub async fn list_movies(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PaginationParams>,
) -> impl IntoResponse {
    let pool = state.db.pool();
    let svc = MovieService::new(pool.clone());

    // When limit is provided, use paginated response with total count.
    // Otherwise return flat array for backward compatibility with *arr APIs.
    if params.limit.is_some() {
        match svc.list_paginated(params.limit, params.offset).await {
            Ok((list, total)) => {
                let file_ids: Vec<i64> = list.iter().filter_map(|m| m.movie_file_id).collect();
                let files = fetch_media_files(pool, &file_ids).await.unwrap_or_default();
                let responses: Vec<MovieResponse> =
                    list.into_iter().map(|m| enrich_movie(m, &files)).collect();
                Json(json!({
                    "data": responses,
                    "total": total,
                    "limit": params.limit,
                    "offset": params.offset.unwrap_or(0),
                }))
                .into_response()
            }
            Err(e) => super::api_error(StatusCode::INTERNAL_SERVER_ERROR, e),
        }
    } else {
        match svc.list().await {
            Ok(list) => {
                let file_ids: Vec<i64> = list.iter().filter_map(|m| m.movie_file_id).collect();
                let files = fetch_media_files(pool, &file_ids).await.unwrap_or_default();
                let responses: Vec<MovieResponse> =
                    list.into_iter().map(|m| enrich_movie(m, &files)).collect();
                Json(responses).into_response()
            }
            Err(e) => super::api_error(StatusCode::INTERNAL_SERVER_ERROR, e),
        }
    }
}

/// Get a movie by ID.
#[utoipa::path(
    get,
    path = "/api/v1/movies/{id}",
    tag = "Movies",
    operation_id = "getMovie",
    params(("id" = i64, Path, description = "Movie ID")),
    responses(
        (status = 200, description = "Movie details"),
        (status = 404, description = "Movie not found"),
    ),
    security(("ApiKeyAuth" = []), ("BearerAuth" = [])),
)]
pub async fn get_movie(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let pool = state.db.pool();
    let svc = MovieService::new(pool.clone());
    match svc.get(id).await {
        Ok(m) => {
            let file_ids: Vec<i64> = m.movie_file_id.into_iter().collect();
            let files = fetch_media_files(pool, &file_ids).await.unwrap_or_default();
            Json(enrich_movie(m, &files)).into_response()
        }
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

/// Create a new movie.
#[utoipa::path(
    post,
    path = "/api/v1/movies",
    tag = "Movies",
    operation_id = "createMovie",
    request_body = serde_json::Value,
    responses(
        (status = 201, description = "Movie created"),
        (status = 400, description = "Invalid input"),
    ),
    security(("ApiKeyAuth" = []), ("BearerAuth" = [])),
)]
pub async fn create_movie(
    State(state): State<Arc<AppState>>,
    Json(mut input): Json<CreateMovieInput>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    // Auto-fill path from first movie media library folder if empty
    if input.path.is_empty() {
        let root: Option<(String,)> = sqlx::query_as(
            "SELECT path FROM media_library_folders WHERE media_type = 'movie' ORDER BY id LIMIT 1",
        )
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();
        let root_path = root.map(|r| r.0).unwrap_or_else(|| "/movies".to_string());
        let year_str = input.year.map(|y| format!(" ({y})")).unwrap_or_default();
        input.path = format!(
            "{}/{}{}",
            root_path.trim_end_matches('/'),
            input.title,
            year_str
        );
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

    let svc = MovieService::new(pool.clone());
    match svc.create(input).await {
        Ok(m) => {
            let files = HashMap::new();
            (StatusCode::CREATED, Json(enrich_movie(m, &files))).into_response()
        }
        Err(e) => super::api_error(StatusCode::BAD_REQUEST, e),
    }
}

/// Update an existing movie.
#[utoipa::path(
    put,
    path = "/api/v1/movies/{id}",
    tag = "Movies",
    operation_id = "updateMovie",
    params(("id" = i64, Path, description = "Movie ID")),
    request_body = serde_json::Value,
    responses(
        (status = 200, description = "Movie updated"),
        (status = 400, description = "Invalid input"),
    ),
    security(("ApiKeyAuth" = []), ("BearerAuth" = [])),
)]
pub async fn update_movie(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(input): Json<UpdateMovieInput>,
) -> impl IntoResponse {
    let pool = state.db.pool();
    let svc = MovieService::new(pool.clone());
    match svc.update(id, input).await {
        Ok(m) => {
            let file_ids: Vec<i64> = m.movie_file_id.into_iter().collect();
            let files = fetch_media_files(pool, &file_ids).await.unwrap_or_default();
            Json(enrich_movie(m, &files)).into_response()
        }
        Err(e) => super::api_error(StatusCode::BAD_REQUEST, e),
    }
}

/// Delete a movie (admin only).
#[utoipa::path(
    delete,
    path = "/api/v1/movies/{id}",
    tag = "Movies",
    operation_id = "deleteMovie",
    params(("id" = i64, Path, description = "Movie ID")),
    responses(
        (status = 204, description = "Movie deleted"),
        (status = 500, description = "Internal server error"),
    ),
    security(("ApiKeyAuth" = []), ("BearerAuth" = [])),
)]
pub async fn delete_movie(
    State(state): State<Arc<AppState>>,
    RequireAdmin(_admin): RequireAdmin,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let svc = MovieService::new(state.db.pool().clone());
    match svc.delete(id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => super::api_error(StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}

// ── TMDB Lookup ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct LookupQuery {
    term: String,
}

/// Search TMDB for movies by title.
#[utoipa::path(
    get,
    path = "/api/v1/movies/lookup",
    tag = "Movies",
    operation_id = "lookupMovie",
    params(
        ("term" = String, Query, description = "Search term for TMDB movie lookup"),
    ),
    responses(
        (status = 200, description = "TMDB search results"),
        (status = 502, description = "TMDB search failed"),
        (status = 503, description = "TMDB API key not configured"),
    ),
    security(("ApiKeyAuth" = []), ("BearerAuth" = [])),
)]
pub async fn lookup_movie(
    State(state): State<Arc<AppState>>,
    Query(query): Query<LookupQuery>,
) -> impl IntoResponse {
    let client = match &state.tmdb_client {
        Some(c) => Arc::clone(c),
        None => {
            // Fall back to resolving TMDB API key from env or DB
            let api_key = std::env::var("STACKARR_TMDB_API_KEY").ok();
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
            Arc::new(TmdbClient::new(api_key))
        }
    };

    match client.search_movie(&query.term, None).await {
        Ok(results) => {
            let transformed: Vec<serde_json::Value> = results
                .results
                .iter()
                .map(|m| {
                    let year = m
                        .release_date
                        .as_deref()
                        .and_then(|d| d.get(..4))
                        .and_then(|y| y.parse::<i32>().ok())
                        .unwrap_or(0);
                    let poster_url = m.poster_path.as_deref().map(|p| {
                        super::proxy_image_url(&format!("https://image.tmdb.org/t/p/w342{p}"))
                    });
                    json!({
                        "title": m.title,
                        "year": year,
                        "overview": m.overview,
                        "studio": "",
                        "tmdbId": m.id,
                        "posterUrl": poster_url,
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

// ── Bulk update ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkUpdateMoviesRequest {
    movie_ids: Vec<i64>,
    quality_profile_id: Option<i32>,
    monitored: Option<bool>,
}

#[utoipa::path(
    put,
    path = "/api/v1/movies/bulk",
    tag = "Movies",
    operation_id = "bulkUpdateMovies",
    request_body = serde_json::Value,
    responses(
        (status = 200, description = "Movies updated"),
        (status = 400, description = "Invalid input"),
    ),
    security(("ApiKeyAuth" = []), ("BearerAuth" = [])),
)]
pub async fn bulk_update_movies(
    State(state): State<Arc<AppState>>,
    Json(body): Json<BulkUpdateMoviesRequest>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    if body.movie_ids.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "movieIds must not be empty"})),
        )
            .into_response();
    }

    let mut updated: u64 = 0;

    if let Some(qp) = body.quality_profile_id {
        match sqlx::query("UPDATE movies SET quality_profile_id = $1 WHERE id = ANY($2)")
            .bind(qp)
            .bind(&body.movie_ids)
            .execute(pool)
            .await
        {
            Ok(r) => updated = r.rows_affected(),
            Err(e) => {
                tracing::error!(error = %e, "failed to bulk update movies quality_profile_id");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "internal server error"})),
                )
                    .into_response();
            }
        }
    }

    if let Some(monitored) = body.monitored {
        match sqlx::query("UPDATE movies SET monitored = $1 WHERE id = ANY($2)")
            .bind(monitored)
            .bind(&body.movie_ids)
            .execute(pool)
            .await
        {
            Ok(r) => updated = r.rows_affected(),
            Err(e) => {
                tracing::error!(error = %e, "failed to bulk update movies monitored");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "internal server error"})),
                )
                    .into_response();
            }
        }
    }

    Json(json!({"updated": updated})).into_response()
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/movies", get(list_movies).post(create_movie))
        .route("/api/v1/movies/bulk", put(bulk_update_movies))
        .route(
            "/api/v1/movies/{id}",
            get(get_movie).put(update_movie).delete(delete_movie),
        )
        .route("/api/v1/movies/lookup", get(lookup_movie))
}
