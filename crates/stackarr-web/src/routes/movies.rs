use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use stackarr_media::{CreateMovieInput, MovieService, UpdateMovieInput};
use stackarr_metadata::TmdbClient;

use crate::AppState;

async fn list_movies(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let svc = MovieService::new(state.db.pool().clone());
    match svc.list().await {
        Ok(list) => Json(list).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn get_movie(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let svc = MovieService::new(state.db.pool().clone());
    match svc.get(id).await {
        Ok(m) => Json(m).into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn create_movie(
    State(state): State<Arc<AppState>>,
    Json(input): Json<CreateMovieInput>,
) -> impl IntoResponse {
    let svc = MovieService::new(state.db.pool().clone());
    match svc.create(input).await {
        Ok(m) => (StatusCode::CREATED, Json(m)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

async fn update_movie(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(input): Json<UpdateMovieInput>,
) -> impl IntoResponse {
    let svc = MovieService::new(state.db.pool().clone());
    match svc.update(id, input).await {
        Ok(m) => Json(m).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

async fn delete_movie(
    State(state): State<Arc<AppState>>,
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
        .route("/api/v1/movie/lookup", get(lookup_movie))
}
