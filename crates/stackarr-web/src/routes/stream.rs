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

use serde::Serialize;
use stackarr_plex::PlexApi;
use stackarr_plex::types::PlexServer;
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
        && let Some(arr) = maps.as_array()
    {
        let path_str = path.to_string_lossy();
        for entry in arr {
            if let (Some(from), Some(to)) = (
                entry.get(0).and_then(|v| v.as_str()),
                entry.get(1).and_then(|v| v.as_str()),
            ) && path_str.starts_with(from)
            {
                let remapped = format!("{}{}", to, &path_str[from.len()..]);
                return PathBuf::from(remapped);
            }
        }
    }

    path
}

/// Resolve the full filesystem path for a media file by joining the
/// parent entity's directory path with the file's relative path.
async fn resolve_media_path(
    pool: &sqlx::PgPool,
    media_file_id: i64,
) -> Result<PathBuf, StatusCode> {
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
    let cached: Option<(Option<serde_json::Value>,)> =
        sqlx::query_as("SELECT media_info FROM media_files WHERE id = $1")
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
                .into_response();
        }
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response();
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
            tracing::error!(
                media_file_id,
                path = %file_path.display(),
                exists = file_path.exists(),
                error = %e,
                "ffprobe failed",
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "media probe failed"})),
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
                .into_response();
        }
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response();
        }
    };

    let range_header = headers.get("range").and_then(|v| v.to_str().ok());

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
                HeaderValue::from_str(&resp.content_type)
                    .unwrap_or(HeaderValue::from_static("application/octet-stream")),
            );
            response_headers.insert(
                "content-length",
                HeaderValue::from_str(&resp.content_length.to_string())
                    .unwrap_or(HeaderValue::from_static("0")),
            );
            response_headers.insert("accept-ranges", HeaderValue::from_static("bytes"));

            if let Some(range) = &resp.content_range
                && let Ok(val) = HeaderValue::from_str(range)
            {
                response_headers.insert("content-range", val);
            }

            let body = Body::from_stream(resp.body);
            (status, response_headers, body).into_response()
        }
        Err(stackarr_stream::StreamError::NotFound(_)) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "file not found on disk"})),
        )
            .into_response(),
        Err(stackarr_stream::StreamError::InvalidRange(msg)) => (
            StatusCode::RANGE_NOT_SATISFIABLE,
            Json(json!({"error": msg})),
        )
            .into_response(),
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
                .into_response();
        }
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response();
        }
    };

    // Single-rendition transcode — bandwidth test on client picks the best tier.
    // Multi-rendition (lazy escalation) will be added in a future update.
    let result = {
        mgr.create_transcode_session(media_file_id, &file_path, &request)
            .await
    };

    match result {
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
                Json(json!({"error": "transcode session failed to start"})),
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
        if playlist_path.exists()
            && tokio::fs::metadata(&playlist_path)
                .await
                .map(|m| m.len() > 0)
                .unwrap_or(false)
        {
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
        return super::api_error(StatusCode::NOT_FOUND, "session not found");
    }

    mgr.heartbeat(session_id);

    let Some(session_dir) = mgr.get_session_dir(session_id) else {
        return super::api_error(StatusCode::NOT_FOUND, "session directory not found");
    };

    // Wait for the segment to be written by ffmpeg
    if let Err(e) =
        stackarr_stream::hls::wait_for_segment(&session_dir, &segment, Duration::from_secs(30))
            .await
    {
        tracing::warn!(segment = %segment, error = %e, "segment wait timed out");
        return super::api_error(StatusCode::NOT_FOUND, "segment not ready");
    }

    match stackarr_stream::hls::read_segment(&session_dir, &segment).await {
        Ok(data) => {
            let mut headers = HeaderMap::new();
            headers.insert("content-type", HeaderValue::from_static("video/mp2t"));
            (StatusCode::OK, headers, data).into_response()
        }
        Err(_) => super::api_error(StatusCode::NOT_FOUND, "segment not found"),
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
                .into_response();
        }
    };

    // Create a temp file for the extracted subtitle
    let config = mgr.config();
    let cache_dir = mgr.transcode_dir().join("subtitles");
    let _ = tokio::fs::create_dir_all(&cache_dir).await;
    let output_path = cache_dir.join(format!("{media_file_id}_{track_index}.vtt"));

    // Use cached version if available
    if !output_path.exists()
        && let Err(e) = stackarr_stream::subtitle::extract_to_webvtt(
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

    match tokio::fs::read_to_string(&output_path).await {
        Ok(vtt) => {
            let mut headers = HeaderMap::new();
            headers.insert(
                "content-type",
                HeaderValue::from_static("text/vtt; charset=utf-8"),
            );
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
async fn list_sessions(State(state): State<Arc<AppState>>) -> impl IntoResponse {
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

// ── Multi-rendition sub-playlist/segment routes ──────────────────────────

/// GET /api/v1/stream/{media_file_id}/hls/{session_id}/v{rendition}/stream.m3u8
async fn hls_sub_playlist(
    State(state): State<Arc<AppState>>,
    Path((media_file_id, session_id, rendition)): Path<(i64, Uuid, String)>,
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

    let rendition_dir = session_dir.join(&rendition);
    if !rendition_dir.exists() {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "rendition not found"})),
        )
            .into_response();
    }

    // Wait for sub-playlist to appear
    let playlist_path = rendition_dir.join("stream.m3u8");
    for _ in 0..30 {
        if playlist_path.exists()
            && tokio::fs::metadata(&playlist_path)
                .await
                .map(|m| m.len() > 0)
                .unwrap_or(false)
        {
            break;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    let api_prefix = format!("/api/v1/stream/{media_file_id}/hls/{session_id}/{rendition}");
    match stackarr_stream::hls::read_sub_playlist(&rendition_dir, &api_prefix).await {
        Ok(playlist) => {
            let mut headers = HeaderMap::new();
            headers.insert(
                "content-type",
                HeaderValue::from_static("application/vnd.apple.mpegurl"),
            );
            (StatusCode::OK, headers, playlist).into_response()
        }
        Err(e) => {
            tracing::error!(%session_id, rendition = %rendition, error = %e, "failed to read sub-playlist");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "sub-playlist not ready"})),
            )
                .into_response()
        }
    }
}

