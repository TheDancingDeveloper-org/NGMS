use std::path::{Path, PathBuf};

use crate::config::PgPaths;
use crate::error::{PostgresError, PostgresResult};

/// Manages a PostgreSQL instance as a child process.
///
/// Handles the full lifecycle: initdb, configuration, startup, health checking,
/// and graceful shutdown. The postgres process runs as a direct child (not a daemon)
/// so Rust owns its lifetime.
pub struct PostgresManager {
    paths: PgPaths,
    pgdata: PathBuf,
    port: u16,
    pg_user: String,
    pg_database: String,
    child: Option<tokio::process::Child>,
}

impl PostgresManager {
    /// Create a new manager. Does not start PostgreSQL yet.
    pub fn new(paths: PgPaths, data_dir: &Path, port: u16) -> Self {
        Self {
            paths,
            pgdata: data_dir.join("postgres").join("data"),
            port,
            pg_user: "stackarr".to_string(),
            pg_database: "stackarr".to_string(),
            child: None,
        }
    }

    /// Full startup sequence: crash recovery → initdb → configure → start → health check → create db.
    /// Returns the connection URL on success.
    pub async fn start(&mut self) -> PostgresResult<String> {
        // 1. Handle stale postmaster.pid (crash recovery)
        self.recover_from_crash().await?;

        // 2. Check version compatibility
        self.check_version().await?;

        // 3. Initialize data directory if needed (first run)
        let first_run = !self.pgdata.join("PG_VERSION").exists();
        if first_run {
            self.init_db().await?;
        }

        // 4. Write configuration files
        self.write_config().await?;

        // 5. Start postgres as a child process
        self.spawn_postgres().await?;

        // 6. Wait for postgres to accept connections
        self.wait_for_ready().await?;

        // 7. Create database if first run
        if first_run {
            self.create_database().await?;
        }

        let url = self.connection_url();
        tracing::info!(%url, port = self.port, "managed PostgreSQL is ready");
        Ok(url)
    }

    /// Graceful shutdown via pg_ctl stop.
    pub async fn stop(&mut self) -> PostgresResult<()> {
        if self.child.is_none() {
            return Ok(());
        }

        tracing::info!("stopping managed PostgreSQL");

        let status = tokio::process::Command::new(self.pg_ctl_path())
            .args(["stop", "-D"])
            .arg(&self.pgdata)
            .args(["-m", "fast", "-w", "-t", "30"])
            .envs(self.pg_env())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .status()
            .await
            .map_err(|e| PostgresError::Shutdown(format!("failed to run pg_ctl stop: {e}")))?;

        // Wait for child process to exit
        if let Some(ref mut child) = self.child {
            let _ = child.wait().await;
        }
        self.child = None;

        if status.success() {
            tracing::info!("managed PostgreSQL stopped");
            Ok(())
        } else {
            Err(PostgresError::Shutdown(format!(
                "pg_ctl stop exited with code {}",
                status.code().unwrap_or(-1)
            )))
        }
    }

    /// Check if PostgreSQL is accepting connections.
    pub async fn health_check(&self) -> bool {
        let result = tokio::process::Command::new(self.bin_path("pg_isready"))
            .args(["-h", "127.0.0.1", "-p", &self.port.to_string()])
            .envs(self.pg_env())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .await;

        matches!(result, Ok(status) if status.success())
    }

    /// Returns the connection URL for this managed instance.
    pub fn connection_url(&self) -> String {
        format!(
            "postgresql://{}@127.0.0.1:{}/{}",
            self.pg_user, self.port, self.pg_database
        )
    }

    // --- Private methods ---

    /// Detect and clean up stale postmaster.pid from unclean shutdown.
    async fn recover_from_crash(&self) -> PostgresResult<()> {
        let pid_file = self.pgdata.join("postmaster.pid");
        if !pid_file.exists() {
            return Ok(());
        }

        let content = tokio::fs::read_to_string(&pid_file)
            .await
            .unwrap_or_default();

        let pid: Option<u32> = content
            .lines()
            .next()
            .and_then(|line| line.trim().parse().ok());

        let process_alive = if let Some(pid) = pid {
            is_process_alive(pid)
        } else {
            false
        };

        if process_alive {
            // Postgres is actually running — try a clean stop
            tracing::warn!(pid = ?pid, "found running PostgreSQL from previous session, stopping it");
            let _ = tokio::process::Command::new(self.pg_ctl_path())
                .args(["stop", "-D"])
                .arg(&self.pgdata)
                .args(["-m", "fast", "-w", "-t", "10"])
                .envs(self.pg_env())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .await;
        } else {
            // Stale pid file — remove it
            tracing::warn!("removing stale postmaster.pid (previous unclean shutdown)");
            let _ = tokio::fs::remove_file(&pid_file).await;
        }

        Ok(())
    }

