// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{delete, get, put};
use axum::{Json, Router};
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;

use crate::AppState;
use crate::middleware::RequireAdmin;

// ── User management ──────────────────────────────────────────────────────────

async fn list_users(
    State(state): State<Arc<AppState>>,
    RequireAdmin(_admin): RequireAdmin,
) -> impl IntoResponse {
    match state.db.list_users().await {
        Ok(users) => {
            let users_json: Vec<serde_json::Value> = users
                .iter()
                .map(|u| {
                    json!({
                        "id": u.id,
                        "username": u.username,
                        "displayName": u.display_name,
                        "role": u.role,
                        "avatarUrl": u.avatar_url,
                        "enabled": u.enabled,
                        "createdAt": u.created_at,
                        "updatedAt": u.updated_at,
                    })
                })
                .collect();
            Json(serde_json::Value::Array(users_json)).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to list users");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateUserRequest {
    username: String,
    password: String,
    display_name: Option<String>,
    role: Option<String>,
}

async fn create_user(
    State(state): State<Arc<AppState>>,
    RequireAdmin(_admin): RequireAdmin,
    Json(body): Json<CreateUserRequest>,
) -> impl IntoResponse {
    let username = body.username.trim().to_lowercase();
    if username.is_empty() || body.password.len() < 6 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "username required and password must be at least 6 characters"})),
        )
            .into_response();
    }

    let role = body.role.as_deref().unwrap_or("user");
    if role != "admin" && role != "user" {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "role must be 'admin' or 'user'"})),
        )
            .into_response();
    }

    let password = body.password.clone();
    let password_hash =
        match tokio::task::spawn_blocking(move || stackarr_core::auth::hash_password(&password))
            .await
        {
            Ok(Ok(h)) => h,
            _ => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "internal server error"})),
                )
                    .into_response();
            }
        };

    let display_name = body
        .display_name
        .as_deref()
        .unwrap_or(&username)
        .to_string();

    match state
        .db
        .create_user(&username, &display_name, &password_hash, role)
        .await
    {
        Ok(user) => (
            StatusCode::CREATED,
            Json(json!({
                "id": user.id,
                "username": user.username,
                "displayName": user.display_name,
                "role": user.role,
                "enabled": user.enabled,
                "createdAt": user.created_at,
            })),
        )
            .into_response(),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("unique") || msg.contains("duplicate") {
                return (
                    StatusCode::CONFLICT,
                    Json(json!({"error": "username already taken"})),
                )
                    .into_response();
            }
            tracing::error!(error = %e, "failed to create user");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateUserRequest {
    display_name: Option<String>,
    role: Option<String>,
    enabled: Option<bool>,
    avatar_url: Option<String>,
    password: Option<String>,
}

async fn update_user(
    State(state): State<Arc<AppState>>,
    RequireAdmin(_admin): RequireAdmin,
    Path(id): Path<i64>,
    Json(body): Json<UpdateUserRequest>,
) -> impl IntoResponse {
    // Get current user data
    let existing = match state.db.get_user_by_id(id).await {
        Ok(Some(u)) => u,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "user not found"})),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to get user");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response();
        }
    };

    let display_name = body
        .display_name
        .as_deref()
        .unwrap_or(&existing.display_name);
    let role = body.role.as_deref().unwrap_or(&existing.role);
    let enabled = body.enabled.unwrap_or(existing.enabled);
    let avatar_url = body
        .avatar_url
        .as_deref()
        .or(existing.avatar_url.as_deref());

    // Update password if provided
    if let Some(ref password) = body.password {
        if password.len() < 6 {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "password must be at least 6 characters"})),
            )
                .into_response();
        }
        let pw = password.clone();
        let hash = match tokio::task::spawn_blocking(move || {
            stackarr_core::auth::hash_password(&pw)
        })
        .await
        {
            Ok(Ok(h)) => h,
            _ => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "internal server error"})),
                )
                    .into_response();
            }
        };
        let _ = state.db.update_user_password(id, &hash).await;
    }

    match state
        .db
        .update_user(id, display_name, role, enabled, avatar_url)
        .await
    {
        Ok(Some(user)) => Json(json!({
            "id": user.id,
            "username": user.username,
            "displayName": user.display_name,
            "role": user.role,
            "enabled": user.enabled,
            "avatarUrl": user.avatar_url,
            "createdAt": user.created_at,
            "updatedAt": user.updated_at,
        }))
        .into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "user not found"})),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to update user");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

