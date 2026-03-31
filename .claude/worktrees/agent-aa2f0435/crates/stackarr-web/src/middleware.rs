use std::sync::Arc;

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::json;

use crate::AppState;

/// Extractor that validates the API key from:
/// 1. `X-Api-Key` header (Sonarr/Radarr compatible)
/// 2. `Authorization: Bearer <key>` header
/// 3. `?apikey=<key>` query parameter
pub struct RequireApiKey;

#[derive(Deserialize)]
struct ApiKeyQuery {
    apikey: Option<String>,
}

impl RequireApiKey {
    fn extract_key(headers: &HeaderMap, query: &str) -> Option<String> {
        // 1. X-Api-Key header
        if let Some(val) = headers.get("x-api-key") {
            if let Ok(s) = val.to_str() {
                if !s.is_empty() {
                    return Some(s.to_string());
                }
            }
        }

        // 2. Authorization: Bearer <key>
        if let Some(val) = headers.get("authorization") {
            if let Ok(s) = val.to_str() {
                if let Some(token) = s.strip_prefix("Bearer ") {
                    let token = token.trim();
                    if !token.is_empty() {
                        return Some(token.to_string());
                    }
                }
            }
        }

        // 3. ?apikey= query parameter
        if let Ok(params) = serde_urlencoded::from_str::<ApiKeyQuery>(query) {
            if let Some(key) = params.apikey {
                if !key.is_empty() {
                    return Some(key);
                }
            }
        }

        None
    }
}

impl FromRequestParts<Arc<AppState>> for RequireApiKey {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let pool = state.db.pool();

        // Load the stored API key from DB
        let stored_key: Option<String> = sqlx::query_scalar::<_, serde_json::Value>(
            "SELECT value FROM app_config WHERE key = 'api_key'",
        )
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .and_then(|v| v.as_str().map(String::from));

        // If no API key is stored (first boot), allow all requests
        let stored_key = match stored_key {
            Some(k) if !k.is_empty() => k,
            _ => return Ok(Self),
        };

        let query_str = parts.uri.query().unwrap_or("");
        let provided_key = Self::extract_key(&parts.headers, query_str);

        match provided_key {
            Some(key) if key == stored_key => Ok(Self),
            Some(_) => Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "invalid API key"})),
            )
                .into_response()),
            None => Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "API key required — use X-Api-Key header, Authorization: Bearer <key>, or ?apikey= query parameter"})),
            )
                .into_response()),
        }
    }
}

/// Auth type indicating how the request was authenticated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthType {
    /// Authenticated via admin API key (full access)
    ApiKey,
    /// Authenticated via remote client token (streaming access only)
    ClientToken,
}

/// Extractor that accepts either the admin API key OR a valid remote client token.
/// Use this on routes that should be accessible to both admins and remote clients
/// (e.g., streaming, library browsing).
pub struct RequireAuth(pub AuthType);

impl FromRequestParts<Arc<AppState>> for RequireAuth {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let pool = state.db.pool();

        // Load the stored admin API key
        let stored_key: Option<String> = sqlx::query_scalar::<_, serde_json::Value>(
            "SELECT value FROM app_config WHERE key = 'api_key'",
        )
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .and_then(|v| v.as_str().map(String::from));

        // If no API key stored (first boot), allow all
        let stored_key = match stored_key {
            Some(k) if !k.is_empty() => k,
            _ => return Ok(Self(AuthType::ApiKey)),
        };

        let query_str = parts.uri.query().unwrap_or("");
        let provided_key = RequireApiKey::extract_key(&parts.headers, query_str);

        let Some(key) = provided_key else {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "authentication required"})),
            )
                .into_response());
        };

        // Check if it matches the admin API key
        if key == stored_key {
            return Ok(Self(AuthType::ApiKey));
        }

        // Try as a remote client token (UUID format)
        if let Ok(token_uuid) = uuid::Uuid::parse_str(&key) {
            let valid = state
                .db
                .validate_remote_client(token_uuid)
                .await
                .unwrap_or(false);
            if valid {
                // Update last_seen in background (don't block the request)
                let db = state.db.clone();
                tokio::spawn(async move {
                    let _ = db.touch_remote_client(token_uuid).await;
                });
                return Ok(Self(AuthType::ClientToken));
            }
        }

        Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "invalid API key or client token"})),
        )
            .into_response())
    }
}

// ── User-based authentication ────────────────────────────────────────────────

/// How the current request was authenticated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMethod {
    /// Authenticated via a session cookie (web login).
    Session,
    /// Authenticated via a device token (Bearer UUID).
    DeviceToken,
    /// Authenticated via the legacy API key.
    ApiKey,
}

/// The authenticated user extracted from the request.
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub user_id: i64,
    pub username: String,
    pub role: String,
    pub auth_method: AuthMethod,
}

