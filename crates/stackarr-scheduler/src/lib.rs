use std::time::Duration;

use anyhow::Result;
use sqlx::PgPool;
use tokio::time::interval;

/// Background scheduler that spawns periodic tasks.
pub struct Scheduler {
    pool: PgPool,
    rss_interval: Duration,
    import_interval: Duration,
    refresh_interval: Duration,
    import_list_interval: Duration,
    plex_recent_interval: Duration,
    plex_full_interval: Duration,
    plex_watchlist_interval: Duration,
    plex_token_interval: Duration,
    availability_sync_interval: Duration,
}

impl Scheduler {
    /// Create a scheduler with default intervals.
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            rss_interval: Duration::from_secs(15 * 60),       // 15 min
            import_interval: Duration::from_secs(60),          // 1 min
            refresh_interval: Duration::from_secs(12 * 3600),  // 12 hours
            import_list_interval: Duration::from_secs(3600),   // 1 hour
            plex_recent_interval: Duration::from_secs(5 * 60), // 5 min
            plex_full_interval: Duration::from_secs(24 * 3600), // 24 hours
            plex_watchlist_interval: Duration::from_secs(3600), // 1 hour
            plex_token_interval: Duration::from_secs(12 * 3600), // 12 hours
            availability_sync_interval: Duration::from_secs(24 * 3600), // 24 hours
        }
    }

    /// Create a scheduler with custom intervals.
    pub fn with_intervals(
        pool: PgPool,
        rss_secs: u64,
        import_secs: u64,
        refresh_secs: u64,
    ) -> Self {
        Self {
            pool,
            rss_interval: Duration::from_secs(rss_secs),
            import_interval: Duration::from_secs(import_secs),
            refresh_interval: Duration::from_secs(refresh_secs),
            import_list_interval: Duration::from_secs(3600),
            plex_recent_interval: Duration::from_secs(5 * 60),
            plex_full_interval: Duration::from_secs(24 * 3600),
            plex_watchlist_interval: Duration::from_secs(3600),
            plex_token_interval: Duration::from_secs(12 * 3600),
            availability_sync_interval: Duration::from_secs(24 * 3600),
        }
    }

    /// Start all scheduled tasks. Returns a handle that, when dropped,
    /// will stop the scheduler (via the tokio JoinSet going out of scope).
    pub fn start(self) -> Result<SchedulerHandle> {
        let mut join_set = tokio::task::JoinSet::new();

        // RSS sync task
        let rss_dur = self.rss_interval;
        join_set.spawn(async move {
            let mut tick = interval(rss_dur);
            loop {
                tick.tick().await;
                tracing::info!("scheduler: running RSS sync task");
                if let Err(e) = rss_sync_task().await {
                    tracing::error!(error = %e, "RSS sync task failed");
                }
            }
        });

        // Import scan task
        let import_dur = self.import_interval;
        let import_pool = self.pool.clone();
        join_set.spawn(async move {
            let mut tick = interval(import_dur);
            loop {
                tick.tick().await;
                tracing::info!("scheduler: running import scan task");
                if let Err(e) = import_scan_task(import_pool.clone()).await {
                    tracing::error!(error = %e, "import scan task failed");
                }
            }
        });

        // Metadata refresh task
        let refresh_dur = self.refresh_interval;
        let refresh_pool = self.pool.clone();
        join_set.spawn(async move {
            let mut tick = interval(refresh_dur);
            loop {
                tick.tick().await;
                tracing::info!("scheduler: running metadata refresh task");
                if let Err(e) = metadata_refresh_task(refresh_pool.clone()).await {
                    tracing::error!(error = %e, "metadata refresh task failed");
                }
            }
        });

        // Import list sync task
        let import_list_dur = self.import_list_interval;
        let pool = self.pool.clone();
        join_set.spawn(async move {
            let mut tick = interval(import_list_dur);
            loop {
                tick.tick().await;
                tracing::info!("scheduler: running import list sync task");
                if let Err(e) = import_list_sync_task(pool.clone()).await {
                    tracing::error!(error = %e, "import list sync task failed");
                }
            }
        });

        // ── Plex tasks ────────────────────────────────────────────────────

        // Plex recently added scan (every 5 min)
        let plex_recent_dur = self.plex_recent_interval;
        let plex_recent_pool = self.pool.clone();
        join_set.spawn(async move {
            let mut tick = interval(plex_recent_dur);
            loop {
                tick.tick().await;
                tracing::debug!("scheduler: running Plex recent scan");
                let scanner = stackarr_plex::PlexScanner::new(plex_recent_pool.clone());
                if let Err(e) = scanner.recent_scan().await {
                    tracing::error!(error = %e, "Plex recent scan failed");
                }
            }
        });

        // Plex full scan (every 24 hours)
        let plex_full_dur = self.plex_full_interval;
        let plex_full_pool = self.pool.clone();
        join_set.spawn(async move {
            let mut tick = interval(plex_full_dur);
            loop {
                tick.tick().await;
                tracing::info!("scheduler: running Plex full library scan");
                let scanner = stackarr_plex::PlexScanner::new(plex_full_pool.clone());
                if let Err(e) = scanner.full_scan().await {
                    tracing::error!(error = %e, "Plex full scan failed");
                }
            }
        });

        // Plex watchlist sync (every 1 hour)
        let plex_wl_dur = self.plex_watchlist_interval;
        let plex_wl_pool = self.pool.clone();
        join_set.spawn(async move {
            let mut tick = interval(plex_wl_dur);
            loop {
                tick.tick().await;
                tracing::debug!("scheduler: running Plex watchlist sync");
                let sync = stackarr_plex::WatchlistSync::new(plex_wl_pool.clone());
                if let Err(e) = sync.run().await {
                    tracing::error!(error = %e, "Plex watchlist sync failed");
                }
            }
        });

        // Plex token refresh (every 12 hours)
        let plex_token_dur = self.plex_token_interval;
        let plex_token_pool = self.pool.clone();
        join_set.spawn(async move {
            let mut tick = interval(plex_token_dur);
            loop {
                tick.tick().await;
                tracing::debug!("scheduler: running Plex token refresh");
                let refresh = stackarr_plex::TokenRefresh::new(plex_token_pool.clone());
                if let Err(e) = refresh.run().await {
                    tracing::error!(error = %e, "Plex token refresh failed");
                }
            }
        });

        // Availability sync (every 24 hours)
        let avail_dur = self.availability_sync_interval;
        let avail_pool = self.pool.clone();
        join_set.spawn(async move {
            let mut tick = interval(avail_dur);
            loop {
                tick.tick().await;
                tracing::info!("scheduler: running availability sync");
                let sync = stackarr_plex::AvailabilitySync::new(avail_pool.clone());
                if let Err(e) = sync.run().await {
                    tracing::error!(error = %e, "availability sync failed");
                }
            }
        });

        tracing::info!("scheduler started with 9 background tasks");
        Ok(SchedulerHandle { _join_set: join_set })
    }
}

