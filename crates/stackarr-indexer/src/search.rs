use std::collections::HashSet;
use std::sync::Arc;

use anyhow::Context;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

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
}

impl SearchService {
    pub fn new(indexers: Vec<Arc<NewznabClient>>) -> Self {
        Self { indexers }
    }

    /// Search for a TV series across all configured indexers.
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

        deduplicate(&mut all_releases);
        Ok(all_releases)
    }

    /// Search for a movie across all configured indexers.
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

// ── Dedup ───────────────────────────────────────────────────────────────────

/// Remove duplicate releases by GUID.
fn deduplicate(releases: &mut Vec<ReleaseInfo>) {
    let mut seen = HashSet::new();
    releases.retain(|r| seen.insert(r.guid.clone()));
}
