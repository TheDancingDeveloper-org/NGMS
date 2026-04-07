use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use sqlx::PgPool;
use uuid::Uuid;

use stackarr_core::config::{HwAccelConfig, StreamingConfig};

use crate::error::{StreamError, StreamResult};
use crate::ffmpeg::{self, TranscodeConfig, TranscodeJob};
use crate::types::{SessionInfo, TranscodeRequest, TranscodeResponse};

/// Detected hardware acceleration capability.
#[derive(Debug, Clone)]
pub enum DetectedAccel {
    /// Hardware accel available (vaapi, qsv, nvenc)
    Hardware { accel_type: String, device: String },
    /// No hardware accel — software only
    Software,
}

impl std::fmt::Display for DetectedAccel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Hardware { accel_type, device } => write!(f, "{accel_type} ({device})"),
            Self::Software => write!(f, "software (libx264)"),
        }
    }
}

/// Probe available hardware acceleration by running a quick ffmpeg test encode.
pub async fn probe_hwaccel(ffmpeg_path: &str, config: &HwAccelConfig) -> DetectedAccel {
    let device = config.device.as_deref().unwrap_or("/dev/dri/renderD128");

    // Order: try configured type first, then fallback chain
    let candidates: Vec<&str> = if config.enabled {
        match config.accel_type.as_str() {
            "vaapi" => vec!["vaapi", "qsv"],
            "qsv" => vec!["qsv", "vaapi"],
            "nvenc" => vec!["nvenc"],
            other => vec![other],
        }
    } else {
        // If hwaccel disabled, still probe so we can log what's available
        vec!["vaapi", "qsv"]
    };

    for accel in &candidates {
        let result = test_hwaccel(ffmpeg_path, accel, device).await;
        if result {
            tracing::info!(
                accel_type = accel,
                device,
                "hardware acceleration available"
            );
            if config.enabled {
                return DetectedAccel::Hardware {
                    accel_type: accel.to_string(),
                    device: device.to_string(),
                };
            } else {
                tracing::info!("hardware acceleration detected but disabled in config");
                return DetectedAccel::Software;
            }
        } else {
            tracing::debug!(
                accel_type = accel,
                device,
                "hardware acceleration not available"
            );
        }
    }

    tracing::info!("no hardware acceleration available — using software encoding");
    DetectedAccel::Software
}

/// Test a specific hwaccel type with a minimal ffmpeg invocation.
async fn test_hwaccel(ffmpeg_path: &str, accel_type: &str, device: &str) -> bool {
    let mut cmd = tokio::process::Command::new(ffmpeg_path);
    cmd.args(["-hide_banner", "-loglevel", "error"]);

    match accel_type {
        "vaapi" => {
            cmd.arg("-vaapi_device").arg(device);
            cmd.args(["-f", "lavfi", "-i", "color=black:s=64x64:d=0.1"]);
            cmd.args(["-vf", "format=nv12,hwupload"]);
            cmd.args(["-c:v", "h264_vaapi", "-frames:v", "1"]);
        }
        "qsv" => {
            cmd.arg("-init_hw_device")
                .arg(format!("qsv=hw,child_device={device}"));
            cmd.args(["-f", "lavfi", "-i", "color=black:s=64x64:d=0.1"]);
            cmd.args(["-vf", "hwupload=extra_hw_frames=64,format=qsv"]);
            cmd.args(["-c:v", "h264_qsv", "-frames:v", "1"]);
        }
        "nvenc" => {
            cmd.args(["-f", "lavfi", "-i", "color=black:s=64x64:d=0.1"]);
            cmd.args(["-c:v", "h264_nvenc", "-frames:v", "1"]);
        }
        _ => return false,
    }

    cmd.args(["-f", "null", "-y", "/dev/null"]);
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());

    match tokio::time::timeout(Duration::from_secs(10), cmd.status()).await {
        Ok(Ok(status)) => status.success(),
        _ => false,
    }
}

/// Internal session state.
struct Session {
    id: Uuid,
    media_file_id: i64,
    session_type: SessionType,
    transcode_jobs: Vec<TranscodeJob>,
    transcode_dir: Option<PathBuf>,
    started_at: DateTime<Utc>,
    last_activity: DateTime<Utc>,
    /// True if this session has multiple renditions (ABR).
    multi_rendition: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionType {
    Direct,
    Transcode,
}

impl std::fmt::Display for SessionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Direct => write!(f, "direct"),
            Self::Transcode => write!(f, "transcode"),
        }
    }
}

/// Manages active streaming sessions, transcode processes, and cleanup.
pub struct SessionManager {
    sessions: DashMap<Uuid, Session>,
    config: StreamingConfig,
    detected_accel: DetectedAccel,
    pool: PgPool,
}

