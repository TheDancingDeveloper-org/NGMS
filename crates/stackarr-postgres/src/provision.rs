// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

use std::path::{Path, PathBuf};

use crate::config::{PgPaths, PgVersionInfo};
use crate::error::{PostgresError, PostgresResult};

/// PostgreSQL major version to provision.
const PG_MAJOR: u32 = 17;

/// Ensure PostgreSQL binaries are available, downloading if necessary.
///
/// Strategy:
/// 1. Check well-known system paths (package manager installs).
/// 2. Check `{data_dir}/postgres/bin/` for previously provisioned binaries.
/// 3. Download portable PostgreSQL build to `{data_dir}/postgres/`.
/// 4. (If `embed` feature) Extract from embedded archive instead of downloading.
pub async fn ensure_postgres(data_dir: &Path) -> PostgresResult<PgPaths> {
    // Step 1: Check well-known system paths
    for base in well_known_paths() {
        let paths = PgPaths {
            bin_dir: base.join("bin"),
            lib_dir: base.join("lib"),
            share_dir: base.join("share"),
        };
        if is_pg_executable(&paths.bin_dir).await {
            tracing::info!(path = %base.display(), "found system PostgreSQL");
            return Ok(paths);
        }
    }

    // Step 2: Check data_dir/postgres/
    let pg_dir = data_dir.join("postgres");
    let local_paths = PgPaths {
        bin_dir: pg_dir.join("bin"),
        lib_dir: pg_dir.join("lib"),
        share_dir: pg_dir.join("share"),
    };

    if is_pg_executable(&local_paths.bin_dir).await {
        tracing::info!(path = %pg_dir.display(), "using previously provisioned PostgreSQL");
        return Ok(local_paths);
    }

    // Step 3/4: Provision (download or extract embedded)
    tracing::info!("PostgreSQL not found — provisioning");
    tokio::fs::create_dir_all(&pg_dir).await.map_err(|e| {
        PostgresError::Provision(format!("failed to create {}: {e}", pg_dir.display()))
    })?;

    #[cfg(feature = "embed")]
    {
        extract_embedded(&pg_dir).await?;
    }
    #[cfg(not(feature = "embed"))]
    {
        let url = download_url()?;
        download_and_extract(url, &pg_dir).await?;
    }

    // Verify the provisioned binaries work
    if !is_pg_executable(&local_paths.bin_dir).await {
        return Err(PostgresError::Provision(
            "provisioned PostgreSQL binaries are not executable".into(),
        ));
    }

    // Write version metadata
    let version_str = detect_pg_version(&local_paths.bin_dir).await;
    let version_info = PgVersionInfo {
        pg_major: PG_MAJOR,
        pg_version: version_str,
        provisioned_at: chrono::Utc::now(),
        #[cfg(feature = "embed")]
        source: "embedded".to_string(),
        #[cfg(not(feature = "embed"))]
        source: "managed".to_string(),
    };
    let version_json = serde_json::to_string_pretty(&version_info)
        .map_err(|e| PostgresError::Provision(format!("failed to serialize version info: {e}")))?;
    tokio::fs::write(pg_dir.join("version.json"), version_json)
        .await
        .map_err(|e| PostgresError::Provision(format!("failed to write version.json: {e}")))?;

    tracing::info!(path = %pg_dir.display(), "PostgreSQL provisioned successfully");
    Ok(local_paths)
}

/// Check if a pg_isready binary exists and is executable in the given bin directory.
async fn is_pg_executable(bin_dir: &Path) -> bool {
    let pg_isready = bin_dir.join(pg_binary_name("pg_isready"));
    if !pg_isready.exists() {
        return false;
    }

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tokio::process::Command::new(&pg_isready)
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status(),
    )
    .await;

    matches!(result, Ok(Ok(status)) if status.success())
}

/// Detect the PostgreSQL version from the postgres binary.
async fn detect_pg_version(bin_dir: &Path) -> String {
    let postgres = bin_dir.join(pg_binary_name("postgres"));
    let output = tokio::process::Command::new(&postgres)
        .arg("--version")
        .output()
        .await;

    match output {
        Ok(out) if out.status.success() => {
            let version_line = String::from_utf8_lossy(&out.stdout);
            // e.g. "postgres (PostgreSQL) 17.4"
            version_line
                .split_whitespace()
                .last()
                .unwrap_or("unknown")
                .to_string()
        }
        _ => "unknown".to_string(),
    }
}

