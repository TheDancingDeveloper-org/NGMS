use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;

use uuid::Uuid;

use crate::AppState;
use crate::middleware::{RateLimit, RequireUser};

// ── Login ────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoginRequest {
    username: String,
    password: String,
    device_name: Option<String>, // if provided, also create a user_device and return deviceToken
}

async fn login_handler(
    State(state): State<Arc<AppState>>,
    _: RateLimit,
    headers: axum::http::HeaderMap,
    Json(body): Json<LoginRequest>,
) -> impl IntoResponse {
    let user_agent = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let ip = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.trim().to_string())
        });

    let device_name = body.device_name.clone();
    login_inner(state, body, user_agent, ip, device_name).await
}

async fn login_inner(
    state: Arc<AppState>,
    body: LoginRequest,
    user_agent: Option<String>,
    ip_address: Option<String>,
    device_name: Option<String>,
) -> impl IntoResponse {
    let username = body.username.trim();
    if username.is_empty() || body.password.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "username and password are required"})),
        )
            .into_response();
    }

    // Look up user
    let user = match state.db.get_user_by_username(username).await {
        Ok(Some(u)) => u,
        Ok(None) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "invalid username or password"})),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!(error = %e, "login: failed to query user");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response();
        }
    };

    if !user.enabled {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "account is disabled"})),
        )
            .into_response();
    }

    // Verify password (blocking operation)
    let hash = user.password_hash.clone();
    let password = body.password.clone();
    let valid = match tokio::task::spawn_blocking(move || {
        stackarr_core::auth::verify_password(&password, &hash)
    })
    .await
    {
        Ok(Ok(v)) => v,
        _ => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "invalid username or password"})),
            )
                .into_response();
        }
    };

    if !valid {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "invalid username or password"})),
        )
            .into_response();
    }

    // Create session
    let token = stackarr_core::auth::generate_session_token();
    let token_hash = stackarr_core::auth::hash_token(&token);
    let expires_at = Utc::now() + chrono::Duration::days(30);

    if let Err(e) = state
        .db
        .create_session(
            user.id,
            &token_hash,
            user_agent.as_deref(),
            ip_address.as_deref(),
            expires_at,
        )
        .await
    {
        tracing::error!(error = %e, "login: failed to create session");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "internal server error"})),
        )
            .into_response();
    }

    // Create device token if device_name was provided (mobile/Tauri clients)
    let device_token = if let Some(ref name) = device_name {
        let dt = Uuid::new_v4();
        match state
            .db
            .create_user_device(user.id, dt, Some(name.as_str()), None)
            .await
        {
            Ok(_) => Some(dt),
            Err(e) => {
                tracing::warn!(error = %e, "login: failed to create user device");
                None
            }
        }
    } else {
        None
    };

    // Build Set-Cookie header
    let cookie = format!(
        "stackarr_session={token}; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age={}",
        30 * 24 * 60 * 60
    );

    let mut response_json = json!({
        "user": {
            "id": user.id,
            "username": user.username,
            "displayName": user.display_name,
            "role": user.role,
            "avatarUrl": user.avatar_url,
        },
        "token": token,
    });

    if let Some(dt) = device_token {
        response_json["deviceToken"] = json!(dt);
    }

    let mut response = Json(response_json).into_response();

    response
        .headers_mut()
        .insert("set-cookie", cookie.parse().expect("valid cookie header"));

    response
}

// ── Logout ───────────────────────────────────────────────────────────────────

async fn logout(
    State(state): State<Arc<AppState>>,
    RequireUser(user): RequireUser,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    // Extract session token from cookie and delete it
    if let Some(session_token) = extract_session_cookie(&headers) {
        let token_hash = stackarr_core::auth::hash_token(session_token);
        let _ = state.db.delete_session(&token_hash).await;
    }

    // Clear cookie
    let cookie = "stackarr_session=; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=0";

    let mut response = Json(json!({"ok": true})).into_response();
    response
        .headers_mut()
        .insert("set-cookie", cookie.parse().expect("valid cookie header"));

    let _ = user; // suppress unused warning
    response
}

