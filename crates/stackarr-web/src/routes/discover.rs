use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use stackarr_core::models::discover::{
    CreateDiscoverSliderInput, DiscoverSlider, ReorderSlidersInput, UpdateDiscoverSliderInput,
};
use stackarr_metadata::{DiscoverFilters, TmdbClient};

use crate::AppState;

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Resolve the TMDB API key from env or database.
async fn resolve_tmdb_key(state: &AppState) -> Option<String> {
    if let Ok(key) = std::env::var("STACKARR_TMDB_API_KEY") {
        if !key.is_empty() {
            return Some(key);
        }
    }
    let pool = state.db.pool();
    let val: Option<serde_json::Value> = sqlx::query_scalar(
        "SELECT value FROM app_config WHERE key = 'tmdb_api_key'",
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();
    val.and_then(|v| v.as_str().map(|s| s.to_string()))
        .filter(|s| !s.is_empty())
}

fn no_tmdb_key() -> impl IntoResponse {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({"error": "TMDB API key not configured"})),
    )
}

fn tmdb_error(e: impl std::fmt::Display) -> impl IntoResponse {
    (
        StatusCode::BAD_GATEWAY,
        Json(json!({"error": format!("TMDB request failed: {e}")})),
    )
}

// ── Trending ────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrendingQuery {
    media_type: Option<String>,
    time_window: Option<String>,
    page: Option<i64>,
    language: Option<String>,
}

async fn get_trending(
    State(state): State<Arc<AppState>>,
    Query(query): Query<TrendingQuery>,
) -> impl IntoResponse {
    let Some(key) = resolve_tmdb_key(&state).await else {
        return no_tmdb_key().into_response();
    };
    let client = TmdbClient::new(key);
    let media_type = query.media_type.as_deref().unwrap_or("all");
    let time_window = query.time_window.as_deref().unwrap_or("day");
    match client
        .get_trending(media_type, time_window, query.page, query.language.as_deref())
        .await
    {
        Ok(results) => Json(results).into_response(),
        Err(e) => tmdb_error(e).into_response(),
    }
}

// ── Discover Movies ─────────────────────────────────────────────────────────

async fn discover_movies(
    State(state): State<Arc<AppState>>,
    Query(filters): Query<DiscoverFilters>,
) -> impl IntoResponse {
    let Some(key) = resolve_tmdb_key(&state).await else {
        return no_tmdb_key().into_response();
    };
    let client = TmdbClient::new(key);
    match client.discover_movies(&filters).await {
        Ok(results) => Json(results).into_response(),
        Err(e) => tmdb_error(e).into_response(),
    }
}

#[derive(Deserialize)]
struct GenrePathParam {
    genre_id: i64,
}

async fn discover_movies_by_genre(
    State(state): State<Arc<AppState>>,
    Path(params): Path<GenrePathParam>,
    Query(mut filters): Query<DiscoverFilters>,
) -> impl IntoResponse {
    let Some(key) = resolve_tmdb_key(&state).await else {
        return no_tmdb_key().into_response();
    };
    filters.with_genres = Some(params.genre_id.to_string());
    let client = TmdbClient::new(key);
    match client.discover_movies(&filters).await {
        Ok(results) => Json(results).into_response(),
        Err(e) => tmdb_error(e).into_response(),
    }
}

#[derive(Deserialize)]
struct StudioPathParam {
    studio_id: i64,
}

async fn discover_movies_by_studio(
    State(state): State<Arc<AppState>>,
    Path(params): Path<StudioPathParam>,
    Query(mut filters): Query<DiscoverFilters>,
) -> impl IntoResponse {
    let Some(key) = resolve_tmdb_key(&state).await else {
        return no_tmdb_key().into_response();
    };
    filters.with_companies = Some(params.studio_id.to_string());
    let client = TmdbClient::new(key);
    match client.discover_movies(&filters).await {
        Ok(results) => Json(results).into_response(),
        Err(e) => tmdb_error(e).into_response(),
    }
}

