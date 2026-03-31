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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // ── Playlist rewriting ────────────────────────────────────────────

    #[tokio::test]
    async fn test_read_playlist_rewrites_segments() {
        let dir = tempfile::tempdir().unwrap();
        let playlist = "#EXTM3U\n#EXT-X-VERSION:3\n#EXTINF:6.0,\n0000.ts\n#EXTINF:6.0,\n0001.ts\n#EXT-X-ENDLIST\n";
        fs::write(dir.path().join("master.m3u8"), playlist).unwrap();

        let result = read_playlist(dir.path(), "/api/v1/stream/42/hls/abc-123").await.unwrap();
        assert!(result.contains("/api/v1/stream/42/hls/abc-123/0000.ts"));
        assert!(result.contains("/api/v1/stream/42/hls/abc-123/0001.ts"));
        assert!(result.contains("#EXTM3U"));
        assert!(result.contains("#EXTINF:6.0,"));
    }

    #[tokio::test]
    async fn test_read_playlist_preserves_non_segment_lines() {
        let dir = tempfile::tempdir().unwrap();
        let playlist = "#EXTM3U\n#EXT-X-TARGETDURATION:6\n#EXT-X-MEDIA-SEQUENCE:0\n#EXTINF:6.006,\n0000.ts\n#EXT-X-ENDLIST";
        fs::write(dir.path().join("master.m3u8"), playlist).unwrap();

        let result = read_playlist(dir.path(), "/prefix").await.unwrap();
        assert!(result.contains("#EXTM3U"));
        assert!(result.contains("#EXT-X-TARGETDURATION:6"));
        assert!(result.contains("#EXT-X-MEDIA-SEQUENCE:0"));
        assert!(result.contains("#EXT-X-ENDLIST"));
    }

    #[tokio::test]
    async fn test_read_playlist_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("master.m3u8"), "").unwrap();

        let result = read_playlist(dir.path(), "/prefix").await.unwrap();
        assert_eq!(result, "");
    }

    #[tokio::test]
    async fn test_read_playlist_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let result = read_playlist(dir.path(), "/prefix").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_read_playlist_no_segments() {
        let dir = tempfile::tempdir().unwrap();
        let playlist = "#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-ENDLIST";
        fs::write(dir.path().join("master.m3u8"), playlist).unwrap();

        let result = read_playlist(dir.path(), "/prefix").await.unwrap();
        assert!(!result.contains("/prefix"));
        assert!(result.contains("#EXT-X-ENDLIST"));
    }

    // ── Segment reading ───────────────────────────────────────────────

    #[tokio::test]
    async fn test_read_segment_success() {
        let dir = tempfile::tempdir().unwrap();
        let data = vec![0u8; 1024];
        fs::write(dir.path().join("0000.ts"), &data).unwrap();

        let result = read_segment(dir.path(), "0000.ts").await.unwrap();
        assert_eq!(result.len(), 1024);
    }

    #[tokio::test]
    async fn test_read_segment_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let err = read_segment(dir.path(), "9999.ts").await.unwrap_err();
        match err {
            StreamError::NotFound(msg) => assert!(msg.contains("9999.ts")),
            other => panic!("expected NotFound, got: {other:?}"),
        }
    }

    // ── Path traversal prevention ─────────────────────────────────────

    #[tokio::test]
    async fn test_read_segment_rejects_dotdot() {
        let dir = tempfile::tempdir().unwrap();
        let err = read_segment(dir.path(), "../../../etc/passwd").await.unwrap_err();
        match err {
            StreamError::NotFound(msg) => assert!(msg.contains("invalid segment name")),
            other => panic!("expected NotFound for path traversal, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_read_segment_rejects_forward_slash() {
        let dir = tempfile::tempdir().unwrap();
        let err = read_segment(dir.path(), "subdir/0000.ts").await.unwrap_err();
        match err {
            StreamError::NotFound(msg) => assert!(msg.contains("invalid segment name")),
            other => panic!("expected NotFound for slash, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_read_segment_rejects_backslash() {
        let dir = tempfile::tempdir().unwrap();
        let err = read_segment(dir.path(), "subdir\\0000.ts").await.unwrap_err();
        match err {
            StreamError::NotFound(msg) => assert!(msg.contains("invalid segment name")),
            other => panic!("expected NotFound for backslash, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_read_segment_rejects_non_ts_extension() {
        let dir = tempfile::tempdir().unwrap();
        let err = read_segment(dir.path(), "0000.m3u8").await.unwrap_err();
        match err {
            StreamError::NotFound(msg) => assert!(msg.contains("invalid segment type")),
            other => panic!("expected NotFound for non-ts, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_read_segment_rejects_no_extension() {
        let dir = tempfile::tempdir().unwrap();
        let err = read_segment(dir.path(), "segment_data").await.unwrap_err();
        match err {
            StreamError::NotFound(msg) => assert!(msg.contains("invalid segment type")),
            other => panic!("expected NotFound for no extension, got: {other:?}"),
        }
    }

    // ── wait_for_segment ──────────────────────────────────────────────

    #[tokio::test]
    async fn test_wait_for_segment_already_exists() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("0000.ts"), b"data").unwrap();

        let result = wait_for_segment(dir.path(), "0000.ts", Duration::from_secs(1)).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_wait_for_segment_timeout() {
        let dir = tempfile::tempdir().unwrap();
        // Don't create the segment file
        let result = wait_for_segment(dir.path(), "9999.ts", Duration::from_millis(300)).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            StreamError::Transcode(msg) => assert!(msg.contains("timed out")),
            other => panic!("expected Transcode timeout, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_wait_for_segment_appears_during_wait() {
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path().to_path_buf();

        // Spawn a task that creates the segment after a short delay
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            fs::write(dir_path.join("0001.ts"), b"segment data").unwrap();
        });

        let result = wait_for_segment(dir.path(), "0001.ts", Duration::from_secs(5)).await;
        assert!(result.is_ok());
    }
}