/// Platform-specific binary name (appends .exe on Windows).
fn pg_binary_name(name: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

/// Well-known system paths where PostgreSQL might be installed.
fn well_known_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    #[cfg(target_os = "linux")]
    {
        // Common Linux package manager locations
        paths.push(PathBuf::from("/usr/lib/postgresql/17"));
        paths.push(PathBuf::from("/usr/lib/postgresql/16"));
        paths.push(PathBuf::from("/usr/pgsql-17")); // RHEL/CentOS
        paths.push(PathBuf::from("/usr/pgsql-16"));
    }

    #[cfg(target_os = "macos")]
    {
        // Homebrew (ARM and Intel)
        paths.push(PathBuf::from("/opt/homebrew/opt/postgresql@17"));
        paths.push(PathBuf::from("/usr/local/opt/postgresql@17"));
        paths.push(PathBuf::from("/opt/homebrew/opt/postgresql@16"));
        paths.push(PathBuf::from("/usr/local/opt/postgresql@16"));
        // Postgres.app
        paths.push(PathBuf::from(
            "/Applications/Postgres.app/Contents/Versions/17",
        ));
        paths.push(PathBuf::from(
            "/Applications/Postgres.app/Contents/Versions/16",
        ));
    }

    #[cfg(target_os = "windows")]
    {
        paths.push(PathBuf::from("C:\\Program Files\\PostgreSQL\\17"));
        paths.push(PathBuf::from("C:\\Program Files\\PostgreSQL\\16"));
    }

    paths
}

/// Platform-specific download URL for portable PostgreSQL builds.
/// Uses EDB (EnterpriseDB) portable builds which are relocatable.
#[cfg(any(test, not(feature = "embed")))]
fn download_url() -> PostgresResult<&'static str> {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        Ok("https://get.enterprisedb.com/postgresql/postgresql-17.4-1-linux-x64-binaries.tar.gz")
    }

    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        Ok("https://get.enterprisedb.com/postgresql/postgresql-17.4-1-linux-arm64-binaries.tar.gz")
    }

    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        Ok("https://get.enterprisedb.com/postgresql/postgresql-17.4-1-osx-x64-binaries.tar.gz")
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        Ok("https://get.enterprisedb.com/postgresql/postgresql-17.4-1-osx-arm64-binaries.tar.gz")
    }

    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        Ok("https://get.enterprisedb.com/postgresql/postgresql-17.4-1-windows-x64-binaries.zip")
    }

    #[cfg(not(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "windows", target_arch = "x86_64"),
    )))]
    {
        Err(PostgresError::Provision(
            "automatic PostgreSQL download is not supported on this platform — \
             please install PostgreSQL manually and use database.mode = \"external\""
                .into(),
        ))
    }
}

/// Download the archive from `url` and extract PostgreSQL binaries into `target_dir`.
#[cfg(not(feature = "embed"))]
async fn download_and_extract(url: &str, target_dir: &Path) -> PostgresResult<()> {
    let archive_path = target_dir.join("pg-download.tmp");

    // Stream download to temp file
    tracing::info!(%url, "downloading PostgreSQL");
    let response = reqwest::get(url)
        .await
        .map_err(|e| PostgresError::Provision(format!("download failed: {e}")))?;

    if !response.status().is_success() {
        return Err(PostgresError::Provision(format!(
            "download returned HTTP {}",
            response.status()
        )));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| PostgresError::Provision(format!("failed to read download: {e}")))?;

    let size_mb = bytes.len() / (1024 * 1024);
    tracing::info!(size_mb, "download complete, extracting");

    tokio::fs::write(&archive_path, &bytes)
        .await
        .map_err(|e| PostgresError::Provision(format!("failed to write archive: {e}")))?;

    // Extract platform-specific
    if cfg!(target_os = "windows") {
        extract_zip(&archive_path, target_dir).await?;
    } else {
        extract_tar_gz(&archive_path, target_dir).await?;
    }

    // Clean up archive
    let _ = tokio::fs::remove_file(&archive_path).await;

    Ok(())
}

/// Extract a tar.gz archive, pulling out the pgsql/ directory contents to target_dir.
/// EDB portable builds have structure: pgsql/bin/, pgsql/lib/, pgsql/share/, etc.
async fn extract_tar_gz(archive: &Path, target_dir: &Path) -> PostgresResult<()> {
    let archive_str = archive.to_string_lossy();

    // EDB portable builds extract to pgsql/ — strip that prefix
    let status = tokio::process::Command::new("tar")
        .args(["xzf", &archive_str])
        .arg("--strip-components=1")
        .arg("-C")
        .arg(target_dir)
        .status()
        .await
        .map_err(|e| PostgresError::Provision(format!("failed to run tar: {e}")))?;

    if !status.success() {
        return Err(PostgresError::Provision(format!(
            "tar extraction failed with exit code: {}",
            status.code().unwrap_or(-1)
        )));
    }

    // Ensure execute permissions on key binaries
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let bin_dir = target_dir.join("bin");
        if let Ok(mut entries) = tokio::fs::read_dir(&bin_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if path.is_file() {
                    let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755));
                }
            }
        }
    }

    Ok(())
}

