use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use sqlx::PgPool;
use tokio::sync::RwLock;
use tokio::time::interval;

use stackarr_download::DownloadClientManager;
use stackarr_indexer::IndexerManager;
use stackarr_metadata::TmdbClient;

pub mod health;

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
    health_check_interval: Duration,
    recycle_bin_cleanup_interval: Duration,
    download_manager: Option<Arc<RwLock<DownloadClientManager>>>,
    indexer_manager: Option<Arc<RwLock<IndexerManager>>>,
    tmdb_client: Option<Arc<TmdbClient>>,
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
            health_check_interval: Duration::from_secs(5 * 60), // 5 min
            recycle_bin_cleanup_interval: Duration::from_secs(6 * 3600), // 6 hours
            download_manager: None,
            indexer_manager: None,
            tmdb_client: None,
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
            health_check_interval: Duration::from_secs(5 * 60),
            recycle_bin_cleanup_interval: Duration::from_secs(6 * 3600),
            download_manager: None,
            indexer_manager: None,
            tmdb_client: None,
        }
    }

    /// Provide a shared TMDB client for metadata refresh and import list tasks.
    pub fn with_tmdb_client(mut self, client: Option<Arc<TmdbClient>>) -> Self {
        self.tmdb_client = client;
        self
    }

    /// Provide download and indexer managers for health checking.
    pub fn with_managers(
        mut self,
        download_manager: Arc<RwLock<DownloadClientManager>>,
        indexer_manager: Arc<RwLock<IndexerManager>>,
    ) -> Self {
        self.download_manager = Some(download_manager);
        self.indexer_manager = Some(indexer_manager);
        self
    }

    /// Start all scheduled tasks. Returns a handle that, when dropped,
    /// will stop the scheduler (via the tokio JoinSet going out of scope).
    pub async fn start(self) -> Result<SchedulerHandle> {
        let mut join_set = tokio::task::JoinSet::new();
        let mut task_count = 0;

        // Check which modules are enabled
        let enabled = get_enabled_modules(&self.pool).await;

        // Only start core tasks if any module is enabled (skip during first boot)
        if !enabled.is_empty() {
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
            task_count += 1;

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
            task_count += 1;

            // Metadata refresh task
            let refresh_dur = self.refresh_interval;
            let refresh_pool = self.pool.clone();
            let refresh_tmdb = self.tmdb_client.clone();
            join_set.spawn(async move {
                let mut tick = interval(refresh_dur);
                loop {
                    tick.tick().await;
                    tracing::info!("scheduler: running metadata refresh task");
                    if let Err(e) = metadata_refresh_task(refresh_pool.clone(), refresh_tmdb.clone()).await {
                        tracing::error!(error = %e, "metadata refresh task failed");
                    }
                }
            });
            task_count += 1;

            // Import list sync task
            let import_list_dur = self.import_list_interval;
            let pool = self.pool.clone();
            let import_list_tmdb = self.tmdb_client.clone();
            join_set.spawn(async move {
                let mut tick = interval(import_list_dur);
                loop {
                    tick.tick().await;
                    tracing::info!("scheduler: running import list sync task");
                    if let Err(e) = import_list_sync_task(pool.clone(), import_list_tmdb.clone()).await {
                        tracing::error!(error = %e, "import list sync task failed");
                    }
                }
            });
            task_count += 1;

            // Scheduled disk scan task (every 12 hours)
            let disk_scan_pool = self.pool.clone();
            join_set.spawn(async move {
                let mut tick = interval(Duration::from_secs(12 * 3600));
                loop {
                    tick.tick().await;
                    tracing::info!("scheduler: running scheduled disk scan");
                    if let Err(e) = scheduled_disk_scan(disk_scan_pool.clone()).await {
                        tracing::error!(error = %e, "scheduled disk scan failed");
                    }
                }
            });
            task_count += 1;

            // ── Health check task ──────────────────────────────────────
            if let (Some(dl_mgr), Some(idx_mgr)) =
                (self.download_manager.clone(), self.indexer_manager.clone())
            {
                let health_pool = self.pool.clone();
                let health_dur = self.health_check_interval;
                join_set.spawn(async move {
                    // Delay the first health check by 30 seconds to let services start
                    tokio::time::sleep(Duration::from_secs(30)).await;
                    let mut tick = interval(health_dur);
                    loop {
                        tick.tick().await;
                        tracing::info!("scheduler: running health check");
                        if let Err(e) = health::health_check_task(
                            health_pool.clone(),
                            dl_mgr.clone(),
                            idx_mgr.clone(),
                        )
                        .await
                        {
                            tracing::error!(error = %e, "health check task failed");
                        }
                    }
                });
                task_count += 1;
            }

            // ── Plex tasks (only if Plex module is enabled) ─────────────
            if enabled.contains(&"plex_integration".to_string()) {
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
                task_count += 5;
            }
        }

        // ── Activity / notification cleanup (daily) ─────────────────
        {
            let cleanup_pool = self.pool.clone();
            join_set.spawn(async move {
                let mut tick = interval(Duration::from_secs(24 * 3600));
                loop {
                    tick.tick().await;
                    let db = stackarr_core::Database::from_pool(cleanup_pool.clone());
                    match db.delete_old_activities(7).await {
                        Ok(n) if n > 0 => tracing::info!(deleted = n, "pruned old activities"),
                        Ok(_) => {}
                        Err(e) => tracing::error!(error = %e, "failed to prune old activities"),
                    }
                    match db.delete_old_notifications(30).await {
                        Ok(n) if n > 0 => tracing::info!(deleted = n, "pruned old notifications"),
                        Ok(_) => {}
                        Err(e) => tracing::error!(error = %e, "failed to prune old notifications"),
                    }
                }
            });
            task_count += 1;
        }

        // ── Recycle bin cleanup (every 6 hours) ─────────────────────
        {
            let cleanup_pool = self.pool.clone();
            let cleanup_dur = self.recycle_bin_cleanup_interval;
            join_set.spawn(async move {
                let mut tick = interval(cleanup_dur);
                loop {
                    tick.tick().await;
                    tracing::debug!("scheduler: running recycle bin cleanup");
                    match stackarr_import::recycle_bin::cleanup_expired_from_config(
                        cleanup_pool.clone(),
                    )
                    .await
                    {
                        Ok(n) if n > 0 => {
                            tracing::info!(deleted = n, "cleaned up expired recycle bin entries")
                        }
                        Ok(_) => {}
                        Err(e) => {
                            tracing::error!(error = %e, "recycle bin cleanup failed")
                        }
                    }
                }
            });
            task_count += 1;
        }

        if task_count == 0 {
            tracing::info!("scheduler: first boot — no tasks started (waiting for setup)");
        } else {
            tracing::info!("scheduler started with {} background tasks", task_count);
        }
        Ok(SchedulerHandle { _join_set: join_set })
    }
}

