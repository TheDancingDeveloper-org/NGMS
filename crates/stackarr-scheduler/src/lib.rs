use std::time::Duration;

use anyhow::Result;
use tokio::time::interval;

/// Background scheduler that spawns periodic tasks.
pub struct Scheduler {
    rss_interval: Duration,
    import_interval: Duration,
    refresh_interval: Duration,
}

impl Scheduler {
    /// Create a scheduler with default intervals.
    pub fn new() -> Self {
        Self {
            rss_interval: Duration::from_secs(15 * 60),       // 15 min
            import_interval: Duration::from_secs(60),          // 1 min
            refresh_interval: Duration::from_secs(12 * 3600),  // 12 hours
        }
    }

    /// Create a scheduler with custom intervals.
    pub fn with_intervals(
        rss_secs: u64,
        import_secs: u64,
        refresh_secs: u64,
    ) -> Self {
        Self {
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
        join_set.spawn(async move {
            let mut tick = interval(refresh_dur);
            loop {
                tick.tick().await;
                tracing::info!("scheduler: running metadata refresh task");
                if let Err(e) = metadata_refresh_task().await {
                    tracing::error!(error = %e, "metadata refresh task failed");
                }
            }
        });

        tracing::info!("scheduler started with 3 background tasks");
        Ok(SchedulerHandle { _join_set: join_set })
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
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

async fn metadata_refresh_task() -> Result<()> {
    // TODO: refresh series/movie metadata from TMDB for stale entries.
    tracing::debug!("metadata refresh: no-op stub");
    Ok(())
}
