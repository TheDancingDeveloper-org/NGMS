pub mod backup;

/// Extract an image URL from a JSONB images array by cover type (e.g. "poster", "fanart").
pub(crate) fn extract_image_url(images: &Option<serde_json::Value>, cover_type: &str) -> Option<String> {
    images.as_ref()?.as_array()?.iter().find_map(|img| {
        if img.get("coverType")?.as_str()? == cover_type {
            img.get("remoteUrl")?.as_str().map(String::from)
        } else {
            None
        }
    })
}
pub mod blocklist;
pub mod calendar;
pub mod discover;
pub mod downloadclients;
pub mod episodes;
pub mod health;
pub mod history;
pub mod importlists;
pub mod indexarr;
pub mod indexers;
pub mod logs;
pub mod movies;
pub mod naming;
pub mod plex;
pub mod quality;
pub mod queue;
pub mod remote;
pub mod releases;
pub mod search;
pub mod medialibraryfolders;
pub mod series;
pub mod stream;
pub mod system;
pub mod tags;
pub mod torrent;
pub mod usenet;
pub mod wanted;
