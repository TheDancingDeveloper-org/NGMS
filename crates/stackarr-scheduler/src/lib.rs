use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use sqlx::PgPool;
use tokio::sync::RwLock;
use tokio::time::interval;

use stackarr_download::DownloadClientManager;
use stackarr_indexer::IndexerManager;
use stackarr_metadata::TmdbClient;

pub mod auto_search;
pub mod health;
pub mod rss;

/// Background scheduler that spawns periodic tasks.
pub struct Scheduler {
    pool: PgPool,
    rss_interval: Duration,
    download_sync_interval: Duration,
    importer_interval: Duration,
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
            rss_interval: Duration::from_secs(15 * 60), // 15 min
            download_sync_interval: Duration::from_secs(60), // 1 min
            importer_interval: Duration::from_secs(30), // 30 sec
            refresh_interval: Duration::from_secs(12 * 3600), // 12 hours
            import_list_interval: Duration::from_secs(3600), // 1 hour
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
            download_sync_interval: Duration::from_secs(import_secs),
            importer_interval: Duration::from_secs(30),
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
            let rss_pool = self.pool.clone();
            let rss_dm = self.download_manager.clone();
            join_set.spawn(async move {
                let mut tick = interval(rss_dur);
                loop {
                    tick.tick().await;
                    if let Some(ref dm) = rss_dm {
                        tracing::info!("scheduler: running RSS sync task");
                        if let Err(e) = rss::rss_sync(&rss_pool, dm).await {
                            tracing::error!(error = %e, "RSS sync task failed");
                        }
                    } else {
                        tracing::debug!("RSS sync: no download manager available");
                    }
                }
            });
            task_count += 1;

            // Download status sync task — polls clients and updates queue table
            let sync_dur = self.download_sync_interval;
            let sync_pool = self.pool.clone();
            let sync_dm = self.download_manager.clone();
            join_set.spawn(async move {
                let mut tick = interval(sync_dur);
                loop {
                    tick.tick().await;
                    tracing::info!("scheduler: running download sync task");
                    if let Err(e) = download_sync_task(sync_pool.clone(), sync_dm.clone()).await {
                        tracing::error!(error = %e, "download sync task failed");
                    }
                }
            });
            task_count += 1;