async fn delete_user(
    State(state): State<Arc<AppState>>,
    RequireAdmin(admin): RequireAdmin,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    // Prevent self-deletion
    if admin.user_id == id {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "cannot delete your own account"})),
        )
            .into_response();
    }

    match state.db.delete_user(id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "user not found"})),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to delete user");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

// ── Invite management ────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateInviteRequest {
    role: Option<String>,
    expires_in_hours: Option<i64>,
}

async fn create_invite(
    State(state): State<Arc<AppState>>,
    RequireAdmin(admin): RequireAdmin,
    Json(body): Json<CreateInviteRequest>,
) -> impl IntoResponse {
    let role = body.role.as_deref().unwrap_or("user");
    if role != "admin" && role != "user" {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "role must be 'admin' or 'user'"})),
        )
            .into_response();
    }

    let code = stackarr_core::auth::generate_invite_code();
    let expires_at = body
        .expires_in_hours
        .map(|h| Utc::now() + chrono::Duration::hours(h));

    let invite = match state
        .db
        .create_invite(&code, admin.user_id, role, expires_at)
        .await
    {
        Ok(inv) => inv,
        Err(e) => {
            tracing::error!(error = %e, "failed to create invite");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response();
        }
    };

    // Register with bootstrap for unified server discovery (best-effort)
    let config = state.config.load();
    if config.bootstrap.enabled
        && let (Some(url), Some(token)) = (
            config.bootstrap.url.as_ref(),
            config.bootstrap.token.as_ref(),
        )
    {
        let server_id = state.db.ensure_server_id().await.ok();
        if let Some(server_id) = server_id {
            let ttl_secs = invite
                .expires_at
                .map(|exp| (exp - Utc::now()).num_seconds().max(0) as u64)
                .unwrap_or(86400); // 24h default for no-expiry invites

            let client = reqwest::Client::new();
            match client
                .post(format!("{url}/api/v1/claims"))
                .bearer_auth(token)
                .json(&json!({
                    "serverId": server_id,
                    "code": invite.code,
                    "claimType": "invite",
                    "inviteCode": invite.code,
                    "ttlSecs": ttl_secs,
                }))
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => {
                    tracing::info!(code = %invite.code, "invite registered with bootstrap");
                }
                Ok(resp) => {
                    tracing::warn!(
                        code = %invite.code,
                        status = %resp.status(),
                        "failed to register invite with bootstrap"
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        code = %invite.code,
                        error = %e,
                        "failed to reach bootstrap for invite registration"
                    );
                }
            }
        }
    }

    (
        StatusCode::CREATED,
        Json(json!({
            "id": invite.id,
            "code": invite.code,
            "role": invite.role,
            "expiresAt": invite.expires_at,
            "createdAt": invite.created_at,
        })),
    )
        .into_response()
}

async fn list_invites(
    State(state): State<Arc<AppState>>,
    RequireAdmin(_admin): RequireAdmin,
) -> impl IntoResponse {
    match state.db.list_invites().await {
        Ok(invites) => Json(serde_json::to_value(invites).unwrap_or_default()).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to list invites");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

async fn delete_invite(
    State(state): State<Arc<AppState>>,
    RequireAdmin(_admin): RequireAdmin,
    Path(id): Path<i32>,
) -> impl IntoResponse {
    match state.db.delete_invite(id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "invite not found"})),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to delete invite");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

// ── Router ───────────────────────────────────────────────────────────────────

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/admin/users", get(list_users).post(create_user))
        .route(
            "/api/v1/admin/users/{id}",
            put(update_user).delete(delete_user),
        )
        .route(
            "/api/v1/admin/invites",
            get(list_invites).post(create_invite),
        )
        .route("/api/v1/admin/invites/{id}", delete(delete_invite))
}