impl SessionManager {
    pub fn new(config: StreamingConfig, detected_accel: DetectedAccel, pool: PgPool) -> Self {
        Self {
            sessions: DashMap::new(),
            config,
            detected_accel,
            pool,
        }
    }

    /// The detected hardware acceleration capability.
    pub fn detected_accel(&self) -> &DetectedAccel {
        &self.detected_accel
    }

    /// The configured transcode directory.
    pub fn transcode_dir(&self) -> PathBuf {
        self.config
            .transcode_dir
            .clone()
            .unwrap_or_else(|| PathBuf::from("/config/transcode"))
    }

    /// Access the streaming config.
    pub fn config(&self) -> &StreamingConfig {
        &self.config
    }

    /// Record a direct-play session.
    pub async fn create_direct_session(&self, media_file_id: i64) -> StreamResult<Uuid> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        // Record in DB
        sqlx::query(
            "INSERT INTO streaming_sessions (id, media_file_id, session_type, status, started_at, last_activity)
             VALUES ($1, $2, 'direct', 'active', $3, $3)",
        )
        .bind(id)
        .bind(media_file_id)
        .bind(now)
        .execute(&self.pool)
        .await?;

        self.sessions.insert(
            id,
            Session {
                id,
                media_file_id,
                session_type: SessionType::Direct,
                transcode_jobs: Vec::new(),
                transcode_dir: None,
                started_at: now,
                last_activity: now,
                multi_rendition: false,
            },
        );