async fn discover_movies_upcoming(
    State(state): State<Arc<AppState>>,
    Query(mut filters): Query<DiscoverFilters>,
) -> impl IntoResponse {
    let Some(key) = resolve_tmdb_key(&state).await else {
        return no_tmdb_key().into_response();
    };
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    filters.primary_release_date_gte = Some(today);
    if filters.sort_by.is_none() {
        filters.sort_by = Some("popularity.desc".to_string());
    }
    let client = TmdbClient::new(key);
    match client.discover_movies(&filters).await {
        Ok(results) => Json(results).into_response(),
        Err(e) => tmdb_error(e).into_response(),
    }
}

// ── Discover TV ─────────────────────────────────────────────────────────────

async fn discover_tv(
    State(state): State<Arc<AppState>>,
    Query(filters): Query<DiscoverFilters>,
) -> impl IntoResponse {
    let Some(key) = resolve_tmdb_key(&state).await else {
        return no_tmdb_key().into_response();
    };
    let client = TmdbClient::new(key);
    match client.discover_tv(&filters).await {
        Ok(results) => Json(results).into_response(),
        Err(e) => tmdb_error(e).into_response(),
    }
}

async fn discover_tv_by_genre(
    State(state): State<Arc<AppState>>,
    Path(params): Path<GenrePathParam>,
    Query(mut filters): Query<DiscoverFilters>,
) -> impl IntoResponse {
    let Some(key) = resolve_tmdb_key(&state).await else {
        return no_tmdb_key().into_response();
    };
    filters.with_genres = Some(params.genre_id.to_string());
    let client = TmdbClient::new(key);
    match client.discover_tv(&filters).await {
        Ok(results) => Json(results).into_response(),
        Err(e) => tmdb_error(e).into_response(),
    }
}

#[derive(Deserialize)]
struct NetworkPathParam {
    network_id: i64,
}

async fn discover_tv_by_network(
    State(state): State<Arc<AppState>>,
    Path(params): Path<NetworkPathParam>,
    Query(mut filters): Query<DiscoverFilters>,
) -> impl IntoResponse {
    let Some(key) = resolve_tmdb_key(&state).await else {
        return no_tmdb_key().into_response();
    };
    filters.with_networks = Some(params.network_id.to_string());
    let client = TmdbClient::new(key);
    match client.discover_tv(&filters).await {
        Ok(results) => Json(results).into_response(),
        Err(e) => tmdb_error(e).into_response(),
    }
}

async fn discover_tv_upcoming(
    State(state): State<Arc<AppState>>,
    Query(mut filters): Query<DiscoverFilters>,
) -> impl IntoResponse {
    let Some(key) = resolve_tmdb_key(&state).await else {
        return no_tmdb_key().into_response();
    };
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    filters.first_air_date_gte = Some(today);
    if filters.sort_by.is_none() {
        filters.sort_by = Some("popularity.desc".to_string());
    }
    let client = TmdbClient::new(key);
    match client.discover_tv(&filters).await {
        Ok(results) => Json(results).into_response(),
        Err(e) => tmdb_error(e).into_response(),
    }
}

// ── Recommendations & Similar ───────────────────────────────────────────────

#[derive(Deserialize)]
struct MediaIdParam {
    id: i64,
}

#[derive(Deserialize)]
struct RecQuery {
    page: Option<i64>,
    language: Option<String>,
}

async fn movie_recommendations(
    State(state): State<Arc<AppState>>,
    Path(params): Path<MediaIdParam>,
    Query(query): Query<RecQuery>,
) -> impl IntoResponse {
    let Some(key) = resolve_tmdb_key(&state).await else {
        return no_tmdb_key().into_response();
    };
    let client = TmdbClient::new(key);
    match client
        .get_movie_recommendations(params.id, query.page, query.language.as_deref())
        .await
    {
        Ok(results) => Json(results).into_response(),
        Err(e) => tmdb_error(e).into_response(),
    }
}

