use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};

use stackarr_media::{CreateMovieInput, MovieService, UpdateMovieInput};

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

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/movies", get(list_movies).post(create_movie))
        .route(
            "/api/v1/movies/{id}",
            get(get_movie).put(update_movie).delete(delete_movie),
        )
}
