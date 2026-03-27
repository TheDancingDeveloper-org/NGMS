use std::path::{Path, PathBuf};

use crate::error::{StreamError, StreamResult};

/// Resolved absolute paths to ffmpeg and ffprobe binaries.
pub struct FfmpegPaths {
    pub ffmpeg: String,
    pub ffprobe: String,
}

/// Ensure ffmpeg and ffprobe are available, downloading static builds if necessary.
///
/// Strategy:
/// 1. If the configured paths are executable, use them (Docker / system install / user override).
/// 2. If binaries exist in `{data_dir}/ffmpeg/`, use those (previous download).
/// 3. Download platform-specific static builds to `{data_dir}/ffmpeg/`.
pub async fn ensure_ffmpeg(
    configured_ffmpeg: &str,
    configured_ffprobe: &str,
    data_dir: &Path,
) -> StreamResult<FfmpegPaths> {
    // Step 1: Check configured paths
    if is_executable(configured_ffmpeg).await && is_executable(configured_ffprobe).await {
        tracing::info!(
            ffmpeg = configured_ffmpeg,
            ffprobe = configured_ffprobe,
            "using system ffmpeg"
        );
        return Ok(FfmpegPaths {
            ffmpeg: configured_ffmpeg.to_string(),
            ffprobe: configured_ffprobe.to_string(),
        });
    }

    // Step 2: Check data_dir/ffmpeg/
    let local_dir = data_dir.join("ffmpeg");
    let (ffmpeg_bin, ffprobe_bin) = binary_names();
    let local_ffmpeg = local_dir.join(ffmpeg_bin);
    let local_ffprobe = local_dir.join(ffprobe_bin);

    let local_ffmpeg_str = local_ffmpeg.to_string_lossy().to_string();
    let local_ffprobe_str = local_ffprobe.to_string_lossy().to_string();

    if is_executable(&local_ffmpeg_str).await && is_executable(&local_ffprobe_str).await {
        tracing::info!(path = %local_dir.display(), "using previously downloaded ffmpeg");
        return Ok(FfmpegPaths {
            ffmpeg: local_ffmpeg_str,
            ffprobe: local_ffprobe_str,
        });
    }

    // Step 3: Download
    tracing::info!("ffmpeg/ffprobe not found — downloading static build");
    let url = download_url()?;

    tokio::fs::create_dir_all(&local_dir)
        .await
        .map_err(|e| StreamError::Provision(format!("failed to create {}: {e}", local_dir.display())))?;

    download_and_extract(url, &local_dir).await?;

    // Verify the downloaded binaries work
    if !is_executable(&local_ffmpeg_str).await {
        return Err(StreamError::Provision(format!(
            "downloaded ffmpeg at {} is not executable",
            local_ffmpeg.display()
        )));
    }
    if !is_executable(&local_ffprobe_str).await {
        return Err(StreamError::Provision(format!(
            "downloaded ffprobe at {} is not executable",
            local_ffprobe.display()
        )));
    }

    tracing::info!(path = %local_dir.display(), "ffmpeg provisioned successfully");
    Ok(FfmpegPaths {
        ffmpeg: local_ffmpeg_str,
        ffprobe: local_ffprobe_str,
    })
}

/// Check if a binary is executable by running `{path} -version` with a timeout.
async fn is_executable(path: &str) -> bool {
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tokio::process::Command::new(path)
            .arg("-version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status(),
    )
    .await;

    matches!(result, Ok(Ok(status)) if status.success())
}

/// Platform-specific binary names.
fn binary_names() -> (&'static str, &'static str) {
    if cfg!(target_os = "windows") {
        ("ffmpeg.exe", "ffprobe.exe")
    } else {
        ("ffmpeg", "ffprobe")
    }
}

/// Platform-specific download URL for static ffmpeg builds.
fn download_url() -> StreamResult<&'static str> {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        Ok("https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-linux64-gpl.tar.xz")
    }

    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        Ok("https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-linuxarm64-gpl.tar.xz")
    }

    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        Ok("https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip")
    }

    #[cfg(not(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "windows", target_arch = "x86_64"),
    )))]
    {
        Err(StreamError::Provision(
            "automatic ffmpeg download is not supported on this platform — please install ffmpeg manually".into(),
        ))
    }
}