async fn movie_similar(
    State(state): State<Arc<AppState>>,
    Path(params): Path<MediaIdParam>,
    Query(query): Query<RecQuery>,
) -> impl IntoResponse {
    let Some(key) = resolve_tmdb_key(&state).await else {
        return no_tmdb_key().into_response();
    };
    let client = TmdbClient::new(key);
    match client
        .get_movie_similar(params.id, query.page, query.language.as_deref())
        .await
    {
        Ok(results) => Json(results).into_response(),
        Err(e) => tmdb_error(e).into_response(),
    }
}

async fn tv_recommendations(
    State(state): State<Arc<AppState>>,
    Path(params): Path<MediaIdParam>,
    Query(query): Query<RecQuery>,
) -> impl IntoResponse {
    let Some(key) = resolve_tmdb_key(&state).await else {
        return no_tmdb_key().into_response();
    };
    let client = TmdbClient::new(key);
    match client
        .get_tv_recommendations(params.id, query.page, query.language.as_deref())
        .await
    {
        Ok(results) => Json(results).into_response(),
        Err(e) => tmdb_error(e).into_response(),
    }
}

async fn tv_similar(
    State(state): State<Arc<AppState>>,
    Path(params): Path<MediaIdParam>,
    Query(query): Query<RecQuery>,
) -> impl IntoResponse {
    let Some(key) = resolve_tmdb_key(&state).await else {
        return no_tmdb_key().into_response();
    };
    let client = TmdbClient::new(key);
    match client
        .get_tv_similar(params.id, query.page, query.language.as_deref())
        .await
    {
        Ok(results) => Json(results).into_response(),
        Err(e) => tmdb_error(e).into_response(),
    }
}

// ── Genres ───────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct LangQuery {
    language: Option<String>,
}

async fn get_movie_genres(
    State(state): State<Arc<AppState>>,
    Query(query): Query<LangQuery>,
) -> impl IntoResponse {
    let Some(key) = resolve_tmdb_key(&state).await else {
        return no_tmdb_key().into_response();
    };
    let client = TmdbClient::new(key);
    match client.get_movie_genres(query.language.as_deref()).await {
        Ok(genres) => Json(genres).into_response(),
        Err(e) => tmdb_error(e).into_response(),
    }
}

async fn get_tv_genres(
    State(state): State<Arc<AppState>>,
    Query(query): Query<LangQuery>,
) -> impl IntoResponse {
    let Some(key) = resolve_tmdb_key(&state).await else {
        return no_tmdb_key().into_response();
    };
    let client = TmdbClient::new(key);
    match client.get_tv_genres(query.language.as_deref()).await {
        Ok(genres) => Json(genres).into_response(),
        Err(e) => tmdb_error(e).into_response(),
    }
}

async fn get_languages(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let Some(key) = resolve_tmdb_key(&state).await else {
        return no_tmdb_key().into_response();
    };
    let client = TmdbClient::new(key);
    match client.get_languages().await {
        Ok(languages) => Json(languages).into_response(),
        Err(e) => tmdb_error(e).into_response(),
    }
}

// ── Keywords ────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct KeywordPathParam {
    keyword_id: i64,
}

async fn get_keyword(
    State(state): State<Arc<AppState>>,
    Path(params): Path<KeywordPathParam>,
) -> impl IntoResponse {
    let Some(key) = resolve_tmdb_key(&state).await else {
        return no_tmdb_key().into_response();
    };
    let client = TmdbClient::new(key);
    match client.get_keyword(params.keyword_id).await {
        Ok(kw) => Json(kw).into_response(),
        Err(e) => tmdb_error(e).into_response(),
    }
}

async fn get_movies_by_keyword(
    State(state): State<Arc<AppState>>,
    Path(params): Path<KeywordPathParam>,
    Query(query): Query<RecQuery>,
) -> impl IntoResponse {
    let Some(key) = resolve_tmdb_key(&state).await else {
        return no_tmdb_key().into_response();
    };
    let client = TmdbClient::new(key);
    match client
        .get_movies_by_keyword(params.keyword_id, query.page, query.language.as_deref())
        .await
    {
        Ok(results) => Json(results).into_response(),
        Err(e) => tmdb_error(e).into_response(),
    }
}

// ── Discover Sliders CRUD ───────────────────────────────────────────────────

