use std::sync::Arc;

use arc_swap::ArcSwap;
use stackarr_core::config::{AppConfig, EnabledModules};
use stackarr_core::db::Database;
use stackarr_download::DownloadClientManager;
use stackarr_indexer::{IndexarrClient, IndexerManager};
use tokio::sync::RwLock;

/// Shared application state available to all request handlers.
#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub config: Arc<ArcSwap<AppConfig>>,
    pub modules: EnabledModules,
    // Embedded engines (initialized on boot if configured)
    pub torrent_session: Option<Arc<librtbit::Session>>,
    pub torrent_api: Option<librtbit::Api>,
    pub usenet_queue: Option<Arc<nzb_web::QueueManager>>,
    // Indexarr sidecar (initialized when config.indexarr.enabled + api_key)
    pub indexarr_client: Option<Arc<IndexarrClient>>,
    // Indexer + download client managers (loaded from DB at startup)
    pub indexer_manager: Arc<RwLock<IndexerManager>>,
    pub download_manager: Arc<RwLock<DownloadClientManager>>,
}
