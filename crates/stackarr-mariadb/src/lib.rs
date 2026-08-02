//! MariaDB connection policy for StackArr.
//!
//! The database server is deliberately a separate process. Standard deployments
//! provide it externally; the standalone image supervises MariaDB with s6.

mod error;

use std::time::Duration;

pub use error::{MariaDbError, MariaDbResult};
use sqlx::mysql::MySqlPoolOptions;
use sqlx::{MySqlPool, Row};

pub const REQUIRED_MAJOR: u32 = 11;
pub const REQUIRED_MINOR: u32 = 4;

/// Connect using the session invariants required by the baseline schema, then
/// verify that the server is the pinned MariaDB 11.4 LTS line.
pub async fn connect(url: &str, max_connections: u32) -> MariaDbResult<MySqlPool> {
    let pool = MySqlPoolOptions::new()
        .max_connections(max_connections)
        .idle_timeout(Duration::from_secs(300))
        .max_lifetime(Duration::from_secs(1800))
        .acquire_timeout(Duration::from_secs(10))
        .after_connect(|connection, _metadata| {
            Box::pin(async move {
                sqlx::query("SET SESSION time_zone = '+00:00'")
                    .execute(&mut *connection)
                    .await?;
                Ok(())
            })
        })
        .connect(url)
        .await
        .map_err(MariaDbError::Connect)?;

    validate_server(&pool).await?;
    Ok(pool)
}

pub async fn validate_server(pool: &MySqlPool) -> MariaDbResult<String> {
    let row = sqlx::query("SELECT VERSION() AS version")
        .fetch_one(pool)
        .await
        .map_err(MariaDbError::Inspect)?;
    let version: String = row.try_get("version").map_err(MariaDbError::Inspect)?;

    match parse_mariadb_version(&version) {
        Some((REQUIRED_MAJOR, minor)) if minor == REQUIRED_MINOR => {
            tracing::info!(%version, "connected to supported MariaDB server");
            Ok(version)
        }
        _ => Err(MariaDbError::UnsupportedServer(version)),
    }
}

fn parse_mariadb_version(version: &str) -> Option<(u32, u32)> {
    if !version.to_ascii_lowercase().contains("mariadb") {
        return None;
    }
    let numeric = version
        .split('-')
        .find(|part| part.chars().next().is_some_and(|c| c.is_ascii_digit()))?;
    let mut parts = numeric.split('.');
    Some((parts.next()?.parse().ok()?, parts.next()?.parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_pinned_mariadb_version_shape() {
        assert_eq!(
            parse_mariadb_version("11.4.8-MariaDB-ubu2404"),
            Some((11, 4))
        );
    }

    #[test]
    fn rejects_mysql_and_unparseable_versions() {
        assert_eq!(parse_mariadb_version("8.0.42 MySQL Community Server"), None);
        assert_eq!(parse_mariadb_version("MariaDB development build"), None);
    }
}
