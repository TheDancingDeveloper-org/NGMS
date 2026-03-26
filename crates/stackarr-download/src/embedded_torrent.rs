//! Embedded torrent download client backed by librtbit (rustTorrent).
//!
//! Only compiled when the `torrent-embedded` feature is enabled.
//!
//! **Note:** The `torrent-embedded` feature is currently a stub because
//! librtbit depends on `rusqlite 0.34` while the workspace (via
//! stackarr-migrate) depends on `rusqlite 0.32`, creating a native link
//! conflict.  Once the workspace is unified on a single rusqlite version,
//! uncomment the `librtbit` / `librtbit-core` deps in `Cargo.toml` and
//! the code below will compile.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use async_trait::async_trait;
use tracing::{debug, info};

use librtbit::{
    AddTorrent, AddTorrentOptions, AddTorrentResponse, Api, ApiTorrentListOpts, Session,
    SessionOptions, TorrentStatsState,
};

use crate::client::{
    ClientStatus, DownloadClient, DownloadItem, DownloadItemStatus, DownloadProtocol, GrabRequest,
};

/// An embedded BitTorrent client powered by librtbit.
///
/// Runs the torrent engine in-process -- no external qBittorrent / Transmission
/// installation required.
pub struct EmbeddedTorrentClient {
    session: Arc<Session>,
    api: Api,
}

impl EmbeddedTorrentClient {
    /// Create and start an embedded torrent session.
    ///
    /// * `download_dir`  -- default output directory for downloaded torrents
    /// * `complete_dir`  -- optional directory to move completed torrents to
    /// * `dht_enabled`   -- whether to enable DHT peer discovery
    pub async fn new(
        download_dir: PathBuf,
        complete_dir: Option<PathBuf>,
        dht_enabled: bool,
    ) -> anyhow::Result<Self> {
        let opts = SessionOptions {
            disable_dht: !dht_enabled,
            completed_folder: complete_dir,
            ..Default::default()
        };

        let session = Session::new_with_opts(download_dir, opts)
            .await
            .context("failed to start embedded torrent session")?;

        let api = Api::new(Arc::clone(&session), None);

        info!("embedded torrent client started (librtbit {})", librtbit::version());

        Ok(Self { session, api })
    }
}

/// Map a librtbit `TorrentStatsState` to our unified `DownloadItemStatus`.
fn map_torrent_state(state: TorrentStatsState, finished: bool) -> DownloadItemStatus {
    match state {
        TorrentStatsState::Initializing => DownloadItemStatus::Verifying,
        TorrentStatsState::Live if finished => DownloadItemStatus::Seeding,
        TorrentStatsState::Live => DownloadItemStatus::Downloading,
        TorrentStatsState::Paused => DownloadItemStatus::Paused,
        TorrentStatsState::Error => DownloadItemStatus::Failed,
    }
}

#[async_trait]
impl DownloadClient for EmbeddedTorrentClient {
    fn name(&self) -> &str {
        "Embedded Torrent (librtbit)"
    }

    fn protocol(&self) -> DownloadProtocol {
        DownloadProtocol::Torrent
    }

    async fn add(&self, request: &GrabRequest) -> anyhow::Result<String> {
        let add = AddTorrent::from_url(request.download_url.clone());

        let opts = AddTorrentOptions {
            overwrite: true,
            category: request.category.clone(),
            ..Default::default()
        };

        let response = self
            .session
            .add_torrent(add, Some(opts))
            .await
            .context("embedded torrent: failed to add torrent")?;

        // Extract the info hash as the canonical download ID.
        let info_hash = match response {
            AddTorrentResponse::Added(_id, handle) => handle.info_hash().as_string(),
            AddTorrentResponse::AlreadyManaged(_id, handle) => handle.info_hash().as_string(),
            AddTorrentResponse::ListOnly(list) => list.info_hash.as_string(),
        };

        debug!(info_hash = %info_hash, title = %request.title, "torrent added to embedded client");
        Ok(info_hash)
    }

    async fn get_items(&self) -> anyhow::Result<Vec<DownloadItem>> {
        let list = self.api.api_torrent_list_ext(ApiTorrentListOpts {
            with_stats: true,
            ..Default::default()
        });

        let items = list
            .torrents
            .into_iter()
            .map(|t| {
                let (status, total_size, remaining_size, finished) = match &t.stats {
                    Some(stats) => {
                        let remaining = stats.total_bytes.saturating_sub(stats.progress_bytes);
                        (
                            map_torrent_state(stats.state, stats.finished),
                            stats.total_bytes,
                            remaining,
                            stats.finished,
                        )
                    }
                    None => (DownloadItemStatus::Queued, 0, 0, false),
                };

                DownloadItem {
                    download_id: t.info_hash.clone(),
                    title: t.name.unwrap_or_else(|| t.info_hash.clone()),
                    status,
                    total_size,
                    remaining_size,
                    output_path: Some(PathBuf::from(&t.output_folder)),
                    category: t.category.clone(),
                    can_move_files: finished,
                    can_be_removed: true,
                    protocol: DownloadProtocol::Torrent,
                }
            })
            .collect();

        Ok(items)
    }

    async fn remove(&self, id: &str, delete_data: bool) -> anyhow::Result<()> {
        let idx = id
            .try_into()
            .context("embedded torrent: invalid torrent id/hash")?;

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

        debug!(id = %id, delete_data, "torrent removed from embedded client");
        Ok(())
    }

    async fn pause(&self, id: &str) -> anyhow::Result<()> {
        let idx = id
            .try_into()
            .context("embedded torrent: invalid torrent id/hash")?;

        self.api
            .api_torrent_action_pause(idx)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        debug!(id = %id, "torrent paused");
        Ok(())
    }

    async fn resume(&self, id: &str) -> anyhow::Result<()> {
        let idx = id
            .try_into()
            .context("embedded torrent: invalid torrent id/hash")?;

        self.api
            .api_torrent_action_start(idx)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        debug!(id = %id, "torrent resumed");
        Ok(())
    }

    async fn test(&self) -> anyhow::Result<()> {
        // If we can list torrents the session is alive.
        let _list = self.api.api_torrent_list();
        Ok(())
    }

    async fn status(&self) -> anyhow::Result<ClientStatus> {
        let snap = self.api.api_session_stats();
        let torrent_count = self.api.api_torrent_list().torrents.len();

        Ok(ClientStatus {
            name: format!(
                "Embedded Torrent (librtbit {})",
                librtbit::version()
            ),
            protocol: DownloadProtocol::Torrent,
            version: format!(
                "librtbit {} ({} torrents, {} peers)",
                librtbit::version(),
                torrent_count,
                snap.peers.live,
            ),
            is_connected: true,
        })
    }
}
