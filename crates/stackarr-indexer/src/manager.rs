use std::collections::HashMap;
use std::sync::Arc;

use crate::indexarr::IndexarrClient;
use crate::newznab::{NewznabClient, Protocol, ReleaseInfo};
use crate::search::{MovieSearchCriteria, SearchService, TextSearchCriteria, TvSearchCriteria};

use stackarr_cardigann::CardigannEngine;
use stackarr_cardigann::search::{CardigannIndexer, CardigannRelease, SearchQuery, SearchType};

/// Configuration for a registered Newznab/Torznab indexer.
#[derive(Clone)]
struct RegisteredIndexer {
    id: i64,
    #[allow(dead_code)]
    name: String,
    enabled: bool,
    priority: i32,
    client: Arc<NewznabClient>,
}

/// A registered Cardigann indexer.
#[derive(Clone)]
struct RegisteredCardigannIndexer {
    id: i64,
    #[allow(dead_code)]
    name: String,
    enabled: bool,
    priority: i32,
    indexer: Arc<CardigannIndexer>,
}

/// Manages all configured indexers (Newznab, Torznab, and Cardigann) and
/// exposes search through [`SearchService`].
///
/// All internal data is `Arc`-wrapped so cloning is cheap (reference count bumps).
/// Callers should clone under a read lock and then drop the lock before performing
/// network I/O on the clone.
#[derive(Clone)]
pub struct IndexerManager {
    indexers: Vec<RegisteredIndexer>,
    cardigann_indexers: Vec<RegisteredCardigannIndexer>,
    indexarr: Option<Arc<IndexarrClient>>,
    cardigann_engine: Option<Arc<CardigannEngine>>,
}

impl IndexerManager {
    pub fn new() -> Self {
        Self {
            indexers: Vec::new(),
            cardigann_indexers: Vec::new(),
            indexarr: None,
            cardigann_engine: None,
        }
    }

    /// Set the Cardigann engine for loading definitions.
    pub fn set_cardigann_engine(&mut self, engine: Arc<CardigannEngine>) {
        self.cardigann_engine = Some(engine);
    }

    /// Get the Cardigann engine.
    pub fn cardigann_engine(&self) -> Option<&Arc<CardigannEngine>> {
        self.cardigann_engine.as_ref()
    }

    /// Set the Indexarr sidecar client for inclusion in search fanout.
    pub fn set_indexarr(&mut self, client: Arc<IndexarrClient>) {
        self.indexarr = Some(client);
    }

    /// Register a new Newznab/Torznab indexer.
    pub fn add_indexer(
        &mut self,
        id: i64,
        name: impl Into<String>,
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        protocol: Protocol,
        priority: i32,
    ) {
        let name_str = name.into();
        let client = Arc::new(NewznabClient::new(
            base_url, api_key, id, &name_str, protocol,
        ));
        self.indexers.push(RegisteredIndexer {
            id,
            name: name_str,
            enabled: true,
            priority,
            client,
        });
    }

    /// Register a Cardigann indexer from a definition + user config.
    pub fn add_cardigann_indexer(
        &mut self,
        id: i64,
        name: impl Into<String>,
        indexer: CardigannIndexer,
        priority: i32,
    ) {
        self.cardigann_indexers.push(RegisteredCardigannIndexer {
            id,
            name: name.into(),
            enabled: true,
            priority,
            indexer: Arc::new(indexer),
        });
    }

    /// Remove an indexer by database ID (checks both types).
    pub fn remove_indexer(&mut self, id: i64) -> bool {
        let before = self.indexers.len() + self.cardigann_indexers.len();
        self.indexers.retain(|i| i.id != id);
        self.cardigann_indexers.retain(|i| i.id != id);
        let after = self.indexers.len() + self.cardigann_indexers.len();
        after < before
    }

    /// Enable or disable an indexer.
    pub fn set_enabled(&mut self, id: i64, enabled: bool) {
        if let Some(idx) = self.indexers.iter_mut().find(|i| i.id == id) {
            idx.enabled = enabled;
        }
        if let Some(idx) = self.cardigann_indexers.iter_mut().find(|i| i.id == id) {
            idx.enabled = enabled;
        }
    }

    /// Get a Newznab client by ID.
    pub fn get_client(&self, id: i64) -> Option<Arc<NewznabClient>> {
        self.indexers
            .iter()
            .find(|i| i.id == id)
            .map(|i| Arc::clone(&i.client))
    }

    /// Number of registered indexers (all types).
    pub fn len(&self) -> usize {
        self.indexers.len() + self.cardigann_indexers.len()
    }

    /// Whether no indexers are registered.
    pub fn is_empty(&self) -> bool {
        self.indexers.is_empty() && self.cardigann_indexers.is_empty()
    }

