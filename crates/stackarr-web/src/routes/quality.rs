use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;

use stackarr_quality::custom_formats::{
    CustomFormatDef, CustomFormatEngine, FormatSpec, ReleaseContext,
};
use stackarr_quality::{
    CreateCustomFormatInput, CreateProfileInput, CustomFormatService, QualityProfileService,
    UpdateCustomFormatInput, UpdateProfileInput,
};

use crate::AppState;

// ── Quality Profile handlers ───────────────────────────────────────────────

async fn list_profiles(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let svc = QualityProfileService::new(state.db.pool().clone());
    match svc.list().await {
        Ok(list) => Json(list).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn get_profile(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> impl IntoResponse {
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

// ── Custom Format handlers ─────────────────────────────────────────────────

async fn list_custom_formats(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let svc = CustomFormatService::new(state.db.pool().clone());
    match svc.list().await {
        Ok(list) => Json(list).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn get_custom_format(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let svc = CustomFormatService::new(state.db.pool().clone());
    match svc.get(id).await {
        Ok(cf) => Json(cf).into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn create_custom_format(
    State(state): State<Arc<AppState>>,
    Json(input): Json<CreateCustomFormatInput>,
) -> impl IntoResponse {
    let svc = CustomFormatService::new(state.db.pool().clone());
    match svc.create(input).await {
        Ok(cf) => (StatusCode::CREATED, Json(cf)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

async fn update_custom_format(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(input): Json<UpdateCustomFormatInput>,
) -> impl IntoResponse {
    let svc = CustomFormatService::new(state.db.pool().clone());
    match svc.update(id, input).await {
        Ok(cf) => Json(cf).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

async fn delete_custom_format(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let svc = CustomFormatService::new(state.db.pool().clone());
    match svc.delete(id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

// ── Custom Format test endpoint ────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestCustomFormatRequest {
    release_title: String,
    specifications: Vec<FormatSpec>,
}

async fn test_custom_format(Json(input): Json<TestCustomFormatRequest>) -> impl IntoResponse {
    let engine = CustomFormatEngine::new();
    let format = CustomFormatDef {
        id: 0,
        name: "test".to_string(),
        specifications: input.specifications,
    };

    let ctx = ReleaseContext::default();
    let result =
        engine.score_release_with_context(&input.release_title, &[format], &[(0, 0)], &ctx);
    let matched = !result.matched_formats.is_empty();

    Json(serde_json::json!({
        "matched": matched,
        "releaseTitle": input.release_title,
    }))
}

// ── Router ─────────────────────────────────────────────────────────────────

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
        .route(
            "/api/v1/customformat",
            get(list_custom_formats).post(create_custom_format),
        )
        .route(
            "/api/v1/customformat/{id}",
            get(get_custom_format)
                .put(update_custom_format)
                .delete(delete_custom_format),
        )
        .route("/api/v1/customformat/test", post(test_custom_format))
}