/// Extractor that requires a logged-in user.
///
/// Resolution order:
/// 1. `stackarr_session` cookie -> hash -> validate_session
/// 2. Bearer token (UUID) -> validate_user_device
/// 3. X-Api-Key / ?apikey= / Bearer (non-UUID) -> match legacy API key
/// 4. First-boot bypass if no users exist
/// 5. Return 401
pub struct RequireUser(pub AuthenticatedUser);

fn extract_cookie<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get("cookie")?.to_str().ok().and_then(|cookies| {
        cookies.split(';').find_map(|c| {
            let c = c.trim();
            c.strip_prefix(name)
                .and_then(|rest| rest.strip_prefix('='))
        })
    })
}

impl FromRequestParts<Arc<AppState>> for RequireUser {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        // 1. Try session cookie
        if let Some(session_token) = extract_cookie(&parts.headers, "stackarr_session") {
            if !session_token.is_empty() {
                let token_hash = stackarr_core::auth::hash_token(session_token);
                if let Ok(Some(user)) = state.db.validate_session(&token_hash).await {
                    // Touch session in background
                    let db = state.db.clone();
                    let hash = token_hash.clone();
                    tokio::spawn(async move {
                        let _ = db.touch_session(&hash).await;
                    });
                    return Ok(Self(AuthenticatedUser {
                        user_id: user.id,
                        username: user.username,
                        role: user.role,
                        auth_method: AuthMethod::Session,
                    }));
                }
            }
        }

        // Extract bearer / api key
        let query_str = parts.uri.query().unwrap_or("");
        let provided_key = RequireApiKey::extract_key(&parts.headers, query_str);

        if let Some(ref key) = provided_key {
            // 2. Try as device token (UUID format)
            if let Ok(token_uuid) = uuid::Uuid::parse_str(key) {
                if let Ok(Some(user)) = state.db.validate_user_device(token_uuid).await {
                    let db = state.db.clone();
                    tokio::spawn(async move {
                        let _ = db.touch_user_device(token_uuid).await;
                    });
                    return Ok(Self(AuthenticatedUser {
                        user_id: user.id,
                        username: user.username,
                        role: user.role,
                        auth_method: AuthMethod::DeviceToken,
                    }));
                }

                // Also try legacy remote_clients for backward compat
                if let Ok(true) = state.db.validate_remote_client(token_uuid).await {
                    let db = state.db.clone();
                    tokio::spawn(async move {
                        let _ = db.touch_remote_client(token_uuid).await;
                    });
                    // Legacy client token — treat as a basic user
                    return Ok(Self(AuthenticatedUser {
                        user_id: 0,
                        username: "client".to_string(),
                        role: "user".to_string(),
                        auth_method: AuthMethod::DeviceToken,
                    }));
                }
            }

            // 3. Try as legacy API key
            let stored_key: Option<String> = sqlx::query_scalar::<_, serde_json::Value>(
                "SELECT value FROM app_config WHERE key = 'api_key'",
            )
            .fetch_optional(state.db.pool())
            .await
            .ok()
            .flatten()
            .and_then(|v| v.as_str().map(String::from));

            if let Some(ref stored) = stored_key {
                if !stored.is_empty() && key == stored {
                    return Ok(Self(AuthenticatedUser {
                        user_id: 0,
                        username: "admin".to_string(),
                        role: "admin".to_string(),
                        auth_method: AuthMethod::ApiKey,
                    }));
                }
            }
        }

        // 4. First-boot bypass: if no users exist, allow unauthenticated access
        if let Ok(0) = state.db.count_users().await {
            // Also check if no API key is stored
            let has_api_key: bool = sqlx::query_scalar::<_, serde_json::Value>(
                "SELECT value FROM app_config WHERE key = 'api_key'",
            )
            .fetch_optional(state.db.pool())
            .await
            .ok()
            .flatten()
            .and_then(|v| v.as_str().map(|s| !s.is_empty()))
            .unwrap_or(false);

            if !has_api_key {
                return Ok(Self(AuthenticatedUser {
                    user_id: 0,
                    username: "admin".to_string(),
                    role: "admin".to_string(),
                    auth_method: AuthMethod::ApiKey,
                }));
            }
        }

        // 5. Unauthorized
        Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "authentication required"})),
        )
            .into_response())
    }
}

/// Extractor that requires an admin user.
/// Returns 403 if the user is not an admin.
pub struct RequireAdmin(pub AuthenticatedUser);

impl FromRequestParts<Arc<AppState>> for RequireAdmin {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let RequireUser(user) = RequireUser::from_request_parts(parts, state).await?;

        if user.role != "admin" {
            return Err((
                StatusCode::FORBIDDEN,
                Json(json!({"error": "admin access required"})),
            )
                .into_response());
        }

        Ok(Self(user))
    }
}

/// Mask a string, showing only the first 4 and last 4 characters.
pub fn mask_secret(s: &str) -> String {
    if s.len() <= 8 {
        "*".repeat(s.len())
    } else {
        format!("{}…{}", &s[..4], &s[s.len() - 4..])
    }
}

