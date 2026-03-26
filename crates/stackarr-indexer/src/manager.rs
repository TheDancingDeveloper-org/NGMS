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
