use std::path::Path;

use crate::error::{StreamError, StreamResult};
use crate::types::{AudioStream, MediaInfo, SubtitleStream, VideoStream};

/// Probe a media file using ffprobe and return structured info.
pub async fn probe(ffprobe_path: &str, file_path: &Path) -> StreamResult<MediaInfo> {
    let output = tokio::process::Command::new(ffprobe_path)
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
        ])
        .arg(file_path)
        .output()
        .await
        .map_err(|e| StreamError::Probe(format!("failed to run ffprobe: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(StreamError::Probe(format!(
            "ffprobe exited with {}: {stderr}",
            output.status
        )));
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| StreamError::Probe(format!("failed to parse ffprobe output: {e}")))?;

    parse_ffprobe_json(&json)
}

fn parse_ffprobe_json(json: &serde_json::Value) -> StreamResult<MediaInfo> {
    let format = &json["format"];
    let streams = json["streams"]
        .as_array()
        .ok_or_else(|| StreamError::Probe("no streams array in ffprobe output".into()))?;

    let container = format["format_name"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();

    let duration_secs = format["duration"]
        .as_str()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);

    let bitrate = format["bit_rate"]
        .as_str()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    let mut video_streams = Vec::new();
    let mut audio_streams = Vec::new();
    let mut subtitle_streams = Vec::new();
    let mut video_idx = 0usize;
    let mut audio_idx = 0usize;
    let mut sub_idx = 0usize;

    for stream in streams {
        let codec_type = stream["codec_type"].as_str().unwrap_or("");

        match codec_type {
            "video" => {
                // Skip attached pictures (album art, thumbnails)
                let disposition = &stream["disposition"];
                if disposition["attached_pic"].as_i64() == Some(1) {
                    continue;
                }

                let codec = stream["codec_name"].as_str().unwrap_or("").to_string();
                let width = stream["width"].as_u64().unwrap_or(0) as u32;
                let height = stream["height"].as_u64().unwrap_or(0) as u32;
                let stream_bitrate = stream["bit_rate"]
                    .as_str()
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(0);
                let profile = stream["profile"].as_str().unwrap_or("").to_string();
                let level = stream["level"].as_u64().unwrap_or(0) as u32;

                // HDR detection
                let color_transfer = stream["color_transfer"].as_str().unwrap_or("");
                let is_hdr = matches!(
                    color_transfer,
                    "smpte2084" | "arib-std-b67" | "smpte428" | "bt2020-10" | "bt2020-12"
                );

                // Frame rate parsing (e.g. "24000/1001" or "24/1")
                let frame_rate = stream["r_frame_rate"]
                    .as_str()
                    .and_then(parse_frame_rate)
                    .unwrap_or(0.0);

                video_streams.push(VideoStream {
                    index: video_idx,
                    codec,
                    width,
                    height,
                    bitrate: stream_bitrate,
                    profile,
                    level,
                    is_hdr,
                    frame_rate,
                });
                video_idx += 1;
            }
            "audio" => {
                let codec = stream["codec_name"].as_str().unwrap_or("").to_string();
                let channels = stream["channels"].as_u64().unwrap_or(0) as u32;
                let tags = &stream["tags"];
                let language = tags["language"].as_str().unwrap_or("und").to_string();
                let title = tags["title"].as_str().unwrap_or("").to_string();
                let stream_bitrate = stream["bit_rate"]
                    .as_str()
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(0);
                let disposition = &stream["disposition"];
                let is_default = disposition["default"].as_i64() == Some(1);

                audio_streams.push(AudioStream {
                    index: audio_idx,
                    codec,
                    channels,
                    language,
                    title,
                    bitrate: stream_bitrate,
                    is_default,
                });
                audio_idx += 1;
            }
            "subtitle" => {
                let codec = stream["codec_name"].as_str().unwrap_or("").to_string();
                let tags = &stream["tags"];
                let language = tags["language"].as_str().unwrap_or("und").to_string();
                let title = tags["title"].as_str().unwrap_or("").to_string();
                let disposition = &stream["disposition"];
                let forced = disposition["forced"].as_i64() == Some(1);
                let is_default = disposition["default"].as_i64() == Some(1);

                subtitle_streams.push(SubtitleStream {
                    index: sub_idx,
                    codec,
                    language,
                    title,
                    forced,
                    is_default,
                });
                sub_idx += 1;
            }
            _ => {}
        }
    }

    Ok(MediaInfo {
        container,
        duration_secs,
        bitrate,
        video_streams,
        audio_streams,
        subtitle_streams,
    })
}

