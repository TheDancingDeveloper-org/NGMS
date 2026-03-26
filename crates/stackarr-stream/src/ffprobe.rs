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

    #[test]
    fn test_parse_frame_rate() {
        assert!((parse_frame_rate("24000/1001").unwrap() - 23.976).abs() < 0.01);
        assert!((parse_frame_rate("24/1").unwrap() - 24.0).abs() < 0.01);
        assert!((parse_frame_rate("30").unwrap() - 30.0).abs() < 0.01);
        assert!(parse_frame_rate("0/0").is_none());
    }

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
}
