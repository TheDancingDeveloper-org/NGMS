use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anyhow::Context;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::indexarr::IndexarrClient;
use crate::newznab::{NewznabClient, ReleaseInfo};

/// Criteria for searching TV episodes.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TvSearchCriteria {
    pub query: Option<String>,
    pub tvdb_id: Option<i64>,
    pub season: Option<i32>,
    pub episode: Option<i32>,
    pub categories: Vec<i32>,
}

/// Criteria for searching movies.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MovieSearchCriteria {
    pub query: Option<String>,
    pub tmdb_id: Option<i64>,
    pub imdb_id: Option<String>,
    pub categories: Vec<i32>,
}

/// Fans out searches to multiple indexers, aggregates, and deduplicates.
pub struct SearchService {
    indexers: Vec<Arc<NewznabClient>>,
    indexarr: Option<Arc<IndexarrClient>>,
}

impl SearchService {
    pub fn new(indexers: Vec<Arc<NewznabClient>>) -> Self {
        Self {
            indexers,
            indexarr: None,
        }
    }

    pub fn with_indexarr(mut self, client: Arc<IndexarrClient>) -> Self {
        self.indexarr = Some(client);
        self
    }

    /// Search for a TV series across all configured indexers (+ Indexarr if enabled).
    pub async fn search_series(
        &self,
        criteria: &TvSearchCriteria,
    ) -> anyhow::Result<Vec<ReleaseInfo>> {
        let handles: Vec<_> = self
            .indexers
            .iter()
            .map(|indexer| {
                let indexer = Arc::clone(indexer);
                let criteria = criteria.clone();
                tokio::spawn(async move { search_series_single(&indexer, &criteria).await })
            })
            .collect();

        // Spawn Indexarr search in parallel if configured
        let indexarr_handle = self.indexarr.as_ref().map(|client| {
            let client = Arc::clone(client);
            let criteria = criteria.clone();
            tokio::spawn(async move {
                search_indexarr_tv(&client, &criteria).await
            })
        });

        let results = futures::future::join_all(handles).await;
        let mut all_releases = Vec::new();
        for result in results {
            match result {
                Ok(Ok(releases)) => all_releases.extend(releases),
                Ok(Err(e)) => {
                    warn!(error = %e, "indexer search failed");
                }
                Err(e) => {
                    warn!(error = %e, "indexer search task panicked");
                }
            }
        }

        // Collect Indexarr results
        if let Some(handle) = indexarr_handle {
            match handle.await {
                Ok(Ok(releases)) => all_releases.extend(releases),
                Ok(Err(e)) => warn!(error = %e, "Indexarr search failed"),
                Err(e) => warn!(error = %e, "Indexarr search task panicked"),
            }
        }

        deduplicate(&mut all_releases);
        Ok(all_releases)
    }

    /// Search for a movie across all configured indexers (+ Indexarr if enabled).
    pub async fn search_movies(
        &self,
        criteria: &MovieSearchCriteria,
    ) -> anyhow::Result<Vec<ReleaseInfo>> {
        let handles: Vec<_> = self
            .indexers
            .iter()
            .map(|indexer| {
                let indexer = Arc::clone(indexer);
                let criteria = criteria.clone();
                tokio::spawn(async move { search_movie_single(&indexer, &criteria).await })
            })
            .collect();

        // Spawn Indexarr search in parallel if configured
        let indexarr_handle = self.indexarr.as_ref().map(|client| {
            let client = Arc::clone(client);
            let criteria = criteria.clone();
            tokio::spawn(async move {
                search_indexarr_movie(&client, &criteria).await
            })
        });

        let results = futures::future::join_all(handles).await;
        let mut all_releases = Vec::new();
        for result in results {
            match result {
                Ok(Ok(releases)) => all_releases.extend(releases),
                Ok(Err(e)) => {
                    warn!(error = %e, "indexer movie search failed");
                }
                Err(e) => {
                    warn!(error = %e, "indexer movie search task panicked");
                }
            }
        }

        // Collect Indexarr results
        if let Some(handle) = indexarr_handle {
            match handle.await {
                Ok(Ok(releases)) => all_releases.extend(releases),
                Ok(Err(e)) => warn!(error = %e, "Indexarr movie search failed"),
                Err(e) => warn!(error = %e, "Indexarr movie search task panicked"),
            }
        }

        deduplicate(&mut all_releases);
        Ok(all_releases)
    }
}

