// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

pub mod activities;
pub mod admin;
pub mod auth;
pub mod backup;
pub mod bootstrap;
pub mod user;

/// Return a structured JSON error response instead of a raw string.
pub(crate) fn api_error(
    status: axum::http::StatusCode,
    err: impl std::fmt::Display,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    (
        status,
        axum::Json(serde_json::json!({"error": err.to_string()})),
    )
        .into_response()
}

/// Wrap an external image URL through the local image proxy.
pub(crate) fn proxy_image_url(url: &str) -> String {
    format!("/api/v1/images/{url}")
}

/// Extract an image URL from a JSONB images array by cover type (e.g. "poster", "fanart").
/// Returns a proxied URL through `/api/v1/images/` for local caching.
pub(crate) fn extract_image_url(
    images: &Option<serde_json::Value>,
    cover_type: &str,
) -> Option<String> {
    images.as_ref()?.as_array()?.iter().find_map(|img| {
        if img.get("coverType")?.as_str()? == cover_type {
            img.get("remoteUrl")?.as_str().map(proxy_image_url)
        } else {
            None
        }
    })
}
/// Resolve quality JSONB `{"quality": 18, ...}` to a named version `{"quality": "Bluray-2160p", ...}`.
pub(crate) fn resolve_quality(q: &serde_json::Value) -> serde_json::Value {
    if let Some(obj) = q.as_object()
        && let Some(num) = obj.get("quality").and_then(|v| v.as_i64())
    {
        let name = stackarr_quality::quality_name(num as i32);
        let mut resolved = obj.clone();
        resolved.insert(
            "quality".to_string(),
            serde_json::Value::String(name.to_string()),
        );
        return serde_json::Value::Object(resolved);
    }
    q.clone()
}

/// Resolve quality in a `MediaFile`, returning a new copy with named quality.
pub(crate) fn resolve_media_file_quality(
    mut file: stackarr_core::models::media::MediaFile,
) -> stackarr_core::models::media::MediaFile {
    file.quality = resolve_quality(&file.quality);
    file
}

pub mod blocklist;
pub mod calendar;
pub mod dav;
pub mod discover;
pub mod downloadclients;
pub mod episodes;
pub mod filebrowser;
pub mod general;
pub mod health;
pub mod history;
pub mod images;
pub mod import_candidates;
pub mod importlists;
pub mod indexarr;
pub mod indexers;
pub mod logs;
pub mod manual_import;
pub mod medialibraryfolders;
pub mod mediamanagement;
pub mod movies;
pub mod naming;
pub mod notification_providers;
pub mod notifications;
pub mod plex;
pub mod progress;
pub mod quality;
pub mod queue;
pub mod releases;
pub mod remote;
pub mod requests;
pub mod rss;
pub mod scheduler;
pub mod search;
pub mod series;
pub mod stream;
pub mod stremio;
pub mod system;
pub mod tags;
pub mod torrent;
pub mod usenet;
pub mod wanted;
pub mod watchlist;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── proxy_image_url ─────────────────────────────────────────────

    #[test]
    fn test_proxy_image_url_basic() {
        assert_eq!(
            proxy_image_url("https://image.tmdb.org/poster.jpg"),
            "/api/v1/images/https://image.tmdb.org/poster.jpg"
        );
    }

    #[test]
    fn test_proxy_image_url_empty() {
        assert_eq!(proxy_image_url(""), "/api/v1/images/");
    }

    // ── extract_image_url ───────────────────────────────────────────

    #[test]
    fn test_extract_image_url_poster() {
        let images = Some(json!([
            {"coverType": "poster", "remoteUrl": "https://img.example.com/poster.jpg"},
            {"coverType": "fanart", "remoteUrl": "https://img.example.com/fanart.jpg"}
        ]));
        let result = extract_image_url(&images, "poster");
        assert_eq!(
            result,
            Some("/api/v1/images/https://img.example.com/poster.jpg".to_string())
        );
    }

    #[test]
    fn test_extract_image_url_fanart() {
        let images = Some(json!([
            {"coverType": "poster", "remoteUrl": "https://img.example.com/poster.jpg"},
            {"coverType": "fanart", "remoteUrl": "https://img.example.com/fanart.jpg"}
        ]));
        let result = extract_image_url(&images, "fanart");
        assert_eq!(
            result,
            Some("/api/v1/images/https://img.example.com/fanart.jpg".to_string())
        );
    }

    #[test]
    fn test_extract_image_url_not_found() {
        let images = Some(json!([
            {"coverType": "poster", "remoteUrl": "https://img.example.com/poster.jpg"}
        ]));
        assert_eq!(extract_image_url(&images, "banner"), None);
    }

    #[test]
    fn test_extract_image_url_none_images() {
        assert_eq!(extract_image_url(&None, "poster"), None);
    }

    #[test]
    fn test_extract_image_url_empty_array() {
        let images = Some(json!([]));
        assert_eq!(extract_image_url(&images, "poster"), None);
    }

    #[test]
    fn test_extract_image_url_missing_remote_url() {
        let images = Some(json!([{"coverType": "poster"}]));
        assert_eq!(extract_image_url(&images, "poster"), None);
    }

    #[test]
    fn test_extract_image_url_missing_cover_type() {
        let images = Some(json!([{"remoteUrl": "https://img.example.com/poster.jpg"}]));
        assert_eq!(extract_image_url(&images, "poster"), None);
    }

    #[test]
    fn test_extract_image_url_non_array_json() {
        let images =
            Some(json!({"coverType": "poster", "remoteUrl": "https://img.example.com/poster.jpg"}));
        assert_eq!(extract_image_url(&images, "poster"), None);
    }

    // ── resolve_quality ─────────────────────────────────────────────

    #[test]
    fn test_resolve_quality_numeric_to_named() {
        let q = json!({"quality": 7});
        let resolved = resolve_quality(&q);
        // Quality 7 should resolve to a string name
        assert!(resolved["quality"].is_string());
    }

    #[test]
    fn test_resolve_quality_preserves_other_fields() {
        let q = json!({"quality": 7, "revision": {"version": 1}});
        let resolved = resolve_quality(&q);
        assert!(resolved["quality"].is_string());
        assert_eq!(resolved["revision"]["version"], 1);
    }

    #[test]
    fn test_resolve_quality_already_string_passthrough() {
        let q = json!({"quality": "Bluray-1080p"});
        let resolved = resolve_quality(&q);
        // When quality is already a string (not i64), it returns the clone
        assert_eq!(resolved["quality"], "Bluray-1080p");
    }

    #[test]
    fn test_resolve_quality_non_object_passthrough() {
        let q = json!("just a string");
        let resolved = resolve_quality(&q);
        assert_eq!(resolved, json!("just a string"));
    }

    #[test]
    fn test_resolve_quality_null_passthrough() {
        let q = json!(null);
        let resolved = resolve_quality(&q);
        assert_eq!(resolved, json!(null));
    }

    #[test]
    fn test_resolve_quality_zero_is_unknown() {
        let q = json!({"quality": 0});
        let resolved = resolve_quality(&q);
        assert_eq!(resolved["quality"], "Unknown");
    }
}