/// Download the archive from `url` and extract ffmpeg/ffprobe binaries into `target_dir`.
async fn download_and_extract(url: &str, target_dir: &Path) -> StreamResult<()> {
    let archive_path = target_dir.join("ffmpeg-download.tmp");

    // Stream download to temp file
    tracing::info!(%url, "downloading ffmpeg");
    let response = reqwest::get(url)
        .await
        .map_err(|e| StreamError::Provision(format!("download failed: {e}")))?;

    if !response.status().is_success() {
        return Err(StreamError::Provision(format!(
            "download returned HTTP {}",
            response.status()
        )));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| StreamError::Provision(format!("failed to read download: {e}")))?;

    let size_mb = bytes.len() / (1024 * 1024);
    tracing::info!(size_mb, "download complete, extracting");

    tokio::fs::write(&archive_path, &bytes)
        .await
        .map_err(|e| StreamError::Provision(format!("failed to write archive: {e}")))?;

    // Extract platform-specific
    if cfg!(target_os = "windows") {
        extract_zip(&archive_path, target_dir).await?;
    } else {
        extract_tar_xz(&archive_path, target_dir).await?;
    }

    // Clean up archive
    let _ = tokio::fs::remove_file(&archive_path).await;

    Ok(())
}

/// Extract a tar.xz archive on Linux, pulling out just ffmpeg and ffprobe to target_dir.
async fn extract_tar_xz(archive: &Path, target_dir: &Path) -> StreamResult<()> {
    // Use system tar — universally available on Linux and handles xz natively
    let status = tokio::process::Command::new("tar")
        .args(["xf", &archive.to_string_lossy()])
        .arg("--strip-components=2")
        .arg("--wildcards")
        .args(["*/bin/ffmpeg", "*/bin/ffprobe"])
        .arg("-C")
        .arg(target_dir)
        .status()
        .await
        .map_err(|e| StreamError::Provision(format!("failed to run tar: {e}")))?;

    if !status.success() {
        return Err(StreamError::Provision(format!(
            "tar extraction failed with exit code: {}",
            status.code().unwrap_or(-1)
        )));
    }

    // Ensure execute permissions
    let (ffmpeg_bin, ffprobe_bin) = binary_names();
    for bin in [ffmpeg_bin, ffprobe_bin] {
        let path = target_dir.join(bin);
        if path.exists() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755));
            }
        }
    }

    Ok(())
}

/// Extract a zip archive on Windows, pulling out just ffmpeg.exe and ffprobe.exe to target_dir.
async fn extract_zip(archive: &Path, target_dir: &Path) -> StreamResult<()> {
    // Use PowerShell Expand-Archive, then find and move the binaries
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
        .map_err(|e| StreamError::Provision(format!("failed to run powershell: {e}")))?;

    if !status.success() {
        return Err(StreamError::Provision("zip extraction failed".into()));
    }

    // Find and move ffmpeg.exe and ffprobe.exe from nested directories
    let (ffmpeg_bin, ffprobe_bin) = binary_names();
    for bin_name in [ffmpeg_bin, ffprobe_bin] {
        let found = find_file_recursive(&extract_tmp, bin_name).await;
        match found {
            Some(src) => {
                let dest = target_dir.join(bin_name);
                // Try rename first (fast, same filesystem), fall back to copy
                if tokio::fs::rename(&src, &dest).await.is_err() {
                    tokio::fs::copy(&src, &dest)
                        .await
                        .map_err(|e| {
                            StreamError::Provision(format!("failed to move {bin_name}: {e}"))
                        })?;
                }
            }
            None => {
                return Err(StreamError::Provision(format!(
                    "{bin_name} not found in extracted archive"
                )));
            }
        }
    }

    // Clean up extraction temp
    let _ = tokio::fs::remove_dir_all(&extract_tmp).await;

    Ok(())
}

/// Recursively find a file by name in a directory.
async fn find_file_recursive(dir: &Path, name: &str) -> Option<PathBuf> {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let mut entries = match tokio::fs::read_dir(&current).await {
            Ok(e) => e,
            Err(_) => continue,
        };
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.file_name().and_then(|n| n.to_str()) == Some(name) {
                return Some(path);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_is_executable_with_known_binary() {
        // "echo" or "true" should be available on all unix systems
        if cfg!(unix) {
            assert!(is_executable("true").await);
        }
    }

    #[tokio::test]
    async fn test_is_executable_nonexistent() {
        assert!(!is_executable("nonexistent_binary_xyz_12345").await);
    }

    #[test]
    fn test_binary_names() {
        let (ffmpeg, ffprobe) = binary_names();
        assert!(ffmpeg.starts_with("ffmpeg"));
        assert!(ffprobe.starts_with("ffprobe"));
    }

    #[test]
    fn test_download_url_returns_ok() {
        // Should succeed on CI/dev machines (linux x86_64 or windows x86_64)
        if cfg!(any(
            all(target_os = "linux", target_arch = "x86_64"),
            all(target_os = "linux", target_arch = "aarch64"),
            all(target_os = "windows", target_arch = "x86_64"),
        )) {
            let url = download_url().unwrap();
            assert!(url.starts_with("https://"));
        }
    }

    #[test]
    fn test_provision_error_display() {
        let err = StreamError::Provision("test error".to_string());
        assert_eq!(err.to_string(), "ffmpeg provisioning error: test error");
    }
}
