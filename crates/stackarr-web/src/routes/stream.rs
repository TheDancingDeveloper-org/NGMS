use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde_json::json;
use uuid::Uuid;

use stackarr_stream::types::TranscodeRequest;

use crate::AppState;

// ── Helpers ──────────────────────────────────────────────────────────────

fn streaming_not_enabled() -> impl IntoResponse {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({"error": "streaming server is not enabled"})),
    )
}

/// Apply path mappings from `app_config` key `path_maps` (JSON array of `[from, to]` pairs).
/// Falls back to the media_library_folders table for prefix remapping.
async fn apply_path_maps(pool: &sqlx::PgPool, path: PathBuf) -> PathBuf {
    // Try app_config path_maps first (explicit overrides)
    if let Ok(Some(maps)) = sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT value FROM app_config WHERE key = 'path_maps'",
    )
    .fetch_optional(pool)
    .await
    {
        if let Some(arr) = maps.as_array() {
            let path_str = path.to_string_lossy();
            for entry in arr {
                if let (Some(from), Some(to)) = (
                    entry.get(0).and_then(|v| v.as_str()),
                    entry.get(1).and_then(|v| v.as_str()),
                ) {
                    if path_str.starts_with(from) {
                        let remapped = format!("{}{}", to, &path_str[from.len()..]);
                        return PathBuf::from(remapped);
                    }
                }
            }
        }
    }

    path
}

/// Resolve the full filesystem path for a media file by joining the
/// parent entity's directory path with the file's relative path.
async fn resolve_media_path(pool: &sqlx::PgPool, media_file_id: i64) -> Result<PathBuf, StatusCode> {
    // Try movie first (simpler join)
    let movie_row: Option<(String, String)> = sqlx::query_as(
        "SELECT m.path, mf.relative_path
         FROM media_files mf
         JOIN movies m ON m.movie_file_id = mf.id
         WHERE mf.id = $1 AND mf.media_type = 'movie'",
    )
    .bind(media_file_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "failed to query movie path");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if let Some((lib_path, rel_path)) = movie_row {
        let raw = PathBuf::from(lib_path).join(rel_path);
        let mapped = apply_path_maps(pool, raw).await;
        if mapped.exists() {
            return Ok(mapped);
        }
        // If mapped path doesn't exist, log and return it anyway (ffprobe will give a better error)
        tracing::warn!(path = %mapped.display(), media_file_id, "resolved movie path does not exist");
        return Ok(mapped);
    }

    // Try series episode
    let series_row: Option<(String, String)> = sqlx::query_as(
        "SELECT s.path, mf.relative_path
         FROM media_files mf
         JOIN episode_files ef ON ef.media_file_id = mf.id
         JOIN episodes e ON ef.episode_id = e.id
         JOIN series s ON e.series_id = s.id
         WHERE mf.id = $1 AND mf.media_type = 'series'
         LIMIT 1",
    )
    .bind(media_file_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "failed to query series path");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if let Some((lib_path, rel_path)) = series_row {
        let raw = PathBuf::from(lib_path).join(rel_path);
        return Ok(apply_path_maps(pool, raw).await);
    }

    Err(StatusCode::NOT_FOUND)
}

// ── Route Handlers ────────────────────────────────────────────────────────

/// GET /api/v1/stream/{media_file_id}/info
/// Returns ffprobe media information (video/audio/subtitle streams).
async fn stream_info(
    State(state): State<Arc<AppState>>,
    Path(media_file_id): Path<i64>,
) -> impl IntoResponse {
    // Check for cached media_info in DB first (works even without streaming enabled)
    let cached: Option<(Option<serde_json::Value>,)> = sqlx::query_as(
        "SELECT media_info FROM media_files WHERE id = $1",
    )
    .bind(media_file_id)
    .fetch_optional(state.db.pool())
    .await
    .unwrap_or(None);

    if let Some((Some(info),)) = &cached {
        // Only use cached data if it has the streaming MediaInfo shape (videoStreams array).
        // Old Sonarr-imported media_info uses a flat format that the frontend can't consume.
        if !info.is_null() && info.get("videoStreams").is_some() {
            return Json(info.clone()).into_response();
        }
    }

    // For probing we need the session manager (ffprobe path)
    let Some(ref mgr) = state.stream_session_manager else {
        return streaming_not_enabled().into_response();
    };

    // Resolve the file and probe it
    let file_path = match resolve_media_path(state.db.pool(), media_file_id).await {
        Ok(p) => p,
        Err(StatusCode::NOT_FOUND) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "media file not found"})),
            )
                .into_response()
        }
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    };

    let config = mgr.config();
    match stackarr_stream::ffprobe::probe(&config.ffprobe_path, &file_path).await {
        Ok(info) => {
            // Cache the result in DB
            if let Ok(info_json) = serde_json::to_value(&info) {
                let _ = sqlx::query("UPDATE media_files SET media_info = $1 WHERE id = $2")
                    .bind(&info_json)
                    .bind(media_file_id)
                    .execute(state.db.pool())
                    .await;
            }
            Json(json!(info)).into_response()
        }
        Err(e) => {
            tracing::error!(media_file_id, error = %e, "ffprobe failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("probe failed: {e}")})),
            )
                .into_response()
        }
    }
}