    /// Build a [`SearchService`] from enabled Newznab indexers (+ Indexarr),
    /// optionally filtered to specific indexer IDs.
    fn build_search_service(&self, filter_ids: Option<&[i64]>) -> SearchService {
        let clients: Vec<Arc<NewznabClient>> = self
            .indexers
            .iter()
            .filter(|i| i.enabled)
            .filter(|i| filter_ids.is_none_or(|ids| ids.contains(&i.id)))
            .map(|i| Arc::clone(&i.client))
            .collect();
        let mut svc = SearchService::new(clients);
        if let Some(ref client) = self.indexarr {
            svc = svc.with_indexarr(Arc::clone(client));
        }
        svc
    }

    /// Get enabled Cardigann indexers for parallel search,
    /// optionally filtered to specific indexer IDs.
    fn enabled_cardigann_indexers(&self, filter_ids: Option<&[i64]>) -> Vec<Arc<CardigannIndexer>> {
        self.cardigann_indexers
            .iter()
            .filter(|i| i.enabled)
            .filter(|i| filter_ids.is_none_or(|ids| ids.contains(&i.id)))
            .map(|i| Arc::clone(&i.indexer))
            .collect()
    }

    /// Build a map of indexer_id → priority for stamping results.
    fn priority_map(&self) -> HashMap<i64, i32> {
        let mut map = HashMap::new();
        for idx in &self.indexers {
            map.insert(idx.id, idx.priority);
        }
        for idx in &self.cardigann_indexers {
            map.insert(idx.id, idx.priority);
        }
        map
    }

    /// Stamp each result's `indexer_priority` from the registered indexer's priority.
    fn stamp_priorities(results: &mut [ReleaseInfo], priorities: &HashMap<i64, i32>) {
        for r in results.iter_mut() {
            if let Some(&p) = priorities.get(&r.indexer_id) {
                r.indexer_priority = p;
            }
        }
    }

    /// Freehand text search across all enabled indexers (Newznab + Cardigann + Indexarr).
    /// No media-type bias — the Prowlarr-style "search for anything" path.
    pub async fn search_text(
        &self,
        criteria: &TextSearchCriteria,
    ) -> anyhow::Result<Vec<ReleaseInfo>> {
        let filter = criteria.indexer_ids.as_deref();
        let mut results = self
            .build_search_service(filter)
            .search_text(criteria)
            .await?;

        let sq = SearchQuery {
            query: criteria.query.clone(),
            categories: criteria.categories.clone(),
            ..Default::default()
        };
        let cardigann_results = self.search_cardigann(&sq, filter).await;
        results.extend(cardigann_results);

        Self::stamp_priorities(&mut results, &self.priority_map());
        tracing::debug!(results = results.len(), query = %criteria.query, "manager text search completed");
        Ok(results)
    }

    /// Search only Indexarr, skipping all database indexers.
    pub async fn search_indexarr_only(
        &self,
        criteria: &TextSearchCriteria,
    ) -> anyhow::Result<Vec<ReleaseInfo>> {
        let mut svc = SearchService::new(vec![]);
        if let Some(ref client) = self.indexarr {
            svc = svc.with_indexarr(Arc::clone(client));
        } else {
            anyhow::bail!("Indexarr is not configured");
        }
        svc.search_text(criteria).await
    }

    /// Search for a TV series across all enabled indexers (Newznab + Cardigann).
    pub async fn search_series(
        &self,
        criteria: &TvSearchCriteria,
    ) -> anyhow::Result<Vec<ReleaseInfo>> {
        // Newznab/Indexarr search
        let mut results = self
            .build_search_service(None)
            .search_series(criteria)
            .await?;

        // Cardigann search — forward structured params for IMDB/TVDB searches
        let sq = SearchQuery {
            query: criteria.query.clone().unwrap_or_default(),
            categories: criteria.categories.clone(),
            search_type: SearchType::TvSearch,
            tvdb_id: criteria.tvdb_id,
            season: criteria.season,
            episode: criteria.episode,
            ..Default::default()
        };
        let cardigann_results = self.search_cardigann(&sq, None).await;
        results.extend(cardigann_results);

        Self::stamp_priorities(&mut results, &self.priority_map());
        tracing::debug!(
            results = results.len(),
            tvdb_id = criteria.tvdb_id,
            "manager TV search completed"
        );
        Ok(results)
    }