fn extract_session_cookie(headers: &axum::http::HeaderMap) -> Option<&str> {
    headers.get("cookie")?.to_str().ok().and_then(|cookies| {
        cookies.split(';').find_map(|c| {
            let c = c.trim();
            c.strip_prefix("stackarr_session=")
        })
    })
}

// ── Register (with invite code) ──────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegisterRequest {
    username: String,
    password: String,
    display_name: Option<String>,
    invite_code: String,
}

async fn register(
    State(state): State<Arc<AppState>>,
    _: RateLimit,
    headers: axum::http::HeaderMap,
    Json(body): Json<RegisterRequest>,
) -> impl IntoResponse {
    let username = body.username.trim().to_lowercase();
    if username.is_empty() || body.password.len() < 6 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "username required and password must be at least 6 characters"})),
        )
            .into_response();
    }

    // Validate invite
    let invite = match state.db.validate_invite(&body.invite_code).await {
        Ok(Some(inv)) => inv,
        Ok(None) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "invalid or expired invite code"})),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!(error = %e, "register: failed to validate invite");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response();
        }
    };

    // Hash password
    let password = body.password.clone();
    let password_hash =
        match tokio::task::spawn_blocking(move || stackarr_core::auth::hash_password(&password))
            .await
        {
            Ok(Ok(h)) => h,
            Ok(Err(e)) => {
                tracing::error!(error = %e, "register: failed to hash password");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "internal server error"})),
                )
                    .into_response();
            }
            Err(e) => {
                tracing::error!(error = %e, "register: spawn_blocking failed");
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

    // Create user
    let user = match state
        .db
        .create_user(&username, &display_name, &password_hash, &invite.role)
        .await
    {
        Ok(u) => u,
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("unique") || msg.contains("duplicate") {
                return (
                    StatusCode::CONFLICT,
                    Json(json!({"error": "username already taken"})),
                )
                    .into_response();
            }
            tracing::error!(error = %e, "register: failed to create user");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response();
        }
    };

    // Claim invite
    let _ = state.db.claim_invite(&body.invite_code, user.id).await;

    // Create session
    let token = stackarr_core::auth::generate_session_token();
    let token_hash = stackarr_core::auth::hash_token(&token);
    let expires_at = Utc::now() + chrono::Duration::days(30);

    let user_agent = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let ip = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.trim().to_string())
        });

    let _ = state
        .db
        .create_session(
            user.id,
            &token_hash,
            user_agent.as_deref(),
            ip.as_deref(),
            expires_at,
        )
        .await;

    let cookie = format!(
        "stackarr_session={token}; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age={}",
        30 * 24 * 60 * 60
    );

    let mut response = Json(json!({
        "user": {
            "id": user.id,
            "username": user.username,
            "displayName": user.display_name,
            "role": user.role,
            "avatarUrl": user.avatar_url,
        },
        "token": token,
    }))
    .into_response();

    response
        .headers_mut()
        .insert("set-cookie", cookie.parse().expect("valid cookie header"));

    (StatusCode::CREATED, response).into_response()
}

// ── Me ───────────────────────────────────────────────────────────────────────

async fn me(
    State(state): State<Arc<AppState>>,
    RequireUser(auth_user): RequireUser,
) -> impl IntoResponse {
    // If user_id == 0, this is a legacy API key or first-boot bypass
    if auth_user.user_id == 0 {
        return Json(json!({
            "id": 0,
            "username": auth_user.username,
            "displayName": auth_user.username,
            "role": auth_user.role,
            "avatarUrl": serde_json::Value::Null,
            "authMethod": format!("{:?}", auth_user.auth_method).to_lowercase(),
        }))
        .into_response();
    }

    match state.db.get_user_by_id(auth_user.user_id).await {
        Ok(Some(user)) => Json(json!({
            "id": user.id,
            "username": user.username,
            "displayName": user.display_name,
            "role": user.role,
            "avatarUrl": user.avatar_url,
            "authMethod": format!("{:?}", auth_user.auth_method).to_lowercase(),
        }))
        .into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "user not found"})),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "me: failed to fetch user");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