/// Handle to the running scheduler. Tasks are cancelled when this is dropped.
pub struct SchedulerHandle {
    _join_set: tokio::task::JoinSet<()>,
}

// ── Stub task implementations ───────────────────────────────────────────────

async fn rss_sync_task() -> Result<()> {
    // TODO: fetch RSS feeds from configured indexers, run through decision
    // engine, auto-grab approved releases.
    tracing::debug!("RSS sync: no-op stub");
    Ok(())
}

async fn import_scan_task(pool: PgPool) -> Result<()> {
    // 1. Find all completed downloads in the queue
    let completed: Vec<(i64, String, i64, Option<i64>, String, String, Option<i32>)> =
        sqlx::query_as(
            "SELECT q.id, q.media_type, q.media_id, q.episode_id, q.download_id, q.title, q.download_client_id \
             FROM queue q WHERE q.status = 'completed'",
        )
        .fetch_all(&pool)
        .await?;

    if completed.is_empty() {
        tracing::debug!("import scan: no completed downloads to process");
        return Ok(());
    }

    tracing::info!("found {} completed downloads to import", completed.len());

    // 2. Process each completed item
    for (queue_id, media_type, media_id, episode_id, download_id, title, client_id) in &completed {
        // Look up the download client's output path from its config
        let output_path = if let Some(cid) = client_id {
            let client_row: Option<(serde_json::Value,)> = sqlx::query_as(
                "SELECT config FROM download_clients WHERE id = $1 AND enabled = true",
            )
            .bind(cid)
            .fetch_optional(&pool)
            .await?;

            match client_row {
                Some((config,)) => {
                    // Try to extract the output/completed directory from the client config
                    let dir = config
                        .get("output_path")
                        .or_else(|| config.get("completed_download_handling"))
                        .or_else(|| config.get("directory"))
                        .and_then(|v| v.as_str())
                        .map(|s| std::path::PathBuf::from(s).join(title));
                    dir
                }
                None => None,
            }
        } else {
            None
        };

        let output_path = match output_path {
            Some(p) => p,
            None => {
                tracing::warn!(
                    queue_id,
                    download_id,
                    "no output path resolved for completed download, skipping"
                );
                continue;
            }
        };

        if !output_path.exists() {
            tracing::warn!(
                queue_id,
                download_id,
                path = %output_path.display(),
                "output path does not exist, skipping"
            );
            continue;
        }

        // Run the import pipeline
        let ctx = stackarr_import::ImportContext {
            pool: pool.clone(),
            download_id: download_id.clone(),
            output_path,
            media_type: media_type.clone(),
            media_id: *media_id,
            episode_id: *episode_id,
        };

        match stackarr_import::process_completed_download(ctx).await {
            Ok(import_result) => {
                if import_result.errors.is_empty() {
                    tracing::info!(
                        queue_id,
                        download_id,
                        imported = import_result.imported_files.len(),
                        "import succeeded, removing from queue"
                    );

                    // Remove from queue on success
                    sqlx::query("DELETE FROM queue WHERE id = $1")
                        .bind(queue_id)
                        .execute(&pool)
                        .await?;
                } else {
                    tracing::warn!(
                        queue_id,
                        download_id,
                        errors = ?import_result.errors,
                        "import completed with errors"
                    );

                    // Mark as warning but leave in queue for retry
                    sqlx::query(
                        "UPDATE queue SET error_message = $1 WHERE id = $2",
                    )
                    .bind(import_result.errors.join("; "))
                    .bind(queue_id)
                    .execute(&pool)
                    .await?;
                }
            }
            Err(e) => {
                tracing::error!(
                    queue_id,
                    download_id,
                    error = %e,
                    "import failed"
                );

                // Update queue with error
                sqlx::query(
                    "UPDATE queue SET status = 'failed', error_message = $1 WHERE id = $2",
                )
                .bind(e.to_string())
                .bind(queue_id)
                .execute(&pool)
                .await?;
            }
        }
    }

    Ok(())
}