    /// Search for a movie across all enabled indexers (Newznab + Cardigann).
    pub async fn search_movies(
        &self,
        criteria: &MovieSearchCriteria,
    ) -> anyhow::Result<Vec<ReleaseInfo>> {
        // Newznab/Indexarr search
        let mut results = self
            .build_search_service(None)
            .search_movies(criteria)
            .await?;

        // Cardigann search — forward structured params for IMDB/TMDB searches
        let sq = SearchQuery {
            query: criteria.query.clone().unwrap_or_default(),
            categories: criteria.categories.clone(),
            search_type: SearchType::MovieSearch,
            imdb_id: criteria.imdb_id.clone(),
            tmdb_id: criteria.tmdb_id,
            ..Default::default()
        };
        let cardigann_results = self.search_cardigann(&sq, None).await;
        results.extend(cardigann_results);

        Self::stamp_priorities(&mut results, &self.priority_map());
        tracing::debug!(
            results = results.len(),
            tmdb_id = criteria.tmdb_id,
            "manager movie search completed"
        );
        Ok(results)
    }

    /// Convert a Cardigann release to our standard ReleaseInfo.
    fn convert_cardigann_release(r: CardigannRelease) -> ReleaseInfo {
        ReleaseInfo {
            guid: r.guid,
            title: r.title,
            download_url: r.download_url,
            info_url: r.info_url,
            indexer_id: r.indexer_id,
            indexer_name: r.indexer_name,
            protocol: Protocol::Torrent,
            size: r.size,
            age_days: r.age_days,
            publish_date: r.publish_date,
            info_hash: r.info_hash,
            magnet_url: r.magnet_url,
            seeders: r.seeders,
            leechers: r.leechers,
            nzb_url: None,
            tvdb_id: r.tvdb_id,
            imdb_id: r.imdb_id,
            tmdb_id: r.tmdb_id,
            categories: r.categories,
            indexer_flags: r.indexer_flags,
            indexer_priority: 25,
            password: None, // Cardigann doesn't support password attrs
        }
    }

    /// Search across all enabled Cardigann indexers.
    async fn search_cardigann(
        &self,
        search_query: &SearchQuery,
        filter_ids: Option<&[i64]>,
    ) -> Vec<ReleaseInfo> {
        let total_cardigann = self.cardigann_indexers.len();
        let indexers = self.enabled_cardigann_indexers(filter_ids);
        if indexers.is_empty() {
            if total_cardigann == 0 {
                tracing::debug!("no Cardigann indexers registered — skipping torrent search");
            } else {
                tracing::debug!(
                    total = total_cardigann,
                    "all Cardigann indexers disabled or filtered out"
                );
            }
            return Vec::new();
        }

        tracing::debug!(
            count = indexers.len(),
            query = %search_query.query,
            "searching {} Cardigann indexer(s)",
            indexers.len()
        );

        let handles: Vec<_> = indexers
            .into_iter()
            .map(|indexer| {
                let sq = search_query.clone();
                let name = indexer.definition.name.clone();
                let handle = tokio::spawn(async move { indexer.search(&sq).await });
                (name, handle)
            })
            .collect();

        let mut all_releases = Vec::new();
        for (name, handle) in handles {
            match handle.await {
                Ok(Ok(releases)) => {
                    tracing::debug!(
                        indexer = %name,
                        results = releases.len(),
                        "Cardigann indexer returned results"
                    );
                    all_releases.extend(releases.into_iter().map(Self::convert_cardigann_release));
                }
                Ok(Err(e)) => {
                    tracing::warn!(indexer = %name, error = %e, "Cardigann indexer search failed");
                }
                Err(e) => {
                    tracing::warn!(indexer = %name, error = %e, "Cardigann search task panicked");
                }
            }
        }

        all_releases
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
        mgr.add_indexer(
            1,
            "NZBGeek",
            "http://nzbgeek.info",
            "key1",
            Protocol::Usenet,
            25,
        );
        mgr.add_indexer(
            2,
            "Jackett",
            "http://jackett:9117",
            "key2",
            Protocol::Torrent,
            25,
        );
        assert_eq!(mgr.len(), 2);
        assert!(mgr.remove_indexer(1));
        assert_eq!(mgr.len(), 1);
        assert!(!mgr.remove_indexer(999));
    }

    #[test]
    fn test_indexer_manager_set_enabled() {
        let mut mgr = IndexerManager::new();
        mgr.add_indexer(1, "Test", "http://test", "key", Protocol::Usenet, 25);
        let svc = mgr.build_search_service(None);
        assert_eq!(svc.indexer_count(), 1);
        mgr.set_enabled(1, false);
        let svc = mgr.build_search_service(None);
        assert_eq!(svc.indexer_count(), 0);
        mgr.set_enabled(1, true);
        let svc = mgr.build_search_service(None);
        assert_eq!(svc.indexer_count(), 1);
    }

    #[test]
    fn test_indexer_manager_get_client() {
        let mut mgr = IndexerManager::new();
        mgr.add_indexer(5, "MyIndexer", "http://idx", "key", Protocol::Torrent, 25);
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
