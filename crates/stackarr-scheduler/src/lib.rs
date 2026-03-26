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
}

impl Scheduler {
    /// Create a scheduler with default intervals.
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            rss_interval: Duration::from_secs(15 * 60),       // 15 min
            import_interval: Duration::from_secs(60),          // 1 min
            refresh_interval: Duration::from_secs(12 * 3600),  // 12 hours
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
        join_set.spawn(async move {
            let mut tick = interval(import_dur);
            loop {
                tick.tick().await;
                tracing::info!("scheduler: running import scan task");
                if let Err(e) = import_scan_task().await {
                    tracing::error!(error = %e, "import scan task failed");
                }
            }
        });

        // Metadata refresh task
        let refresh_dur = self.refresh_interval;
        let pool = self.pool;
        join_set.spawn(async move {
            let mut tick = interval(refresh_dur);
            loop {
                tick.tick().await;
                tracing::info!("scheduler: running metadata refresh task");
                if let Err(e) = metadata_refresh_task(pool.clone()).await {
                    tracing::error!(error = %e, "metadata refresh task failed");
                }
            }
        });

        tracing::info!("scheduler started with 3 background tasks");
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

async fn import_scan_task() -> Result<()> {
    // TODO: scan download client completed folders, import finished items.
    tracing::debug!("import scan: no-op stub");
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