// ── Per-indexer helpers ─────────────────────────────────────────────────────

async fn search_series_single(
    indexer: &NewznabClient,
    criteria: &TvSearchCriteria,
) -> anyhow::Result<Vec<ReleaseInfo>> {
    debug!(
        indexer = indexer.indexer_name(),
        "searching for TV series"
    );

    if let Some(tvdbid) = criteria.tvdb_id {
        indexer
            .tv_search(tvdbid, criteria.season, criteria.episode)
            .await
            .context("tv_search failed")
    } else if let Some(ref q) = criteria.query {
        indexer
            .search(q, &criteria.categories)
            .await
            .context("free-text search failed")
    } else {
        Ok(Vec::new())
    }
}

async fn search_movie_single(
    indexer: &NewznabClient,
    criteria: &MovieSearchCriteria,
) -> anyhow::Result<Vec<ReleaseInfo>> {
    debug!(
        indexer = indexer.indexer_name(),
        "searching for movie"
    );

    let has_ids = criteria.imdb_id.is_some() || criteria.tmdb_id.is_some();
    if has_ids {
        indexer
            .movie_search(criteria.imdb_id.as_deref(), criteria.tmdb_id)
            .await
            .context("movie_search failed")
    } else if let Some(ref q) = criteria.query {
        indexer
            .search(q, &criteria.categories)
            .await
            .context("free-text search failed")
    } else {
        Ok(Vec::new())
    }
}

// ── Indexarr helpers ────────────────────────────────────────────────────────

async fn search_indexarr_tv(
    client: &IndexarrClient,
    criteria: &TvSearchCriteria,
) -> anyhow::Result<Vec<ReleaseInfo>> {
    debug!("searching Indexarr for TV series");
    let mut params = HashMap::new();
    params.insert("t".to_string(), "tvsearch".to_string());
    if let Some(tvdbid) = criteria.tvdb_id {
        params.insert("tvdbid".to_string(), tvdbid.to_string());
    }
    if let Some(season) = criteria.season {
        params.insert("season".to_string(), season.to_string());
    }
    if let Some(episode) = criteria.episode {
        params.insert("ep".to_string(), episode.to_string());
    }
    if let Some(ref q) = criteria.query {
        params.insert("q".to_string(), q.clone());
    }
    if !criteria.categories.is_empty() {
        let cats: Vec<String> = criteria.categories.iter().map(|c| c.to_string()).collect();
        params.insert("cat".to_string(), cats.join(","));
    }
    client
        .torznab_search(&params)
        .await
        .context("Indexarr TV search failed")
}

async fn search_indexarr_movie(
    client: &IndexarrClient,
    criteria: &MovieSearchCriteria,
) -> anyhow::Result<Vec<ReleaseInfo>> {
    debug!("searching Indexarr for movie");
    let mut params = HashMap::new();
    params.insert("t".to_string(), "movie".to_string());
    if let Some(ref imdb) = criteria.imdb_id {
        params.insert("imdbid".to_string(), imdb.clone());
    }
    if let Some(tmdb) = criteria.tmdb_id {
        params.insert("tmdbid".to_string(), tmdb.to_string());
    }
    if let Some(ref q) = criteria.query {
        params.insert("q".to_string(), q.clone());
    }
    if !criteria.categories.is_empty() {
        let cats: Vec<String> = criteria.categories.iter().map(|c| c.to_string()).collect();
        params.insert("cat".to_string(), cats.join(","));
    }
    client
        .torznab_search(&params)
        .await
        .context("Indexarr movie search failed")
}

// ── Dedup ───────────────────────────────────────────────────────────────────

/// Remove duplicate releases by GUID.
fn deduplicate(releases: &mut Vec<ReleaseInfo>) {
    let mut seen = HashSet::new();
    releases.retain(|r| seen.insert(r.guid.clone()));
}