    /// Check that provisioned PG version matches the data directory version.
    async fn check_version(&self) -> PostgresResult<()> {
        let version_file = self
            .pgdata
            .parent()
            .unwrap_or(&self.pgdata)
            .join("version.json");
        let pg_version_file = self.pgdata.join("PG_VERSION");

        // No data directory yet — nothing to check
        if !pg_version_file.exists() {
            return Ok(());
        }

        // Read the PG_VERSION from data directory (contains just the major version, e.g. "17")
        let data_major: u32 = tokio::fs::read_to_string(&pg_version_file)
            .await
            .unwrap_or_default()
            .trim()
            .parse()
            .unwrap_or(0);

        // Read the provisioned version
        let provisioned_major = if version_file.exists() {
            let content = tokio::fs::read_to_string(&version_file)
                .await
                .unwrap_or_default();
            serde_json::from_str::<crate::config::PgVersionInfo>(&content)
                .map(|v| v.pg_major)
                .unwrap_or(0)
        } else {
            // No version.json but binaries exist — detect from binary
            let version_str = detect_binary_major(&self.paths.bin_dir).await;
            version_str
        };

        if data_major != 0 && provisioned_major != 0 && data_major != provisioned_major {
            return Err(PostgresError::VersionMismatch(format!(
                "data directory is PostgreSQL {data_major} but provisioned binaries are \
                 PostgreSQL {provisioned_major}. Run pg_upgrade or re-provision with the \
                 matching version."
            )));
        }

        Ok(())
    }

