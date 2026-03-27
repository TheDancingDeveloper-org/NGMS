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
    #[error("ffmpeg provisioning error: {0}")]
    Provision(String),
}

pub type StreamResult<T> = Result<T, StreamError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display_not_found() {
        let err = StreamError::NotFound("test.mkv".to_string());
        assert_eq!(err.to_string(), "media file not found: test.mkv");
    }

    #[test]
    fn test_error_display_probe() {
        let err = StreamError::Probe("ffprobe not found".to_string());
        assert_eq!(err.to_string(), "ffprobe error: ffprobe not found");
    }

    #[test]
    fn test_error_display_transcode() {
        let err = StreamError::Transcode("ffmpeg crashed".to_string());
        assert_eq!(err.to_string(), "ffmpeg error: ffmpeg crashed");
    }

    #[test]
    fn test_error_display_session() {
        let err = StreamError::Session("session not found".to_string());
        assert_eq!(err.to_string(), "session error: session not found");
    }

    #[test]
    fn test_error_display_invalid_range() {
        let err = StreamError::InvalidRange("bad range".to_string());
        assert_eq!(err.to_string(), "invalid range: bad range");
    }

    #[test]
    fn test_error_display_max_sessions() {
        let err = StreamError::MaxSessions;
        assert_eq!(err.to_string(), "max concurrent sessions exceeded");
    }

    #[test]
    fn test_error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let err: StreamError = io_err.into();
        match err {
            StreamError::Io(_) => {}
            other => panic!("expected Io variant, got: {other:?}"),
        }
    }

    #[test]
    fn test_stream_result_type() {
        let ok: StreamResult<i32> = Ok(42);
        assert_eq!(ok.unwrap(), 42);

        let err: StreamResult<i32> = Err(StreamError::NotFound("x".into()));
        assert!(err.is_err());
    }
}
