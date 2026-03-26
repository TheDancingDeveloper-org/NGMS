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
