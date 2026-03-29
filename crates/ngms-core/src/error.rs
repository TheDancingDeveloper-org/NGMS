use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("not found: {0}")]
    NotFound(String),

    #[error("already exists: {0}")]
    AlreadyExists(String),

    #[error("validation error: {0}")]
    Validation(String),

    #[error("configuration error: {0}")]
    Config(String),

    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("http error: {0}")]
    Http(String),

    #[error("download client error: {0}")]
    DownloadClient(String),

    #[error("indexer error: {0}")]
    Indexer(String),

    #[error("parse error: {0}")]
    Parse(String),

    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display_not_found() {
        let err = Error::NotFound("series 42".into());
        assert_eq!(err.to_string(), "not found: series 42");
    }

    #[test]
    fn test_error_display_validation() {
        let err = Error::Validation("title is required".into());
        assert_eq!(err.to_string(), "validation error: title is required");
    }

    #[test]
    fn test_error_display_config() {
        let err = Error::Config("bad toml".into());
        assert_eq!(err.to_string(), "configuration error: bad toml");
    }

    #[test]
    fn test_error_display_download_client() {
        let err = Error::DownloadClient("connection refused".into());
        assert_eq!(err.to_string(), "download client error: connection refused");
    }
}