/// Extract a zip archive on Windows.
async fn extract_zip(archive: &Path, target_dir: &Path) -> PostgresResult<()> {
    let extract_tmp = target_dir.join("_extract_tmp");

    let status = tokio::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "Expand-Archive -Path '{}' -DestinationPath '{}' -Force",
                archive.to_string_lossy(),
                extract_tmp.to_string_lossy()
            ),
        ])
        .status()
        .await
        .map_err(|e| PostgresError::Provision(format!("failed to run powershell: {e}")))?;

    if !status.success() {
        return Err(PostgresError::Provision("zip extraction failed".into()));
    }

    // EDB zips extract to pgsql/ — move contents up
    let pgsql_dir = extract_tmp.join("pgsql");
    let source = if pgsql_dir.exists() {
        pgsql_dir
    } else {
        extract_tmp.clone()
    };

    // Move bin/, lib/, share/ to target_dir
    for dir_name in ["bin", "lib", "share", "include"] {
        let src = source.join(dir_name);
        let dst = target_dir.join(dir_name);
        if src.exists() {
            if dst.exists() {
                let _ = tokio::fs::remove_dir_all(&dst).await;
            }
            tokio::fs::rename(&src, &dst)
                .await
                .or_else(|_| {
                    // Cross-device move: fall back to copy
                    std::fs::rename(&src, &dst)
                })
                .map_err(|e| PostgresError::Provision(format!("failed to move {dir_name}: {e}")))?;
        }
    }

    // Clean up extraction temp
    let _ = tokio::fs::remove_dir_all(&extract_tmp).await;

    Ok(())
}

/// Extract PostgreSQL from embedded archive (requires `embed` feature).
#[cfg(feature = "embed")]
async fn extract_embedded(target_dir: &Path) -> PostgresResult<()> {
    use rust_embed::Embed;

    #[derive(Embed)]
    #[folder = "pg-binaries/"]
    struct PgBinaries;

    // Find the embedded archive file
    let archive_name = PgBinaries::iter()
        .find(|name| name.ends_with(".tar.gz") || name.ends_with(".zip"))
        .ok_or_else(|| {
            PostgresError::Provision(
                "no embedded PostgreSQL archive found — rebuild with pg-binaries/ populated".into(),
            )
        })?;

    let archive_data = PgBinaries::get(&archive_name)
        .ok_or_else(|| PostgresError::Provision("failed to read embedded archive".into()))?;

    let archive_path = target_dir.join("pg-embedded.tmp");
    tokio::fs::write(&archive_path, &archive_data.data)
        .await
        .map_err(|e| PostgresError::Provision(format!("failed to write embedded archive: {e}")))?;

    if cfg!(target_os = "windows") {
        extract_zip(&archive_path, target_dir).await?;
    } else {
        extract_tar_gz(&archive_path, target_dir).await?;
    }

    let _ = tokio::fs::remove_file(&archive_path).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pg_binary_name_unix() {
        if cfg!(unix) {
            assert_eq!(pg_binary_name("pg_isready"), "pg_isready");
            assert_eq!(pg_binary_name("postgres"), "postgres");
        }
    }

    #[test]
    fn test_pg_binary_name_windows() {
        if cfg!(target_os = "windows") {
            assert_eq!(pg_binary_name("pg_isready"), "pg_isready.exe");
            assert_eq!(pg_binary_name("postgres"), "postgres.exe");
        }
    }

    #[test]
    fn test_well_known_paths_not_empty() {
        let paths = well_known_paths();
        // Should have at least some paths on any supported platform
        if cfg!(any(
            target_os = "linux",
            target_os = "macos",
            target_os = "windows",
        )) {
            assert!(!paths.is_empty());
        }
    }

    #[test]
    fn test_download_url_returns_ok() {
        if cfg!(any(
            all(target_os = "linux", target_arch = "x86_64"),
            all(target_os = "linux", target_arch = "aarch64"),
            all(target_os = "macos", target_arch = "x86_64"),
            all(target_os = "macos", target_arch = "aarch64"),
            all(target_os = "windows", target_arch = "x86_64"),
        )) {
            let url = download_url().unwrap();
            assert!(url.starts_with("https://"));
            assert!(url.contains("postgresql"));
        }
    }
}