            // Importer task — picks up completed downloads and imports them
            let importer_dur = self.importer_interval;
            let importer_pool = self.pool.clone();
            join_set.spawn(async move {
                let mut tick = interval(importer_dur);
                loop {
                    tick.tick().await;
                    if let Err(e) = importer_task(importer_pool.clone()).await {
                        tracing::error!(error = %e, "importer task failed");
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
                    if let Err(e) =
                        metadata_refresh_task(refresh_pool.clone(), refresh_tmdb.clone()).await
                    {
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
                    if let Err(e) =
                        import_list_sync_task(pool.clone(), import_list_tmdb.clone()).await
                    {
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

            // ── Automatic search for missing media (every 6 hours) ────
            if let (Some(dl_mgr), Some(idx_mgr)) =
                (self.download_manager.clone(), self.indexer_manager.clone())
            {
                let search_pool = self.pool.clone();
                let search_dm = dl_mgr.clone();
                let search_im = idx_mgr.clone();
                join_set.spawn(async move {
                    // Delay first search by 2 minutes to let services initialize
                    tokio::time::sleep(Duration::from_secs(120)).await;
                    let mut tick = interval(Duration::from_secs(6 * 3600)); // 6 hours
                    loop {
                        tick.tick().await;
                        tracing::info!("scheduler: running automatic search for missing media");
                        let db = stackarr_core::Database::from_pool(search_pool.clone());
                        let activity = db
                            .create_activity(
                                "auto_search",
                                "Automatic Search",
                                Some("Searching indexers for missing media..."),
                            )
                            .await
                            .ok();
                        let activity_id = activity.as_ref().map(|a| a.id);

                        match auto_search::auto_search_missing(&search_pool, &search_im, &search_dm)
                            .await
                        {
                            Ok(stats) => {
                                let detail = if stats.grabbed > 0 {
                                    format!(
                                        "Searched {} items, grabbed {} releases",
                                        stats.searched, stats.grabbed
                                    )
                                } else if stats.searched > 0 {
                                    format!(
                                        "Searched {} items, no approved releases found",
                                        stats.searched
                                    )
                                } else {
                                    "No missing monitored media to search".to_string()
                                };
                                if let Some(aid) = activity_id {
                                    let _ = db
                                        .complete_activity(
                                            aid,
                                            "completed",
                                            Some(&detail),
                                            Some(serde_json::json!({
                                                "searched": stats.searched,
                                                "grabbed": stats.grabbed,
                                                "errors": stats.errors,
                                            })),
                                            None,
                                        )
                                        .await;
                                }
                            }
                            Err(e) => {
                                tracing::error!(error = %e, "automatic search failed");
                                if let Some(aid) = activity_id {
                                    let _ = db
                                        .complete_activity(
                                            aid,
                                            "failed",
                                            Some("Automatic search failed"),
                                            None,
                                            Some(&e.to_string()),
                                        )
                                        .await;
                                }
                            }
                        }
                    }
                });
                task_count += 1;
            }

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
                let plex_recent_tmdb = self.tmdb_client.clone();
                join_set.spawn(async move {
                    let mut tick = interval(plex_recent_dur);
                    loop {
                        tick.tick().await;
                        tracing::debug!("scheduler: running Plex recent scan");
                        let scanner = stackarr_plex::PlexScanner::with_tmdb_client(
                            plex_recent_pool.clone(),
                            plex_recent_tmdb.clone(),
                        );
                        if let Err(e) = scanner.recent_scan().await {
                            tracing::error!(error = %e, "Plex recent scan failed");
                        }
                    }
                });

                // Plex full scan (every 24 hours)
                let plex_full_dur = self.plex_full_interval;
                let plex_full_pool = self.pool.clone();
                let plex_full_tmdb = self.tmdb_client.clone();
                join_set.spawn(async move {
                    let mut tick = interval(plex_full_dur);
                    loop {
                        tick.tick().await;
                        tracing::info!("scheduler: running Plex full library scan");
                        let scanner = stackarr_plex::PlexScanner::with_tmdb_client(
                            plex_full_pool.clone(),
                            plex_full_tmdb.clone(),
                        );
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
        Ok(SchedulerHandle {
            _join_set: join_set,
        })
    }
}

/// Handle to the running scheduler. Tasks are cancelled when this is dropped.
pub struct SchedulerHandle {
    _join_set: tokio::task::JoinSet<()>,
}

// ── Module check ────────────────────────────────────────────────────────────

async fn get_enabled_modules(pool: &PgPool) -> Vec<String> {
    sqlx::query_scalar::<_, String>("SELECT module FROM enabled_modules WHERE enabled = true")
        .fetch_all(pool)
        .await
        .unwrap_or_default()
}

/// Sync download client statuses into the queue table.
///
/// Polls all registered download clients, updates item statuses,
/// persists output paths, and handles stale/orphaned downloads.
async fn download_sync_task(
    pool: PgPool,
    download_manager: Option<Arc<RwLock<DownloadClientManager>>>,
) -> Result<()> {
    #[allow(clippy::type_complexity)]
    let pending: Vec<(
        i64,
        String,
        Option<i32>,
        String,
        i32,
        String,
        i64,
        Option<i64>,
        String,
        Option<i32>,
        Option<String>,
    )> = sqlx::query_as(
        "SELECT id, download_id, download_client_id, status, stale_count, \
                media_type, media_id, episode_id, title, indexer_id, output_path \
         FROM queue WHERE status NOT IN ('completed', 'importing', 'failed')",
    )
    .fetch_all(&pool)
    .await?;

    if !pending.is_empty() {
        if let Some(ref dm) = download_manager {
            let dm_guard = dm.read().await;
            let client_items = dm_guard.get_items_all().await;
            drop(dm_guard);

            // Build lookup: download_id → DownloadItem
            let mut item_map = std::collections::HashMap::new();
            let mut reachable_clients = std::collections::HashSet::new();
            for (client_id, items) in &client_items {
                reachable_clients.insert(*client_id);
                for item in items {
                    item_map.insert(item.download_id.clone(), item);
                }
            }

            tracing::info!(
                pending = pending.len(),
                reachable_clients = reachable_clients.len(),
                client_items = item_map.len(),
                "syncing queue with download clients"
            );

            for (
                queue_id,
                download_id,
                client_db_id,
                current_status,
                stale_count,
                media_type,
                media_id,
                episode_id,
                title,
                indexer_id,
                stored_output_path,
            ) in &pending
            {
                if let Some(item) = item_map.get(download_id) {
                    // Map DownloadItemStatus to queue status string
                    let new_status = match item.status {
                        stackarr_download::DownloadItemStatus::Completed
                        | stackarr_download::DownloadItemStatus::Seeding => "completed",
                        stackarr_download::DownloadItemStatus::Failed => "failed",
                        stackarr_download::DownloadItemStatus::Downloading => "downloading",
                        stackarr_download::DownloadItemStatus::Queued => "queued",
                        stackarr_download::DownloadItemStatus::Paused => "paused",
                        stackarr_download::DownloadItemStatus::Extracting
                        | stackarr_download::DownloadItemStatus::Verifying => "post_processing",
                        stackarr_download::DownloadItemStatus::Warning => "warning",
                    };

                    // Always persist output_path from the download client to the DB
                    // so it survives even if the item later disappears from in-memory state
                    let output_path_str =
                        item.output_path.as_ref().map(|p| p.display().to_string());

                    if new_status != current_status.as_str() {
                        let error_msg = if new_status == "failed" {
                            Some("Download failed in client".to_string())
                        } else {
                            None
                        };

                        sqlx::query(
                            "UPDATE queue SET status = $1, output_path = COALESCE($2, output_path), \
                             error_message = COALESCE($3, error_message), stale_count = 0 \
                             WHERE id = $4",
                        )
                        .bind(new_status)
                        .bind(&output_path_str)
                        .bind(&error_msg)
                        .bind(queue_id)
                        .execute(&pool)
                        .await?;

                        tracing::info!(
                            queue_id,
                            download_id,
                            old_status = current_status.as_str(),
                            new_status,
                            "queue status updated"
                        );

                        // Auto-blocklist and history event on failure
                        if new_status == "failed" {
                            record_download_failure(
                                &pool,
                                media_type,
                                *media_id,
                                *episode_id,
                                title,
                                download_id,
                                *indexer_id,
                                "Download failed in client",
                            )
                            .await;
                        }
                    } else {
                        // Status unchanged — still persist output_path and reset stale if needed
                        if output_path_str.is_some() || *stale_count > 0 {
                            sqlx::query(
                                "UPDATE queue SET output_path = COALESCE($1, output_path), stale_count = 0 WHERE id = $2",
                            )
                            .bind(&output_path_str)
                            .bind(queue_id)
                            .execute(&pool)
                            .await?;
                        }
                    }
                } else {
                    // Item not found in any client — check if the download
                    // completed but fell out of the in-memory history window.
                    // If we have a persisted output_path on disk, trust the DB
                    // and transition to completed instead of marking stale.
                    if let Some(path_str) = stored_output_path {
                        let path = std::path::Path::new(path_str);
                        if path.exists() {
                            sqlx::query(
                                "UPDATE queue SET status = 'completed', stale_count = 0 WHERE id = $1",
                            )
                            .bind(queue_id)
                            .execute(&pool)
                            .await?;

                            tracing::info!(
                                queue_id,
                                download_id,
                                path = %path_str,
                                "download left client but output path exists on disk, marking completed"
                            );
                            continue;
                        }
                    }

                    // Embedded usenet client stores client_id=NULL (-2 sentinel);
                    // treat NULL as reachable if the embedded engine is in the set.
                    let client_reachable = client_db_id
                        .map(|cid| reachable_clients.contains(&(cid as i64)))
                        .unwrap_or_else(|| reachable_clients.contains(&-2));

                    if client_reachable {
                        let new_stale = stale_count + 1;
                        if new_stale >= 5 {
                            sqlx::query(
                                "UPDATE queue SET status = 'failed', \
                                 error_message = 'Download no longer tracked by client', \
                                 stale_count = $1 WHERE id = $2",
                            )
                            .bind(new_stale)
                            .bind(queue_id)
                            .execute(&pool)
                            .await?;

                            tracing::warn!(
                                queue_id,
                                download_id,
                                stale_count = new_stale,
                                "marking stale queue item as failed"
                            );

                            // Auto-blocklist stale items
                            record_download_failure(
                                &pool,
                                media_type,
                                *media_id,
                                *episode_id,
                                title,
                                download_id,
                                *indexer_id,
                                "Download no longer tracked by client",
                            )
                            .await;
                        } else {
                            sqlx::query("UPDATE queue SET stale_count = $1 WHERE id = $2")
                                .bind(new_stale)
                                .bind(queue_id)
                                .execute(&pool)
                                .await?;
                        }
                    }
                }
            }
        } else {
            tracing::debug!("download sync: no download manager available, skipping status sync");
        }
    }

    Ok(())
}

/// Independent importer job — picks up completed downloads from the queue
/// table and runs the import pipeline for each one. Runs on its own timer
/// (every 30 seconds) so imports are never blocked by download client sync.
async fn importer_task(pool: PgPool) -> Result<()> {
    #[allow(clippy::type_complexity)]
    let completed: Vec<(
        i64,
        String,
        i64,
        Option<i64>,
        String,
        String,
        Option<i32>,
        Option<String>,
        Option<i32>,
        i32,
        serde_json::Value,
        Option<serde_json::Value>,
    )> = sqlx::query_as(
        "SELECT q.id, q.media_type, q.media_id, q.episode_id, q.download_id, q.title, \
                    q.download_client_id, q.output_path, q.indexer_id, q.stale_count, \
                    q.quality, q.languages \
             FROM queue q WHERE q.status = 'completed'",
    )
    .fetch_all(&pool)
    .await?;

    if completed.is_empty() {
        tracing::debug!("import scan: no completed downloads to process");
        return Ok(());
    }

    tracing::info!("found {} completed downloads to import", completed.len());

    for (
        queue_id,
        media_type,
        media_id,
        episode_id,
        download_id,
        title,
        client_id,
        stored_path,
        indexer_id,
        stale_count,
        quality,
        languages,
    ) in &completed
    {
        // Resolve output path: prefer stored path from Phase A, fall back to config
        let output_path = if let Some(p) = stored_path {
            let path = std::path::PathBuf::from(p);
            if path.exists() {
                Some(path)
            } else {
                tracing::warn!(
                    queue_id,
                    download_id,
                    path = %p,
                    "stored output path does not exist"
                );
                None
            }
        } else {
            None
        };

        // Fall back to config-based path resolution
        let output_path = match output_path {
            Some(p) => p,
            None => {
                let fallback = resolve_output_path_from_config(&pool, *client_id, title).await;
                match fallback {
                    Some(p) if p.exists() => p,
                    Some(p) => {
                        tracing::warn!(
                            queue_id,
                            download_id,
                            path = %p.display(),
                            "output path does not exist, skipping"
                        );
                        continue;
                    }
                    None => {
                        tracing::warn!(
                            queue_id,
                            download_id,
                            "no output path resolved for completed download, skipping"
                        );
                        continue;
                    }
                }
            }
        };

        // Mark as importing so the UI shows progress and Phase B won't re-pick
        // this item on the next scheduler tick
        sqlx::query("UPDATE queue SET status = 'importing' WHERE id = $1")
            .bind(queue_id)
            .execute(&pool)
            .await?;

        // Create an "import_started" activity record in history
        let activity_id: Option<(i64,)> = sqlx::query_as(
            "INSERT INTO history (media_type, media_id, episode_id, event_type, quality, languages, source_title, download_id, indexer_id, data) \
             VALUES ($1, $2, $3, 'import_started', $4, $5, $6, $7, $8, '{}'::jsonb) \
             RETURNING id",
        )
        .bind(media_type)
        .bind(media_id)
        .bind(episode_id)
        .bind(quality)
        .bind(languages)
        .bind(title)
        .bind(download_id)
        .bind(indexer_id)
        .fetch_optional(&pool)
        .await?;

        tracing::info!(
            queue_id,
            download_id,
            path = %output_path.display(),
            "importing completed download"
        );

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
                        "import succeeded, moving to history"
                    );

                    // Resolve download client name for the history record
                    let client_name: Option<String> = if let Some(cid) = client_id {
                        sqlx::query_scalar("SELECT name FROM download_clients WHERE id = $1")
                            .bind(cid)
                            .fetch_optional(&pool)
                            .await
                            .ok()
                            .flatten()
                    } else {
                        Some("Embedded Usenet".to_string())
                    };

                    // Update the activity record to mark import completed
                    if let Some((aid,)) = activity_id {
                        let import_data = serde_json::json!({
                            "imported_files": import_result.imported_files.len(),
                            "skipped_files": import_result.skipped_files.len(),
                        });
                        let _ = sqlx::query(
                            "UPDATE history SET event_type = 'imported', data = $1 WHERE id = $2",
                        )
                        .bind(&import_data)
                        .bind(aid)
                        .execute(&pool)
                        .await;
                    }

                    // Insert a completed import record into history
                    if let Err(e) = sqlx::query(
                        "INSERT INTO history (media_type, media_id, episode_id, event_type, quality, languages, source_title, download_id, indexer_id, download_client, data) \
                         VALUES ($1, $2, $3, 'download_imported', $4, $5, $6, $7, $8, $9, $10::jsonb)",
                    )
                    .bind(media_type)
                    .bind(media_id)
                    .bind(episode_id)
                    .bind(quality)
                    .bind(languages)
                    .bind(title)
                    .bind(download_id)
                    .bind(indexer_id)
                    .bind(&client_name)
                    .bind(serde_json::json!({
                        "imported_files": import_result.imported_files.len(),
                        "skipped_files": import_result.skipped_files.len(),
                    }))
                    .execute(&pool)
                    .await
                    {
                        tracing::warn!(error = %e, "failed to record download_imported history");
                    }

                    // Remove from queue now that it's in history
                    sqlx::query("DELETE FROM queue WHERE id = $1")
                        .bind(queue_id)
                        .execute(&pool)
                        .await?;

                    // Dispatch import complete notification
                    stackarr_notify::dispatch_event(
                        &pool,
                        &stackarr_notify::NotificationEvent::Import {
                            title: title.clone(),
                            quality: String::new(),
                        },
                    )
                    .await;
                } else {
                    // Bump stale_count as an import-retry counter;
                    // revert status back to completed so the next tick retries
                    let new_count = stale_count + 1;
                    let error_msg = import_result.errors.join("; ");

                    if new_count >= 10 {
                        tracing::error!(
                            queue_id,
                            download_id,
                            attempts = new_count,
                            errors = ?import_result.errors,
                            "import failed after max retries, marking as failed"
                        );
                        sqlx::query(
                            "UPDATE queue SET status = 'failed', error_message = $1, stale_count = $2 WHERE id = $3",
                        )
                        .bind(&error_msg)
                        .bind(new_count)
                        .bind(queue_id)
                        .execute(&pool)
                        .await?;
                    } else {
                        tracing::warn!(
                            queue_id,
                            download_id,
                            attempt = new_count,
                            errors = ?import_result.errors,
                            "import completed with errors, reverting to completed for retry"
                        );
                        sqlx::query(
                            "UPDATE queue SET status = 'completed', error_message = $1, stale_count = $2 WHERE id = $3",
                        )
                        .bind(&error_msg)
                        .bind(new_count)
                        .bind(queue_id)
                        .execute(&pool)
                        .await?;
                    }
                }
            }
            Err(e) => {
                tracing::error!(
                    queue_id,
                    download_id,
                    error = %e,
                    "import failed"
                );

                sqlx::query("UPDATE queue SET status = 'failed', error_message = $1 WHERE id = $2")
                    .bind(e.to_string())
                    .bind(queue_id)
                    .execute(&pool)
                    .await?;

                record_download_failure(
                    &pool,
                    media_type,
                    *media_id,
                    *episode_id,
                    title,
                    download_id,
                    *indexer_id,
                    &format!("Import failed: {e}"),
                )
                .await;
            }
        }
    }

    Ok(())
}

/// Resolve the output path from the download client's stored config (fallback).
/// For embedded clients (client_id=None), look up the usenet/torrent complete dir
/// from the `app_config` table so we don't depend on in-memory engine state.
async fn resolve_output_path_from_config(
    pool: &PgPool,
    client_id: Option<i32>,
    title: &str,
) -> Option<std::path::PathBuf> {
    match client_id {
        Some(cid) => {
            // External download client — look up its config
            let client_row: Option<(serde_json::Value,)> = sqlx::query_as(
                "SELECT config FROM download_clients WHERE id = $1 AND enabled = true",
            )
            .bind(cid)
            .fetch_optional(pool)
            .await
            .ok()?;

            let (config,) = client_row?;
            config
                .get("output_path")
                .or_else(|| config.get("completed_download_handling"))
                .or_else(|| config.get("directory"))
                .and_then(|v| v.as_str())
                .map(|s| std::path::PathBuf::from(s).join(title))
        }
        None => {
            // Embedded client (usenet/torrent) — look up complete dir from app_config DB
            let dir: Option<(serde_json::Value,)> =
                sqlx::query_as("SELECT value FROM app_config WHERE key = 'usenet_complete_dir'")
                    .fetch_optional(pool)
                    .await
                    .ok()?;

            let complete_dir = dir
                .and_then(|(v,)| v.as_str().map(|s| s.to_string()))
                .unwrap_or_else(|| "/downloads/usenet/complete".to_string());

            Some(std::path::PathBuf::from(complete_dir).join(title))
        }
    }
}

/// Record a download failure: add to blocklist and create a download_failed history event.
#[allow(clippy::too_many_arguments)]
async fn record_download_failure(
    pool: &PgPool,
    media_type: &str,
    media_id: i64,
    episode_id: Option<i64>,
    title: &str,
    download_id: &str,
    indexer_id: Option<i32>,
    message: &str,
) {
    // Add to blocklist so auto-search doesn't re-grab
    if let Err(e) = sqlx::query(
        "INSERT INTO blocklist (media_type, media_id, source_title, quality, message, indexer_id) \
         VALUES ($1, $2, $3, '{}'::jsonb, $4, $5)",
    )
    .bind(media_type)
    .bind(media_id)
    .bind(title)
    .bind(message)
    .bind(indexer_id)
    .execute(pool)
    .await
    {
        tracing::warn!(error = %e, title, "failed to add to blocklist");
    }

    // Create download_failed history event
    if let Err(e) = sqlx::query(
        "INSERT INTO history (media_type, media_id, episode_id, event_type, quality, source_title, download_id, indexer_id, data) \
         VALUES ($1, $2, $3, 'download_failed', '{}'::jsonb, $4, $5, $6, $7::jsonb)",
    )
    .bind(media_type)
    .bind(media_id)
    .bind(episode_id)
    .bind(title)
    .bind(download_id)
    .bind(indexer_id)
    .bind(serde_json::json!({ "message": message }))
    .execute(pool)
    .await
    {
        tracing::warn!(error = %e, title, "failed to record download_failed history");
    }

    tracing::info!(
        title,
        message,
        "added to blocklist and recorded download_failed"
    );

    // Dispatch download failure notification
    stackarr_notify::dispatch_event(
        pool,
        &stackarr_notify::NotificationEvent::DownloadFailure {
            title: title.to_string(),
            message: message.to_string(),
        },
    )
    .await;
}

// ── Real metadata refresh task ──────────────────────────────────────────────

async fn metadata_refresh_task(pool: PgPool, tmdb_client: Option<Arc<TmdbClient>>) -> Result<()> {
    let refresh_svc = stackarr_media::MetadataRefreshService::new(pool.clone());

    // 1. Find stale series
    let stale_series = refresh_svc.find_stale_series().await?;
    if !stale_series.is_empty() {
        tracing::info!(
            "refreshing metadata for {} stale series",
            stale_series.len()
        );
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
        tracing::info!(
            "refreshing metadata for {} stale movies",
            stale_movies.len()
        );
    }

    if let Some(ref tmdb) = tmdb_client {
        for movie_id in stale_movies {
            let svc = stackarr_media::MovieService::new(pool.clone());
            if let Ok(movie) = svc.get(movie_id).await {
                if let Some(tmdb_id) = movie.tmdb_id {
                    match tmdb.get_movie(tmdb_id).await {
                        Ok(detail) => {
                            let studio =
                                detail.production_companies.first().map(|c| c.name.as_str());
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

    let folders: Vec<(String, String)> =
        sqlx::query_as("SELECT path, media_type FROM media_library_folders")
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

        let progress_detail = if total_found > 0 {
            format!(
                "Scanning folder {}/{}: {} ({} files so far, {} matched)",
                i + 1,
                folder_count,
                path,
                total_found,
                total_matched
            )
        } else {
            format!("Scanning folder {}/{}: {}", i + 1, folder_count, path)
        };
        let _ = db
            .update_activity_progress(
                activity.id,
                Some(&progress_detail),
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
            .complete_activity(
                activity.id,
                "completed",
                Some(&detail),
                Some(result_json),
                None,
            )
            .await;
        let _ = db
            .update_activity_progress(
                activity.id,
                None,
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
        let detail = format!(
            "{total_found} files found, {total_matched} matched (errors in {} folders)",
            errors.len()
        );
        let _ = db
            .complete_activity(
                activity.id,
                "failed",
                Some(&detail),
                Some(result_json),
                Some(&error_msg),
            )
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
        assert_eq!(sched.download_sync_interval, Duration::from_secs(60));
        assert_eq!(sched.refresh_interval, Duration::from_secs(12 * 3600));
        assert_eq!(sched.import_list_interval, Duration::from_secs(3600));
        assert_eq!(sched.plex_recent_interval, Duration::from_secs(5 * 60));
        assert_eq!(sched.plex_full_interval, Duration::from_secs(24 * 3600));
        assert_eq!(
            sched.availability_sync_interval,
            Duration::from_secs(24 * 3600)
        );
    }

    #[tokio::test]
    async fn test_custom_intervals() {
        let sched = Scheduler::with_intervals(dummy_pool(), 300, 30, 7200);
        assert_eq!(sched.rss_interval, Duration::from_secs(300));
        assert_eq!(sched.download_sync_interval, Duration::from_secs(30));
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
