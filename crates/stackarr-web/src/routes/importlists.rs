use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::json;

use stackarr_media::import_lists::{
    CreateImportListInput, ImportListService, UpdateImportListInput,
};
use stackarr_metadata::TmdbClient;

use crate::AppState;

async fn list_all(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let svc = ImportListService::new(state.db.pool().clone());
    match svc.list().await {
        Ok(list) => Json(list).into_response(),
        Err(e) => super::api_error(StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}

async fn create(
    State(state): State<Arc<AppState>>,
    Json(input): Json<CreateImportListInput>,
) -> impl IntoResponse {
    let svc = ImportListService::new(state.db.pool().clone());
    match svc.create(input).await {
        Ok(il) => (StatusCode::CREATED, Json(il)).into_response(),
        Err(e) => super::api_error(StatusCode::BAD_REQUEST, e),
    }
}

async fn update(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(input): Json<UpdateImportListInput>,
) -> impl IntoResponse {
    let svc = ImportListService::new(state.db.pool().clone());
    match svc.update(id, input).await {
        Ok(il) => Json(il).into_response(),
        Err(e) => super::api_error(StatusCode::BAD_REQUEST, e),
    }
}

async fn delete(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> impl IntoResponse {
    let svc = ImportListService::new(state.db.pool().clone());
    match svc.delete(id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => super::api_error(StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}

async fn sync_one(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> impl IntoResponse {
    let tmdb_client = match get_tmdb_client(&state).await {
        Ok(c) => c,
        Err(resp) => return resp,
    };

    let svc = ImportListService::new(state.db.pool().clone());
    match svc.sync(id, &tmdb_client).await {
        Ok(result) => Json(result).into_response(),
        Err(e) => super::api_error(StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}

async fn sync_all(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let tmdb_client = match get_tmdb_client(&state).await {
        Ok(c) => c,
        Err(resp) => return resp,
    };

    let svc = ImportListService::new(state.db.pool().clone());
    match svc.sync_all(&tmdb_client).await {
        Ok(results) => Json(results).into_response(),
        Err(e) => super::api_error(StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}

/// Resolve a TMDB API key from env or DB and build a client.
async fn get_tmdb_client(state: &AppState) -> Result<TmdbClient, axum::response::Response> {
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
                        return Err((
                            StatusCode::SERVICE_UNAVAILABLE,
                            Json(json!({"error": "TMDB API key not configured"})),
                        )
                            .into_response());
                    }
                },
                _ => {
                    return Err((
                        StatusCode::SERVICE_UNAVAILABLE,
                        Json(json!({"error": "TMDB API key not configured"})),
                    )
                        .into_response());
                }
            }
        }
    };

    Ok(TmdbClient::new(api_key))
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/importlist", get(list_all).post(create))
        .route(
            "/api/v1/importlist/{id}",
            axum::routing::put(update).delete(delete),
        )
        .route("/api/v1/importlist/{id}/sync", post(sync_one))
        .route("/api/v1/importlist/sync", post(sync_all))
}
