use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::AppState;
use crate::middleware::{RequireAdmin, redact_sensitive_fields};

// ── Types ────────────────────────────────────────────────────────────────────

#[derive(Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NotificationProviderResponse {
    pub id: i32,
    pub name: String,
    pub provider_type: String,
    pub config: serde_json::Value,
    pub on_grab: bool,
    pub on_import: bool,
    pub on_upgrade: bool,
    pub on_health_issue: bool,
    pub on_failure: bool,
    pub enabled: bool,
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateProviderRequest {
    pub name: String,
    pub provider_type: String,
    pub config: serde_json::Value,
    pub on_grab: Option<bool>,
    pub on_import: Option<bool>,
    pub on_upgrade: Option<bool>,
    pub on_health_issue: Option<bool>,
    pub on_failure: Option<bool>,
    pub enabled: Option<bool>,
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProviderRequest {
    pub name: Option<String>,
    pub provider_type: Option<String>,
    pub config: Option<serde_json::Value>,
    pub on_grab: Option<bool>,
    pub on_import: Option<bool>,
    pub on_upgrade: Option<bool>,
    pub on_health_issue: Option<bool>,
    pub on_failure: Option<bool>,
    pub enabled: Option<bool>,
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TestProviderRequest {
    pub provider_type: String,
    pub config: serde_json::Value,
}

const VALID_PROVIDER_TYPES: &[&str] = &["webhook", "discord", "telegram", "slack", "email"];

// ── Handlers ─────────────────────────────────────────────────────────────────

/// List all notification providers.
#[utoipa::path(
    get,
    path = "/api/v1/notification/provider",
    tag = "Notifications",
    operation_id = "listNotificationProviders",
    responses(
        (status = 200, description = "List of notification providers", body = Vec<NotificationProviderResponse>),
        (status = 500, description = "Internal server error"),
    ),
    security(("ApiKeyAuth" = []), ("BearerAuth" = [])),
)]
pub async fn list_providers(
    _admin: RequireAdmin,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    match sqlx::query_as::<_, NotificationProviderResponse>(
        "SELECT id, name, provider_type, config, on_grab, on_import, on_upgrade, \
                on_health_issue, on_failure, enabled \
         FROM notification_providers ORDER BY id",
    )
    .fetch_all(pool)
    .await
    {
        Ok(providers) => {
            let mut value = serde_json::to_value(&providers).unwrap_or_default();
            redact_sensitive_fields(&mut value);
            Json(value).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to list notification providers");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

/// Get a single notification provider by ID.
#[utoipa::path(
    get,
    path = "/api/v1/notification/provider/{id}",
    tag = "Notifications",
    operation_id = "getNotificationProvider",
    params(("id" = i32, Path, description = "Provider ID")),
    responses(
        (status = 200, description = "Notification provider details", body = NotificationProviderResponse),
        (status = 404, description = "Provider not found"),
    ),
    security(("ApiKeyAuth" = []), ("BearerAuth" = [])),
)]
pub async fn get_provider(
    _admin: RequireAdmin,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    match sqlx::query_as::<_, NotificationProviderResponse>(
        "SELECT id, name, provider_type, config, on_grab, on_import, on_upgrade, \
                on_health_issue, on_failure, enabled \
         FROM notification_providers WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(provider)) => {
            let mut value = serde_json::to_value(&provider).unwrap_or_default();
            redact_sensitive_fields(&mut value);
            Json(value).into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "notification provider not found"})),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to get notification provider");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