fn parse_frame_rate(s: &str) -> Option<f64> {
    let parts: Vec<&str> = s.split('/').collect();
    if parts.len() == 2 {
        let num = parts[0].parse::<f64>().ok()?;
        let den = parts[1].parse::<f64>().ok()?;
        if den > 0.0 {
            return Some(num / den);
        }
    }
    s.parse::<f64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Frame rate parsing ────────────────────────────────────────────

    #[test]
    fn test_parse_frame_rate() {
        assert!((parse_frame_rate("24000/1001").unwrap() - 23.976).abs() < 0.01);
        assert!((parse_frame_rate("24/1").unwrap() - 24.0).abs() < 0.01);
        assert!((parse_frame_rate("30").unwrap() - 30.0).abs() < 0.01);
        assert!(parse_frame_rate("0/0").is_none());
    }

    #[test]
    fn test_parse_frame_rate_ntsc() {
        assert!((parse_frame_rate("30000/1001").unwrap() - 29.97).abs() < 0.01);
    }

    #[test]
    fn test_parse_frame_rate_50fps() {
        assert!((parse_frame_rate("50/1").unwrap() - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_frame_rate_60fps() {
        assert!((parse_frame_rate("60000/1001").unwrap() - 59.94).abs() < 0.01);
    }

    #[test]
    fn test_parse_frame_rate_plain_float() {
        assert!((parse_frame_rate("25").unwrap() - 25.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_frame_rate_invalid() {
        assert!(parse_frame_rate("abc").is_none());
        assert!(parse_frame_rate("").is_none());
        assert!(parse_frame_rate("/").is_none());
    }

    #[test]
    fn test_parse_frame_rate_zero_denominator() {
        assert!(parse_frame_rate("24/0").is_none());
    }

    // ── Basic ffprobe JSON parsing ────────────────────────────────────

    #[test]
    fn test_parse_ffprobe_json() {
        let json: serde_json::Value = serde_json::json!({
            "format": {
                "format_name": "matroska,webm",
                "duration": "2400.123",
                "bit_rate": "5000000"
            },
            "streams": [
                {
                    "codec_type": "video",
                    "codec_name": "h264",
                    "width": 1920,
                    "height": 1080,
                    "bit_rate": "4500000",
                    "profile": "High",
                    "level": 41,
                    "color_transfer": "bt709",
                    "r_frame_rate": "24000/1001",
                    "disposition": { "default": 1, "attached_pic": 0 }
                },
                {
                    "codec_type": "audio",
                    "codec_name": "aac",
                    "channels": 6,
                    "bit_rate": "384000",
                    "tags": { "language": "eng", "title": "Surround" },
                    "disposition": { "default": 1 }
                },
                {
                    "codec_type": "subtitle",
                    "codec_name": "srt",
                    "tags": { "language": "eng", "title": "English" },
                    "disposition": { "default": 0, "forced": 0 }
                }
            ]
        });

        let info = parse_ffprobe_json(&json).unwrap();
        assert_eq!(info.container, "matroska,webm");
        assert!((info.duration_secs - 2400.123).abs() < 0.01);
        assert_eq!(info.bitrate, 5_000_000);
        assert_eq!(info.video_streams.len(), 1);
        assert_eq!(info.video_streams[0].codec, "h264");
        assert_eq!(info.video_streams[0].width, 1920);
        assert!(!info.video_streams[0].is_hdr);
        assert_eq!(info.audio_streams.len(), 1);
        assert_eq!(info.audio_streams[0].channels, 6);
        assert_eq!(info.subtitle_streams.len(), 1);
        assert_eq!(info.subtitle_streams[0].language, "eng");
    }

    // ── HDR detection ─────────────────────────────────────────────────

    #[test]
    fn test_hdr_smpte2084_detected() {
        let json = serde_json::json!({
            "format": { "format_name": "mkv", "duration": "100", "bit_rate": "50000000" },
            "streams": [{
                "codec_type": "video",
                "codec_name": "hevc",
                "width": 3840, "height": 2160,
                "profile": "Main 10", "level": 51,
                "color_transfer": "smpte2084",
                "r_frame_rate": "24/1",
                "disposition": { "attached_pic": 0 }
            }]
        });
        let info = parse_ffprobe_json(&json).unwrap();
        assert!(info.video_streams[0].is_hdr);
    }

    #[test]
    fn test_hdr_hlg_detected() {
        let json = serde_json::json!({
            "format": { "format_name": "mkv", "duration": "100", "bit_rate": "50000000" },
            "streams": [{
                "codec_type": "video",
                "codec_name": "hevc",
                "width": 3840, "height": 2160,
                "profile": "Main 10", "level": 51,
                "color_transfer": "arib-std-b67",
                "r_frame_rate": "24/1",
                "disposition": { "attached_pic": 0 }
            }]
        });
        let info = parse_ffprobe_json(&json).unwrap();
        assert!(info.video_streams[0].is_hdr);
    }

    #[test]
    fn test_hdr_bt2020_10_detected() {
        let json = serde_json::json!({
            "format": { "format_name": "mkv", "duration": "100", "bit_rate": "0" },
            "streams": [{
                "codec_type": "video",
                "codec_name": "hevc",
                "width": 3840, "height": 2160,
                "profile": "Main 10", "level": 51,
                "color_transfer": "bt2020-10",
                "r_frame_rate": "24/1",
                "disposition": { "attached_pic": 0 }
            }]
        });
        let info = parse_ffprobe_json(&json).unwrap();
        assert!(info.video_streams[0].is_hdr);
    }

    #[test]
    fn test_sdr_bt709_not_hdr() {
        let json = serde_json::json!({
            "format": { "format_name": "mp4", "duration": "100", "bit_rate": "5000000" },
            "streams": [{
                "codec_type": "video",
                "codec_name": "h264",
                "width": 1920, "height": 1080,
                "profile": "High", "level": 41,
                "color_transfer": "bt709",
                "r_frame_rate": "24/1",
                "disposition": { "attached_pic": 0 }
            }]
        });
        let info = parse_ffprobe_json(&json).unwrap();
        assert!(!info.video_streams[0].is_hdr);
    }

    #[test]
    fn test_missing_color_transfer_not_hdr() {
        let json = serde_json::json!({
            "format": { "format_name": "mp4", "duration": "100", "bit_rate": "5000000" },
            "streams": [{
                "codec_type": "video",
                "codec_name": "h264",
                "width": 1920, "height": 1080,
                "profile": "High", "level": 41,
                "r_frame_rate": "24/1",
                "disposition": { "attached_pic": 0 }
            }]
        });
        let info = parse_ffprobe_json(&json).unwrap();
        assert!(!info.video_streams[0].is_hdr);
    }

    // ── Attached picture filtering ────────────────────────────────────

    #[test]
    fn test_attached_pic_filtered_out() {
        let json = serde_json::json!({
            "format": { "format_name": "mp3", "duration": "300", "bit_rate": "320000" },
            "streams": [
                {
                    "codec_type": "video",
                    "codec_name": "mjpeg",
                    "width": 600, "height": 600,
                    "profile": "", "level": 0,
                    "r_frame_rate": "0/0",
                    "disposition": { "attached_pic": 1, "default": 0 }
                },
                {
                    "codec_type": "audio",
                    "codec_name": "mp3",
                    "channels": 2,
                    "bit_rate": "320000",
                    "tags": { "language": "und" },
                    "disposition": { "default": 1 }
                }
            ]
        });
        let info = parse_ffprobe_json(&json).unwrap();
        assert!(
            info.video_streams.is_empty(),
            "attached picture should be filtered out"
        );
        assert_eq!(info.audio_streams.len(), 1);
    }

    // ── Multiple streams with correct indexing ────────────────────────

    #[test]
    fn test_multiple_video_streams_indexed_separately() {
        let json = serde_json::json!({
            "format": { "format_name": "mkv", "duration": "7200", "bit_rate": "20000000" },
            "streams": [
                {
                    "codec_type": "video",
                    "codec_name": "hevc",
                    "width": 3840, "height": 2160,
                    "bit_rate": "18000000",
                    "profile": "Main 10", "level": 51,
                    "color_transfer": "smpte2084",
                    "r_frame_rate": "24000/1001",
                    "disposition": { "attached_pic": 0 }
                },
                {
                    "codec_type": "video",
                    "codec_name": "h264",
                    "width": 1920, "height": 1080,
                    "bit_rate": "8000000",
                    "profile": "High", "level": 41,
                    "color_transfer": "bt709",
                    "r_frame_rate": "24/1",
                    "disposition": { "attached_pic": 0 }
                }
            ]
        });
        let info = parse_ffprobe_json(&json).unwrap();
        assert_eq!(info.video_streams.len(), 2);
        assert_eq!(info.video_streams[0].index, 0);
        assert_eq!(info.video_streams[0].codec, "hevc");
        assert_eq!(info.video_streams[1].index, 1);
        assert_eq!(info.video_streams[1].codec, "h264");
    }

    #[test]
    fn test_multiple_audio_streams_indexed_separately() {
        let json = serde_json::json!({
            "format": { "format_name": "mkv", "duration": "100", "bit_rate": "5000000" },
            "streams": [
                {
                    "codec_type": "audio",
                    "codec_name": "truehd",
                    "channels": 8,
                    "bit_rate": "4000000",
                    "tags": { "language": "eng", "title": "TrueHD 7.1" },
                    "disposition": { "default": 1 }
                },
                {
                    "codec_type": "audio",
                    "codec_name": "aac",
                    "channels": 2,
                    "bit_rate": "192000",
                    "tags": { "language": "eng", "title": "Stereo" },
                    "disposition": { "default": 0 }
                },
                {
                    "codec_type": "audio",
                    "codec_name": "ac3",
                    "channels": 6,
                    "bit_rate": "640000",
                    "tags": { "language": "fra", "title": "French" },
                    "disposition": { "default": 0 }
                }
            ]
        });
        let info = parse_ffprobe_json(&json).unwrap();
        assert_eq!(info.audio_streams.len(), 3);
        assert_eq!(info.audio_streams[0].index, 0);
        assert_eq!(info.audio_streams[0].codec, "truehd");
        assert!(info.audio_streams[0].is_default);
        assert_eq!(info.audio_streams[1].index, 1);
        assert!(!info.audio_streams[1].is_default);
        assert_eq!(info.audio_streams[2].index, 2);
        assert_eq!(info.audio_streams[2].language, "fra");
    }

    #[test]
    fn test_subtitle_forced_and_default_flags() {
        let json = serde_json::json!({
            "format": { "format_name": "mkv", "duration": "100", "bit_rate": "5000000" },
            "streams": [
                {
                    "codec_type": "subtitle",
                    "codec_name": "srt",
                    "tags": { "language": "eng", "title": "English" },
                    "disposition": { "default": 1, "forced": 0 }
                },
                {
                    "codec_type": "subtitle",
                    "codec_name": "srt",
                    "tags": { "language": "eng", "title": "English (Forced)" },
                    "disposition": { "default": 0, "forced": 1 }
                },
                {
                    "codec_type": "subtitle",
                    "codec_name": "hdmv_pgs_subtitle",
                    "tags": { "language": "jpn", "title": "Japanese PGS" },
                    "disposition": { "default": 0, "forced": 0 }
                }
            ]
        });
        let info = parse_ffprobe_json(&json).unwrap();
        assert_eq!(info.subtitle_streams.len(), 3);
        assert!(info.subtitle_streams[0].is_default);
        assert!(!info.subtitle_streams[0].forced);
        assert!(!info.subtitle_streams[1].is_default);
        assert!(info.subtitle_streams[1].forced);
        assert_eq!(info.subtitle_streams[2].codec, "hdmv_pgs_subtitle");
    }

    // ── Missing / partial fields ──────────────────────────────────────

    #[test]
    fn test_missing_format_fields_use_defaults() {
        let json = serde_json::json!({
            "format": {},
            "streams": [{
                "codec_type": "video",
                "codec_name": "h264",
                "width": 1280, "height": 720,
                "r_frame_rate": "30/1",
                "disposition": { "attached_pic": 0 }
            }]
        });
        let info = parse_ffprobe_json(&json).unwrap();
        assert_eq!(info.container, "unknown");
        assert_eq!(info.duration_secs, 0.0);
        assert_eq!(info.bitrate, 0);
    }

    #[test]
    fn test_missing_stream_bitrate_defaults_to_zero() {
        let json = serde_json::json!({
            "format": { "format_name": "mp4", "duration": "100", "bit_rate": "5000000" },
            "streams": [{
                "codec_type": "video",
                "codec_name": "h264",
                "width": 1920, "height": 1080,
                "profile": "Main",
                "level": 40,
                "r_frame_rate": "24/1",
                "disposition": { "attached_pic": 0 }
            }]
        });
        let info = parse_ffprobe_json(&json).unwrap();
        assert_eq!(info.video_streams[0].bitrate, 0);
    }

    #[test]
    fn test_missing_audio_tags_default() {
        let json = serde_json::json!({
            "format": { "format_name": "avi", "duration": "60", "bit_rate": "1000000" },
            "streams": [{
                "codec_type": "audio",
                "codec_name": "mp3",
                "channels": 2,
                "disposition": { "default": 0 }
            }]
        });
        let info = parse_ffprobe_json(&json).unwrap();
        assert_eq!(info.audio_streams[0].language, "und");
        assert_eq!(info.audio_streams[0].title, "");
    }

    #[test]
    fn test_unknown_codec_type_ignored() {
        let json = serde_json::json!({
            "format": { "format_name": "mkv", "duration": "100", "bit_rate": "5000000" },
            "streams": [
                {
                    "codec_type": "data",
                    "codec_name": "bin_data"
                },
                {
                    "codec_type": "video",
                    "codec_name": "h264",
                    "width": 1920, "height": 1080,
                    "r_frame_rate": "24/1",
                    "disposition": { "attached_pic": 0 }
                }
            ]
        });
        let info = parse_ffprobe_json(&json).unwrap();
        assert_eq!(info.video_streams.len(), 1);
        assert!(info.audio_streams.is_empty());
    }

    #[test]
    fn test_no_streams_array_returns_error() {
        let json = serde_json::json!({
            "format": { "format_name": "mp4", "duration": "100", "bit_rate": "5000000" }
        });
        assert!(parse_ffprobe_json(&json).is_err());
    }

    #[test]
    fn test_empty_streams_array() {
        let json = serde_json::json!({
            "format": { "format_name": "mp4", "duration": "100", "bit_rate": "5000000" },
            "streams": []
        });
        let info = parse_ffprobe_json(&json).unwrap();
        assert!(info.video_streams.is_empty());
        assert!(info.audio_streams.is_empty());
        assert!(info.subtitle_streams.is_empty());
    }

    // ── Complex real-world JSON ───────────────────────────────────────

    #[test]
    fn test_full_remux_file() {
        let json = serde_json::json!({
            "format": {
                "format_name": "matroska,webm",
                "duration": "8523.456",
                "bit_rate": "35000000"
            },
            "streams": [
                {
                    "codec_type": "video",
                    "codec_name": "hevc",
                    "width": 3840, "height": 2160,
                    "bit_rate": "30000000",
                    "profile": "Main 10",
                    "level": 51,
                    "color_transfer": "smpte2084",
                    "r_frame_rate": "24000/1001",
                    "disposition": { "default": 1, "attached_pic": 0 }
                },
                {
                    "codec_type": "audio",
                    "codec_name": "truehd",
                    "channels": 8,
                    "bit_rate": "5000000",
                    "tags": { "language": "eng", "title": "TrueHD Atmos 7.1" },
                    "disposition": { "default": 1 }
                },
                {
                    "codec_type": "audio",
                    "codec_name": "ac3",
                    "channels": 6,
                    "bit_rate": "640000",
                    "tags": { "language": "eng", "title": "AC3 5.1 Compatibility" },
                    "disposition": { "default": 0 }
                },
                {
                    "codec_type": "audio",
                    "codec_name": "aac",
                    "channels": 2,
                    "bit_rate": "192000",
                    "tags": { "language": "eng", "title": "Stereo Commentary" },
                    "disposition": { "default": 0 }
                },
                {
                    "codec_type": "subtitle",
                    "codec_name": "hdmv_pgs_subtitle",
                    "tags": { "language": "eng", "title": "English PGS" },
                    "disposition": { "default": 1, "forced": 0 }
                },
                {
                    "codec_type": "subtitle",
                    "codec_name": "hdmv_pgs_subtitle",
                    "tags": { "language": "eng", "title": "English Forced PGS" },
                    "disposition": { "default": 0, "forced": 1 }
                },
                {
                    "codec_type": "subtitle",
                    "codec_name": "srt",
                    "tags": { "language": "spa", "title": "Spanish" },
                    "disposition": { "default": 0, "forced": 0 }
                }
            ]
        });
        let info = parse_ffprobe_json(&json).unwrap();

        assert_eq!(info.container, "matroska,webm");
        assert!((info.duration_secs - 8523.456).abs() < 0.01);
        assert_eq!(info.bitrate, 35_000_000);

        assert_eq!(info.video_streams.len(), 1);
        assert!(info.video_streams[0].is_hdr);
        assert_eq!(info.video_streams[0].width, 3840);
        assert_eq!(info.video_streams[0].height, 2160);
        assert!((info.video_streams[0].frame_rate - 23.976).abs() < 0.01);

        assert_eq!(info.audio_streams.len(), 3);
        assert_eq!(info.audio_streams[0].channels, 8);
        assert!(info.audio_streams[0].is_default);
        assert_eq!(info.audio_streams[1].channels, 6);
        assert_eq!(info.audio_streams[2].channels, 2);

        assert_eq!(info.subtitle_streams.len(), 3);
        assert!(info.subtitle_streams[0].is_default);
        assert!(info.subtitle_streams[1].forced);
        assert_eq!(info.subtitle_streams[2].language, "spa");
    }

    #[test]
    fn test_av1_webdl_file() {
        let json = serde_json::json!({
            "format": { "format_name": "mp4", "duration": "3600", "bit_rate": "4000000" },
            "streams": [
                {
                    "codec_type": "video",
                    "codec_name": "av1",
                    "width": 1920, "height": 1080,
                    "bit_rate": "3500000",
                    "profile": "Main",
                    "level": 41,
                    "r_frame_rate": "24/1",
                    "disposition": { "attached_pic": 0 }
                },
                {
                    "codec_type": "audio",
                    "codec_name": "opus",
                    "channels": 6,
                    "bit_rate": "256000",
                    "tags": { "language": "eng" },
                    "disposition": { "default": 1 }
                }
            ]
        });
        let info = parse_ffprobe_json(&json).unwrap();
        assert_eq!(info.video_streams[0].codec, "av1");
        assert_eq!(info.audio_streams[0].codec, "opus");
        assert!(!info.video_streams[0].is_hdr);
    }
}
