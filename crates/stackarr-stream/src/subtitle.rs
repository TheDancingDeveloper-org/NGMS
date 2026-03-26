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
        .map_err(|e| StreamError::Transcode(format!("failed to run ffmpeg for subtitle extraction: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(StreamError::Transcode(format!(
            "subtitle extraction failed: {stderr}"
        )));
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