// ── Real metadata refresh task ──────────────────────────────────────────────

async fn metadata_refresh_task(pool: PgPool) -> Result<()> {
    let refresh_svc = stackarr_media::MetadataRefreshService::new(pool.clone());

    // 1. Find stale series
    let stale_series = refresh_svc.find_stale_series().await?;
    if !stale_series.is_empty() {
        tracing::info!("refreshing metadata for {} stale series", stale_series.len());
    }

    // 2. For each, try to refresh from TMDB (if TMDB key available)
    let tmdb_key = std::env::var("STACKARR_TMDB_API_KEY").ok();
    if let Some(ref key) = tmdb_key {
        let tmdb = stackarr_metadata::TmdbClient::new(key.clone());

        for series_id in stale_series {
            let svc = stackarr_media::SeriesService::new(pool.clone());
            if let Ok(series) = svc.get(series_id).await {
                if let Some(tmdb_id) = series.tmdb_id {
                    match tmdb.get_series(tmdb_id).await {
                        Ok(detail) => {
                            let _ = refresh_svc
                                .update_series_metadata(
                                    series_id,
                                    detail.overview.as_deref(),
                                    &detail.status.unwrap_or_default(),
                                    detail.networks.first().map(|n| n.name.as_str()),
                                    detail.episode_run_time.first().copied(),
                                    None, // images — would need TMDB image URL conversion
                                    None, // genres — would need TmdbGenre → String mapping
                                )
                                .await;
                        }
                        Err(e) => {
                            tracing::warn!(series_id, error = %e, "failed to refresh series from TMDB");
                        }
                    }
                }
                if let Err(e) = refresh_svc.mark_series_synced(series_id).await {
                    tracing::warn!(series_id, error = %e, "failed to mark series synced");
                }
            }
        }
    } else {
        // No TMDB key — just mark them synced so we don't retry every tick
        for series_id in stale_series {
            let _ = refresh_svc.mark_series_synced(series_id).await;
        }
    }

    // 3. Same for movies
    let stale_movies = refresh_svc.find_stale_movies().await?;
    if !stale_movies.is_empty() {
        tracing::info!("refreshing metadata for {} stale movies", stale_movies.len());
    }

    if let Some(ref key) = tmdb_key {
        let tmdb = stackarr_metadata::TmdbClient::new(key.clone());
        for movie_id in stale_movies {
            let svc = stackarr_media::MovieService::new(pool.clone());
            if let Ok(movie) = svc.get(movie_id).await {
                if let Some(tmdb_id) = movie.tmdb_id {
                    match tmdb.get_movie(tmdb_id).await {
                        Ok(detail) => {
                            let studio = detail.production_companies.first().map(|c| c.name.as_str());
                            let _ = refresh_svc
                                .update_movie_metadata(
                                    movie_id,
                                    detail.overview.as_deref(),
                                    studio,
                                    None, // images
                                    None, // genres
                                )
                                .await;
                        }
                        Err(e) => {
                            tracing::warn!(movie_id, error = %e, "failed to refresh movie from TMDB");
                        }
                    }
                }
                if let Err(e) = refresh_svc.mark_movie_synced(movie_id).await {
                    tracing::warn!(movie_id, error = %e, "failed to mark movie synced");
                }
            }
        }
    } else {
        for movie_id in stale_movies {
            let _ = refresh_svc.mark_movie_synced(movie_id).await;
        }
    }

    Ok(())
}

