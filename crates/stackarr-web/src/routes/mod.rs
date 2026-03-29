pub mod activities;
pub mod admin;
pub mod auth;
pub mod backup;
pub mod bootstrap;
pub mod user;

/// Wrap an external image URL through the local image proxy.
pub(crate) fn proxy_image_url(url: &str) -> String {
    format!("/api/v1/images/{url}")
}

/// Extract an image URL from a JSONB images array by cover type (e.g. "poster", "fanart").
/// Returns a proxied URL through `/api/v1/images/` for local caching.
pub(crate) fn extract_image_url(images: &Option<serde_json::Value>, cover_type: &str) -> Option<String> {
    images.as_ref()?.as_array()?.iter().find_map(|img| {
        if img.get("coverType")?.as_str()? == cover_type {
            img.get("remoteUrl")?.as_str().map(|url| proxy_image_url(url))
        } else {
            None
        }
    })
}
/// Resolve quality JSONB `{"quality": 18, ...}` to a named version `{"quality": "Bluray-2160p", ...}`.
pub(crate) fn resolve_quality(q: &serde_json::Value) -> serde_json::Value {
    if let Some(obj) = q.as_object() {
        if let Some(num) = obj.get("quality").and_then(|v| v.as_i64()) {
            let name = stackarr_quality::quality_name(num as i32);
            let mut resolved = obj.clone();
            resolved.insert("quality".to_string(), serde_json::Value::String(name.to_string()));
            return serde_json::Value::Object(resolved);
        }
    }
    q.clone()
}

/// Resolve quality in a `MediaFile`, returning a new copy with named quality.
pub(crate) fn resolve_media_file_quality(mut file: stackarr_core::models::media::MediaFile) -> stackarr_core::models::media::MediaFile {
    file.quality = resolve_quality(&file.quality);
    file
}

pub mod blocklist;
pub mod calendar;
pub mod discover;
pub mod downloadclients;
pub mod episodes;
pub mod general;
pub mod health;
pub mod images;
pub mod history;
pub mod importlists;
pub mod indexarr;
pub mod indexers;
pub mod logs;
pub mod movies;
pub mod naming;
pub mod notifications;
pub mod plex;
pub mod progress;
pub mod quality;
pub mod queue;
pub mod remote;
pub mod releases;
pub mod rss;
pub mod requests;
pub mod search;
pub mod mediamanagement;
pub mod medialibraryfolders;
pub mod series;
pub mod stremio;
pub mod stream;
pub mod system;
pub mod tags;
pub mod torrent;
pub mod usenet;
pub mod wanted;
pub mod watchlist;