async fn list_sliders(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let pool = state.db.pool();
    match sqlx::query_as::<_, DiscoverSlider>(
        "SELECT id, slider_type, display_order, is_built_in, enabled, title, custom_data, created_at, updated_at \
         FROM discover_sliders ORDER BY display_order ASC",
    )
    .fetch_all(pool)
    .await
    {
        Ok(sliders) => Json(sliders).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to list discover sliders");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal server error").into_response()
        }
    }
}

async fn create_slider(
    State(state): State<Arc<AppState>>,
    Json(input): Json<CreateDiscoverSliderInput>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    // Get the next display_order
    let max_order: Option<(i32,)> =
        sqlx::query_as("SELECT COALESCE(MAX(display_order), 0) FROM discover_sliders")
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();
    let next_order = max_order.map(|r| r.0 + 1).unwrap_or(1);

    let slider_type = serde_json::to_value(&input.slider_type)
        .ok()
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_default();

    match sqlx::query_as::<_, DiscoverSlider>(
        "INSERT INTO discover_sliders (slider_type, display_order, is_built_in, enabled, title, custom_data) \
         VALUES ($1, $2, false, $3, $4, $5) \
         RETURNING id, slider_type, display_order, is_built_in, enabled, title, custom_data, created_at, updated_at",
    )
    .bind(&slider_type)
    .bind(next_order)
    .bind(input.enabled.unwrap_or(true))
    .bind(&input.title)
    .bind(&input.custom_data)
    .fetch_one(pool)
    .await
    {
        Ok(slider) => (StatusCode::CREATED, Json(slider)).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to create discover slider");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal server error").into_response()
        }
    }
}

async fn update_slider(
    State(state): State<Arc<AppState>>,
    Path(slider_id): Path<i64>,
    Json(input): Json<UpdateDiscoverSliderInput>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    // Build dynamic update
    let existing = sqlx::query_as::<_, DiscoverSlider>(
        "SELECT id, slider_type, display_order, is_built_in, enabled, title, custom_data, created_at, updated_at \
         FROM discover_sliders WHERE id = $1",
    )
    .bind(slider_id)
    .fetch_optional(pool)
    .await;

    let existing = match existing {
        Ok(Some(s)) => s,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            tracing::error!(error = %e, slider_id, "failed to fetch discover slider for update");
            return (StatusCode::INTERNAL_SERVER_ERROR, "internal server error").into_response();
        }
    };

    let new_title = input.title.or(existing.title);
    let new_enabled = input.enabled.unwrap_or(existing.enabled);
    let new_custom_data = if input.custom_data.is_some() {
        input.custom_data
    } else {
        existing.custom_data
    };

    match sqlx::query_as::<_, DiscoverSlider>(
        "UPDATE discover_sliders SET title = $1, enabled = $2, custom_data = $3, updated_at = NOW() \
         WHERE id = $4 \
         RETURNING id, slider_type, display_order, is_built_in, enabled, title, custom_data, created_at, updated_at",
    )
    .bind(&new_title)
    .bind(new_enabled)
    .bind(&new_custom_data)
    .bind(slider_id)
    .fetch_one(pool)
    .await
    {
        Ok(slider) => Json(slider).into_response(),
        Err(e) => {
            tracing::error!(error = %e, slider_id, "failed to update discover slider");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal server error").into_response()
        }
    }
}

async fn delete_slider(
    State(state): State<Arc<AppState>>,
    Path(slider_id): Path<i64>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    // Don't allow deleting built-in sliders
    let is_built_in: Option<(bool,)> =
        sqlx::query_as("SELECT is_built_in FROM discover_sliders WHERE id = $1")
            .bind(slider_id)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();

    match is_built_in {
        Some((true,)) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Cannot delete built-in sliders. Disable them instead."})),
            )
                .into_response();
        }
        None => return StatusCode::NOT_FOUND.into_response(),
        _ => {}
    }

    match sqlx::query("DELETE FROM discover_sliders WHERE id = $1")
        .bind(slider_id)
        .execute(pool)
        .await
    {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => {
            tracing::error!(error = %e, slider_id, "failed to delete discover slider");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal server error").into_response()
        }
    }
}

