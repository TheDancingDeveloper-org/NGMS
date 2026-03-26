/// Streaming-specific errors.
#[derive(Debug, thiserror::Error)]
pub enum StreamError {
    #[error("media file not found: {0}")]
    NotFound(String),
    #[error("ffprobe error: {0}")]
    Probe(String),
    #[error("ffmpeg error: {0}")]
    Transcode(String),
    #[error("session error: {0}")]
    Session(String),
    #[error("invalid range: {0}")]
    InvalidRange(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("max concurrent sessions exceeded")]
    MaxSessions,
}

pub type StreamResult<T> = Result<T, StreamError>;
