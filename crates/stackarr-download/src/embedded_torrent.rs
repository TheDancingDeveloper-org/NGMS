use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use librtbit::api::{ApiTorrentListOpts, TorrentIdOrHash};

use crate::client::{
    ClientStatus, DownloadClient, DownloadItem, DownloadItemStatus, DownloadProtocol, GrabRequest,
};

/// Wrapper around the embedded librtbit torrent session.
pub struct EmbeddedTorrentClient {
    api: Arc<librtbit::Api>,
    /// When set, every HTTP(S) .torrent grab is fetched here before being
    /// handed to librtbit. Magnet URIs are skipped silently (nothing to
    /// archive). Writes are best-effort; failures never block the grab.
    archive_dir: Option<PathBuf>,
}

impl EmbeddedTorrentClient {
    pub fn new(api: Arc<librtbit::Api>) -> Self {
        Self {
            api,
            archive_dir: None,
        }
    }

    pub fn with_archive_dir(mut self, dir: Option<PathBuf>) -> Self {
        self.archive_dir = dir;
        self
    }
}

#[async_trait]
impl DownloadClient for EmbeddedTorrentClient {
    fn name(&self) -> &str {
        "Embedded Torrent Client"
    }

    fn protocol(&self) -> DownloadProtocol {
        DownloadProtocol::Torrent
    }

    async fn add(&self, request: &GrabRequest) -> anyhow::Result<String> {
        let url = request.download_url.as_str();
        let is_magnet = url.starts_with("magnet:");

        // If archiving is enabled and this is an HTTP(S) .torrent, fetch the
        // bytes ourselves so we can both persist them and hand them to
        // librtbit — avoiding a double download.
        let add = if !is_magnet && self.archive_dir.is_some() {
            match reqwest::get(url).await {
                Ok(resp) if resp.status().is_success() => match resp.bytes().await {
                    Ok(bytes) => {
                        if let Some(ref dir) = self.archive_dir {
                            let file_path = dir.join(torrent_archive_filename(&request.title));
                            if let Err(e) = persist_archive(dir, &file_path, &bytes).await {
                                tracing::warn!(
                                    error = %e,
                                    path = %file_path.display(),
                                    "archive: failed to save .torrent (continuing)"
                                );
                            } else {
                                tracing::debug!(path = %file_path.display(), "archive: saved .torrent");
                            }
                        }
                        librtbit::AddTorrent::TorrentFileBytes(bytes)
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "archive: failed to read .torrent bytes, falling back to URL");
                        librtbit::AddTorrent::from_url(url)
                    }
                },
                Ok(resp) => {
                    tracing::warn!(status = %resp.status(), "archive: HTTP error fetching .torrent, falling back to URL");
                    librtbit::AddTorrent::from_url(url)
                }
                Err(e) => {
                    tracing::warn!(error = %e, "archive: HTTP fetch failed, falling back to URL");
                    librtbit::AddTorrent::from_url(url)
                }
            }
        } else {
            librtbit::AddTorrent::from_url(url)
        };

        let opts = librtbit::AddTorrentOptions {
            overwrite: true,
            ..Default::default()
        };

        let resp = self
            .api
            .api_add_torrent(add, Some(opts))
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        // Post-add: rename the archive file to include the info_hash so the
        // cleanup scan can dedupe and the user can correlate archive → session.
        if let (Some(dir), false) = (&self.archive_dir, is_magnet) {
            let info_hash = resp.details.info_hash.clone();
            let old = dir.join(torrent_archive_filename(&request.title));
            let new = dir.join(format!(
                "{}-{info_hash}.torrent",
                sanitize_archive_title(&request.title)
            ));
            if old != new
                && tokio::fs::try_exists(&old).await.unwrap_or(false)
                && let Err(e) = tokio::fs::rename(&old, &new).await
            {
                tracing::debug!(error = %e, "archive: could not rename torrent with info_hash");
            }
        }