/// GET /api/v1/stream/{media_file_id}/direct
/// Serve the media file directly with HTTP range request support.
async fn stream_direct(
    State(state): State<Arc<AppState>>,
    Path(media_file_id): Path<i64>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let Some(ref mgr) = state.stream_session_manager else {
        return streaming_not_enabled().into_response();
    };

    let file_path = match resolve_media_path(state.db.pool(), media_file_id).await {
        Ok(p) => p,
        Err(StatusCode::NOT_FOUND) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "media file not found"})),
            )
                .into_response()
        }
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    };

    let range_header = headers
        .get("range")
        .and_then(|v| v.to_str().ok());

    match stackarr_stream::direct::serve_file(&file_path, range_header).await {
        Ok(resp) => {
            // Record direct session (fire and forget)
            let mgr_clone = Arc::clone(mgr);
            tokio::spawn(async move {
                let _ = mgr_clone.create_direct_session(media_file_id).await;
            });

            let status = if resp.status == 206 {
                StatusCode::PARTIAL_CONTENT
            } else {
                StatusCode::OK
            };

            let mut response_headers = HeaderMap::new();
            response_headers.insert(
                "content-type",
                HeaderValue::from_str(&resp.content_type).unwrap_or(HeaderValue::from_static("application/octet-stream")),
            );
            response_headers.insert(
                "content-length",
                HeaderValue::from_str(&resp.content_length.to_string()).unwrap_or(HeaderValue::from_static("0")),
            );
            response_headers.insert("accept-ranges", HeaderValue::from_static("bytes"));

            if let Some(range) = &resp.content_range {
                if let Ok(val) = HeaderValue::from_str(range) {
                    response_headers.insert("content-range", val);
                }
            }

            let body = Body::from_stream(resp.body);
            (status, response_headers, body).into_response()
        }
        Err(stackarr_stream::StreamError::NotFound(_)) => {
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "file not found on disk"})),
            )
                .into_response()
        }
        Err(stackarr_stream::StreamError::InvalidRange(msg)) => {
            (
                StatusCode::RANGE_NOT_SATISFIABLE,
                Json(json!({"error": msg})),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(media_file_id, error = %e, "direct play failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "streaming failed"})),
            )
                .into_response()
        }
    }
}

/// POST /api/v1/stream/{media_file_id}/transcode
/// Start a transcode session and return the HLS playlist URL.
async fn start_transcode(
    State(state): State<Arc<AppState>>,
    Path(media_file_id): Path<i64>,
    Json(request): Json<TranscodeRequest>,
) -> impl IntoResponse {
    let Some(ref mgr) = state.stream_session_manager else {
        return streaming_not_enabled().into_response();
    };

    let file_path = match resolve_media_path(state.db.pool(), media_file_id).await {
        Ok(p) => p,
        Err(StatusCode::NOT_FOUND) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "media file not found"})),
            )
                .into_response()
        }
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    };

    match mgr
        .create_transcode_session(media_file_id, &file_path, &request)
        .await
    {
        Ok(resp) => (StatusCode::CREATED, Json(json!(resp))).into_response(),
        Err(stackarr_stream::StreamError::MaxSessions) => (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error": "max concurrent transcode sessions reached"})),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(media_file_id, error = %e, "failed to start transcode");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("transcode failed: {e}")})),
            )
                .into_response()
        }
    }
}

/// GET /api/v1/stream/{media_file_id}/hls/{session_id}/master.m3u8
/// Serve the HLS master playlist.
async fn hls_playlist(
    State(state): State<Arc<AppState>>,
    Path((media_file_id, session_id)): Path<(i64, Uuid)>,
) -> impl IntoResponse {
    let Some(ref mgr) = state.stream_session_manager else {
        return streaming_not_enabled().into_response();
    };

    if !mgr.validate_session(session_id, media_file_id) {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "session not found"})),
        )
            .into_response();
    }

    mgr.heartbeat(session_id);

    let Some(session_dir) = mgr.get_session_dir(session_id) else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "session directory not found"})),
        )
            .into_response();
    };

    // Wait for the playlist to be created by ffmpeg (software encoding 4K can be slow)
    let playlist_path = session_dir.join("master.m3u8");
    for _ in 0..15 {
        if playlist_path.exists() && tokio::fs::metadata(&playlist_path).await.map(|m| m.len() > 0).unwrap_or(false) {
            break;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    let api_prefix = format!("/api/v1/stream/{media_file_id}/hls/{session_id}");
    match stackarr_stream::hls::read_playlist(&session_dir, &api_prefix).await {
        Ok(playlist) => {
            let mut headers = HeaderMap::new();
            headers.insert(
                "content-type",
                HeaderValue::from_static("application/vnd.apple.mpegurl"),
            );
            (StatusCode::OK, headers, playlist).into_response()
        }
        Err(e) => {
            tracing::error!(session_id = %session_id, error = %e, "failed to read playlist");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "playlist not ready"})),
            )
                .into_response()
        }
    }
}

