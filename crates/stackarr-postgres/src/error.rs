// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

/// PostgreSQL management errors.
#[derive(Debug, thiserror::Error)]
pub enum PostgresError {
    #[error("postgres provisioning error: {0}")]
    Provision(String),
    #[error("initdb failed: {0}")]
    InitDb(String),
    #[error("postgres failed to start: {0}")]
    Start(String),
    #[error("postgres health check failed after {0}s")]
    HealthTimeout(u64),
    #[error("postgres shutdown failed: {0}")]
    Shutdown(String),
    #[error("postgres version mismatch: {0}")]
    VersionMismatch(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub type PostgresResult<T> = Result<T, PostgresError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display_provision() {
        let err = PostgresError::Provision("download failed".to_string());
        assert_eq!(
            err.to_string(),
            "postgres provisioning error: download failed"
        );
    }

    #[test]
    fn test_error_display_initdb() {
        let err = PostgresError::InitDb("locale error".to_string());
        assert_eq!(err.to_string(), "initdb failed: locale error");
    }

    #[test]
    fn test_error_display_start() {
        let err = PostgresError::Start("port in use".to_string());
        assert_eq!(err.to_string(), "postgres failed to start: port in use");
    }

    #[test]
    fn test_error_display_health_timeout() {
        let err = PostgresError::HealthTimeout(30);
        assert_eq!(err.to_string(), "postgres health check failed after 30s");
    }

    #[test]
    fn test_error_display_shutdown() {
        let err = PostgresError::Shutdown("pg_ctl failed".to_string());
        assert_eq!(err.to_string(), "postgres shutdown failed: pg_ctl failed");
    }

    #[test]
    fn test_error_display_version_mismatch() {
        let err = PostgresError::VersionMismatch("data is v16, binary is v17".to_string());
        assert_eq!(
            err.to_string(),
            "postgres version mismatch: data is v16, binary is v17"
        );
    }

    #[test]
    fn test_error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let err: PostgresError = io_err.into();
        match err {
            PostgresError::Io(_) => {}
            other => panic!("expected Io variant, got: {other:?}"),
        }
    }

    #[test]
    fn test_postgres_result_type() {
        let ok: PostgresResult<i32> = Ok(42);
        assert_eq!(ok.unwrap(), 42);

        let err: PostgresResult<i32> = Err(PostgresError::Provision("x".into()));
        assert!(err.is_err());
    }
}
