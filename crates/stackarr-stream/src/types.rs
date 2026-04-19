use serde::{Deserialize, Serialize};

/// Complete media information extracted by ffprobe.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaInfo {
    pub container: String,
    pub duration_secs: f64,
    pub bitrate: u64,
    pub video_streams: Vec<VideoStream>,
    pub audio_streams: Vec<AudioStream>,
    pub subtitle_streams: Vec<SubtitleStream>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoStream {
    pub index: usize,
    pub codec: String,
    pub width: u32,
    pub height: u32,
    pub bitrate: u64,
    pub profile: String,
    pub level: u32,
    pub is_hdr: bool,
    pub is_dolby_vision: bool,
    pub color_transfer: String,
    pub frame_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioStream {
    pub index: usize,
    pub codec: String,
    pub channels: u32,
    pub language: String,
    pub title: String,
    pub bitrate: u64,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleStream {
    pub index: usize,
    pub codec: String,
    pub language: String,
    pub title: String,
    pub forced: bool,
    pub is_default: bool,
}

/// Transcode request from the client.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscodeRequest {
    #[serde(default)]
    pub video_stream_index: usize,
    #[serde(default)]
    pub audio_stream_index: usize,
    pub subtitle_stream_index: Option<usize>,
    pub max_width: Option<u32>,
    pub max_height: Option<u32>,
    pub video_bitrate: Option<u64>,
}

/// Session info returned to clients.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub session_id: uuid::Uuid,
    pub media_file_id: i64,
    pub session_type: String,
    pub status: String,
    pub started_at: String,
    pub last_activity: String,
    pub transcode_progress: Option<f32>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub resolution: Option<String>,
}

/// Response when a transcode session is created.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscodeResponse {
    pub session_id: uuid::Uuid,
    pub playlist_url: String,
    /// Encoder used: "hw:vaapi", "hw:qsv", "software", or "software (fallback)"
    pub encoder: String,
}

