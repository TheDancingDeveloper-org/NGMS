use std::sync::Arc;

use crate::indexarr::IndexarrClient;
use crate::newznab::{NewznabClient, Protocol, ReleaseInfo};
use crate::search::{MovieSearchCriteria, SearchService, TvSearchCriteria};

/// Configuration for a registered indexer.
struct RegisteredIndexer {
    id: i64,
    #[allow(dead_code)]
    name: String,
    enabled: bool,
    client: Arc<NewznabClient>,
}

/// Manages all configured Newznab / Torznab indexers and exposes search
/// through [`SearchService`].
pub struct IndexerManager {
    indexers: Vec<RegisteredIndexer>,
    indexarr: Option<Arc<IndexarrClient>>,
}

impl IndexerManager {
    pub fn new() -> Self {
        Self {
            indexers: Vec::new(),
            indexarr: None,
        }
    }

    /// Set the Indexarr sidecar client for inclusion in search fanout.
    pub fn set_indexarr(&mut self, client: Arc<IndexarrClient>) {
        self.indexarr = Some(client);
    }

    /// Register a new indexer.
    pub fn add_indexer(
        &mut self,
        id: i64,
        name: impl Into<String>,
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        protocol: Protocol,
    ) {
        let name_str = name.into();
        let client = Arc::new(NewznabClient::new(
            base_url,
            api_key,
            id,
            &name_str,
            protocol,
        ));
        self.indexers.push(RegisteredIndexer {
            id,
            name: name_str,
            enabled: true,
            client,
        });
    }

    /// Remove an indexer by database ID.
    pub fn remove_indexer(&mut self, id: i64) -> bool {
        let before = self.indexers.len();
        self.indexers.retain(|i| i.id != id);
        self.indexers.len() < before
    }

    /// Enable or disable an indexer.
    pub fn set_enabled(&mut self, id: i64, enabled: bool) {
        if let Some(idx) = self.indexers.iter_mut().find(|i| i.id == id) {
            idx.enabled = enabled;
        }
    }

    /// Get an indexer client by ID.
    pub fn get_client(&self, id: i64) -> Option<Arc<NewznabClient>> {
        self.indexers
            .iter()
            .find(|i| i.id == id)
            .map(|i| Arc::clone(&i.client))
    }

    /// Number of registered indexers.
    pub fn len(&self) -> usize {
        self.indexers.len()
    }

    /// Whether no indexers are registered.
    pub fn is_empty(&self) -> bool {
        self.indexers.is_empty()
    }

    /// Build a [`SearchService`] from all currently enabled indexers (+ Indexarr).
    fn build_search_service(&self) -> SearchService {
        let clients: Vec<Arc<NewznabClient>> = self
            .indexers
            .iter()
            .filter(|i| i.enabled)
            .map(|i| Arc::clone(&i.client))
            .collect();
        let mut svc = SearchService::new(clients);
        if let Some(ref client) = self.indexarr {
            svc = svc.with_indexarr(Arc::clone(client));
        }
        svc
    }

    /// Search for a TV series across all enabled indexers.
    pub async fn search_series(
        &self,
        criteria: &TvSearchCriteria,
    ) -> anyhow::Result<Vec<ReleaseInfo>> {
        self.build_search_service().search_series(criteria).await
    }

    /// Search for a movie across all enabled indexers.
    pub async fn search_movies(
        &self,
        criteria: &MovieSearchCriteria,
    ) -> anyhow::Result<Vec<ReleaseInfo>> {
        self.build_search_service().search_movies(criteria).await
    }
}

impl Default for IndexerManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_indexer_manager_add_remove() {
        let mut mgr = IndexerManager::new();
        assert!(mgr.is_empty());
        mgr.add_indexer(1, "NZBGeek", "http://nzbgeek.info", "key1", Protocol::Usenet);
        mgr.add_indexer(2, "Jackett", "http://jackett:9117", "key2", Protocol::Torrent);
        assert_eq!(mgr.len(), 2);
        assert!(mgr.remove_indexer(1));
        assert_eq!(mgr.len(), 1);
        assert!(!mgr.remove_indexer(999));
    }

    #[test]
    fn test_indexer_manager_set_enabled() {
        let mut mgr = IndexerManager::new();
        mgr.add_indexer(1, "Test", "http://test", "key", Protocol::Usenet);
        // Starts enabled
        let svc = mgr.build_search_service();
        assert_eq!(svc.indexer_count(), 1);
        // Disable it
        mgr.set_enabled(1, false);
        let svc = mgr.build_search_service();
        assert_eq!(svc.indexer_count(), 0);
        // Re-enable
        mgr.set_enabled(1, true);
        let svc = mgr.build_search_service();
        assert_eq!(svc.indexer_count(), 1);
    }

    #[test]
    fn test_indexer_manager_get_client() {
        let mut mgr = IndexerManager::new();
        mgr.add_indexer(5, "MyIndexer", "http://idx", "key", Protocol::Torrent);
        let client = mgr.get_client(5);
        assert!(client.is_some());
        assert_eq!(client.unwrap().indexer_name(), "MyIndexer");
    }

    #[test]
    fn test_indexer_manager_get_client_missing() {
        let mgr = IndexerManager::new();
        assert!(mgr.get_client(1).is_none());
    }
}