async fn reorder_sliders(
    State(state): State<Arc<AppState>>,
    Json(input): Json<ReorderSlidersInput>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    for (idx, slider_id) in input.slider_ids.iter().enumerate() {
        let order = (idx + 1) as i32;
        if let Err(e) =
            sqlx::query("UPDATE discover_sliders SET display_order = $1, updated_at = NOW() WHERE id = $2")
                .bind(order)
                .bind(slider_id)
                .execute(pool)
                .await
        {
            tracing::error!(error = %e, slider_id, "failed to reorder discover slider");
            return (StatusCode::INTERNAL_SERVER_ERROR, "internal server error").into_response();
        }
    }

    // Return the updated list
    list_sliders(State(state)).await.into_response()
}

async fn reset_sliders(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let pool = state.db.pool();

    // Delete custom sliders, re-enable and reorder built-ins
    if let Err(e) = sqlx::query("DELETE FROM discover_sliders WHERE is_built_in = false")
        .execute(pool)
        .await
    {
        tracing::error!(error = %e, "failed to reset discover sliders");
        return (StatusCode::INTERNAL_SERVER_ERROR, "internal server error").into_response();
    }

    // Reset built-in order and enabled state
    let defaults = [
        ("trending", 1),
        ("popular_movies", 2),
        ("popular_tv", 3),
        ("upcoming_movies", 4),
        ("upcoming_tv", 5),
        ("recently_added", 6),
        ("movie_genres", 7),
        ("tv_genres", 8),
    ];

    for (slider_type, order) in defaults {
        let _ = sqlx::query(
            "UPDATE discover_sliders SET display_order = $1, enabled = true, updated_at = NOW() \
             WHERE slider_type = $2 AND is_built_in = true",
        )
        .bind(order)
        .bind(slider_type)
        .execute(pool)
        .await;
    }

    list_sliders(State(state)).await.into_response()
}

// ── Router ──────────────────────────────────────────────────────────────────

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        // Trending
        .route("/api/v1/discover/trending", get(get_trending))
        // Movies discovery
        .route("/api/v1/discover/movies", get(discover_movies))
        .route("/api/v1/discover/movies/upcoming", get(discover_movies_upcoming))
        .route("/api/v1/discover/movies/genre/{genre_id}", get(discover_movies_by_genre))
        .route("/api/v1/discover/movies/studio/{studio_id}", get(discover_movies_by_studio))
        // TV discovery
        .route("/api/v1/discover/tv", get(discover_tv))
        .route("/api/v1/discover/tv/upcoming", get(discover_tv_upcoming))
        .route("/api/v1/discover/tv/genre/{genre_id}", get(discover_tv_by_genre))
        .route("/api/v1/discover/tv/network/{network_id}", get(discover_tv_by_network))
        // Recommendations & similar
        .route("/api/v1/discover/movies/{id}/recommendations", get(movie_recommendations))
        .route("/api/v1/discover/movies/{id}/similar", get(movie_similar))
        .route("/api/v1/discover/tv/{id}/recommendations", get(tv_recommendations))
        .route("/api/v1/discover/tv/{id}/similar", get(tv_similar))
        // Genres & languages
        .route("/api/v1/discover/genres/movie", get(get_movie_genres))
        .route("/api/v1/discover/genres/tv", get(get_tv_genres))
        .route("/api/v1/discover/languages", get(get_languages))
        // Keywords
        .route("/api/v1/discover/keyword/{keyword_id}", get(get_keyword))
        .route("/api/v1/discover/keyword/{keyword_id}/movies", get(get_movies_by_keyword))
        // Slider management
        .route("/api/v1/discover/sliders", get(list_sliders).post(reorder_sliders))
        .route("/api/v1/discover/sliders/add", post(create_slider))
        .route("/api/v1/discover/sliders/reset", post(reset_sliders))
        .route(
            "/api/v1/discover/sliders/{slider_id}",
            put(update_slider).delete(delete_slider),
        )
}