/// GET /api/v1/stream/{media_file_id}/hls/{session_id}/v{rendition}/{segment}
async fn hls_sub_segment(
    State(state): State<Arc<AppState>>,
    Path((media_file_id, session_id, rendition, segment)): Path<(i64, Uuid, String, String)>,
) -> impl IntoResponse {
    let Some(ref mgr) = state.stream_session_manager else {
        return streaming_not_enabled().into_response();
    };

    if !mgr.validate_session(session_id, media_file_id) {
        return super::api_error(StatusCode::NOT_FOUND, "session not found");
    }

    mgr.heartbeat(session_id);

    let Some(session_dir) = mgr.get_session_dir(session_id) else {
        return super::api_error(StatusCode::NOT_FOUND, "session directory not found");
    };

    let rendition_dir = session_dir.join(&rendition);

    // Wait for segment
    if let Err(e) =
        stackarr_stream::hls::wait_for_segment(&rendition_dir, &segment, Duration::from_secs(30))
            .await
    {
        tracing::warn!(segment = %segment, rendition = %rendition, error = %e, "sub-segment wait timed out");
        return super::api_error(StatusCode::NOT_FOUND, "segment not ready");
    }

    match stackarr_stream::hls::read_segment(&rendition_dir, &segment).await {
        Ok(data) => {
            let mut headers = HeaderMap::new();
            headers.insert("content-type", HeaderValue::from_static("video/mp2t"));
            (StatusCode::OK, headers, data).into_response()
        }
        Err(_) => super::api_error(StatusCode::NOT_FOUND, "segment not found"),
    }
}

