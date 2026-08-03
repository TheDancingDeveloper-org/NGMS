// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

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

    #[test]
    fn test_error_display_already_exists() {
        let err = Error::AlreadyExists("series 'The Office'".into());
        assert_eq!(err.to_string(), "already exists: series 'The Office'");
    }

    #[test]
    fn test_error_display_http() {
        let err = Error::Http("timeout".into());
        assert_eq!(err.to_string(), "http error: timeout");
    }

    #[test]
    fn test_error_display_indexer() {
        let err = Error::Indexer("rate limited".into());
        assert_eq!(err.to_string(), "indexer error: rate limited");
    }

    #[test]
    fn test_error_display_parse() {
        let err = Error::Parse("invalid episode format".into());
        assert_eq!(err.to_string(), "parse error: invalid episode format");
    }

    #[test]
    fn test_error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let err: Error = io_err.into();
        assert!(err.to_string().contains("file missing"));
    }

    #[test]
    fn test_error_from_serde_json() {
        let json_err = serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
        let err: Error = json_err.into();
        assert!(err.to_string().contains("serialization error"));
    }

    #[test]
    fn test_error_from_anyhow() {
        let anyhow_err = anyhow::anyhow!("something went wrong");
        let err: Error = anyhow_err.into();
        assert_eq!(err.to_string(), "something went wrong");
    }
}
