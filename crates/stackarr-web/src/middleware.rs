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
