use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};

use stackarr_media::{CreateSeriesInput, SeriesService, UpdateSeriesInput};

use crate::AppState;

async fn list_series(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let svc = SeriesService::new(state.db.pool().clone());
    match svc.list().await {
        Ok(list) => Json(list).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn get_series(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let svc = SeriesService::new(state.db.pool().clone());
    match svc.get(id).await {
        Ok(s) => Json(s).into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn create_series(
    State(state): State<Arc<AppState>>,
    Json(input): Json<CreateSeriesInput>,
) -> impl IntoResponse {
    let svc = SeriesService::new(state.db.pool().clone());
    match svc.create(input).await {
        Ok(s) => (StatusCode::CREATED, Json(s)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

async fn update_series(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(input): Json<UpdateSeriesInput>,
) -> impl IntoResponse {
    let svc = SeriesService::new(state.db.pool().clone());
    match svc.update(id, input).await {
        Ok(s) => Json(s).into_response(),
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

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/series", get(list_series).post(create_series))
        .route(
            "/api/v1/series/{id}",
            get(get_series).put(update_series).delete(delete_series),
        )
}