/// A quality tier available for a specific media file.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityTier {
    pub name: String,
    pub max_width: u32,
    pub max_height: u32,
    /// Video bitrate in bits per second.
    pub video_bitrate: u64,
    /// Audio bitrate in bits per second.
    pub audio_bitrate: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── MediaInfo serde roundtrip ─────────────────────────────────────

    #[test]
    fn media_info_serde_roundtrip() {
        let info = MediaInfo {
            container: "matroska,webm".to_string(),
            duration_secs: 5400.5,
            bitrate: 8_000_000,
            video_streams: vec![VideoStream {
                index: 0,
                codec: "hevc".to_string(),
                width: 3840,
                height: 2160,
                bitrate: 7_000_000,
                profile: "Main 10".to_string(),
                level: 51,
                is_hdr: true,
                is_dolby_vision: false,
                color_transfer: "smpte2084".to_string(),
                frame_rate: 23.976,
            }],
            audio_streams: vec![AudioStream {
                index: 0,
                codec: "truehd".to_string(),
                channels: 8,
                language: "eng".to_string(),
                title: "TrueHD Atmos 7.1".to_string(),
                bitrate: 4_000_000,
                is_default: true,
            }],
            subtitle_streams: vec![SubtitleStream {
                index: 0,
                codec: "ass".to_string(),
                language: "jpn".to_string(),
                title: "Full Subtitles".to_string(),
                forced: false,
                is_default: false,
            }],
        };

        let json = serde_json::to_string(&info).unwrap();
        let deserialized: MediaInfo = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.container, "matroska,webm");
        assert!((deserialized.duration_secs - 5400.5).abs() < 0.01);
        assert_eq!(deserialized.bitrate, 8_000_000);
        assert_eq!(deserialized.video_streams.len(), 1);
        assert_eq!(deserialized.video_streams[0].codec, "hevc");
        assert!(deserialized.video_streams[0].is_hdr);
        assert_eq!(deserialized.audio_streams[0].channels, 8);
        assert_eq!(deserialized.subtitle_streams[0].language, "jpn");
    }

    #[test]
    fn media_info_camel_case_keys() {
        let info = MediaInfo {
            container: "mp4".to_string(),
            duration_secs: 120.0,
            bitrate: 5000,
            video_streams: vec![VideoStream {
                index: 0,
                codec: "h264".to_string(),
                width: 1920,
                height: 1080,
                bitrate: 4500,
                profile: "High".to_string(),
                level: 41,
                is_hdr: false,
                is_dolby_vision: false,
                color_transfer: "bt709".to_string(),
                frame_rate: 24.0,
            }],
            audio_streams: vec![],
            subtitle_streams: vec![],
        };

        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"durationSecs\""));
        assert!(json.contains("\"videoStreams\""));
        assert!(json.contains("\"audioStreams\""));
        assert!(json.contains("\"subtitleStreams\""));
        assert!(json.contains("\"isHdr\""));
        assert!(json.contains("\"frameRate\""));
        assert!(json.contains("\"isDefault\"") == false); // no audio streams
    }

    #[test]
    fn media_info_empty_streams() {
        let info = MediaInfo {
            container: "avi".to_string(),
            duration_secs: 0.0,
            bitrate: 0,
            video_streams: vec![],
            audio_streams: vec![],
            subtitle_streams: vec![],
        };

        let json = serde_json::to_string(&info).unwrap();
        let deserialized: MediaInfo = serde_json::from_str(&json).unwrap();
        assert!(deserialized.video_streams.is_empty());
        assert!(deserialized.audio_streams.is_empty());
        assert!(deserialized.subtitle_streams.is_empty());
    }

    #[test]
    fn media_info_multiple_streams() {
        let info = MediaInfo {
            container: "mkv".to_string(),
            duration_secs: 7200.0,
            bitrate: 20_000_000,
            video_streams: vec![
                VideoStream {
                    index: 0,
                    codec: "hevc".to_string(),
                    width: 3840,
                    height: 2160,
                    bitrate: 18_000_000,
                    profile: "Main 10".to_string(),
                    level: 51,
                    is_hdr: true,
                    is_dolby_vision: false,
                    color_transfer: "smpte2084".to_string(),
                    frame_rate: 23.976,
                },
                VideoStream {
                    index: 1,
                    codec: "h264".to_string(),
                    width: 1920,
                    height: 1080,
                    bitrate: 8_000_000,
                    profile: "High".to_string(),
                    level: 41,
                    is_hdr: false,
                    is_dolby_vision: false,
                    color_transfer: "bt709".to_string(),
                    frame_rate: 24.0,
                },
            ],
            audio_streams: vec![
                AudioStream {
                    index: 0,
                    codec: "truehd".to_string(),
                    channels: 8,
                    language: "eng".to_string(),
                    title: "TrueHD 7.1".to_string(),
                    bitrate: 4_000_000,
                    is_default: true,
                },
                AudioStream {
                    index: 1,
                    codec: "aac".to_string(),
                    channels: 2,
                    language: "eng".to_string(),
                    title: "Stereo".to_string(),
                    bitrate: 192_000,
                    is_default: false,
                },
                AudioStream {
                    index: 2,
                    codec: "ac3".to_string(),
                    channels: 6,
                    language: "fra".to_string(),
                    title: "French 5.1".to_string(),
                    bitrate: 640_000,
                    is_default: false,
                },
            ],
            subtitle_streams: vec![
                SubtitleStream {
                    index: 0,
                    codec: "srt".to_string(),
                    language: "eng".to_string(),
                    title: "English".to_string(),
                    forced: false,
                    is_default: true,
                },
                SubtitleStream {
                    index: 1,
                    codec: "srt".to_string(),
                    language: "eng".to_string(),
                    title: "English (Forced)".to_string(),
                    forced: true,
                    is_default: false,
                },
            ],
        };

        let json = serde_json::to_string(&info).unwrap();
        let de: MediaInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(de.video_streams.len(), 2);
        assert_eq!(de.audio_streams.len(), 3);
        assert_eq!(de.subtitle_streams.len(), 2);
        assert_eq!(de.video_streams[1].codec, "h264");
        assert_eq!(de.audio_streams[2].language, "fra");
        assert!(de.subtitle_streams[1].forced);
    }

    // ── TranscodeRequest deserialization ───────────────────────────────

    #[test]
    fn transcode_request_defaults() {
        let json = r#"{}"#;
        let req: TranscodeRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.video_stream_index, 0);
        assert_eq!(req.audio_stream_index, 0);
        assert!(req.subtitle_stream_index.is_none());
        assert!(req.max_width.is_none());
        assert!(req.max_height.is_none());
        assert!(req.video_bitrate.is_none());
    }

    #[test]
    fn transcode_request_with_all_fields() {
        let json = r#"{
            "videoStreamIndex": 1,
            "audioStreamIndex": 2,
            "subtitleStreamIndex": 0,
            "maxWidth": 1920,
            "maxHeight": 1080,
            "videoBitrate": 5000000
        }"#;
        let req: TranscodeRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.video_stream_index, 1);
        assert_eq!(req.audio_stream_index, 2);
        assert_eq!(req.subtitle_stream_index, Some(0));
        assert_eq!(req.max_width, Some(1920));
        assert_eq!(req.max_height, Some(1080));
        assert_eq!(req.video_bitrate, Some(5_000_000));
    }

    #[test]
    fn transcode_request_partial_fields() {
        let json = r#"{"maxWidth": 1280}"#;
        let req: TranscodeRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.video_stream_index, 0);
        assert_eq!(req.audio_stream_index, 0);
        assert_eq!(req.max_width, Some(1280));
        assert!(req.max_height.is_none());
    }

    // ── SessionInfo serialization ─────────────────────────────────────

    #[test]
    fn session_info_serialize_camel_case() {
        let info = SessionInfo {
            session_id: uuid::Uuid::nil(),
            media_file_id: 42,
            session_type: "transcode".to_string(),
            status: "active".to_string(),
            started_at: "2024-01-01T00:00:00Z".to_string(),
            last_activity: "2024-01-01T00:05:00Z".to_string(),
            transcode_progress: Some(0.5),
            video_codec: Some("h264".to_string()),
            audio_codec: Some("aac".to_string()),
            resolution: Some("1920x1080".to_string()),
        };

        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"sessionId\""));
        assert!(json.contains("\"mediaFileId\""));
        assert!(json.contains("\"sessionType\""));
        assert!(json.contains("\"startedAt\""));
        assert!(json.contains("\"lastActivity\""));
        assert!(json.contains("\"transcodeProgress\""));
        assert!(json.contains("\"videoCodec\""));
        assert!(json.contains("\"audioCodec\""));
    }

    #[test]
    fn session_info_null_optionals() {
        let info = SessionInfo {
            session_id: uuid::Uuid::nil(),
            media_file_id: 1,
            session_type: "direct".to_string(),
            status: "active".to_string(),
            started_at: "2024-01-01T00:00:00Z".to_string(),
            last_activity: "2024-01-01T00:00:00Z".to_string(),
            transcode_progress: None,
            video_codec: None,
            audio_codec: None,
            resolution: None,
        };

        let json = serde_json::to_string(&info).unwrap();
        let val: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(val["transcodeProgress"].is_null());
        assert!(val["videoCodec"].is_null());
        assert!(val["audioCodec"].is_null());
        assert!(val["resolution"].is_null());
    }

    // ── TranscodeResponse serialization ───────────────────────────────

    #[test]
    fn transcode_response_serialize() {
        let id = uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let resp = TranscodeResponse {
            session_id: id,
            playlist_url: "/api/v1/stream/1/hls/550e8400-e29b-41d4-a716-446655440000/master.m3u8"
                .to_string(),
            encoder: "hw:vaapi".to_string(),
        };

        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"sessionId\""));
        assert!(json.contains("\"playlistUrl\""));
        assert!(json.contains("550e8400"));
        assert!(json.contains("master.m3u8"));
    }
}