    /// Initialize the PostgreSQL data directory.
    async fn init_db(&self) -> PostgresResult<()> {
        tracing::info!(pgdata = %self.pgdata.display(), "initializing PostgreSQL data directory");

        tokio::fs::create_dir_all(&self.pgdata)
            .await
            .map_err(|e| PostgresError::InitDb(format!("failed to create PGDATA: {e}")))?;

        let output = tokio::process::Command::new(self.bin_path("initdb"))
            .args(["-D"])
            .arg(&self.pgdata)
            .args([
                "-U",
                &self.pg_user,
                "--auth=trust",
                "--encoding=UTF-8",
                "--locale=C",
                "--no-instructions",
            ])
            .envs(self.pg_env())
            .output()
            .await
            .map_err(|e| PostgresError::InitDb(format!("failed to run initdb: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(PostgresError::InitDb(format!("initdb failed: {stderr}")));
        }

        tracing::info!("PostgreSQL data directory initialized");
        Ok(())
    }

    /// Write postgresql.conf and pg_hba.conf tuned for embedded use.
    async fn write_config(&self) -> PostgresResult<()> {
        // postgresql.conf — tuned for single-user embedded use
        let postgresql_conf = format!(
            r#"# StackArr managed PostgreSQL configuration
# DO NOT EDIT — regenerated on each startup

listen_addresses = '127.0.0.1'
port = {port}
max_connections = 50

# Memory (conservative — shared with StackArr process)
shared_buffers = 128MB
work_mem = 4MB
maintenance_work_mem = 64MB
effective_cache_size = 256MB

# WAL (minimal — no replication needed)
wal_level = minimal
max_wal_senders = 0
max_wal_size = 256MB

# Reliability
fsync = on
synchronous_commit = on

# Logging (stderr only — captured by StackArr)
log_destination = 'stderr'
logging_collector = off
log_min_messages = warning
log_min_error_statement = error

# Performance
random_page_cost = 1.1
effective_io_concurrency = 200
"#,
            port = self.port,
        );

        // pg_hba.conf — trust auth for localhost only
        let pg_hba_conf = "\
# StackArr managed PostgreSQL HBA configuration
# Trust authentication for localhost only — no external access
local   all   all               trust
host    all   all   127.0.0.1/32   trust
host    all   all   ::1/128        trust
";

        tokio::fs::write(self.pgdata.join("postgresql.conf"), postgresql_conf)
            .await
            .map_err(|e| PostgresError::Start(format!("failed to write postgresql.conf: {e}")))?;

        tokio::fs::write(self.pgdata.join("pg_hba.conf"), pg_hba_conf)
            .await
            .map_err(|e| PostgresError::Start(format!("failed to write pg_hba.conf: {e}")))?;

        Ok(())
    }

    /// Spawn the postgres process as a direct child.
    async fn spawn_postgres(&mut self) -> PostgresResult<()> {
        tracing::info!(port = self.port, "starting PostgreSQL");

        let child = tokio::process::Command::new(self.bin_path("postgres"))
            .arg("-D")
            .arg(&self.pgdata)
            .envs(self.pg_env())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| PostgresError::Start(format!("failed to spawn postgres: {e}")))?;

        self.child = Some(child);
        Ok(())
    }

    /// Wait for PostgreSQL to accept connections, with exponential backoff.
    async fn wait_for_ready(&self) -> PostgresResult<()> {
        let max_wait = std::time::Duration::from_secs(30);
        let start = std::time::Instant::now();
        let mut interval = std::time::Duration::from_millis(100);

        while start.elapsed() < max_wait {
            if self.health_check().await {
                return Ok(());
            }

            // Check if child process has exited (crashed)
            // We can't await on the child here without &mut, so just check health
            tokio::time::sleep(interval).await;
            interval = std::cmp::min(interval * 2, std::time::Duration::from_secs(2));
        }

        Err(PostgresError::HealthTimeout(max_wait.as_secs()))
    }

    /// Create the stackarr database (first run only).
    async fn create_database(&self) -> PostgresResult<()> {
        tracing::info!(database = %self.pg_database, "creating database");

        // Use psql to create the database (connecting as the superuser created by initdb)
        let output = tokio::process::Command::new(self.bin_path("psql"))
            .args([
                "-h",
                "127.0.0.1",
                "-p",
                &self.port.to_string(),
                "-U",
                &self.pg_user,
                "-d",
                "postgres",
                "-c",
                &format!("CREATE DATABASE {} ENCODING 'UTF8';", self.pg_database),
            ])
            .envs(self.pg_env())
            .output()
            .await
            .map_err(|e| PostgresError::Start(format!("failed to run psql: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Ignore "already exists" — this is fine
            if !stderr.contains("already exists") {
                return Err(PostgresError::Start(format!(
                    "failed to create database: {stderr}"
                )));
            }
        }

        Ok(())
    }

    /// Build the path to a PostgreSQL binary.
    fn bin_path(&self, name: &str) -> PathBuf {
        let bin_name = if cfg!(target_os = "windows") {
            format!("{name}.exe")
        } else {
            name.to_string()
        };
        self.paths.bin_dir.join(bin_name)
    }

    /// Path to pg_ctl binary.
    fn pg_ctl_path(&self) -> PathBuf {
        self.bin_path("pg_ctl")
    }

    /// Environment variables needed for portable PostgreSQL.
    fn pg_env(&self) -> Vec<(String, String)> {
        let mut env = vec![(
            "PGDATA".to_string(),
            self.pgdata.to_string_lossy().to_string(),
        )];

        // LD_LIBRARY_PATH for Linux portable builds
        #[cfg(target_os = "linux")]
        {
            env.push((
                "LD_LIBRARY_PATH".to_string(),
                self.paths.lib_dir.to_string_lossy().to_string(),
            ));
        }

        // DYLD_LIBRARY_PATH for macOS
        #[cfg(target_os = "macos")]
        {
            env.push((
                "DYLD_LIBRARY_PATH".to_string(),
                self.paths.lib_dir.to_string_lossy().to_string(),
            ));
        }

        env
    }
}

/// Drop safety: best-effort synchronous shutdown if the manager is dropped
/// without calling stop() first.
impl Drop for PostgresManager {
    fn drop(&mut self) {
        if self.child.is_some() {
            tracing::warn!(
                "PostgresManager dropped without stop() — attempting synchronous shutdown"
            );
            let pg_ctl = self.pg_ctl_path();
            let pgdata = self.pgdata.clone();
            let lib_dir = self.paths.lib_dir.clone();

            let mut cmd = std::process::Command::new(pg_ctl);
            cmd.args(["stop", "-D"]).arg(&pgdata).args(["-m", "fast"]);

            #[cfg(target_os = "linux")]
            {
                cmd.env("LD_LIBRARY_PATH", &lib_dir);
            }
            #[cfg(target_os = "macos")]
            {
                cmd.env("DYLD_LIBRARY_PATH", &lib_dir);
            }

            cmd.env("PGDATA", &pgdata);

            let _ = cmd.status();
        }
    }
}

/// Check if a process with the given PID is alive.
/// Uses /proc on Linux, kill(0) on other Unix, tasklist on Windows.
fn is_process_alive(pid: u32) -> bool {
    #[cfg(target_os = "linux")]
    {
        std::path::Path::new(&format!("/proc/{pid}")).exists()
    }
    #[cfg(all(unix, not(target_os = "linux")))]
    {
        // kill(pid, 0) checks existence without sending a signal.
        // Safety: signal 0 has no side effects.
        unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
    }
    #[cfg(windows)]
    {
        // Use tasklist to check if PID exists (no extra crate dependency)
        std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).contains(&pid.to_string()))
            .unwrap_or(false)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = pid;
        false
    }
}

/// Detect the PostgreSQL major version from the postgres binary.
async fn detect_binary_major(bin_dir: &Path) -> u32 {
    let postgres = if cfg!(target_os = "windows") {
        bin_dir.join("postgres.exe")
    } else {
        bin_dir.join("postgres")
    };

    let output = tokio::process::Command::new(&postgres)
        .arg("--version")
        .output()
        .await;

    match output {
        Ok(out) if out.status.success() => {
            let version_line = String::from_utf8_lossy(&out.stdout);
            // e.g. "postgres (PostgreSQL) 17.4" — extract "17"
            version_line
                .split_whitespace()
                .last()
                .and_then(|v| v.split('.').next())
                .and_then(|v| v.parse().ok())
                .unwrap_or(0)
        }
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_paths() -> PgPaths {
        PgPaths {
            bin_dir: PathBuf::from("/tmp/test-pg/bin"),
            lib_dir: PathBuf::from("/tmp/test-pg/lib"),
            share_dir: PathBuf::from("/tmp/test-pg/share"),
        }
    }

    #[test]
    fn test_connection_url() {
        let mgr = PostgresManager::new(test_paths(), Path::new("/tmp/test-data"), 5433);
        assert_eq!(
            mgr.connection_url(),
            "postgresql://stackarr@127.0.0.1:5433/stackarr"
        );
    }

    #[test]
    fn test_connection_url_custom_port() {
        let mgr = PostgresManager::new(test_paths(), Path::new("/tmp/test-data"), 5500);
        assert_eq!(
            mgr.connection_url(),
            "postgresql://stackarr@127.0.0.1:5500/stackarr"
        );
    }

    #[test]
    fn test_bin_path() {
        let mgr = PostgresManager::new(test_paths(), Path::new("/tmp/test-data"), 5433);
        let path = mgr.bin_path("pg_ctl");
        if cfg!(target_os = "windows") {
            assert!(path.to_string_lossy().ends_with("pg_ctl.exe"));
        } else {
            assert!(path.to_string_lossy().ends_with("pg_ctl"));
        }
    }

    #[test]
    fn test_pg_env_includes_pgdata() {
        let mgr = PostgresManager::new(test_paths(), Path::new("/tmp/test-data"), 5433);
        let env = mgr.pg_env();
        assert!(env.iter().any(|(k, _)| k == "PGDATA"));
    }

    #[cfg(unix)]
    #[test]
    fn test_is_process_alive_self() {
        // Our own process should be alive
        let pid = std::process::id();
        assert!(is_process_alive(pid));
    }

    #[test]
    fn test_is_process_alive_nonexistent() {
        // PID 999999999 should not exist
        assert!(!is_process_alive(999_999_999));
    }
}
