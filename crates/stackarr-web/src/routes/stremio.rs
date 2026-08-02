use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use serde_json::json;

use crate::AppState;

// ── Stremio addon protocol types ────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    id: &'static str,
    version: &'static str,
    name: &'static str,
    description: &'static str,
    resources: Vec<&'static str>,
    types: Vec<&'static str>,
    catalogs: Vec<CatalogDescriptor>,
    id_prefixes: Option<Vec<&'static str>>,
    behavior_hints: BehaviorHints,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CatalogDescriptor {
    r#type: &'static str,
    id: &'static str,
    name: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BehaviorHints {
    configurable: bool,
    configuration_required: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CatalogResponse {
    metas: Vec<MetaPreview>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MetaPreview {
    id: String,
    r#type: String,
    name: String,
    poster: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    year: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    genres: Option<Vec<String>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MetaResponse {
    meta: MetaDetail,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MetaDetail {
    id: String,
    r#type: String,
    name: String,
    poster: Option<String>,
    background: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    year: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    genres: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    runtime: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    videos: Vec<VideoEntry>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VideoEntry {
    id: String,
    title: String,
    season: i32,
    episode: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    overview: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    released: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StreamResponse {
    streams: Vec<Stream>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Stream {
    /// Direct URL to the stream
    url: String,
    /// Human-readable title shown in Stremio
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    behavior_hints: Option<StreamBehaviorHints>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StreamBehaviorHints {
    not_web_ready: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    proxy_headers: Option<serde_json::Value>,
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn addon_disabled() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(json!({"error": "stremio addon is not enabled"})),
    )
}

async fn is_addon_enabled(pool: &sqlx::MySqlPool) -> bool {
    sqlx::query_scalar::<_, bool>(
        "SELECT enabled FROM enabled_modules WHERE module = 'stremio_addon'",
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .unwrap_or(false)
}

/// Extract a TMDB image URL from the JSONB images array.
fn image_url(images: &Option<serde_json::Value>, cover_type: &str) -> Option<String> {
    images.as_ref()?.as_array()?.iter().find_map(|img| {
        if img.get("coverType")?.as_str()? == cover_type {
            img.get("remoteUrl")?.as_str().map(String::from)
        } else {
            None
        }
    })
}

/// Build the base URL for stream links from the request context.
/// Users can override this with the `stremio_base_url` key in `app_config`.
async fn base_url(pool: &sqlx::MySqlPool, config: &stackarr_core::config::AppConfig) -> String {
    if let Ok(Some(val)) = sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT value FROM app_config WHERE key = 'stremio_base_url'",
    )
    .fetch_optional(pool)
    .await
        && let Some(url) = val.as_str().filter(|s| !s.is_empty())
    {
        return url.trim_end_matches('/').to_string();
    }
    format!(
        "http://{}:{}",
        config.general.bind_addr, config.general.port
    )
}

fn format_size(bytes: i64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
    } else {
        format!("{:.0} MB", bytes as f64 / 1_048_576.0)
    }
}

fn quality_label(quality: &serde_json::Value) -> String {
    if let Some(num) = quality.get("quality").and_then(|v| v.as_i64()) {
        stackarr_quality::quality_name(num as i32).to_string()
    } else if let Some(s) = quality.get("quality").and_then(|v| v.as_str()) {
        s.to_string()
    } else {
        "Unknown".to_string()
    }
}

// ── Route Handlers ──────────────────────────────────────────────────────────

/// GET /api/v1/stremio/manifest.json
async fn manifest(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    if !is_addon_enabled(state.db.pool()).await {
        return addon_disabled().into_response();
    }

    Json(Manifest {
        id: "com.stackarr.addon",
        version: env!("CARGO_PKG_VERSION"),
        name: "StackArr",
        description: "Stream your StackArr media library in Stremio",
        resources: vec!["catalog", "meta", "stream"],
        types: vec!["movie", "series"],
        catalogs: vec![
            CatalogDescriptor {
                r#type: "movie",
                id: "stackarr-movies",
                name: "StackArr Movies",
            },
            CatalogDescriptor {
                r#type: "series",
                id: "stackarr-series",
                name: "StackArr Series",
            },
        ],
        id_prefixes: Some(vec!["tt"]),
        behavior_hints: BehaviorHints {
            configurable: false,
            configuration_required: false,
        },
    })
    .into_response()
}

/// GET /api/v1/stremio/catalog/{type}/{id}.json
async fn catalog(
    State(state): State<Arc<AppState>>,
    Path((media_type, _catalog_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let pool = state.db.pool();
    if !is_addon_enabled(pool).await {
        return addon_disabled().into_response();
    }

    let metas = match media_type.as_str() {
        "movie" => {
            #[allow(clippy::type_complexity)]
            let rows: Vec<(
                Option<String>,
                String,
                Option<String>,
                Option<i32>,
                Option<sqlx::types::Json<Vec<String>>>,
                Option<i64>,
            )> = sqlx::query_as(
                "SELECT m.imdb_id, m.title, m.overview, m.year, m.genres, m.movie_file_id
                 FROM movies m
                 WHERE m.imdb_id IS NOT NULL AND m.movie_file_id IS NOT NULL
                 ORDER BY m.sort_title",
            )
            .fetch_all(pool)
            .await
            .unwrap_or_default();

            rows.into_iter()
                .filter_map(|(imdb_id, title, overview, year, genres, _)| {
                    let id = imdb_id?;
                    Some(MetaPreview {
                        id,
                        r#type: "movie".to_string(),
                        name: title,
                        poster: None, // Stremio fetches posters from cinemeta
                        description: overview,
                        year: year.map(|y| y.to_string()),
                        genres: genres.map(|value| value.0),
                    })
                })
                .collect()
        }
        "series" => {
            #[allow(clippy::type_complexity)]
            let rows: Vec<(
                Option<String>,
                String,
                Option<String>,
                Option<i32>,
                Option<sqlx::types::Json<Vec<String>>>,
            )> = sqlx::query_as(
                "SELECT s.imdb_id, s.title, s.overview, s.year, s.genres
                 FROM series s
                 WHERE s.imdb_id IS NOT NULL
                 ORDER BY s.sort_title",
            )
            .fetch_all(pool)
            .await
            .unwrap_or_default();

            rows.into_iter()
                .filter_map(|(imdb_id, title, overview, year, genres)| {
                    let id = imdb_id?;
                    Some(MetaPreview {
                        id,
                        r#type: "series".to_string(),
                        name: title,
                        poster: None,
                        description: overview,
                        year: year.map(|y| y.to_string()),
                        genres: genres.map(|value| value.0),
                    })
                })
                .collect()
        }
        _ => Vec::new(),
    };

    Json(CatalogResponse { metas }).into_response()
}

/// GET /api/v1/stremio/meta/{type}/{id}.json
///
/// Returns metadata for a movie or series. For series, includes the episode list
/// so Stremio knows which seasons/episodes are available.
async fn meta(
    State(state): State<Arc<AppState>>,
    Path((media_type, id)): Path<(String, String)>,
) -> impl IntoResponse {
    let pool = state.db.pool();
    if !is_addon_enabled(pool).await {
        return addon_disabled().into_response();
    }

    // Strip .json suffix if present (Stremio sends "tt1234567.json")
    let imdb_id = id.trim_end_matches(".json");

    match media_type.as_str() {
        "movie" => {
            #[allow(clippy::type_complexity)]
            let row: Option<(
                String,
                Option<String>,
                Option<i32>,
                Option<sqlx::types::Json<Vec<String>>>,
                Option<serde_json::Value>,
                Option<i32>,
            )> = sqlx::query_as(
                "SELECT m.title, m.overview, m.year, m.genres, m.images, CAST(NULL AS SIGNED)
                     FROM movies m WHERE m.imdb_id = ?",
            )
            .bind(imdb_id)
            .fetch_optional(pool)
            .await
            .unwrap_or(None);

            match row {
                Some((title, overview, year, genres, images, _)) => Json(MetaResponse {
                    meta: MetaDetail {
                        id: imdb_id.to_string(),
                        r#type: "movie".to_string(),
                        name: title,
                        poster: image_url(&images, "poster"),
                        background: image_url(&images, "fanart"),
                        description: overview,
                        year: year.map(|y| y.to_string()),
                        genres: genres.map(|value| value.0),
                        runtime: None,
                        videos: Vec::new(),
                    },
                })
                .into_response(),
                None => {
                    (StatusCode::NOT_FOUND, Json(json!({"error": "not found"}))).into_response()
                }
            }
        }
        "series" => {
            #[allow(clippy::type_complexity)]
            let series_row: Option<(
                i64,
                String,
                Option<String>,
                Option<i32>,
                Option<sqlx::types::Json<Vec<String>>>,
                Option<serde_json::Value>,
                Option<i32>,
            )> = sqlx::query_as(
                "SELECT s.id, s.title, s.overview, s.year, s.genres, s.images, s.runtime
                     FROM series s WHERE s.imdb_id = ?",
            )
            .bind(imdb_id)
            .fetch_optional(pool)
            .await
            .unwrap_or(None);

            match series_row {
                Some((series_id, title, overview, year, genres, images, runtime)) => {
                    // Fetch episodes that have files
                    #[allow(clippy::type_complexity)]
                    let episodes: Vec<(
                        i32,
                        i32,
                        Option<String>,
                        Option<String>,
                        Option<chrono::NaiveDate>,
                    )> = sqlx::query_as(
                        "SELECT e.season_number, e.episode_number, e.title, e.overview, e.air_date
                             FROM episodes e
                             WHERE e.series_id = ? AND e.episode_file_id IS NOT NULL
                             ORDER BY e.season_number, e.episode_number",
                    )
                    .bind(series_id)
                    .fetch_all(pool)
                    .await
                    .unwrap_or_default();

                    let videos: Vec<VideoEntry> = episodes
                        .into_iter()
                        .map(
                            |(season, episode, ep_title, ep_overview, air_date)| VideoEntry {
                                id: format!("{imdb_id}:{season}:{episode}"),
                                title: ep_title
                                    .unwrap_or_else(|| format!("S{season:02}E{episode:02}")),
                                season,
                                episode,
                                overview: ep_overview,
                                released: air_date.map(|d| format!("{d}T00:00:00.000Z")),
                            },
                        )
                        .collect();

                    Json(MetaResponse {
                        meta: MetaDetail {
                            id: imdb_id.to_string(),
                            r#type: "series".to_string(),
                            name: title,
                            poster: image_url(&images, "poster"),
                            background: image_url(&images, "fanart"),
                            description: overview,
                            year: year.map(|y| y.to_string()),
                            genres: genres.map(|value| value.0),
                            runtime: runtime.map(|r| format!("{r} min")),
                            videos,
                        },
                    })
                    .into_response()
                }
                None => {
                    (StatusCode::NOT_FOUND, Json(json!({"error": "not found"}))).into_response()
                }
            }
        }
        _ => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "unknown type"})),
        )
            .into_response(),
    }
}

/// GET /api/v1/stremio/stream/{type}/{id}.json
///
/// Returns available streams for a movie or episode.
/// Movie: id = "tt1234567"
/// Series episode: id = "tt1234567:1:3" (imdb_id:season:episode)
async fn stream(
    State(state): State<Arc<AppState>>,
    Path((media_type, id)): Path<(String, String)>,
) -> impl IntoResponse {
    let pool = state.db.pool();
    if !is_addon_enabled(pool).await {
        return addon_disabled().into_response();
    }

    let raw_id = id.trim_end_matches(".json");
    let config = state.config.load();
    let host = base_url(pool, &config).await;

    let streams = match media_type.as_str() {
        "movie" => resolve_movie_streams(pool, raw_id, &host).await,
        "series" => resolve_series_streams(pool, raw_id, &host).await,
        _ => Vec::new(),
    };

    Json(StreamResponse { streams }).into_response()
}

/// Resolve streams for a movie by IMDB ID.
async fn resolve_movie_streams(pool: &sqlx::MySqlPool, imdb_id: &str, host: &str) -> Vec<Stream> {
    let row: Option<(i64, i64, String, serde_json::Value)> = sqlx::query_as(
        "SELECT mf.id, mf.size, mf.relative_path, mf.quality
         FROM movies m
         JOIN media_files mf ON m.movie_file_id = mf.id
         WHERE m.imdb_id = ? AND m.movie_file_id IS NOT NULL",
    )
    .bind(imdb_id)
    .fetch_optional(pool)
    .await
    .unwrap_or(None);

    match row {
        Some((media_file_id, size, rel_path, quality)) => {
            build_stream_entries(media_file_id, size, &rel_path, &quality, host)
        }
        None => Vec::new(),
    }
}

/// Resolve streams for a series episode by "imdb_id:season:episode".
async fn resolve_series_streams(pool: &sqlx::MySqlPool, raw_id: &str, host: &str) -> Vec<Stream> {
    let parts: Vec<&str> = raw_id.splitn(3, ':').collect();
    if parts.len() != 3 {
        return Vec::new();
    }
    let imdb_id = parts[0];
    let season: i32 = match parts[1].parse() {
        Ok(n) => n,
        Err(_) => return Vec::new(),
    };
    let episode: i32 = match parts[2].parse() {
        Ok(n) => n,
        Err(_) => return Vec::new(),
    };

    let row: Option<(i64, i64, String, serde_json::Value)> = sqlx::query_as(
        "SELECT mf.id, mf.size, mf.relative_path, mf.quality
         FROM series s
         JOIN episodes e ON e.series_id = s.id
         JOIN episode_files ef ON ef.episode_id = e.id
         JOIN media_files mf ON ef.media_file_id = mf.id
         WHERE s.imdb_id = ?
           AND e.season_number = ?
           AND e.episode_number = ?
           AND e.episode_file_id IS NOT NULL
         LIMIT 1",
    )
    .bind(imdb_id)
    .bind(season)
    .bind(episode)
    .fetch_optional(pool)
    .await
    .unwrap_or(None);

    match row {
        Some((media_file_id, size, rel_path, quality)) => {
            build_stream_entries(media_file_id, size, &rel_path, &quality, host)
        }
        None => Vec::new(),
    }
}

/// Build Stremio stream entries for a resolved media file.
/// Returns a direct-play stream and (if streaming module is enabled) an HLS option.
fn build_stream_entries(
    media_file_id: i64,
    size: i64,
    rel_path: &str,
    quality: &serde_json::Value,
    host: &str,
) -> Vec<Stream> {
    let quality_str = quality_label(quality);
    let size_str = format_size(size);
    let filename = std::path::Path::new(rel_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(rel_path);

    let mut streams = Vec::new();

    // Direct play (always available)
    streams.push(Stream {
        url: format!("{host}/api/v1/stream/{media_file_id}/direct"),
        title: format!("StackArr Direct Play\n{quality_str} | {size_str}\n{filename}"),
        behavior_hints: Some(StreamBehaviorHints {
            not_web_ready: true,
            proxy_headers: None,
        }),
    });

    streams
}

// ── Router ──────────────────────────────────────────────────────────────────

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/stremio/manifest.json", get(manifest))
        .route("/api/v1/stremio/catalog/{type}/{id_json}", get(catalog))
        .route("/api/v1/stremio/meta/{type}/{id_json}", get(meta))
        .route("/api/v1/stremio/stream/{type}/{id_json}", get(stream))
}
