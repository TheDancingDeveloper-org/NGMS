//! DAV streaming engine manager.
//!
//! Wraps the nzbdav components (provider, store, pipeline processor) into a
//! single `DavManager` struct that lives in `AppState` behind an `ArcSwapOption`.

use std::sync::Arc;

use nzbdav_core::database::DavDatabase;
use nzbdav_dav::DatabaseStore;
use nzbdav_pipeline::queue_item_processor::QueueItemProcessor;
use nzbdav_stream::UsenetArticleProvider;

/// Holds the initialized DAV streaming components.
pub struct DavManager {
    /// Usenet article provider with dedicated connection pools.
    pub provider: Arc<UsenetArticleProvider>,
    /// WebDAV store (resolves paths, builds streaming bodies).
    pub store: Arc<DatabaseStore>,
    /// Pipeline processor for inline NZB processing.
    pub processor: Arc<QueueItemProcessor>,
    /// Database implementation (PostgresDavDatabase).
    pub db: Arc<dyn DavDatabase>,
}
