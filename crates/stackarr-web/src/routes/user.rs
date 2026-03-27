use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{delete, get, put};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::middleware::RequireUser;
use crate::AppState;

// ── Update profile ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateProfileRequest {
    display_name: Option<String>,
    avatar_url: Option<String>,
    current_password: Option<String>,
    new_password: Option<String>,
}

async fn update_profile(
    State(state): State<Arc<AppState>>,
    RequireUser(auth_user): RequireUser,
    Json(body): Json<UpdateProfileRequest>,
) -> impl IntoResponse {
    if auth_user.user_id == 0 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "legacy API key users cannot update profile"})),
        )
            .into_response();
    }

    let existing = match state.db.get_user_by_id(auth_user.user_id).await {
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

    // Handle password change
    if let Some(ref new_password) = body.new_password {
        if new_password.len() < 6 {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "password must be at least 6 characters"})),
            )
                .into_response();
        }

        // Require current password
        let current = match &body.current_password {
            Some(cp) => cp.clone(),
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "current password is required to change password"})),
                )
                    .into_response();
            }
        };

        let hash = existing.password_hash.clone();
        let valid = match tokio::task::spawn_blocking(move || {
            stackarr_core::auth::verify_password(&current, &hash)
        })
        .await
        {
            Ok(Ok(v)) => v,
            _ => false,
        };

        if !valid {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "current password is incorrect"})),
            )
                .into_response();
        }

        let pw = new_password.clone();
        let new_hash = match tokio::task::spawn_blocking(move || {
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

        let _ = state
            .db
            .update_user_password(auth_user.user_id, &new_hash)
            .await;
    }

    let display_name = body
        .display_name
        .as_deref()
        .unwrap_or(&existing.display_name);
    let avatar_url = body.avatar_url.as_deref().or(existing.avatar_url.as_deref());

    match state
        .db
        .update_user(
            auth_user.user_id,
            display_name,
            &existing.role,
            existing.enabled,
            avatar_url,
        )
        .await
    {
        Ok(Some(user)) => Json(json!({
            "id": user.id,
            "username": user.username,
            "displayName": user.display_name,
            "role": user.role,
            "avatarUrl": user.avatar_url,
        }))
        .into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "user not found"})),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to update profile");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

// ── Devices ──────────────────────────────────────────────────────────────────

async fn list_devices(
    State(state): State<Arc<AppState>>,
    RequireUser(auth_user): RequireUser,
) -> impl IntoResponse {
    if auth_user.user_id == 0 {
        return Json(serde_json::Value::Array(vec![])).into_response();
    }

    match state.db.list_user_devices(auth_user.user_id).await {
        Ok(devices) => Json(serde_json::to_value(devices).unwrap_or_default()).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to list devices");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

async fn delete_device(
    State(state): State<Arc<AppState>>,
    RequireUser(auth_user): RequireUser,
    Path(id): Path<i32>,
) -> impl IntoResponse {
    // Verify the device belongs to the user
    if auth_user.user_id == 0 {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "legacy API key users cannot manage devices"})),
        )
            .into_response();
    }

    match state.db.list_user_devices(auth_user.user_id).await {
        Ok(devices) => {
            if !devices.iter().any(|d| d.id == id) {
                return (
                    StatusCode::NOT_FOUND,
                    Json(json!({"error": "device not found"})),
                )
                    .into_response();
            }
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to verify device ownership");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response();
        }
    }

    match state.db.delete_user_device(id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "device not found"})),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to delete device");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

// ── Sessions ─────────────────────────────────────────────────────────────────

async fn list_sessions(
    State(state): State<Arc<AppState>>,
    RequireUser(auth_user): RequireUser,
) -> impl IntoResponse {
    if auth_user.user_id == 0 {
        return Json(serde_json::Value::Array(vec![])).into_response();
    }

    match state.db.list_sessions(auth_user.user_id).await {
        Ok(sessions) => {
            let sessions_json: Vec<serde_json::Value> = sessions
                .iter()
                .map(|s| {
                    json!({
                        "id": s.id,
                        "userAgent": s.user_agent,
                        "ipAddress": s.ip_address,
                        "createdAt": s.created_at,
                        "expiresAt": s.expires_at,
                        "lastActive": s.last_active,
                    })
                })
                .collect();
            Json(serde_json::Value::Array(sessions_json)).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to list sessions");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

async fn delete_all_sessions(
    State(state): State<Arc<AppState>>,
    RequireUser(auth_user): RequireUser,
) -> impl IntoResponse {
    if auth_user.user_id == 0 {
        return StatusCode::NO_CONTENT.into_response();
    }

    match state.db.delete_all_sessions(auth_user.user_id).await {
        Ok(count) => Json(json!({"deleted": count})).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to delete sessions");
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
        .route("/api/v1/user/profile", put(update_profile))
        .route("/api/v1/user/devices", get(list_devices))
        .route("/api/v1/user/devices/{id}", delete(delete_device))
        .route(
            "/api/v1/user/sessions",
            get(list_sessions).delete(delete_all_sessions),
        )
}