// ── Auth Status (public) ─────────────────────────────────────────────────────

async fn auth_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let setup_required = matches!(state.db.count_users().await, Ok(0));
    Json(json!({
        "setupRequired": setup_required,
        "registrationEnabled": true,
    }))
}

// ── First-Boot Setup ─────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetupRequest {
    username: String,
    password: String,
    display_name: Option<String>,
    server_name: Option<String>,
}

async fn setup(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<SetupRequest>,
) -> impl IntoResponse {
    // Only allow setup when no users exist
    match state.db.count_users().await {
        Ok(0) => {}
        Ok(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "setup already completed"})),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!(error = %e, "setup: failed to count users");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response();
        }
    }

    let username = body.username.trim().to_lowercase();
    if username.is_empty() || body.password.len() < 6 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "username required and password must be at least 6 characters"})),
        )
            .into_response();
    }

    // Hash password
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

    // Create admin user
    let user = match state
        .db
        .create_user(&username, &display_name, &password_hash, "admin")
        .await
    {
        Ok(u) => u,
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("unique") || msg.contains("duplicate") {
                return (
                    StatusCode::CONFLICT,
                    Json(json!({"error": "username already taken"})),
                )
                    .into_response();
            }
            tracing::error!(error = %e, "setup: failed to create admin user");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response();
        }
    };

    tracing::info!(username = %user.username, "first-boot admin user created via setup");

    // Generate and store API key
    let api_key = stackarr_core::auth::generate_session_token();
    if let Err(e) = sqlx::query(
        "INSERT INTO app_config (key, value) VALUES ('api_key', $1) \
         ON CONFLICT (key) DO UPDATE SET value = $1",
    )
    .bind(serde_json::json!(&api_key))
    .execute(state.db.pool())
    .await
    {
        tracing::warn!(error = %e, "setup: failed to store API key");
    }
    state.set_cached_api_key(Some(api_key.clone()));

    // Store server name if provided
    if let Some(ref name) = body.server_name
        && !name.trim().is_empty()
    {
        let _ = sqlx::query(
            "INSERT INTO app_config (key, value) VALUES ('instance_name', $1) \
                 ON CONFLICT (key) DO UPDATE SET value = $1",
        )
        .bind(serde_json::json!(name.trim()))
        .execute(state.db.pool())
        .await;
    }

    // Create session
    let token = stackarr_core::auth::generate_session_token();
    let token_hash = stackarr_core::auth::hash_token(&token);
    let expires_at = Utc::now() + chrono::Duration::days(30);

    let user_agent = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let ip = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.trim().to_string())
        });

    let _ = state
        .db
        .create_session(
            user.id,
            &token_hash,
            user_agent.as_deref(),
            ip.as_deref(),
            expires_at,
        )
        .await;

    let cookie = format!(
        "stackarr_session={token}; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age={}",
        30 * 24 * 60 * 60
    );

    let mut response = Json(json!({
        "user": {
            "id": user.id,
            "username": user.username,
            "displayName": user.display_name,
            "role": user.role,
            "avatarUrl": user.avatar_url,
        },
        "token": token,
    }))
    .into_response();

    response
        .headers_mut()
        .insert("set-cookie", cookie.parse().expect("valid cookie header"));

    (StatusCode::CREATED, response).into_response()
}

// ── Router ───────────────────────────────────────────────────────────────────

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/auth/status", get(auth_status))
        .route("/api/v1/auth/setup", post(setup))
        .route("/api/v1/auth/login", post(login_handler))
        .route("/api/v1/auth/logout", post(logout))
        .route("/api/v1/auth/register", post(register))
        .route("/api/v1/auth/me", get(me))
}