        // Use the torrent ID or info_hash as the download identifier
        let id = match resp.id {
            Some(id) => id.to_string(),
            None => resp.details.info_hash.clone(),
        };
        Ok(id)
    }

    async fn get_items(&self) -> anyhow::Result<Vec<DownloadItem>> {
        let list = self.api.api_torrent_list_ext(ApiTorrentListOpts {
            with_stats: true,
            ..Default::default()
        });

        Ok(list
            .torrents
            .into_iter()
            .map(|t| {
                let (total_size, remaining_size, status) = match t.stats {
                    Some(ref stats) => {
                        let remaining = stats.total_bytes.saturating_sub(stats.progress_bytes);
                        let status = if stats.finished {
                            // Seeding if finished and still live
                            if matches!(stats.state, librtbit::TorrentStatsState::Live) {
                                DownloadItemStatus::Seeding
                            } else {
                                DownloadItemStatus::Completed
                            }
                        } else {
                            match stats.state {
                                librtbit::TorrentStatsState::Live => {
                                    DownloadItemStatus::Downloading
                                }
                                librtbit::TorrentStatsState::Paused => DownloadItemStatus::Paused,
                                librtbit::TorrentStatsState::Error => DownloadItemStatus::Failed,
                                librtbit::TorrentStatsState::Initializing => {
                                    DownloadItemStatus::Queued
                                }
                            }
                        };
                        (stats.total_bytes, remaining, status)
                    }
                    None => (0, 0, DownloadItemStatus::Queued),
                };

                DownloadItem {
                    download_id: t
                        .id
                        .map(|id| id.to_string())
                        .unwrap_or_else(|| t.info_hash.clone()),
                    title: t.name.unwrap_or_else(|| t.info_hash.clone()),
                    status,
                    total_size,
                    remaining_size,
                    output_path: Some(std::path::PathBuf::from(&t.output_folder)),
                    category: t.category,
                    can_move_files: true,
                    can_be_removed: true,
                    protocol: DownloadProtocol::Torrent,
                    error_message: None,
                }
            })
            .collect())
    }

    async fn remove(&self, id: &str, delete_data: bool) -> anyhow::Result<()> {
        let idx = parse_torrent_id(id)?;
        if delete_data {
            self.api
                .api_torrent_action_delete(idx)
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
        } else {
            self.api
                .api_torrent_action_forget(idx)
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
        }
        Ok(())
    }

    async fn pause(&self, id: &str) -> anyhow::Result<()> {
        let idx = parse_torrent_id(id)?;
        self.api
            .api_torrent_action_pause(idx)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(())
    }

    async fn resume(&self, id: &str) -> anyhow::Result<()> {
        let idx = parse_torrent_id(id)?;
        self.api
            .api_torrent_action_start(idx)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(())
    }

    async fn test(&self) -> anyhow::Result<()> {
        // Verify the session is still alive (not cancelled/stopped)
        let session = self.api.session();
        if session.cancellation_token().is_cancelled() {
            anyhow::bail!("torrent session has been stopped");
        }

        // Verify we have a listening address (TCP listener is up)
        if session.listen_addr().is_none() {
            anyhow::bail!("torrent session has no listen address — TCP listener may have failed");
        }

        // Verify we can retrieve session stats (session internals are functional)
        let stats = self.api.api_session_stats();
        if stats.uptime_seconds == 0 {
            anyhow::bail!("torrent session reports zero uptime");
        }

        Ok(())
    }

    async fn status(&self) -> anyhow::Result<ClientStatus> {
        Ok(ClientStatus {
            name: "Embedded Torrent Client".into(),
            protocol: DownloadProtocol::Torrent,
            version: env!("CARGO_PKG_VERSION").into(),
            is_connected: true,
        })
    }
}

fn parse_torrent_id(id: &str) -> anyhow::Result<TorrentIdOrHash> {
    TorrentIdOrHash::parse(id).map_err(|e| anyhow::anyhow!("invalid torrent id: {e}"))
}

fn sanitize_archive_title(title: &str) -> String {
    let sanitized: String = title
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, ' ' | '.' | '-' | '_' | '(' | ')') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed: String = sanitized
        .trim()
        .trim_matches('.')
        .chars()
        .take(120)
        .collect();
    if trimmed.is_empty() {
        "torrent".to_string()
    } else {
        trimmed
    }
}

fn torrent_archive_filename(title: &str) -> String {
    format!("{}.torrent", sanitize_archive_title(title))
}

async fn persist_archive(
    dir: &std::path::Path,
    file_path: &std::path::Path,
    bytes: &[u8],
) -> std::io::Result<()> {
    tokio::fs::create_dir_all(dir).await?;
    tokio::fs::write(file_path, bytes).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_illegal_chars() {
        assert_eq!(sanitize_archive_title("a/b\\c"), "a_b_c");
    }

    #[test]
    fn caps_length() {
        let s = sanitize_archive_title(&"x".repeat(500));
        assert!(s.len() <= 120);
    }

    #[test]
    fn empty_falls_back() {
        assert_eq!(sanitize_archive_title(""), "torrent");
        assert_eq!(sanitize_archive_title("   "), "torrent");
    }
}
