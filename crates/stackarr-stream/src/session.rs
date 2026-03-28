use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use sqlx::PgPool;
use uuid::Uuid;

use stackarr_core::config::StreamingConfig;

use crate::error::{StreamError, StreamResult};
use crate::ffmpeg::{self, TranscodeConfig, TranscodeJob};
use crate::types::{SessionInfo, TranscodeRequest, TranscodeResponse};

/// Internal session state.
struct Session {
    id: Uuid,
    media_file_id: i64,
    session_type: SessionType,
    transcode_job: Option<TranscodeJob>,
    transcode_dir: Option<PathBuf>,
    started_at: DateTime<Utc>,
    last_activity: DateTime<Utc>,
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
    pool: PgPool,
}

impl SessionManager {
    pub fn new(config: StreamingConfig, pool: PgPool) -> Self {
        Self {
            sessions: DashMap::new(),
            config,
            pool,
        }
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
                transcode_job: None,
                transcode_dir: None,
                started_at: now,
                last_activity: now,
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

        let transcode_config = TranscodeConfig {
            source_path,
            output_dir: &session_dir,
            video_stream_index: request.video_stream_index,
            audio_stream_index: request.audio_stream_index,
            subtitle_stream_index: request.subtitle_stream_index,
            max_width: request.max_width,
            max_height: request.max_height,
            video_bitrate: request.video_bitrate,
            streaming_config: &self.config,
        };

        let job = ffmpeg::start_transcode(&transcode_config).await?;

        let now = Utc::now();
        let playlist_url = format!(
            "/api/v1/stream/{media_file_id}/hls/{session_id}/master.m3u8"
        );

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
                transcode_job: Some(job),
                transcode_dir: Some(session_dir),
                started_at: now,
                last_activity: now,
            },
        );

        Ok(TranscodeResponse {
            session_id,
            playlist_url,
        })
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
            // Kill transcode process if running
            if let Some(ref mut job) = session.transcode_job {
                job.kill().await;
            }

            // Clean up temp directory
            if let Some(ref dir) = session.transcode_dir {
                if dir.exists() {
                    if let Err(e) = tokio::fs::remove_dir_all(dir).await {
                        tracing::warn!(dir = %dir.display(), error = %e, "failed to clean up transcode dir");
                    }
                }
            }

            // Update DB
            let _ = sqlx::query(
                "UPDATE streaming_sessions SET status = 'completed' WHERE id = $1",
            )
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
