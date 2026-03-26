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
}
