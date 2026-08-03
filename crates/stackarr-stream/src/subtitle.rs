// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

use std::path::Path;

use crate::error::{StreamError, StreamResult};

/// Extract an embedded subtitle track to WebVTT format using ffmpeg.
///
/// `track_index` is the subtitle stream index (0-based among subtitle streams).
pub async fn extract_to_webvtt(
    ffmpeg_path: &str,
    source: &Path,
    track_index: usize,
    output: &Path,
) -> StreamResult<()> {
    let output = tokio::process::Command::new(ffmpeg_path)
        .args(["-nostdin", "-y"])
        .arg("-i")
        .arg(source)
        .arg("-map")
        .arg(format!("0:s:{track_index}"))
        .args(["-f", "webvtt"])
        .arg(output)
        .output()
        .await
        .map_err(|e| {
            StreamError::Transcode(format!("failed to run ffmpeg for subtitle extraction: {e}"))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::error!(%stderr, "subtitle extraction failed");
        return Err(StreamError::Transcode(
            "subtitle extraction failed".to_string(),
        ));
    }

    Ok(())
}

/// Check if a subtitle codec is a bitmap format that cannot be converted to text.
///
/// Bitmap subtitles (PGS, DVB, VOBSUB) must be burned into the video stream
/// rather than served as a separate text track.
pub fn is_bitmap_subtitle(codec: &str) -> bool {
    matches!(
        codec,
        "hdmv_pgs_subtitle" | "pgssub" | "dvb_subtitle" | "dvdsub" | "dvd_subtitle" | "xsub"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Bitmap subtitle detection ─────────────────────────────────────

    #[test]
    fn test_pgs_is_bitmap() {
        assert!(is_bitmap_subtitle("hdmv_pgs_subtitle"));
    }

    #[test]
    fn test_pgssub_is_bitmap() {
        assert!(is_bitmap_subtitle("pgssub"));
    }

    #[test]
    fn test_dvb_subtitle_is_bitmap() {
        assert!(is_bitmap_subtitle("dvb_subtitle"));
    }

    #[test]
    fn test_dvdsub_is_bitmap() {
        assert!(is_bitmap_subtitle("dvdsub"));
    }

    #[test]
    fn test_dvd_subtitle_is_bitmap() {
        assert!(is_bitmap_subtitle("dvd_subtitle"));
    }

    #[test]
    fn test_xsub_is_bitmap() {
        assert!(is_bitmap_subtitle("xsub"));
    }

    // ── Text subtitles are NOT bitmap ─────────────────────────────────

    #[test]
    fn test_srt_is_not_bitmap() {
        assert!(!is_bitmap_subtitle("srt"));
    }

    #[test]
    fn test_subrip_is_not_bitmap() {
        assert!(!is_bitmap_subtitle("subrip"));
    }

    #[test]
    fn test_ass_is_not_bitmap() {
        assert!(!is_bitmap_subtitle("ass"));
    }

    #[test]
    fn test_ssa_is_not_bitmap() {
        assert!(!is_bitmap_subtitle("ssa"));
    }

    #[test]
    fn test_webvtt_is_not_bitmap() {
        assert!(!is_bitmap_subtitle("webvtt"));
    }

    #[test]
    fn test_mov_text_is_not_bitmap() {
        assert!(!is_bitmap_subtitle("mov_text"));
    }

    #[test]
    fn test_empty_string_is_not_bitmap() {
        assert!(!is_bitmap_subtitle(""));
    }

    #[test]
    fn test_unknown_codec_is_not_bitmap() {
        assert!(!is_bitmap_subtitle("unknown_codec"));
    }
}