/// Handle to the running scheduler. Tasks are cancelled when this is dropped.
pub struct SchedulerHandle {
    _join_set: tokio::task::JoinSet<()>,
}

// ── Module check ────────────────────────────────────────────────────────────

async fn get_enabled_modules(pool: &PgPool) -> Vec<String> {
    sqlx::query_scalar::<_, String>(
        "SELECT module FROM enabled_modules WHERE enabled = true",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
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

async fn metadata_refresh_task(pool: PgPool, tmdb_client: Option<Arc<TmdbClient>>) -> Result<()> {
    let refresh_svc = stackarr_media::MetadataRefreshService::new(pool.clone());

    // 1. Find stale series
    let stale_series = refresh_svc.find_stale_series().await?;
    if !stale_series.is_empty() {
        tracing::info!("refreshing metadata for {} stale series", stale_series.len());
    }

    // 2. For each, try to refresh from TMDB (if shared client available)
    if let Some(ref tmdb) = tmdb_client {
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
        // No TMDB client — just mark them synced so we don't retry every tick
        for series_id in stale_series {
            let _ = refresh_svc.mark_series_synced(series_id).await;
        }
    }

    // 3. Same for movies
    let stale_movies = refresh_svc.find_stale_movies().await?;
    if !stale_movies.is_empty() {
        tracing::info!("refreshing metadata for {} stale movies", stale_movies.len());
    }

    if let Some(ref tmdb) = tmdb_client {
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

async fn import_list_sync_task(pool: PgPool, tmdb_client: Option<Arc<TmdbClient>>) -> Result<()> {
    let Some(ref tmdb) = tmdb_client else {
        tracing::debug!("import list sync: no TMDB client available, skipping");
        return Ok(());
    };

    let svc = stackarr_media::import_lists::ImportListService::new(pool);

    match svc.sync_all(tmdb).await {
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

// ── Scheduled disk scan task ────────────────────────────────────────────────

async fn scheduled_disk_scan(pool: PgPool) -> Result<()> {
    let db = stackarr_core::Database::from_pool(pool.clone());

    let folders: Vec<(String, String)> = sqlx::query_as(
        "SELECT path, media_type FROM media_library_folders",
    )
    .fetch_all(&pool)
    .await?;

    if folders.is_empty() {
        tracing::debug!("scheduled disk scan: no media library folders configured");
        return Ok(());
    }

    // Create activity record
    let activity = db
        .create_activity(
            "disk_scan",
            "Library Scan",
            Some("Starting scheduled scan..."),
        )
        .await?;

    let mut total_found = 0usize;
    let mut total_matched = 0usize;
    let mut errors = Vec::new();
    let folder_count = folders.len();

    for (i, (path, media_type)) in folders.iter().enumerate() {
        let scan_path = std::path::Path::new(path);
        if !scan_path.exists() {
            tracing::warn!(path, "scheduled disk scan: path does not exist, skipping");
            continue;
        }

        let _ = db
            .update_activity_progress(
                activity.id,
                Some(&format!("Scanning {path}")),
                Some(serde_json::json!({
                    "folders_total": folder_count,
                    "folders_done": i,
                    "files_found": total_found,
                    "files_matched": total_matched,
                })),
            )
            .await;

        match stackarr_import::disk_scan(&pool, scan_path, media_type).await {
            Ok(result) => {
                total_found += result.files_found;
                total_matched += result.files_matched;
            }
            Err(e) => {
                tracing::error!(path, error = %e, "scheduled disk scan failed for folder");
                errors.push(format!("{path}: {e}"));
            }
        }
    }

    // Complete the activity
    let result_json = serde_json::json!({
        "files_found": total_found,
        "files_matched": total_matched,
        "folders_scanned": folder_count,
    });

    if errors.is_empty() {
        let detail = if total_found > 0 {
            format!("{total_found} files found, {total_matched} matched")
        } else {
            "No new files found".to_string()
        };
        let _ = db
            .complete_activity(activity.id, "completed", Some(result_json), None)
            .await;
        let _ = db
            .update_activity_progress(
                activity.id,
                Some(&detail),
                Some(serde_json::json!({
                    "folders_total": folder_count,
                    "folders_done": folder_count,
                    "files_found": total_found,
                    "files_matched": total_matched,
                })),
            )
            .await;
    } else {
        let error_msg = errors.join("; ");
        let _ = db
            .complete_activity(activity.id, "failed", Some(result_json), Some(&error_msg))
            .await;
    }

    if total_found > 0 {
        tracing::info!(
            found = total_found,
            matched = total_matched,
            "scheduled disk scan completed"
        );
    } else {
        tracing::debug!("scheduled disk scan: no new files found");
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

    /// Verify that all media_type values that can appear in the
    /// `media_library_folders` table are accepted by `disk_scan`.
    /// The scheduler passes media_type strings directly from the DB
    /// to `stackarr_import::disk_scan`, so "tv" (from user-created
    /// folders) must be accepted alongside "series" and "movie".
    #[tokio::test]
    async fn test_disk_scan_accepts_all_db_media_types() {
        let pool = dummy_pool();
        let dir = tempfile::tempdir().unwrap();

        // These are the values that can appear in media_library_folders.media_type
        for media_type in &["series", "tv", "movie"] {
            let result = stackarr_import::disk_scan(&pool, dir.path(), media_type).await;
            if let Err(e) = &result {
                assert!(
                    !e.to_string().contains("unknown media_type"),
                    "media_type '{media_type}' should be accepted by disk_scan, got: {e}"
                );
            }
        }
    }
}
