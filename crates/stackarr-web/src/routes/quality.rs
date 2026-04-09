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

#[utoipa::path(
    get,
    path = "/api/v1/qualityprofile",
    tag = "Quality",
    operation_id = "listQualityProfiles",
    responses(
        (status = 200, description = "List of quality profiles"),
    ),
    security(("ApiKeyAuth" = []), ("BearerAuth" = [])),
)]
pub async fn list_profiles(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let svc = QualityProfileService::new(state.db.pool().clone());
    match svc.list().await {
        Ok(list) => Json(list).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/qualityprofile/{id}",
    tag = "Quality",
    operation_id = "getQualityProfile",
    params(("id" = i64, Path, description = "Quality profile ID")),
    responses(
        (status = 200, description = "Quality profile details"),
        (status = 404, description = "Profile not found"),
    ),
    security(("ApiKeyAuth" = []), ("BearerAuth" = [])),
)]
pub async fn get_profile(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> impl IntoResponse {
    let svc = QualityProfileService::new(state.db.pool().clone());
    match svc.get(id).await {
        Ok(p) => Json(p).into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/qualityprofile",
    tag = "Quality",
    operation_id = "createQualityProfile",
    request_body = serde_json::Value,
    responses(
        (status = 201, description = "Quality profile created"),
        (status = 400, description = "Invalid input"),
    ),
    security(("ApiKeyAuth" = []), ("BearerAuth" = [])),
)]
pub async fn create_profile(
    State(state): State<Arc<AppState>>,
    Json(input): Json<CreateProfileInput>,
) -> impl IntoResponse {
    let svc = QualityProfileService::new(state.db.pool().clone());
    match svc.create(input).await {
        Ok(p) => (StatusCode::CREATED, Json(p)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

#[utoipa::path(
    put,
    path = "/api/v1/qualityprofile/{id}",
    tag = "Quality",
    operation_id = "updateQualityProfile",
    params(("id" = i64, Path, description = "Quality profile ID")),
    request_body = serde_json::Value,
    responses(
        (status = 200, description = "Quality profile updated"),
        (status = 400, description = "Invalid input"),
    ),
    security(("ApiKeyAuth" = []), ("BearerAuth" = [])),
)]
pub async fn update_profile(
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

#[utoipa::path(
    delete,
    path = "/api/v1/qualityprofile/{id}",
    tag = "Quality",
    operation_id = "deleteQualityProfile",
    params(("id" = i64, Path, description = "Quality profile ID")),
    responses(
        (status = 204, description = "Quality profile deleted"),
    ),
    security(("ApiKeyAuth" = []), ("BearerAuth" = [])),
)]
pub async fn delete_profile(
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

#[utoipa::path(
    get,
    path = "/api/v1/customformat",
    tag = "Quality",
    operation_id = "listCustomFormats",
    responses(
        (status = 200, description = "List of custom formats"),
    ),
    security(("ApiKeyAuth" = []), ("BearerAuth" = [])),
)]
pub async fn list_custom_formats(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let svc = CustomFormatService::new(state.db.pool().clone());
    match svc.list().await {
        Ok(list) => Json(list).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/customformat/{id}",
    tag = "Quality",
    operation_id = "getCustomFormat",
    params(("id" = i64, Path, description = "Custom format ID")),
    responses(
        (status = 200, description = "Custom format details"),
        (status = 404, description = "Custom format not found"),
    ),
    security(("ApiKeyAuth" = []), ("BearerAuth" = [])),
)]
pub async fn get_custom_format(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let svc = CustomFormatService::new(state.db.pool().clone());
    match svc.get(id).await {
        Ok(cf) => Json(cf).into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/customformat",
    tag = "Quality",
    operation_id = "createCustomFormat",
    request_body = serde_json::Value,
    responses(
        (status = 201, description = "Custom format created"),
        (status = 400, description = "Invalid input"),
    ),
    security(("ApiKeyAuth" = []), ("BearerAuth" = [])),
)]
pub async fn create_custom_format(
    State(state): State<Arc<AppState>>,
    Json(input): Json<CreateCustomFormatInput>,
) -> impl IntoResponse {
    let svc = CustomFormatService::new(state.db.pool().clone());
    match svc.create(input).await {
        Ok(cf) => (StatusCode::CREATED, Json(cf)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

#[utoipa::path(
    put,
    path = "/api/v1/customformat/{id}",
    tag = "Quality",
    operation_id = "updateCustomFormat",
    params(("id" = i64, Path, description = "Custom format ID")),
    request_body = serde_json::Value,
    responses(
        (status = 200, description = "Custom format updated"),
        (status = 400, description = "Invalid input"),
    ),
    security(("ApiKeyAuth" = []), ("BearerAuth" = [])),
)]
pub async fn update_custom_format(
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

#[utoipa::path(
    delete,
    path = "/api/v1/customformat/{id}",
    tag = "Quality",
    operation_id = "deleteCustomFormat",
    params(("id" = i64, Path, description = "Custom format ID")),
    responses(
        (status = 204, description = "Custom format deleted"),
    ),
    security(("ApiKeyAuth" = []), ("BearerAuth" = [])),
)]
pub async fn delete_custom_format(
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
pub struct TestCustomFormatRequest {
    release_title: String,
    specifications: Vec<FormatSpec>,
}

#[utoipa::path(
    post,
    path = "/api/v1/customformat/test",
    tag = "Quality",
    operation_id = "testCustomFormat",
    request_body = serde_json::Value,
    responses(
        (status = 200, description = "Custom format test result"),
    ),
    security(("ApiKeyAuth" = []), ("BearerAuth" = [])),
)]
pub async fn test_custom_format(Json(input): Json<TestCustomFormatRequest>) -> impl IntoResponse {
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
