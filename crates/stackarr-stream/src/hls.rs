use std::path::Path;
use std::time::Duration;

use crate::error::{StreamError, StreamResult};

/// Read an HLS playlist and rewrite segment URLs to use API routes.
///
/// `api_prefix` should be like `/api/v1/stream/{media_file_id}/hls/{session_id}`.
pub async fn read_playlist(session_dir: &Path, api_prefix: &str) -> StreamResult<String> {
    let playlist_path = session_dir.join("master.m3u8");

    let content = tokio::fs::read_to_string(&playlist_path).await.map_err(|e| {
        StreamError::Transcode(format!(
            "failed to read playlist {}: {e}",
            playlist_path.display()
        ))
    })?;

    // Rewrite segment filenames to API URLs
    let rewritten = content
        .lines()
        .map(|line| {
            if line.ends_with(".ts") {
                format!("{api_prefix}/{line}")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    Ok(rewritten)
}

/// Read a single HLS segment file.
///
/// Validates the segment name to prevent directory traversal.
pub async fn read_segment(session_dir: &Path, segment_name: &str) -> StreamResult<Vec<u8>> {
    // Security: prevent path traversal
    if segment_name.contains("..") || segment_name.contains('/') || segment_name.contains('\\') {
        return Err(StreamError::NotFound(format!(
            "invalid segment name: {segment_name}"
        )));
    }

    // Only allow .ts files
    if !segment_name.ends_with(".ts") {
        return Err(StreamError::NotFound(format!(
            "invalid segment type: {segment_name}"
        )));
    }

    let segment_path = session_dir.join(segment_name);
    tokio::fs::read(&segment_path).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            StreamError::NotFound(format!("segment not found: {segment_name}"))
        } else {
            StreamError::Io(e)
        }
    })
}

/// Wait for a segment file to appear on disk (ffmpeg writes them incrementally).
///
/// Returns `Ok(())` if the segment appears within the timeout, or an error if it times out.
pub async fn wait_for_segment(
    session_dir: &Path,
    segment_name: &str,
    timeout: Duration,
) -> StreamResult<()> {
    let segment_path = session_dir.join(segment_name);
    let deadline = tokio::time::Instant::now() + timeout;

    loop {
        if segment_path.exists() {
            // Wait a tiny bit more for ffmpeg to finish writing the segment
            tokio::time::sleep(Duration::from_millis(50)).await;
            return Ok(());
        }

        if tokio::time::Instant::now() >= deadline {
            return Err(StreamError::Transcode(format!(
                "timed out waiting for segment: {segment_name}"
            )));
        }

        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}