/// Create a new notification provider.
#[utoipa::path(
    post,
    path = "/api/v1/notification/provider",
    tag = "Notifications",
    operation_id = "createNotificationProvider",
    request_body = CreateProviderRequest,
    responses(
        (status = 201, description = "Provider created", body = NotificationProviderResponse),
        (status = 400, description = "Validation error"),
        (status = 500, description = "Internal server error"),
    ),
    security(("ApiKeyAuth" = []), ("BearerAuth" = [])),
)]
pub async fn create_provider(
    _admin: RequireAdmin,
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateProviderRequest>,
) -> impl IntoResponse {
    if body.name.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "name cannot be empty"})),
        )
            .into_response();
    }

    if !VALID_PROVIDER_TYPES.contains(&body.provider_type.as_str()) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("invalid provider_type '{}', must be one of: {}", body.provider_type, VALID_PROVIDER_TYPES.join(", "))})),
        )
            .into_response();
    }

    if let Some(err) = validate_provider_config(&body.provider_type, &body.config) {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": err}))).into_response();
    }

    let pool = state.db.pool();
    let on_grab = body.on_grab.unwrap_or(true);
    let on_import = body.on_import.unwrap_or(true);
    let on_upgrade = body.on_upgrade.unwrap_or(true);
    let on_health_issue = body.on_health_issue.unwrap_or(true);
    let on_failure = body.on_failure.unwrap_or(true);
    let enabled = body.enabled.unwrap_or(true);

    match sqlx::query_as::<_, NotificationProviderResponse>(
        "INSERT INTO notification_providers \
            (name, provider_type, config, on_grab, on_import, on_upgrade, on_health_issue, on_failure, enabled) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) \
         RETURNING id, name, provider_type, config, on_grab, on_import, on_upgrade, on_health_issue, on_failure, enabled",
    )
    .bind(body.name.trim())
    .bind(&body.provider_type)
    .bind(&body.config)
    .bind(on_grab)
    .bind(on_import)
    .bind(on_upgrade)
    .bind(on_health_issue)
    .bind(on_failure)
    .bind(enabled)
    .fetch_one(pool)
    .await
    {
        Ok(provider) => {
            let mut value = serde_json::to_value(&provider).unwrap_or_default();
            redact_sensitive_fields(&mut value);
            (StatusCode::CREATED, Json(value)).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to create notification provider");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

/// Update an existing notification provider.
#[utoipa::path(
    put,
    path = "/api/v1/notification/provider/{id}",
    tag = "Notifications",
    operation_id = "updateNotificationProvider",
    params(("id" = i32, Path, description = "Provider ID")),
    request_body = UpdateProviderRequest,
    responses(
        (status = 200, description = "Provider updated", body = NotificationProviderResponse),
        (status = 400, description = "Validation error"),
        (status = 404, description = "Provider not found"),
    ),
    security(("ApiKeyAuth" = []), ("BearerAuth" = [])),
)]
pub async fn update_provider(
    _admin: RequireAdmin,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
    Json(body): Json<UpdateProviderRequest>,
) -> impl IntoResponse {
    if let Some(ref pt) = body.provider_type
        && !VALID_PROVIDER_TYPES.contains(&pt.as_str())
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("invalid provider_type '{}', must be one of: {}", pt, VALID_PROVIDER_TYPES.join(", "))})),
        )
            .into_response();
    }

    let pool = state.db.pool();

    match sqlx::query_as::<_, NotificationProviderResponse>(
        "UPDATE notification_providers SET \
            name = COALESCE($1, name), \
            provider_type = COALESCE($2, provider_type), \
            config = COALESCE($3, config), \
            on_grab = COALESCE($4, on_grab), \
            on_import = COALESCE($5, on_import), \
            on_upgrade = COALESCE($6, on_upgrade), \
            on_health_issue = COALESCE($7, on_health_issue), \
            on_failure = COALESCE($8, on_failure), \
            enabled = COALESCE($9, enabled) \
         WHERE id = $10 \
         RETURNING id, name, provider_type, config, on_grab, on_import, on_upgrade, on_health_issue, on_failure, enabled",
    )
    .bind(body.name.as_deref().map(str::trim))
    .bind(&body.provider_type)
    .bind(&body.config)
    .bind(body.on_grab)
    .bind(body.on_import)
    .bind(body.on_upgrade)
    .bind(body.on_health_issue)
    .bind(body.on_failure)
    .bind(body.enabled)
    .bind(id)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(provider)) => {
            let mut value = serde_json::to_value(&provider).unwrap_or_default();
            redact_sensitive_fields(&mut value);
            Json(value).into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "notification provider not found"})),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to update notification provider");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

/// Delete a notification provider.
#[utoipa::path(
    delete,
    path = "/api/v1/notification/provider/{id}",
    tag = "Notifications",
    operation_id = "deleteNotificationProvider",
    params(("id" = i32, Path, description = "Provider ID")),
    responses(
        (status = 204, description = "Provider deleted"),
        (status = 404, description = "Provider not found"),
    ),
    security(("ApiKeyAuth" = []), ("BearerAuth" = [])),
)]
pub async fn delete_provider(
    _admin: RequireAdmin,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    match sqlx::query("DELETE FROM notification_providers WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
    {
        Ok(result) => {
            if result.rows_affected() == 0 {
                (
                    StatusCode::NOT_FOUND,
                    Json(json!({"error": "notification provider not found"})),
                )
                    .into_response()
            } else {
                StatusCode::NO_CONTENT.into_response()
            }
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to delete notification provider");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

/// Test a saved notification provider by sending a test notification.
#[utoipa::path(
    post,
    path = "/api/v1/notification/provider/{id}/test",
    tag = "Notifications",
    operation_id = "testSavedNotificationProvider",
    params(("id" = i32, Path, description = "Provider ID")),
    responses(
        (status = 200, description = "Test result"),
        (status = 404, description = "Provider not found"),
    ),
    security(("ApiKeyAuth" = []), ("BearerAuth" = [])),
)]
pub async fn test_saved_provider(
    _admin: RequireAdmin,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    let row: Option<(String, serde_json::Value)> =
        match sqlx::query_as("SELECT provider_type, config FROM notification_providers WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                return Json(json!({
                    "success": false,
                    "message": format!("database error: {e}")
                }))
                .into_response();
            }
        };

    let (provider_type, config) = match row {
        Some(r) => r,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"success": false, "message": "notification provider not found"})),
            )
                .into_response();
        }
    };

    test_provider_impl(&provider_type, &config).await.into_response()
}