/// GET /api/v1/stream/{media_file_id}/hls/{session_id}/{segment}
/// Serve a single HLS segment.
async fn hls_segment(
    State(state): State<Arc<AppState>>,
    Path((media_file_id, session_id, segment)): Path<(i64, Uuid, String)>,
) -> impl IntoResponse {
    let Some(ref mgr) = state.stream_session_manager else {
        return streaming_not_enabled().into_response();
    };

    if !mgr.validate_session(session_id, media_file_id) {
        return (StatusCode::NOT_FOUND, "session not found").into_response();
    }

    mgr.heartbeat(session_id);

    let Some(session_dir) = mgr.get_session_dir(session_id) else {
        return (StatusCode::NOT_FOUND, "session directory not found").into_response();
    };

    // Wait for the segment to be written by ffmpeg
    if let Err(e) =
        stackarr_stream::hls::wait_for_segment(&session_dir, &segment, Duration::from_secs(30))
            .await
    {
        tracing::warn!(segment = %segment, error = %e, "segment wait timed out");
        return (StatusCode::NOT_FOUND, "segment not ready").into_response();
    }

    match stackarr_stream::hls::read_segment(&session_dir, &segment).await {
        Ok(data) => {
            let mut headers = HeaderMap::new();
            headers.insert("content-type", HeaderValue::from_static("video/mp2t"));
            (StatusCode::OK, headers, data).into_response()
        }
        Err(_) => (StatusCode::NOT_FOUND, "segment not found").into_response(),
    }
}

/// GET /api/v1/stream/{media_file_id}/subtitles/{track_index}
/// Extract and serve a subtitle track as WebVTT.
async fn stream_subtitle(
    State(state): State<Arc<AppState>>,
    Path((media_file_id, track_index)): Path<(i64, usize)>,
) -> impl IntoResponse {
    let Some(ref mgr) = state.stream_session_manager else {
        return streaming_not_enabled().into_response();
    };

    let file_path = match resolve_media_path(state.db.pool(), media_file_id).await {
        Ok(p) => p,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "media file not found"})),
            )
                .into_response()
        }
    };

    // Create a temp file for the extracted subtitle
    let config = mgr.config();
    let cache_dir = mgr.transcode_dir().join("subtitles");
    let _ = tokio::fs::create_dir_all(&cache_dir).await;
    let output_path = cache_dir.join(format!("{media_file_id}_{track_index}.vtt"));

    // Use cached version if available
    if !output_path.exists() {
        if let Err(e) = stackarr_stream::subtitle::extract_to_webvtt(
            &config.ffmpeg_path,
            &file_path,
            track_index,
            &output_path,
        )
        .await
        {
            tracing::error!(media_file_id, track_index, error = %e, "subtitle extraction failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("subtitle extraction failed: {e}")})),
            )
                .into_response();
        }
    }

    match tokio::fs::read_to_string(&output_path).await {
        Ok(vtt) => {
            let mut headers = HeaderMap::new();
            headers.insert("content-type", HeaderValue::from_static("text/vtt; charset=utf-8"));
            (StatusCode::OK, headers, vtt).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to read subtitle file");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "failed to read subtitle"})),
            )
                .into_response()
        }
    }
}

/// GET /api/v1/stream/sessions
/// List all active streaming sessions.
async fn list_sessions(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let Some(ref mgr) = state.stream_session_manager else {
        return streaming_not_enabled().into_response();
    };

    Json(mgr.list_sessions()).into_response()
}

/// DELETE /api/v1/stream/sessions/{session_id}
/// Stop a streaming session.
async fn stop_session(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<Uuid>,
) -> impl IntoResponse {
    let Some(ref mgr) = state.stream_session_manager else {
        return streaming_not_enabled().into_response();
    };

    match mgr.stop_session(session_id).await {
        Ok(()) => (StatusCode::OK, Json(json!({"status": "stopped"}))).into_response(),
        Err(e) => {
            tracing::error!(%session_id, error = %e, "failed to stop session");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "failed to stop session"})),
            )
                .into_response()
        }
    }
}

// ── Router ──────────────────────────────────────────────────────────────

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/stream/{media_file_id}/info", get(stream_info))
        .route("/api/v1/stream/{media_file_id}/direct", get(stream_direct))
        .route(
            "/api/v1/stream/{media_file_id}/transcode",
            post(start_transcode),
        )
        .route(
            "/api/v1/stream/{media_file_id}/hls/{session_id}/master.m3u8",
            get(hls_playlist),
        )
        .route(
            "/api/v1/stream/{media_file_id}/hls/{session_id}/{segment}",
            get(hls_segment),
        )
        .route(
            "/api/v1/stream/{media_file_id}/subtitles/{track_index}",
            get(stream_subtitle),
        )
        .route("/api/v1/stream/sessions", get(list_sessions))
        .route(
            "/api/v1/stream/sessions/{session_id}",
            delete(stop_session),
        )
}