// ── Bandwidth Test ──────────────────────────────────────────────────────

/// GET /api/v1/stream/bandwidth-test?size={bytes}
/// Returns a zero-filled payload for client bandwidth measurement.
async fn bandwidth_test(
    axum::extract::Query(params): axum::extract::Query<BandwidthTestParams>,
) -> impl IntoResponse {
    let size = params.size.unwrap_or(2_000_000).min(10_000_000) as usize;
    let data = vec![0u8; size];
    let mut headers = HeaderMap::new();
    headers.insert(
        "content-type",
        HeaderValue::from_static("application/octet-stream"),
    );
    headers.insert("cache-control", HeaderValue::from_static("no-store"));
    (StatusCode::OK, headers, data).into_response()
}

#[derive(serde::Deserialize)]
struct BandwidthTestParams {
    size: Option<u64>,
}

/// GET /api/v1/stream/{media_file_id}/quality-tiers
/// Returns quality tiers applicable to this media file (filtered by source resolution).
async fn quality_tiers(
    State(state): State<Arc<AppState>>,
    Path(media_file_id): Path<i64>,
) -> impl IntoResponse {
    // Get source resolution from cached media_info
    let cached: Option<(Option<serde_json::Value>,)> =
        sqlx::query_as("SELECT media_info FROM media_files WHERE id = $1")
            .bind(media_file_id)
            .fetch_optional(state.db.pool())
            .await
            .unwrap_or(None);

    let (source_width, source_height) = if let Some((Some(info),)) = &cached {
        let w = info
            .get("videoStreams")
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
            .and_then(|s| s.get("width"))
            .and_then(|v| v.as_u64())
            .unwrap_or(1920) as u32;
        let h = info
            .get("videoStreams")
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
            .and_then(|s| s.get("height"))
            .and_then(|v| v.as_u64())
            .unwrap_or(1080) as u32;
        (w, h)
    } else {
        (1920, 1080) // default assumption
    };

    let config = state.config.load();
    let tiers: Vec<stackarr_stream::types::QualityTier> = config
        .streaming
        .quality_tiers
        .iter()
        .filter(|t| t.max_height <= source_height)
        .map(|t| stackarr_stream::types::QualityTier {
            name: t.name.clone(),
            max_width: t.max_width,
            max_height: t.max_height,
            video_bitrate: t.video_bitrate,
            audio_bitrate: t.audio_bitrate,
        })
        .collect();

    // Always include "Original" as the top tier
    let mut result = vec![stackarr_stream::types::QualityTier {
        name: "Original".to_string(),
        max_width: source_width,
        max_height: source_height,
        video_bitrate: 0, // 0 = direct play / no transcode
        audio_bitrate: 0,
    }];
    result.extend(tiers);

    Json(json!(result)).into_response()
}

// ── Router ──────────────────────────────────────────────────────────────

// ── Unified sessions (StackArr + Plex) ────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UnifiedSession {
    id: String,
    source: String,
    title: Option<String>,
    user: Option<String>,
    player: Option<String>,
    state: String,
    progress_percent: Option<f64>,
    session_type: String,
    started_at: Option<String>,
    video_codec: Option<String>,
    audio_codec: Option<String>,
    resolution: Option<String>,
    bitrate: Option<i64>,
    video_decision: Option<String>,
    audio_decision: Option<String>,
    transcode_speed: Option<f64>,
    platform: Option<String>,
    is_local: Option<bool>,
}

