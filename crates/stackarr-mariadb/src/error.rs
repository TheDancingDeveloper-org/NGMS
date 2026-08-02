use thiserror::Error;

pub type MariaDbResult<T> = Result<T, MariaDbError>;

#[derive(Debug, Error)]
pub enum MariaDbError {
    #[error("failed to connect to MariaDB: {0}")]
    Connect(#[source] sqlx::Error),
    #[error("failed to inspect MariaDB server: {0}")]
    Inspect(#[source] sqlx::Error),
    #[error("unsupported database server `{0}`; StackArr requires MariaDB 11.4 LTS")]
    UnsupportedServer(String),
}
