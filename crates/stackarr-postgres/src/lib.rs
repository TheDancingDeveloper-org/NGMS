//! Managed PostgreSQL for StackArr.
//!
//! This crate provides automatic provisioning and lifecycle management of a
//! PostgreSQL instance. It supports three modes:
//!
//! - **External**: User provides their own PostgreSQL (existing behavior).
//! - **Managed**: PostgreSQL binaries are downloaded on first run and managed
//!   as a child process.
//! - **Embedded**: PostgreSQL binaries are baked into the binary at compile time
//!   (requires the `embed` feature flag).

pub mod config;
pub mod error;
pub mod lifecycle;
pub mod provision;

pub use config::{PgMode, PgPaths};
pub use error::PostgresError;
pub use lifecycle::PostgresManager;
pub use provision::ensure_postgres;

use std::path::Path;

use error::PostgresResult;

/// Provision PostgreSQL binaries and start a managed instance.
///
/// This is the main entry point for managed/embedded modes. It:
/// 1. Ensures PostgreSQL binaries are available (download or extract).
/// 2. Initializes the data directory if needed.
/// 3. Starts PostgreSQL as a child process.
/// 4. Returns the manager handle and connection URL.
///
/// The caller must call `manager.stop()` before exiting, or the `Drop` impl
/// will attempt a best-effort synchronous shutdown.
pub async fn start_managed_postgres(
    data_dir: &Path,
    port: u16,
) -> PostgresResult<(PostgresManager, String)> {
    let paths = ensure_postgres(data_dir).await?;
    let mut manager = PostgresManager::new(paths, data_dir, port);
    let url = manager.start().await?;
    Ok((manager, url))
}