/// GET /api/v1/stream/sessions/unified
async fn list_unified_sessions(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let mut sessions: Vec<UnifiedSession> = Vec::new();

    // StackArr sessions
    if let Some(ref mgr) = state.stream_session_manager {
        for s in mgr.list_sessions() {
            sessions.push(UnifiedSession {
                id: s.session_id.to_string(),
                source: "stackarr".to_string(),
                title: None,
                user: None,
                player: None,
                state: s.status.clone(),
                progress_percent: s.transcode_progress.map(|p| (p * 100.0) as f64),
                session_type: s.session_type.clone(),
                started_at: Some(s.started_at.clone()),
                video_codec: s.video_codec.clone(),
                audio_codec: s.audio_codec.clone(),
                resolution: s.resolution.clone(),
                bitrate: None,
                video_decision: Some(s.session_type.clone()),
                audio_decision: None,
                transcode_speed: None,
                platform: None,
                is_local: None,
            });
        }
    }

    // Plex sessions from all configured servers
    let pool = state.db.pool();
    let servers = sqlx::query_as::<_, PlexServer>(
        "SELECT id, name, machine_id, ip, port, use_ssl, verify_tls, auth_token, web_app_url, created_at, updated_at \
         FROM plex_servers ORDER BY id",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    for server in &servers {
        let Some(api) = PlexApi::from_server(server) else {
            continue;
        };
        let plex_sessions = match api.get_active_sessions().await {
            Ok(s) => s,
            Err(e) => {
                tracing::debug!(server_id = server.id, error = %e, "failed to fetch plex sessions");
                continue;
            }
        };

        for ps in plex_sessions {
            let title = if let Some(ref gp) = ps.grandparent_title {
                let ep_title = &ps.title;
                let season = ps.parent_title.as_deref().unwrap_or("");
                Some(format!("{gp} - {season} - {ep_title}"))
            } else {
                Some(ps.title.clone())
            };

            let progress = match (ps.view_offset, ps.duration) {
                (Some(offset), Some(dur)) if dur > 0 => Some((offset as f64 / dur as f64) * 100.0),
                _ => None,
            };

            let (video_codec, audio_codec, video_decision, audio_decision, transcode_speed) =
                if let Some(ref ts) = ps.transcode_session {
                    (
                        ts.video_codec.clone(),
                        ts.audio_codec.clone(),
                        ts.video_decision.clone(),
                        ts.audio_decision.clone(),
                        ts.speed,
                    )
                } else {
                    (None, None, None, None, None)
                };

            let (resolution, bitrate) = ps
                .media
                .first()
                .map(|m| (m.video_resolution.clone(), m.bitrate))
                .unwrap_or((None, None));

            let is_transcode = ps
                .transcode_session
                .as_ref()
                .and_then(|ts| ts.video_decision.as_deref())
                .map(|d| d == "transcode")
                .unwrap_or(false);

            let player_state = ps
                .player
                .as_ref()
                .and_then(|p| p.state.as_deref())
                .unwrap_or("playing");

            sessions.push(UnifiedSession {
                id: ps.rating_key.clone(),
                source: "plex".to_string(),
                title,
                user: ps.user.as_ref().map(|u| u.title.clone()),
                player: ps.player.as_ref().map(|p| p.title.clone()),
                state: player_state.to_string(),
                progress_percent: progress,
                session_type: if is_transcode {
                    "transcode".to_string()
                } else {
                    "direct".to_string()
                },
                started_at: None,
                video_codec,
                audio_codec,
                resolution,
                bitrate,
                video_decision,
                audio_decision,
                transcode_speed,
                platform: ps.player.as_ref().and_then(|p| p.platform.clone()),
                is_local: ps.player.as_ref().and_then(|p| p.local),
            });
        }
    }

    Json(sessions).into_response()
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/stream/bandwidth-test", get(bandwidth_test))
        .route("/api/v1/stream/{media_file_id}/info", get(stream_info))
        .route(
            "/api/v1/stream/{media_file_id}/quality-tiers",
            get(quality_tiers),
        )
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
            "/api/v1/stream/{media_file_id}/hls/{session_id}/{rendition}/stream.m3u8",
            get(hls_sub_playlist),
        )
        .route(
            "/api/v1/stream/{media_file_id}/hls/{session_id}/{rendition}/{segment}",
            get(hls_sub_segment),
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
            "/api/v1/stream/sessions/unified",
            get(list_unified_sessions),
        )
        .route("/api/v1/stream/sessions/{session_id}", delete(stop_session))
}
