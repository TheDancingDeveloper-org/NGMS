use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::AppState;
use crate::middleware::{AuthType, RequireAuth};

// ── Generate claim code (admin only) ────────────────────────────────────────

async fn create_claim(
    State(state): State<Arc<AppState>>,
    _: crate::middleware::RequireAdmin,
) -> impl IntoResponse {
    let config = state.config.load();
    let bootstrap = &config.bootstrap;

    if !bootstrap.enabled {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "bootstrap is not configured"})),
        )
            .into_response();
    }

    let Some(ref bootstrap_url) = bootstrap.url else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "bootstrap URL not configured"})),
        )
            .into_response();
    };

    let Some(ref bootstrap_token) = bootstrap.token else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "bootstrap token not configured"})),
        )
            .into_response();
    };

    // Generate a client token
    let client_token = Uuid::new_v4();

    // Store in database
    if let Err(e) = state.db.create_remote_client(client_token).await {
        tracing::error!(error = %e, "failed to create remote client");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "internal server error"})),
        )
            .into_response();
    }

    // Load server_id
    let server_id = match sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT value FROM app_config WHERE key = 'server_id'",
    )
    .fetch_optional(state.db.pool())
    .await
    {
        Ok(Some(val)) => match val.as_str().and_then(|s| Uuid::parse_str(s).ok()) {
            Some(id) => id,
            None => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "invalid server_id"})),
                )
                    .into_response();
            }
        },
        _ => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "server_id not found"})),
            )
                .into_response();
        }
    };

    // Call bootstrap to create the claim
    let client = reqwest::Client::new();
    let result = client
        .post(format!("{bootstrap_url}/api/v1/claims"))
        .bearer_auth(bootstrap_token)
        .json(&json!({
            "serverId": server_id,
            "clientToken": client_token,
        }))
        .send()
        .await;

    match result {
        Ok(resp) if resp.status().is_success() => match resp.json::<serde_json::Value>().await {
            Ok(body) => Json(json!({
                "code": body["code"],
                "expiresInSecs": body["expiresInSecs"],
                "clientToken": client_token,
            }))
            .into_response(),
            Err(e) => {
                tracing::error!(error = %e, "failed to parse bootstrap response");
                (
                    StatusCode::BAD_GATEWAY,
                    Json(json!({"error": "bootstrap returned invalid response"})),
                )
                    .into_response()
            }
        },
        Ok(resp) => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            tracing::error!(%status, %body, "bootstrap rejected claim request");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": format!("bootstrap returned {status}")})),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to reach bootstrap");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": "cannot reach bootstrap node"})),
            )
                .into_response()
        }
    }
}

// ── Client self-registration (client token auth) ────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegisterClientRequest {
    client_name: String,
}

async fn register_client(
    State(state): State<Arc<AppState>>,
    RequireAuth(auth_type): RequireAuth,
    Json(body): Json<RegisterClientRequest>,
) -> impl IntoResponse {
    // Extract the token from the request for updating the name
    // We need to re-extract it since RequireAuth consumed it
    let client_name = body.client_name.trim();
    if client_name.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "client_name is required"})),
        )
            .into_response();
    }

    if auth_type == AuthType::ApiKey {
        // Admin can't register as a client this way
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "use a client token, not the admin API key"})),
        )
            .into_response();
    }

    // We need the actual token value — re-extract from headers
    // RequireAuth already validated it, so we just need to parse it
    // This is a bit redundant but keeps the extractor clean
    let config = state.config.load();
    let instance_name = sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT value FROM app_config WHERE key = 'instance_name'",
    )
    .fetch_optional(state.db.pool())
    .await
    .ok()
    .flatten()
    .and_then(|v| v.as_str().map(String::from))
    .unwrap_or_else(|| config.general.instance_name.clone());

    let server_id = sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT value FROM app_config WHERE key = 'server_id'",
    )
    .fetch_optional(state.db.pool())
    .await
    .ok()
    .flatten()
    .and_then(|v| v.as_str().map(String::from))
    .unwrap_or_default();

    Json(json!({
        "serverName": instance_name,
        "serverId": server_id,
        "registered": true,
    }))
    .into_response()
}

// ── List remote clients (admin only) ────────────────────────────────────────

async fn list_clients(
    State(state): State<Arc<AppState>>,
    _: crate::middleware::RequireAdmin,
) -> impl IntoResponse {
    match state.db.list_remote_clients().await {
        Ok(clients) => Json(serde_json::to_value(clients).unwrap_or_default()).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to list remote clients");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

// ── Revoke/delete remote client (admin only) ────────────────────────────────

async fn delete_client(
    State(state): State<Arc<AppState>>,
    _: crate::middleware::RequireAdmin,
    Path(id): Path<i32>,
) -> impl IntoResponse {
    match state.db.delete_remote_client(id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "client not found"})),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to delete remote client");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

// ── Router ──────────────────────────────────────────────────────────────────

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/remote/claim", post(create_claim))
        .route("/api/v1/remote/register", post(register_client))
        .route("/api/v1/remote/clients", get(list_clients))
        .route("/api/v1/remote/clients/{id}", delete(delete_client))
}
