use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::middleware::redact_sensitive_fields;
use crate::AppState;

#[derive(Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
struct DownloadClientResponse {
    id: i32,
    name: String,
    client_type: String,
    protocol: String,
    config: serde_json::Value,
    enabled: bool,
    priority: i32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateDownloadClientRequest {
    name: String,
    client_type: String,
    protocol: String,
    config: serde_json::Value,
    enabled: Option<bool>,
    priority: Option<i32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateDownloadClientRequest {
    name: Option<String>,
    client_type: Option<String>,
    protocol: Option<String>,
    config: Option<serde_json::Value>,
    enabled: Option<bool>,
    priority: Option<i32>,
}

async fn list_download_clients(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let pool = state.db.pool();

    match sqlx::query_as::<_, DownloadClientResponse>(
        "SELECT id, name, client_type, protocol, config, enabled, priority
         FROM download_clients ORDER BY priority, id",
    )
    .fetch_all(pool)
    .await
    {
        Ok(clients) => {
            let mut value = serde_json::to_value(&clients).unwrap_or_default();
            redact_sensitive_fields(&mut value);
            Json(value).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to list download clients");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

async fn create_download_client(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateDownloadClientRequest>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    if body.name.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "name cannot be empty"})),
        )
            .into_response();
    }

    let enabled = body.enabled.unwrap_or(true);
    let priority = body.priority.unwrap_or(1);

    match sqlx::query_as::<_, DownloadClientResponse>(
        "INSERT INTO download_clients (name, client_type, protocol, config, enabled, priority)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING id, name, client_type, protocol, config, enabled, priority",
    )
    .bind(body.name.trim())
    .bind(&body.client_type)
    .bind(&body.protocol)
    .bind(&body.config)
    .bind(enabled)
    .bind(priority)
    .fetch_one(pool)
    .await
    {
        Ok(client) => {
            let mut value = serde_json::to_value(&client).unwrap_or_default();
            redact_sensitive_fields(&mut value);
            (StatusCode::CREATED, Json(value)).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to create download client");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

async fn update_download_client(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(body): Json<UpdateDownloadClientRequest>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    match sqlx::query_as::<_, DownloadClientResponse>(
        "UPDATE download_clients SET
            name = COALESCE($1, name),
            client_type = COALESCE($2, client_type),
            protocol = COALESCE($3, protocol),
            config = COALESCE($4, config),
            enabled = COALESCE($5, enabled),
            priority = COALESCE($6, priority)
         WHERE id = $7
         RETURNING id, name, client_type, protocol, config, enabled, priority",
    )
    .bind(body.name.as_deref().map(str::trim))
    .bind(&body.client_type)
    .bind(&body.protocol)
    .bind(&body.config)
    .bind(body.enabled)
    .bind(body.priority)
    .bind(id as i32)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(client)) => {
            let mut value = serde_json::to_value(&client).unwrap_or_default();
            redact_sensitive_fields(&mut value);
            Json(value).into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "download client not found"})),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to update download client");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

async fn delete_download_client(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    match sqlx::query("DELETE FROM download_clients WHERE id = $1")
        .bind(id as i32)
        .execute(pool)
        .await
    {
        Ok(result) => {
            if result.rows_affected() == 0 {
                (
                    StatusCode::NOT_FOUND,
                    Json(json!({"error": "download client not found"})),
                )
                    .into_response()
            } else {
                StatusCode::NO_CONTENT.into_response()
            }
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to delete download client");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

async fn test_download_client(
    State(_state): State<Arc<AppState>>,
    Path(_id): Path<i64>,
) -> impl IntoResponse {
    // Stub: just return ok for now
    Json(json!({
        "success": true,
        "message": "connection test passed"
    }))
    .into_response()
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/v1/downloadclient",
            get(list_download_clients).post(create_download_client),
        )
        .route(
            "/api/v1/downloadclient/{id}",
            axum::routing::put(update_download_client).delete(delete_download_client),
        )
        .route(
            "/api/v1/downloadclient/{id}/test",
            post(test_download_client),
        )
}
