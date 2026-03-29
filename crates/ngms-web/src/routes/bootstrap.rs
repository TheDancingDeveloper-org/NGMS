use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::AppState;

// ── Helper: extract bootstrap config values ──────────────────────────────────

struct BootstrapContext {
    url: String,
    token: String,
    server_id: uuid::Uuid,
}

async fn bootstrap_context(state: &AppState) -> Result<BootstrapContext, (StatusCode, Json<serde_json::Value>)> {
    let config = state.config.load();

    let url = match config.bootstrap.url.as_ref() {
        Some(url) if config.bootstrap.enabled => url.clone(),
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "bootstrap not configured"})),
            ));
        }
    };

    let token = match config.bootstrap.token.as_ref() {
        Some(t) => t.clone(),
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "bootstrap token not configured"})),
            ));
        }
    };

    let server_id = sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT value FROM app_config WHERE key = 'server_id'",
    )
    .fetch_optional(state.db.pool())
    .await
    .ok()
    .flatten()
    .and_then(|v| v.as_str().and_then(|s| uuid::Uuid::parse_str(s).ok()));

    let server_id = match server_id {
        Some(id) => id,
        None => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "server_id not found"})),
            ));
        }
    };

    Ok(BootstrapContext { url, token, server_id })
}

// ── Register server name ─────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegisterNameRequest {
    server_name: Option<String>,
}

async fn register_name(
    State(state): State<Arc<AppState>>,
    _: crate::middleware::RequireApiKey,
    Json(body): Json<RegisterNameRequest>,
) -> impl IntoResponse {
    let ctx = match bootstrap_context(&state).await {
        Ok(ctx) => ctx,
        Err(e) => return e.into_response(),
    };

    let config = state.config.load();
    let server_name = body
        .server_name
        .unwrap_or_else(|| config.general.instance_name.clone());

    let client = reqwest::Client::new();
    let res = match client
        .post(format!("{}/api/v1/servers/register-name", ctx.url))
        .bearer_auth(&ctx.token)
        .json(&json!({
            "serverId": ctx.server_id,
            "serverName": server_name,
        }))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(error = %e, "failed to call bootstrap register-name");
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": "failed to reach bootstrap server"})),
            )
                .into_response();
        }
    };

    let status = res.status();
    let body_json: serde_json::Value = res.json().await.unwrap_or_default();

    if status.is_success() {
        // Mark as registered in app_config
        let _ = sqlx::query(
            "INSERT INTO app_config (key, value) VALUES ('bootstrap_name_registered', '\"true\"')
             ON CONFLICT (key) DO UPDATE SET value = '\"true\"'",
        )
        .execute(state.db.pool())
        .await;
    }

    (
        StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        Json(body_json),
    )
        .into_response()
}

// ── Recover server name ──────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecoverNameRequest {
    server_name: String,
    recovery_phrase: String,
}

async fn recover_name(
    State(state): State<Arc<AppState>>,
    _: crate::middleware::RequireApiKey,
    Json(body): Json<RecoverNameRequest>,
) -> impl IntoResponse {
    let ctx = match bootstrap_context(&state).await {
        Ok(ctx) => ctx,
        Err(e) => return e.into_response(),
    };

    let client = reqwest::Client::new();
    let res = match client
        .post(format!("{}/api/v1/servers/recover-name", ctx.url))
        .bearer_auth(&ctx.token)
        .json(&json!({
            "serverName": body.server_name,
            "recoveryPhrase": body.recovery_phrase,
            "newServerId": ctx.server_id,
        }))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(error = %e, "failed to call bootstrap recover-name");
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": "failed to reach bootstrap server"})),
            )
                .into_response();
        }
    };

    let status = res.status();
    let body_json: serde_json::Value = res.json().await.unwrap_or_default();

    if status.is_success() {
        // Mark as registered in app_config
        let _ = sqlx::query(
            "INSERT INTO app_config (key, value) VALUES ('bootstrap_name_registered', '\"true\"')
             ON CONFLICT (key) DO UPDATE SET value = '\"true\"'",
        )
        .execute(state.db.pool())
        .await;
    }

    (
        StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        Json(body_json),
    )
        .into_response()
}

// ── Bootstrap registration status ────────────────────────────────────────────

async fn bootstrap_status(
    State(state): State<Arc<AppState>>,
    _: crate::middleware::RequireApiKey,
) -> impl IntoResponse {
    let config = state.config.load();
    let enabled = config.bootstrap.enabled && config.bootstrap.url.is_some();

    let name_registered = sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT value FROM app_config WHERE key = 'bootstrap_name_registered'",
    )
    .fetch_optional(state.db.pool())
    .await
    .ok()
    .flatten()
    .and_then(|v| v.as_str().map(|s| s == "true"))
    .unwrap_or(false);

    Json(json!({
        "enabled": enabled,
        "nameRegistered": name_registered,
        "serverName": config.general.instance_name,
    }))
}

// ── Check name availability ───────────────────────────────────────────────────

async fn check_name(
    State(state): State<Arc<AppState>>,
    _: crate::middleware::RequireApiKey,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> impl IntoResponse {
    let ctx = match bootstrap_context(&state).await {
        Ok(ctx) => ctx,
        Err(e) => return e.into_response(),
    };

    let client = reqwest::Client::new();
    let res = match client
        .get(format!("{}/api/v1/servers/check-name/{}", ctx.url, name))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(error = %e, "failed to call bootstrap check-name");
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": "failed to reach bootstrap server"})),
            )
                .into_response();
        }
    };

    let status = res.status();
    let body_json: serde_json::Value = res.json().await.unwrap_or_default();

    (
        StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        Json(body_json),
    )
        .into_response()
}

// ── Check port forward ───────────────────────────────────────────────────────

async fn check_port(
    State(state): State<Arc<AppState>>,
    _: crate::middleware::RequireApiKey,
) -> impl IntoResponse {
    let ctx = match bootstrap_context(&state).await {
        Ok(ctx) => ctx,
        Err(e) => return e.into_response(),
    };

    let client = reqwest::Client::new();
    let res = match client
        .post(format!("{}/api/v1/servers/check-port", ctx.url))
        .bearer_auth(&ctx.token)
        .json(&json!({
            "serverId": ctx.server_id,
        }))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(error = %e, "failed to call bootstrap check-port");
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": "failed to reach bootstrap server"})),
            )
                .into_response();
        }
    };

    let status = res.status();
    let body_json: serde_json::Value = res.json().await.unwrap_or_default();

    (
        StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        Json(body_json),
    )
        .into_response()
}

// ── Router ───────────────────────────────────────────────────────────────────

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/admin/bootstrap/register-name", post(register_name))
        .route("/api/v1/admin/bootstrap/recover-name", post(recover_name))
        .route("/api/v1/admin/bootstrap/status", get(bootstrap_status))
        .route(
            "/api/v1/admin/bootstrap/check-name/{name}",
            get(check_name),
        )
        .route("/api/v1/admin/bootstrap/check-port", post(check_port))
}
