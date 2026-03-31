use std::sync::Arc;

use async_trait::async_trait;
use librtbit::api::{ApiTorrentListOpts, TorrentIdOrHash};

use crate::client::{
    ClientStatus, DownloadClient, DownloadItem, DownloadItemStatus, DownloadProtocol, GrabRequest,
};

/// Wrapper around the embedded librtbit torrent session.
pub struct EmbeddedTorrentClient {
    api: Arc<librtbit::Api>,
}

impl EmbeddedTorrentClient {
    pub fn new(api: Arc<librtbit::Api>) -> Self {
        Self { api }
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
        let add = librtbit::AddTorrent::from_url(&request.download_url);
        let opts = librtbit::AddTorrentOptions {
            overwrite: true,
            ..Default::default()
        };

        let resp = self
            .api
            .api_add_torrent(add, Some(opts))
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;

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
                                librtbit::TorrentStatsState::Live => DownloadItemStatus::Downloading,
                                librtbit::TorrentStatsState::Paused => DownloadItemStatus::Paused,
                                librtbit::TorrentStatsState::Error => DownloadItemStatus::Failed,
                                librtbit::TorrentStatsState::Initializing => DownloadItemStatus::Queued,
                            }
                        };
                        (stats.total_bytes, remaining, status)
                    }
                    None => (0, 0, DownloadItemStatus::Queued),
                };

                DownloadItem {
                    download_id: t.id.map(|id| id.to_string()).unwrap_or_else(|| t.info_hash.clone()),
                    title: t.name.unwrap_or_else(|| t.info_hash.clone()),
                    status,
                    total_size,
                    remaining_size,
                    output_path: Some(std::path::PathBuf::from(&t.output_folder)),
                    category: t.category,
                    can_move_files: true,
                    can_be_removed: true,
                    protocol: DownloadProtocol::Torrent,
                }
            })
            .collect())
    }

    async fn remove(&self, id: &str, delete_data: bool) -> anyhow::Result<()> {
        let idx = parse_torrent_id(id)?;
        if delete_data {
            self.api.api_torrent_action_delete(idx).await.map_err(|e| anyhow::anyhow!("{e}"))?;
        } else {
            self.api.api_torrent_action_forget(idx).await.map_err(|e| anyhow::anyhow!("{e}"))?;
        }
        Ok(())
    }

    async fn pause(&self, id: &str) -> anyhow::Result<()> {
        let idx = parse_torrent_id(id)?;
        self.api.api_torrent_action_pause(idx).await.map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(())
    }

    async fn resume(&self, id: &str) -> anyhow::Result<()> {
        let idx = parse_torrent_id(id)?;
        self.api.api_torrent_action_start(idx).await.map_err(|e| anyhow::anyhow!("{e}"))?;
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