/// Redact known sensitive fields in a JSON value (mutates in place).
pub fn redact_sensitive_fields(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, val) in map.iter_mut() {
                let lower = key.to_lowercase();
                if lower.contains("api_key")
                    || lower.contains("apikey")
                    || lower == "auth_token"
                    || lower == "authtoken"
                    || lower.contains("password")
                    || lower.contains("secret")
                    || lower.contains("token")
                        && !lower.contains("token_refresh")
                        && !lower.contains("plex_rating_key")
                {
                    if let serde_json::Value::String(s) = val {
                        if !s.is_empty() {
                            *val = serde_json::Value::String(mask_secret(s));
                        }
                    }
                } else {
                    redact_sensitive_fields(val);
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr.iter_mut() {
                redact_sensitive_fields(item);
            }
        }
        _ => {}
    }
}

// ── Rate limiting ────────────────────────────────────────────────────────────

use std::net::IpAddr;
use std::num::NonZeroU32;

use governor::clock::DefaultClock;
use governor::state::keyed::DashMapStateStore;
use governor::{Quota, RateLimiter};

/// Shared rate limiter keyed by client IP address.
pub type KeyedRateLimiter = RateLimiter<IpAddr, DashMapStateStore<IpAddr>, DefaultClock>;

/// Create a rate limiter that allows `per_second` requests per second per IP.
pub fn create_rate_limiter(per_second: u32) -> Arc<KeyedRateLimiter> {
    let quota = Quota::per_second(NonZeroU32::new(per_second).unwrap_or(NonZeroU32::MIN));
    Arc::new(RateLimiter::keyed(quota))
}

/// Extract the client IP from request headers or connection info.
pub fn client_ip(parts: &Parts) -> IpAddr {
    // Try X-Forwarded-For first (behind reverse proxy)
    if let Some(forwarded) = parts.headers.get("x-forwarded-for") {
        if let Ok(s) = forwarded.to_str() {
            if let Some(first) = s.split(',').next() {
                if let Ok(ip) = first.trim().parse::<IpAddr>() {
                    return ip;
                }
            }
        }
    }
    // Try X-Real-IP
    if let Some(real_ip) = parts.headers.get("x-real-ip") {
        if let Ok(s) = real_ip.to_str() {
            if let Ok(ip) = s.trim().parse::<IpAddr>() {
                return ip;
            }
        }
    }
    // Fallback to loopback
    IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)
}

/// Rate limit extractor — returns 429 if the client exceeds the limit.
pub struct RateLimit;

impl FromRequestParts<Arc<AppState>> for RateLimit {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let ip = client_ip(parts);
        if let Some(ref limiter) = state.rate_limiter {
            match limiter.check_key(&ip) {
                Ok(_) => Ok(Self),
                Err(_) => Err((
                    StatusCode::TOO_MANY_REQUESTS,
                    Json(json!({"error": "rate limit exceeded — try again later"})),
                )
                    .into_response()),
            }
        } else {
            Ok(Self)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_secret_long() {
        assert_eq!(mask_secret("abcdefghijklmnop"), "abcd…mnop");
    }

    #[test]
    fn test_mask_secret_short() {
        assert_eq!(mask_secret("abc"), "***");
    }

    #[test]
    fn test_mask_secret_exact_8() {
        assert_eq!(mask_secret("12345678"), "********");
    }

    #[test]
    fn test_redact_sensitive_fields() {
        let mut val = json!({
            "name": "NZBGeek",
            "api_key": "supersecretkey123456",
            "base_url": "https://example.com",
            "config": {
                "password": "hunter2",
                "host": "localhost"
            }
        });
        redact_sensitive_fields(&mut val);
        assert_eq!(val["name"], "NZBGeek");
        assert_ne!(val["api_key"], "supersecretkey123456");
        assert_eq!(val["base_url"], "https://example.com");
        assert_ne!(val["config"]["password"], "hunter2");
        assert_eq!(val["config"]["host"], "localhost");
    }

    #[test]
    fn test_redact_empty_string_unchanged() {
        let mut val = json!({"api_key": ""});
        redact_sensitive_fields(&mut val);
        assert_eq!(val["api_key"], "");
    }

    #[test]
    fn test_extract_key_x_api_key() {
        let mut headers = HeaderMap::new();
        headers.insert("x-api-key", "test-key-123".parse().unwrap());
        assert_eq!(
            RequireApiKey::extract_key(&headers, ""),
            Some("test-key-123".to_string())
        );
    }

    #[test]
    fn test_extract_key_bearer() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", "Bearer my-token".parse().unwrap());
        assert_eq!(
            RequireApiKey::extract_key(&headers, ""),
            Some("my-token".to_string())
        );
    }

    #[test]
    fn test_extract_key_query() {
        let headers = HeaderMap::new();
        assert_eq!(
            RequireApiKey::extract_key(&headers, "apikey=qp-key&other=val"),
            Some("qp-key".to_string())
        );
    }

    #[test]
    fn test_extract_key_none() {
        let headers = HeaderMap::new();
        assert_eq!(RequireApiKey::extract_key(&headers, ""), None);
    }
}
