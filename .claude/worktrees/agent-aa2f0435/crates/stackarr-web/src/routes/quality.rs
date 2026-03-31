use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};

use stackarr_quality::{CreateProfileInput, QualityProfileService, UpdateProfileInput};

use crate::AppState;

async fn list_profiles(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let svc = QualityProfileService::new(state.db.pool().clone());
    match svc.list().await {
        Ok(list) => Json(list).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn get_profile(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let svc = QualityProfileService::new(state.db.pool().clone());
    match svc.get(id).await {
        Ok(p) => Json(p).into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn create_profile(
    State(state): State<Arc<AppState>>,
    Json(input): Json<CreateProfileInput>,
) -> impl IntoResponse {
    let svc = QualityProfileService::new(state.db.pool().clone());
    match svc.create(input).await {
        Ok(p) => (StatusCode::CREATED, Json(p)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

async fn update_profile(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(input): Json<UpdateProfileInput>,
) -> impl IntoResponse {
    let svc = QualityProfileService::new(state.db.pool().clone());
    match svc.update(id, input).await {
        Ok(p) => Json(p).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

async fn delete_profile(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let svc = QualityProfileService::new(state.db.pool().clone());
    match svc.delete(id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/v1/qualityprofile",
            get(list_profiles).post(create_profile),
        )
        .route(
            "/api/v1/qualityprofile/{id}",
            get(get_profile).put(update_profile).delete(delete_profile),
        )
}
