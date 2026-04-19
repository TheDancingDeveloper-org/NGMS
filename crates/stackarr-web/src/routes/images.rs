use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::routing::get;
use sha2::{Digest, Sha256};

use crate::state::AppState;

/// Allowed upstream domains for image proxying (SSRF prevention).
const ALLOWED_DOMAINS: &[&str] = &["image.tmdb.org", "artworks.thetvdb.com"];

fn cache_dir(state: &AppState) -> PathBuf {
    let config = state.config.load();
    config.general.data_dir.join("image_cache")
}

fn is_allowed_url(url: &str) -> bool {
    if let Ok(parsed) = url::Url::parse(url)
        && let Some(host) = parsed.host_str()
    {
        return ALLOWED_DOMAINS
            .iter()
            .any(|d| host == *d || host.ends_with(&format!(".{d}")));
    }
    false
}

fn url_to_cache_path(cache_dir: &std::path::Path, url: &str) -> (PathBuf, PathBuf) {
    let hash = format!("{:x}", Sha256::digest(url.as_bytes()));
    let data_path = cache_dir.join(&hash);
    let meta_path = cache_dir.join(format!("{hash}.meta"));
    (data_path, meta_path)
}

async fn proxy_image(
    State(state): State<Arc<AppState>>,
    Path(url): Path<String>,
) -> impl IntoResponse {
    if !is_allowed_url(&url) {
        return super::api_error(StatusCode::FORBIDDEN, "domain not allowed");
    }

    let cache = cache_dir(&state);
    let (data_path, meta_path) = url_to_cache_path(&cache, &url);

    // Cache hit — serve from disk. Async probe avoids a sync stat() on the
    // tokio worker; image requests are high-volume on the hot path.
    if tokio::fs::metadata(&data_path).await.is_ok() {
        let content_type = tokio::fs::read_to_string(&meta_path)
            .await
            .unwrap_or_else(|_| "image/jpeg".to_string());

        match tokio::fs::read(&data_path).await {
            Ok(bytes) => {
                let mut headers = HeaderMap::new();
                if let Ok(ct) = HeaderValue::from_str(&content_type) {
                    headers.insert("content-type", ct);
                }
                headers.insert(
                    "cache-control",
                    HeaderValue::from_static("public, max-age=604800"),
                );
                headers.insert("x-cache", HeaderValue::from_static("HIT"));
                return (StatusCode::OK, headers, bytes).into_response();
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to read cached image, re-fetching");
                // Fall through to fetch
            }
        }
    }

    // Cache miss — fetch upstream
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .unwrap_or_default();

    let resp = match client.get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, url = %url, "image proxy fetch failed");
            return super::api_error(StatusCode::BAD_GATEWAY, "upstream fetch failed");
        }
    };

    if !resp.status().is_success() {
        return super::api_error(StatusCode::BAD_GATEWAY, "upstream returned error");
    }

    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("image/jpeg")
        .to_string();

    let bytes = match resp.bytes().await {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(error = %e, "failed to read upstream response body");
            return super::api_error(StatusCode::BAD_GATEWAY, "failed to read upstream");
        }
    };

    // Write to cache (best-effort, don't fail the request)
    if let Err(e) = tokio::fs::create_dir_all(&cache).await {
        tracing::warn!(error = %e, "failed to create image cache dir");
    } else {
        let _ = tokio::fs::write(&data_path, &bytes).await;
        let _ = tokio::fs::write(&meta_path, &content_type).await;
    }

    let mut headers = HeaderMap::new();
    if let Ok(ct) = HeaderValue::from_str(&content_type) {
        headers.insert("content-type", ct);
    }
    headers.insert(
        "cache-control",
        HeaderValue::from_static("public, max-age=604800"),
    );
    headers.insert("x-cache", HeaderValue::from_static("MISS"));
    (StatusCode::OK, headers, bytes.to_vec()).into_response()
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/api/v1/images/{*url}", get(proxy_image))
}
