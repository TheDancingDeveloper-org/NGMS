use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, put};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::AppState;
use crate::middleware::RequireUser;
use crate::routes::extract_image_url;

// ── Types ────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WatchlistQuery {
    media_type: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RatingBody {
    rating: i16,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RatingsQuery {
    media_type: Option<String>,
}

// ── Watchlist routes ─────────────────────────────────────────────────────────

async fn list_watchlist(
    State(state): State<Arc<AppState>>,
    RequireUser(auth_user): RequireUser,
    Query(q): Query<WatchlistQuery>,
) -> impl IntoResponse {
    let items = match state
        .db
        .get_watchlist(auth_user.user_id, q.media_type.as_deref())
        .await
    {
        Ok(items) => items,
        Err(e) => {
            tracing::error!(error = %e, "failed to get watchlist");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response();
        }
    };

    // Enrich with title/poster from series/movies tables
    let pool = state.db.pool();
    let mut enriched = Vec::with_capacity(items.len());

    for item in &items {
        let (title, poster_url, year) = match item.media_type.as_str() {
            "series" => {
                let row: Option<(String, Option<serde_json::Value>, Option<i32>)> =
                    sqlx::query_as("SELECT title, images, year FROM series WHERE id = $1")
                        .bind(item.media_id)
                        .fetch_optional(pool)
                        .await
                        .unwrap_or(None);
                match row {
                    Some((t, images, y)) => (Some(t), extract_image_url(&images, "poster"), y),
                    None => (None, None, None),
                }
            }
            "movie" => {
                let row: Option<(String, Option<serde_json::Value>, Option<i32>)> =
                    sqlx::query_as("SELECT title, images, year FROM movies WHERE id = $1")
                        .bind(item.media_id)
                        .fetch_optional(pool)
                        .await
                        .unwrap_or(None);
                match row {
                    Some((t, images, y)) => (Some(t), extract_image_url(&images, "poster"), y),
                    None => (None, None, None),
                }
            }
            _ => (None, None, None),
        };

        enriched.push(json!({
            "id": item.id,
            "userId": item.user_id,
            "mediaType": item.media_type,
            "mediaId": item.media_id,
            "tmdbId": item.tmdb_id,
            "addedAt": item.added_at,
            "title": title,
            "posterUrl": poster_url,
            "year": year,
        }));
    }

    Json(serde_json::Value::Array(enriched)).into_response()
}

async fn add_to_watchlist(
    State(state): State<Arc<AppState>>,
    RequireUser(auth_user): RequireUser,
    Path((media_type, media_id)): Path<(String, i64)>,
) -> impl IntoResponse {
    // Validate media_type
    if media_type != "series" && media_type != "movie" {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "mediaType must be 'series' or 'movie'"})),
        )
            .into_response();
    }

    // Look up tmdb_id from the media table
    let pool = state.db.pool();
    let tmdb_id: Option<Option<i64>> = match media_type.as_str() {
        "series" => sqlx::query_scalar("SELECT tmdb_id FROM series WHERE id = $1")
            .bind(media_id)
            .fetch_optional(pool)
            .await
            .unwrap_or(None),
        "movie" => sqlx::query_scalar("SELECT tmdb_id FROM movies WHERE id = $1")
            .bind(media_id)
            .fetch_optional(pool)
            .await
            .unwrap_or(None),
        _ => None,
    };

    let tmdb_id = match tmdb_id {
        Some(Some(id)) => id,
        _ => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "media not found"})),
            )
                .into_response();
        }
    };

    match state
        .db
        .add_to_watchlist(auth_user.user_id, &media_type, media_id, tmdb_id)
        .await
    {
        Ok(item) => Json(serde_json::to_value(item).unwrap_or_default()).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to add to watchlist");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

async fn remove_from_watchlist(
    State(state): State<Arc<AppState>>,
    RequireUser(auth_user): RequireUser,
    Path((media_type, media_id)): Path<(String, i64)>,
) -> impl IntoResponse {
    match state
        .db
        .remove_from_watchlist(auth_user.user_id, &media_type, media_id)
        .await
    {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "item not on watchlist"})),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to remove from watchlist");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

// ── Rating routes ────────────────────────────────────────────────────────────

async fn list_ratings(
    State(state): State<Arc<AppState>>,
    RequireUser(auth_user): RequireUser,
    Query(q): Query<RatingsQuery>,
) -> impl IntoResponse {
    match state
        .db
        .get_user_ratings(auth_user.user_id, q.media_type.as_deref())
        .await
    {
        Ok(ratings) => Json(serde_json::to_value(ratings).unwrap_or_default()).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to get ratings");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

async fn set_rating(
    State(state): State<Arc<AppState>>,
    RequireUser(auth_user): RequireUser,
    Path((media_type, media_id)): Path<(String, i64)>,
    Json(body): Json<RatingBody>,
) -> impl IntoResponse {
    if body.rating < 1 || body.rating > 10 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "rating must be between 1 and 10"})),
        )
            .into_response();
    }

    if media_type != "series" && media_type != "movie" {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "mediaType must be 'series' or 'movie'"})),
        )
            .into_response();
    }

    match state
        .db
        .set_rating(auth_user.user_id, &media_type, media_id, body.rating)
        .await
    {
        Ok(rating) => Json(serde_json::to_value(rating).unwrap_or_default()).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to set rating");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

async fn get_rating(
    State(state): State<Arc<AppState>>,
    RequireUser(auth_user): RequireUser,
    Path((media_type, media_id)): Path<(String, i64)>,
) -> impl IntoResponse {
    let user_rating = state
        .db
        .get_rating(auth_user.user_id, &media_type, media_id)
        .await;
    let average = state.db.get_average_rating(&media_type, media_id).await;

    match (user_rating, average) {
        (Ok(rating), Ok((avg, count))) => Json(json!({
            "userRating": rating.map(|r| r.rating),
            "averageRating": avg,
            "ratingCount": count,
        }))
        .into_response(),
        (Err(e), _) | (_, Err(e)) => {
            tracing::error!(error = %e, "failed to get rating");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

async fn delete_rating(
    State(state): State<Arc<AppState>>,
    RequireUser(auth_user): RequireUser,
    Path((media_type, media_id)): Path<(String, i64)>,
) -> impl IntoResponse {
    match state
        .db
        .delete_rating(auth_user.user_id, &media_type, media_id)
        .await
    {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "rating not found"})),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to delete rating");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

// ── Router ───────────────────────────────────────────────────────────────────

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        // Watchlist
        .route("/api/v1/user/watchlist", get(list_watchlist))
        .route(
            "/api/v1/user/watchlist/{mediaType}/{mediaId}",
            put(add_to_watchlist).delete(remove_from_watchlist),
        )
        // Ratings
        .route("/api/v1/user/ratings", get(list_ratings))
        .route(
            "/api/v1/user/ratings/{mediaType}/{mediaId}",
            put(set_rating).get(get_rating).delete(delete_rating),
        )
}