// ── Import list sync task ───────────────────────────────────────────────────

async fn import_list_sync_task(pool: PgPool) -> Result<()> {
    let tmdb_key = std::env::var("STACKARR_TMDB_API_KEY").ok();

    let tmdb_key = match tmdb_key {
        Some(key) if !key.is_empty() => key,
        _ => {
            // Try loading from DB
            match sqlx::query_scalar::<_, serde_json::Value>(
                "SELECT value FROM app_config WHERE key = 'tmdb_api_key'",
            )
            .fetch_optional(&pool)
            .await
            {
                Ok(Some(val)) => match val.as_str() {
                    Some(k) if !k.is_empty() => k.to_string(),
                    _ => {
                        tracing::debug!(
                            "import list sync: no TMDB API key configured, skipping"
                        );
                        return Ok(());
                    }
                },
                _ => {
                    tracing::debug!(
                        "import list sync: no TMDB API key configured, skipping"
                    );
                    return Ok(());
                }
            }
        }
    };

    let tmdb_client = stackarr_metadata::TmdbClient::new(tmdb_key);
    let svc = stackarr_media::import_lists::ImportListService::new(pool);

    match svc.sync_all(&tmdb_client).await {
        Ok(results) => {
            let total_added: usize = results.iter().map(|r| r.items_added).sum();
            let total_errors: usize = results.iter().map(|r| r.errors.len()).sum();
            if total_added > 0 || total_errors > 0 {
                tracing::info!(
                    lists = results.len(),
                    added = total_added,
                    errors = total_errors,
                    "import list sync completed"
                );
            } else {
                tracing::debug!("import list sync: nothing new to add");
            }
        }
        Err(e) => {
            tracing::error!(error = %e, "import list sync_all failed");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::postgres::PgPoolOptions;

    fn dummy_pool() -> PgPool {
        // connect_lazy requires a tokio context, so tests must be #[tokio::test]
        PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy("postgresql://fake:fake@localhost:5432/fake")
            .expect("lazy pool")
    }

    #[tokio::test]
    async fn test_default_intervals() {
        let sched = Scheduler::new(dummy_pool());
        assert_eq!(sched.rss_interval, Duration::from_secs(15 * 60));
        assert_eq!(sched.import_interval, Duration::from_secs(60));
        assert_eq!(sched.refresh_interval, Duration::from_secs(12 * 3600));
        assert_eq!(sched.import_list_interval, Duration::from_secs(3600));
        assert_eq!(sched.plex_recent_interval, Duration::from_secs(5 * 60));
        assert_eq!(sched.plex_full_interval, Duration::from_secs(24 * 3600));
        assert_eq!(sched.availability_sync_interval, Duration::from_secs(24 * 3600));
    }

    #[tokio::test]
    async fn test_custom_intervals() {
        let sched = Scheduler::with_intervals(dummy_pool(), 300, 30, 7200);
        assert_eq!(sched.rss_interval, Duration::from_secs(300));
        assert_eq!(sched.import_interval, Duration::from_secs(30));
        assert_eq!(sched.refresh_interval, Duration::from_secs(7200));
        // Other intervals remain at defaults
        assert_eq!(sched.import_list_interval, Duration::from_secs(3600));
    }
}