        Ok(id)
    }

    /// Start a new transcode session. Returns the session ID and HLS playlist URL.
    pub async fn create_transcode_session(
        &self,
        media_file_id: i64,
        source_path: &std::path::Path,
        request: &TranscodeRequest,
    ) -> StreamResult<TranscodeResponse> {
        // Check max concurrent sessions
        let active_transcodes = self
            .sessions
            .iter()
            .filter(|s| s.value().session_type == SessionType::Transcode)
            .count();

        if active_transcodes >= self.config.max_concurrent_sessions {
            return Err(StreamError::MaxSessions);
        }

        let session_id = Uuid::new_v4();
        let session_dir = self.transcode_dir().join(session_id.to_string());
        tracing::info!(
            dir = %session_dir.display(),
            source = %source_path.display(),
            "creating transcode session directory"
        );
        tokio::fs::create_dir_all(&session_dir).await.map_err(|e| {
            tracing::error!(
                dir = %session_dir.display(),
                error = %e,
                "failed to create transcode session directory"
            );
            e
        })?;

        // Build effective hwaccel config from detected capabilities
        let (effective_hwaccel, encoder_label) = match &self.detected_accel {
            DetectedAccel::Hardware { accel_type, device } => {
                let hw = HwAccelConfig {
                    enabled: true,
                    accel_type: accel_type.clone(),
                    device: Some(device.clone()),
                };
                let label = format!("hw:{accel_type}");
                (hw, label)
            }
            DetectedAccel::Software => (
                HwAccelConfig {
                    enabled: false,
                    ..Default::default()
                },
                "software".to_string(),
            ),
        };

        // Build config with the effective hwaccel
        let mut effective_streaming_config = self.config.clone();
        effective_streaming_config.hwaccel = effective_hwaccel;

        let transcode_config = TranscodeConfig {
            source_path,
            output_dir: &session_dir,
            video_stream_index: request.video_stream_index,
            audio_stream_index: request.audio_stream_index,
            subtitle_stream_index: request.subtitle_stream_index,
            max_width: request.max_width,
            max_height: request.max_height,
            video_bitrate: request.video_bitrate,
            streaming_config: &effective_streaming_config,
        };

        tracing::info!(
            media_file_id,
            encoder = %encoder_label,
            "starting transcode"
        );

        // Try hwaccel first, fall back to software if it fails quickly
        let (job, final_encoder) = match ffmpeg::start_transcode(&transcode_config).await {
            Ok(job) => {
                // Check if ffmpeg dies within 2 seconds (hwaccel failure)
                let job = Self::verify_ffmpeg_alive(job, &session_dir, 2).await?;
                (job, encoder_label)
            }
            Err(e) if effective_streaming_config.hwaccel.enabled => {
                tracing::warn!(
                    encoder = %encoder_label,
                    error = %e,
                    "hardware transcode failed, falling back to software"
                );
                // Clean up and retry with software
                let _ = tokio::fs::remove_dir_all(&session_dir).await;
                tokio::fs::create_dir_all(&session_dir).await?;

                let mut sw_config = self.config.clone();
                sw_config.hwaccel.enabled = false;
                let sw_transcode = TranscodeConfig {
                    source_path,
                    output_dir: &session_dir,
                    video_stream_index: request.video_stream_index,
                    audio_stream_index: request.audio_stream_index,
                    subtitle_stream_index: request.subtitle_stream_index,
                    max_width: request.max_width,
                    max_height: request.max_height,
                    video_bitrate: request.video_bitrate,
                    streaming_config: &sw_config,
                };
                let job = ffmpeg::start_transcode(&sw_transcode).await?;
                tracing::info!("software transcode fallback started");
                (job, "software (fallback)".to_string())
            }
            Err(e) => return Err(e),
        };

        let now = Utc::now();
        let playlist_url = format!("/api/v1/stream/{media_file_id}/hls/{session_id}/master.m3u8");

        // Record in DB
        sqlx::query(
            "INSERT INTO streaming_sessions (id, media_file_id, session_type, status, started_at, last_activity, transcode_dir)
             VALUES ($1, $2, 'transcode', 'active', $3, $3, $4)",
        )
        .bind(session_id)
        .bind(media_file_id)
        .bind(now)
        .bind(session_dir.to_string_lossy().as_ref())
        .execute(&self.pool)
        .await?;

        self.sessions.insert(
            session_id,
            Session {
                id: session_id,
                media_file_id,
                session_type: SessionType::Transcode,
                transcode_jobs: vec![job],
                transcode_dir: Some(session_dir),
                started_at: now,
                last_activity: now,
                multi_rendition: false,
            },
        );

        tracing::info!(
            media_file_id,
            encoder = %final_encoder,
            %session_id,
            "transcode session created"
        );

        Ok(TranscodeResponse {
            session_id,
            playlist_url,
            encoder: final_encoder,
        })
    }

    /// Wait briefly to verify ffmpeg didn't crash immediately (e.g. hwaccel failure).
    /// If it dies within `wait_secs`, return an error so the caller can retry.
    async fn verify_ffmpeg_alive(
        mut job: TranscodeJob,
        session_dir: &std::path::Path,
        wait_secs: u64,
    ) -> StreamResult<TranscodeJob> {
        tokio::time::sleep(Duration::from_secs(wait_secs)).await;

        if let Some(status) = job.try_wait() {
            // ffmpeg exited already — read stderr for diagnostics
            let stderr = job.take_stderr().await;
            let msg = if !stderr.is_empty() {
                format!("ffmpeg exited with {status}: {stderr}")
            } else {
                format!("ffmpeg exited with {status}")
            };
            // Clean up empty session dir
            let _ = tokio::fs::remove_dir_all(session_dir).await;
            return Err(StreamError::Transcode(msg));
        }

        Ok(job)
    }

    /// Create a multi-rendition transcode session for adaptive bitrate streaming.
    /// Spawns one ffmpeg process per quality tier.
    pub async fn create_multi_rendition_session(
        &self,
        media_file_id: i64,
        source_path: &std::path::Path,
        request: &TranscodeRequest,
        tiers: &[stackarr_core::config::QualityTierConfig],
    ) -> StreamResult<TranscodeResponse> {
        // Count active ffmpeg processes across all sessions
        let active_processes: usize = self
            .sessions
            .iter()
            .map(|s| s.value().transcode_jobs.len())
            .sum();
        let new_processes = tiers.len();

        // Use max_concurrent_sessions * 4 as the process limit
        let max_processes = self.config.max_concurrent_sessions * 4;
        if active_processes + new_processes > max_processes {
            tracing::warn!(
                active = active_processes,
                requested = new_processes,
                limit = max_processes,
                "too many ffmpeg processes"
            );
            return Err(StreamError::MaxSessions);
        }

        let session_id = Uuid::new_v4();
        let session_dir = self.transcode_dir().join(session_id.to_string());
        tracing::info!(
            dir = %session_dir.display(),
            source = %source_path.display(),
            renditions = tiers.len(),
            tiers = ?tiers.iter().map(|t| &t.name).collect::<Vec<_>>(),
            "creating multi-rendition transcode session"
        );
        tokio::fs::create_dir_all(&session_dir).await?;

        let multi_job = ffmpeg::start_multi_rendition_transcode(
            source_path,
            &session_dir,
            request.video_stream_index,
            request.audio_stream_index,
            request.subtitle_stream_index,
            tiers,
            &self.config,
        )
        .await?;

        let now = Utc::now();
        let playlist_url = format!("/api/v1/stream/{media_file_id}/hls/{session_id}/master.m3u8");

        // Record in DB
        sqlx::query(
            "INSERT INTO streaming_sessions (id, media_file_id, session_type, status, started_at, last_activity, transcode_dir)
             VALUES ($1, $2, 'transcode', 'active', $3, $3, $4)",
        )
        .bind(session_id)
        .bind(media_file_id)
        .bind(now)
        .bind(session_dir.to_string_lossy().as_ref())
        .execute(&self.pool)
        .await?;

        let encoder = match &self.detected_accel {
            DetectedAccel::Hardware { accel_type, .. } => format!("hw:{accel_type}"),
            DetectedAccel::Software => "software".to_string(),
        };

        let tier_names: Vec<_> = tiers.iter().map(|t| t.name.as_str()).collect();
        tracing::info!(
            media_file_id,
            encoder = %encoder,
            %session_id,
            tiers = ?tier_names,
            "multi-rendition session created"
        );

        self.sessions.insert(
            session_id,
            Session {
                id: session_id,
                media_file_id,
                session_type: SessionType::Transcode,
                transcode_jobs: multi_job.jobs,
                transcode_dir: Some(session_dir),
                started_at: now,
                last_activity: now,
                multi_rendition: true,
            },
        );

        Ok(TranscodeResponse {
            session_id,
            playlist_url,
            encoder: format!("{encoder} ({})", tier_names.join(", ")),
        })
    }

    /// Whether a session is multi-rendition.
    pub fn is_multi_rendition(&self, session_id: Uuid) -> bool {
        self.sessions
            .get(&session_id)
            .is_some_and(|s| s.multi_rendition)
    }

    /// Get the transcode directory for a session (for serving HLS).
    pub fn get_session_dir(&self, session_id: Uuid) -> Option<PathBuf> {
        self.sessions
            .get(&session_id)
            .and_then(|s| s.transcode_dir.clone())
    }

    /// Verify a session exists and belongs to the specified media file.
    pub fn validate_session(&self, session_id: Uuid, media_file_id: i64) -> bool {
        self.sessions
            .get(&session_id)
            .is_some_and(|s| s.media_file_id == media_file_id)
    }

    /// Update last activity timestamp for a session.
    pub fn heartbeat(&self, session_id: Uuid) {
        if let Some(mut s) = self.sessions.get_mut(&session_id) {
            s.last_activity = Utc::now();
        }
    }

    /// List all active sessions.
    pub fn list_sessions(&self) -> Vec<SessionInfo> {
        self.sessions
            .iter()
            .map(|entry| {
                let s = entry.value();
                SessionInfo {
                    session_id: s.id,
                    media_file_id: s.media_file_id,
                    session_type: s.session_type.to_string(),
                    status: "active".to_string(),
                    started_at: s.started_at.to_rfc3339(),
                    last_activity: s.last_activity.to_rfc3339(),
                    transcode_progress: None,
                    video_codec: None,
                    audio_codec: None,
                    resolution: None,
                }
            })
            .collect()
    }

    /// Stop a session: kill ffmpeg, clean up temp dir, remove from DB.
    pub async fn stop_session(&self, session_id: Uuid) -> StreamResult<()> {
        if let Some((_, mut session)) = self.sessions.remove(&session_id) {
            // Kill all transcode processes
            for job in &mut session.transcode_jobs {
                job.kill().await;
            }

            // Clean up temp directory
            if let Some(ref dir) = session.transcode_dir
                && dir.exists()
                && let Err(e) = tokio::fs::remove_dir_all(dir).await
            {
                tracing::warn!(dir = %dir.display(), error = %e, "failed to clean up transcode dir");
            }

            // Update DB
            let _ = sqlx::query("UPDATE streaming_sessions SET status = 'completed' WHERE id = $1")
                .bind(session_id)
                .execute(&self.pool)
                .await;
        }

        Ok(())
    }

    /// Spawn a background task that cleans up idle sessions.
    pub fn spawn_cleanup_task(self: &Arc<Self>) {
        let mgr = Arc::clone(self);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            loop {
                interval.tick().await;
                mgr.cleanup_idle_sessions().await;
            }
        });
    }

    async fn cleanup_idle_sessions(&self) {
        let idle_threshold = Utc::now() - chrono::Duration::minutes(5);
        let mut to_remove = Vec::new();

        for entry in self.sessions.iter() {
            if entry.value().last_activity < idle_threshold {
                to_remove.push(*entry.key());
            }
        }

        for session_id in to_remove {
            tracing::info!(%session_id, "cleaning up idle streaming session");
            if let Err(e) = self.stop_session(session_id).await {
                tracing::warn!(%session_id, error = %e, "failed to stop idle session");
            }
        }
    }
}