/// Test a notification provider configuration without saving it.
#[utoipa::path(
    post,
    path = "/api/v1/notification/provider/test",
    tag = "Notifications",
    operation_id = "testNotificationProviderConfig",
    request_body = TestProviderRequest,
    responses(
        (status = 200, description = "Test result"),
        (status = 400, description = "Validation error"),
    ),
    security(("ApiKeyAuth" = []), ("BearerAuth" = [])),
)]
pub async fn test_provider_config(
    _admin: RequireAdmin,
    Json(body): Json<TestProviderRequest>,
) -> impl IntoResponse {
    if !VALID_PROVIDER_TYPES.contains(&body.provider_type.as_str()) {
        return Json(json!({
            "success": false,
            "message": format!("invalid provider_type '{}', must be one of: {}", body.provider_type, VALID_PROVIDER_TYPES.join(", "))
        }))
        .into_response();
    }

    if let Some(err) = validate_provider_config(&body.provider_type, &body.config) {
        return Json(json!({"success": false, "message": err})).into_response();
    }

    test_provider_impl(&body.provider_type, &body.config).await.into_response()
}

// ── Helpers ──────────────────────────────────────────────────────────────────

async fn test_provider_impl(provider_type: &str, config: &serde_json::Value) -> Json<serde_json::Value> {
    let provider = match stackarr_notify::build_provider_from_config(provider_type, config) {
        Some(p) => p,
        None => {
            return Json(json!({
                "success": false,
                "message": format!("failed to build {provider_type} provider from config — check required fields")
            }));
        }
    };

    match tokio::time::timeout(std::time::Duration::from_secs(15), provider.test()).await {
        Ok(Ok(())) => Json(json!({"success": true, "message": "test notification sent successfully"})),
        Ok(Err(e)) => Json(json!({"success": false, "message": format!("{e}")})),
        Err(_) => Json(json!({"success": false, "message": "test timed out after 15 seconds"})),
    }
}

fn validate_provider_config(provider_type: &str, config: &serde_json::Value) -> Option<String> {
    match provider_type {
        "webhook" => {
            if config.get("url").and_then(|v| v.as_str()).is_none() {
                return Some("webhook provider requires 'url' in config".to_string());
            }
        }
        "discord" => {
            if config.get("webhook_url").and_then(|v| v.as_str()).is_none()
                && config.get("url").and_then(|v| v.as_str()).is_none()
            {
                return Some(
                    "discord provider requires 'webhook_url' or 'url' in config".to_string(),
                );
            }
        }
        "telegram" => {
            if config.get("bot_token").and_then(|v| v.as_str()).is_none() {
                return Some("telegram provider requires 'bot_token' in config".to_string());
            }
            if config.get("chat_id").and_then(|v| v.as_str()).is_none() {
                return Some("telegram provider requires 'chat_id' in config".to_string());
            }
        }
        "slack" => {
            if config.get("webhook_url").and_then(|v| v.as_str()).is_none()
                && config.get("url").and_then(|v| v.as_str()).is_none()
            {
                return Some(
                    "slack provider requires 'webhook_url' or 'url' in config".to_string(),
                );
            }
        }
        "email" => {
            for field in &["smtp_url", "from", "to"] {
                if config.get(*field).and_then(|v| v.as_str()).is_none() {
                    return Some(format!("email provider requires '{field}' in config"));
                }
            }
        }
        _ => {}
    }
    None
}

// ── Router ───────────────────────────────────────────────────────────────────

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/v1/notification/provider",
            get(list_providers).post(create_provider),
        )
        .route(
            "/api/v1/notification/provider/{id}",
            axum::routing::put(update_provider).delete(delete_provider),
        )
        .route(
            "/api/v1/notification/provider/{id}/test",
            post(test_saved_provider),
        )
        .route(
            "/api/v1/notification/provider/test",
            post(test_provider_config),
        )
}
