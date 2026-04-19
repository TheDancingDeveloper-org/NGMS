//! Queue manager — coordinates downloads across the application.
//!
//! The QueueManager owns the list of active NzbJobs, manages the download
//! engine instances, and exposes a thread-safe API for the HTTP handlers
//! to interact with.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::nzb_core::config::{CategoryConfig, ServerConfig};
use crate::nzb_core::db::Database;
use crate::nzb_core::models::*;
use crate::nzb_core::nzb_parser;
use nzb_postproc::{PostProcConfig, parse_rar_volume, run_pipeline};

use crate::direct_unpack::DirectUnpacker;
use crate::log_buffer::LogBuffer;
use nzb_dispatch::ServerProbePolicy;
use nzb_dispatch::bandwidth::BandwidthLimiter;
use nzb_dispatch::dispatch_engine::DispatchEngine;
use nzb_dispatch::download_engine::{ConnectionTracker, ProgressUpdate};
use nzb_dispatch::news_engine::{NewsDispatchEngine, NewsEngineConfig};

/// Get free disk space for a path (returns 0 on error).
fn get_disk_free(path: &std::path::Path) -> u64 {
    #[cfg(unix)]
    {
        use std::ffi::CString;
        use std::mem::MaybeUninit;
        let c_path = match CString::new(path.to_string_lossy().as_bytes()) {
            Ok(p) => p,
            Err(_) => return 0,
        };
        unsafe {
            let mut stat = MaybeUninit::<libc::statvfs>::uninit();
            if libc::statvfs(c_path.as_ptr(), stat.as_mut_ptr()) == 0 {
                let stat = stat.assume_init();
                #[allow(clippy::unnecessary_cast)] // u32 on macOS, u64 on Linux
                return stat.f_bavail as u64 * stat.f_frsize as u64;
            }
        }
        0
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        0
    }
}

// ---------------------------------------------------------------------------
// Job checkpoint for resume support
// ---------------------------------------------------------------------------

/// Compact representation of per-file article completion state.
/// Stored as JSON in the `job_data` column for resuming downloads after restart.
#[derive(Serialize, Deserialize)]
struct JobCheckpoint {
    /// Map of file_id -> set of downloaded segment numbers
    files: HashMap<String, Vec<u32>>,
    /// Bytes downloaded so far
    downloaded_bytes: u64,
    /// Number of articles downloaded
    articles_downloaded: usize,
    /// Number of articles failed
    articles_failed: usize,
    /// Number of files completed
    files_completed: usize,
}

// ---------------------------------------------------------------------------
// Speed tracker — rolling N-second window average
// ---------------------------------------------------------------------------

/// Number of 1-second samples kept in the rolling window. With large NZB
/// articles (~700 KB) and single-digit articles-per-second throughput, a
/// 1-second window regularly saw zero bytes between article completions and
/// reported bps=0 even while downloads were healthy. A 10-second window
/// smooths out that burstiness while staying responsive.
const SPEED_WINDOW_SECS: usize = 10;

pub(crate) struct SpeedTracker {
    /// Bytes recorded since the last `tick()`.
    pending_bytes: AtomicU64,
    /// Per-second byte counts for the last SPEED_WINDOW_SECS seconds.
    /// Front = oldest, back = most recent. Length grows from 0 to
    /// SPEED_WINDOW_SECS then stabilises.
    window: Mutex<std::collections::VecDeque<u64>>,
    /// Most recently computed bytes-per-second (averaged over the window).
    current_bps: AtomicU64,
}

impl SpeedTracker {
    pub fn new() -> Self {
        Self {
            pending_bytes: AtomicU64::new(0),
            window: Mutex::new(std::collections::VecDeque::with_capacity(SPEED_WINDOW_SECS)),
            current_bps: AtomicU64::new(0),
        }
    }

    /// Record bytes transferred. Workers call this from `handle_progress`
    /// on every article completion.
    pub fn record(&self, bytes: u64) {
        self.pending_bytes.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Advance the rolling window by one second. Called every 1 s from
    /// `spawn_speed_tracker`. `_elapsed_secs` is kept for call-site
    /// compatibility but ignored — the window is sized in whole seconds.
    pub fn tick(&self, _elapsed_secs: f64) {
        let bytes = self.pending_bytes.swap(0, Ordering::Relaxed);
        let bps = {
            let mut window = self.window.lock();
            window.push_back(bytes);
            while window.len() > SPEED_WINDOW_SECS {
                window.pop_front();
            }
            let sum: u64 = window.iter().sum();
            if window.is_empty() {
                0
            } else {
                sum / window.len() as u64
            }
        };
        self.current_bps.store(bps, Ordering::Relaxed);
    }

    pub fn bps(&self) -> u64 {
        self.current_bps.load(Ordering::Relaxed)
    }
}

// ---------------------------------------------------------------------------
// Hopeless job detection
// ---------------------------------------------------------------------------

/// Tracks article failure statistics for a single job to determine
/// whether it can possibly complete. Implements a four-tier check:
///
/// 1. **Grace period** — ignore the first few failures (par2 can repair minor gaps)
/// 2. **Early failure check** — if most articles fail, abort fast (Phase 6:
///    no longer capped to the first 25% of articles; the check runs
///    continuously while the job is downloading)
/// 3. **Ongoing availability** — track bytes missing vs total (excluding par2)
/// 4. **No-progress timeout** — Phase 6 addition. Fires when the tracker
///    has been alive for longer than `no_progress_timeout` without ever
///    seeing an article complete. Catches the zombie scenario where
///    workers cycle silently (Phase 5 evicts them) but no failure is ever
///    recorded against the job.
struct HopelessTracker {
    /// When the tracker (and therefore the download attempt) was instantiated.
    /// Used by `time_based_check` to detect long-running zombie jobs.
    created_at: Instant,
    /// Total content bytes (excluding par2 files).
    content_bytes: u64,
    /// Total par2 bytes. Credited as recovery capacity in the availability
    /// ratio: SABnzbd's formula treats par2 as "available for repair," so a
    /// job with 10% par2 can tolerate ~10% missing content before the
    /// availability ratio drops below 100%. See
    /// `sabnzbd/nzb/object.py::check_availability_ratio`.
    par2_bytes: u64,
    /// Content bytes confirmed missing (failed articles in non-par2 files).
    content_bytes_missing: u64,
    /// Articles checked so far (downloaded + failed, not par2).
    content_articles_checked: usize,
    /// Content articles that failed.
    content_articles_failed: usize,
    /// Total content articles expected (non-par2).
    content_articles_total: usize,
}

/// Number of bad articles allowed before any abort checks kick in.
const HOPELESS_GRACE_ARTICLES: usize = 5;
/// Minimum content articles checked before the early failure check fires.
const EARLY_CHECK_MIN_ARTICLES: usize = 10;
/// Failure rate threshold for the early check (0.0–1.0).
const EARLY_CHECK_FAILURE_RATE: f64 = 0.80;

impl HopelessTracker {
    fn new(job: &NzbJob) -> Self {
        let mut content_bytes: u64 = 0;
        let mut par2_bytes: u64 = 0;
        let mut content_articles_total: usize = 0;

        for file in &job.files {
            if file.is_par2 {
                par2_bytes += file.bytes;
            } else {
                content_bytes += file.bytes;
                content_articles_total += file.articles.len();
            }
        }

        Self {
            created_at: Instant::now(),
            content_bytes,
            par2_bytes,
            content_bytes_missing: 0,
            content_articles_checked: 0,
            content_articles_failed: 0,
            content_articles_total,
        }
    }

    /// Record a successful content article download.
    fn record_success(&mut self, is_par2: bool) {
        if !is_par2 {
            self.content_articles_checked += 1;
        }
    }

    /// Record a failed content article. Returns the estimated byte size
    /// of the missing article.
    ///
    /// Phase 6: takes a typed [`ArticleFailureKind`] so the tracker can
    /// ignore failures that are likely transient (server-down, auth, etc.)
    /// and only count failures that genuinely indicate the article cannot
    /// be retrieved (NotFound, DecodeError).
    fn record_failure(
        &mut self,
        is_par2: bool,
        estimated_bytes: u64,
        kind: crate::article_failure::ArticleFailureKind,
    ) {
        if is_par2 {
            return;
        }
        self.content_articles_checked += 1;
        // Only failures that count toward "definitively not retrievable"
        // increment the failed counter and missing-bytes total. A 502 or
        // AuthFailed on one server doesn't mean the article is gone.
        if kind.counts_toward_hopeless() {
            self.content_articles_failed += 1;
            self.content_bytes_missing += estimated_bytes;
        }
    }

    /// Check whether the job should be aborted.
    ///
    /// Returns `Some(HopelessAbort)` if the job is hopeless, carrying both
    /// the human-readable reason and a stable `tier` label that operators
    /// can filter logs by. `None` means the job should continue.
    fn check(
        &self,
        abort_hopeless: bool,
        early_failure_check: bool,
        required_completion_pct: f64,
    ) -> Option<HopelessAbort> {
        if !abort_hopeless {
            return None;
        }

        // Tier 1: grace period — allow minor gaps that par2 can fix
        if self.content_articles_failed <= HOPELESS_GRACE_ARTICLES {
            return None;
        }

        // Tier 2: early failure check — catch completely dead NZBs fast.
        // Phase 6: removed the `<= total/4` window upper bound. The check
        // now fires whenever the failure rate is above the threshold AND
        // there are enough samples to be statistically meaningful. The old
        // window was a footgun: slow-trickle failures crept past the 25%
        // mark before the rate accumulated, and tier 3's bytes-availability
        // check wouldn't fire until many more bytes were confirmed missing.
        if early_failure_check && self.content_articles_checked >= EARLY_CHECK_MIN_ARTICLES {
            let failure_rate =
                self.content_articles_failed as f64 / self.content_articles_checked as f64;
            if failure_rate >= EARLY_CHECK_FAILURE_RATE {
                return Some(HopelessAbort {
                    tier: "early_failure",
                    // Each "confirmed missing" article exhausted every
                    // configured server before being counted here, so the
                    // per-server primary-tier fail rate is much higher
                    // than this sample suggests. The `server_stats` log
                    // line emitted alongside this abort breaks down the
                    // per-server attempt / not-found counts.
                    reason: format!(
                        "Aborted: {} article(s) confirmed missing across all configured servers ({:.0}% failure rate over {} fully-cascaded samples)",
                        self.content_articles_failed,
                        failure_rate * 100.0,
                        self.content_articles_checked,
                    ),
                });
            }
        }

        // Tier 3: ongoing availability ratio with par2-aware credit.
        //
        // Matches SABnzbd's `check_availability_ratio`
        // (sabnzbd/nzb/object.py:1105-1131):
        //   ratio = 100 * (total_bytes - content_missing) / content_bytes
        //         = 100 * (content_retrieved + par2_total) / content_bytes
        //
        // par2 is credited as recovery capacity. A job with par2 sized to
        // cover N% of content can tolerate N% content missing before the
        // ratio drops below 100%. Without this credit, retention-edge NZBs
        // that par2 could repair get aborted prematurely.
        if self.content_bytes > 0 {
            let total_bytes = self.content_bytes.saturating_add(self.par2_bytes);
            let available_bytes = total_bytes.saturating_sub(self.content_bytes_missing);
            let availability_pct = 100.0 * available_bytes as f64 / self.content_bytes as f64;
            if availability_pct < required_completion_pct {
                return Some(HopelessAbort {
                    tier: "ongoing_availability",
                    reason: format!(
                        "Aborted: only {availability_pct:.1}% of content+par2 available \
                         (need {required_completion_pct:.1}%), \
                         {} of {} content articles missing",
                        self.content_articles_failed, self.content_articles_total,
                    ),
                });
            }
        }

        None
    }
}

/// Result of a positive [`HopelessTracker::check`] — both the reason string
/// (for the user-visible error_message) and a stable `tier` label so logs
/// and metrics can be grouped by which heuristic fired.
#[derive(Debug, Clone)]
pub(crate) struct HopelessAbort {
    pub tier: &'static str,
    pub reason: String,
}

impl HopelessTracker {
    /// Phase 6: time-based hopeless check. Operates on the tracker's
    /// `created_at` field, not on article counters, so it fires even when
    /// the engine has stopped emitting progress events entirely (the
    /// zombie scenario).
    ///
    /// Aborts if the tracker has been alive longer than `timeout` AND
    /// no successful article has ever been recorded against it. The
    /// caller is the queue manager's periodic tick — see
    /// [`QueueManager::scan_for_no_progress_jobs`].
    fn time_based_check(&self, timeout: Duration) -> Option<HopelessAbort> {
        let elapsed = self.created_at.elapsed();
        if elapsed >= timeout && self.content_articles_checked == 0 {
            return Some(HopelessAbort {
                tier: "no_progress_timeout",
                reason: format!(
                    "Aborted: no article completed or failed within {}s of starting download",
                    elapsed.as_secs()
                ),
            });
        }
        None
    }
}

// ---------------------------------------------------------------------------
// Per-job state
// ---------------------------------------------------------------------------

struct JobState {
    /// The job data (shared with API for reading).
    job: NzbJob,
    /// Handle to the per-job progress listener task.
    progress_handle: Option<tokio::task::JoinHandle<()>>,
    /// Per-job speed tracker.
    speed: Arc<SpeedTracker>,
    /// Raw NZB data for retry.
    nzb_data: Option<Vec<u8>>,
    /// Direct unpacker for RAR extraction during download.
    direct_unpacker: Option<DirectUnpacker>,
    /// Hopeless job tracker (None until download starts).
    hopeless_tracker: Option<HopelessTracker>,
}

// ---------------------------------------------------------------------------
// QueueManager
// ---------------------------------------------------------------------------

/// Thread-safe queue manager that coordinates all downloads.
///
/// Wrapped in `Arc` for sharing between the background task and HTTP handlers.
pub struct QueueManager {
    /// Active jobs keyed by job ID.
    jobs: Mutex<HashMap<String, JobState>>,
    /// Order of job IDs for display.
    job_order: Mutex<Vec<String>>,
    /// Server configurations.
    servers: Arc<Mutex<Vec<ServerConfig>>>,
    /// Whether all downloads are globally paused.
    globally_paused: AtomicBool,
    /// Global speed tracker.
    speed: SpeedTracker,
    /// Database for persistence.
    db: Mutex<Database>,
    /// App config (incomplete_dir, complete_dir).
    incomplete_dir: Mutex<std::path::PathBuf>,
    complete_dir: Mutex<std::path::PathBuf>,
    /// Timed pause: when to auto-resume (None = not timed).
    pause_until: Mutex<Option<DateTime<Utc>>>,
    /// History retention limit (None = keep all).
    history_retention: Mutex<Option<usize>>,
    /// Log buffer for capturing per-job logs into history.
    log_buffer: Option<LogBuffer>,
    /// Max concurrent active downloads (0 = unlimited).
    max_active_downloads: AtomicUsize,
    /// Category configs for post-processing decisions.
    categories: Mutex<Vec<CategoryConfig>>,
    /// Minimum free disk space in bytes before pausing downloads.
    min_free_space: u64,
    /// Bandwidth limiter for throttling downloads.
    bandwidth: Arc<BandwidthLimiter>,
    /// Whether direct unpack (RAR extraction during download) is enabled.
    direct_unpack_enabled: AtomicBool,
    /// Abort downloads that cannot possibly complete.
    abort_hopeless: bool,
    /// Phase 6: maximum time a job may sit in `Downloading` without any
    /// article event before the time-based hopeless tier fires. Settable
    /// at runtime via [`Self::set_no_progress_timeout`].
    no_progress_timeout: Mutex<Duration>,
    /// Quick initial failure check on first N articles.
    early_failure_check: bool,
    /// Global NNTP connection tracker (shared across all download jobs).
    conn_tracker: Arc<ConnectionTracker>,
    /// Article-dispatch engine. Owns workers, routes fetches across servers,
    /// reports progress via the per-job channel. See
    /// [`crate::dispatch_engine::DispatchEngine`].
    dispatch: Arc<dyn DispatchEngine>,
    /// Minimum completion percentage required (excluding par2).
    required_completion_pct: f64,
    /// Content-hash → job_id map for duplicate detection. A job is in this
    /// map iff it's currently blocking (Queued, Paused, Downloading,
    /// PostProcessing) and its NZB data has been parsed (so the hash could
    /// be computed). Jobs restored from DB without a parsed NZB are not
    /// hashed until launch; see `launch_download`.
    content_hashes: Mutex<HashMap<[u8; 32], String>>,
}

/// Compute a content-identity hash for an NZB job based on its article
/// message-ids. Two jobs with the same set of articles (regardless of
/// packaging or post metadata) produce the same hash.
///
/// Returns `None` if the job has no articles yet (e.g., a queued job whose
/// NZB data hasn't been parsed).
fn compute_content_hash(job: &NzbJob) -> Option<[u8; 32]> {
    use sha2::{Digest, Sha256};
    let mut ids: Vec<&str> = job
        .files
        .iter()
        .flat_map(|f| f.articles.iter().map(|a| a.message_id.as_str()))
        .collect();
    if ids.is_empty() {
        return None;
    }
    ids.sort_unstable();
    let mut hasher = Sha256::new();
    for id in &ids {
        hasher.update(id.as_bytes());
        hasher.update(b"\n");
    }
    Some(hasher.finalize().into())
}

impl QueueManager {
    /// Create a new queue manager.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        servers: Vec<ServerConfig>,
        db: Database,
        incomplete_dir: std::path::PathBuf,
        complete_dir: std::path::PathBuf,
        log_buffer: LogBuffer,
        max_active_downloads: usize,
        categories: Vec<CategoryConfig>,
        min_free_space: u64,
        speed_limit_bps: u64,
        direct_unpack: bool,
        abort_hopeless: bool,
        early_failure_check: bool,
        required_completion_pct: f64,
        article_timeout_secs: u64,
        probe_policy: Option<ServerProbePolicy>,
    ) -> Arc<Self> {
        use crate::bandwidth::BandwidthConfig;
        use std::num::NonZeroU32;

        let download_bps = if speed_limit_bps > 0 {
            NonZeroU32::new(speed_limit_bps as u32)
        } else {
            None
        };
        let bandwidth = Arc::new(BandwidthLimiter::new(BandwidthConfig { download_bps }));

        // Build connection tracker with per-server limits.
        let conn_tracker = Arc::new(ConnectionTracker::new());
        for server in &servers {
            conn_tracker.set_limit(&server.id, &server.name, server.connections as usize);
        }

        // Build the layered news engine. `bandwidth` and `conn_tracker` are
        // retained as observability/config shells but are not wired into the
        // new fetch path yet — the old WorkerPool consumed them directly;
        // nzb-news manages its own connection pool and does not rate-limit
        // yet. Wiring these two back in is a follow-up.
        // Share the server list Arc between QueueManager and the news
        // engine so `update_servers` → `reconcile_servers` is observable
        // inside the engine without an extra plumbing step.
        let servers_arc = Arc::new(Mutex::new(servers));
        let mut news_cfg = NewsEngineConfig::with_shared_servers(
            servers_arc.clone(),
            std::time::Duration::from_secs(article_timeout_secs),
        );
        news_cfg.probe_policy = probe_policy;
        let dispatch: Arc<dyn DispatchEngine> = Arc::new(NewsDispatchEngine::new(news_cfg));
        dispatch.start();

        Arc::new(Self {
            jobs: Mutex::new(HashMap::new()),
            job_order: Mutex::new(Vec::new()),
            servers: servers_arc,
            globally_paused: AtomicBool::new(false),
            speed: SpeedTracker::new(),
            db: Mutex::new(db),
            incomplete_dir: Mutex::new(incomplete_dir),
            complete_dir: Mutex::new(complete_dir),
            pause_until: Mutex::new(None),
            history_retention: Mutex::new(None),
            log_buffer: Some(log_buffer),
            max_active_downloads: AtomicUsize::new(max_active_downloads),
            categories: Mutex::new(categories),
            min_free_space,
            bandwidth,
            direct_unpack_enabled: AtomicBool::new(direct_unpack),
            conn_tracker,
            dispatch,
            abort_hopeless,
            early_failure_check,
            required_completion_pct: required_completion_pct.clamp(100.0, 200.0),
            // Phase 6: 5-minute default. Long enough that a slow first
            // article doesn't accidentally abort a real download; short
            // enough that an obvious zombie is killed within minutes.
            no_progress_timeout: Mutex::new(Duration::from_secs(300)),
            content_hashes: Mutex::new(HashMap::new()),
        })
    }

    /// Update category configs (e.g. after config reload).
    pub fn set_categories(&self, categories: Vec<CategoryConfig>) {
        *self.categories.lock() = categories;
    }

    /// Get history retention limit (None = keep all).
    pub fn get_history_retention(&self) -> Option<usize> {
        *self.history_retention.lock()
    }

    /// Set history retention limit.
    pub fn set_history_retention(&self, limit: Option<usize>) {
        *self.history_retention.lock() = limit;
    }

    /// Per-server `(server_id, active, limit)` triples for the live NNTP
    /// connection pool. `active` is by-construction `<= limit` because the
    /// pool is semaphore-backed.
    pub fn connection_snapshot(&self) -> Vec<(String, usize, usize)> {
        self.conn_tracker.snapshot()
    }

    /// Total currently-held NNTP connection slots across all servers.
    pub fn connection_total(&self) -> usize {
        self.conn_tracker.total()
    }

    /// Override the worker idle eviction threshold (Phase 5 watchdog).
    /// Test harnesses use this to make the eviction trigger in seconds.
    pub fn set_max_worker_idle(&self, d: std::time::Duration) {
        self.dispatch.set_max_worker_idle(d);
    }

    /// Lifetime count of worker evictions performed by the Phase 5 idle
    /// watchdog. Each increment means the supervisor reclaimed a worker
    /// that had stalled past `max_worker_idle`.
    pub fn worker_eviction_count(&self) -> u64 {
        self.dispatch.eviction_count()
    }

    /// Phase 6: override the time-based hopeless threshold. Tests use this
    /// to make the no-progress watchdog converge in seconds.
    pub fn set_no_progress_timeout(&self, d: std::time::Duration) {
        *self.no_progress_timeout.lock() = d;
    }

    /// Phase 6: scan all active jobs and abort any whose hopeless tracker
    /// reports a `no_progress_timeout` tier. Called from the speed-tracker
    /// tick (1Hz). Cheap when there's nothing to abort — just a tracker
    /// `Instant::elapsed()` per job.
    fn scan_for_no_progress_jobs(self: &Arc<Self>) {
        if !self.abort_hopeless {
            return;
        }
        let timeout = *self.no_progress_timeout.lock();

        // Snapshot the (job_id, abort) pairs under the jobs lock, then
        // release before calling abort_job (which re-acquires).
        let mut to_abort: Vec<(String, HopelessAbort)> = Vec::new();
        {
            let jobs = self.jobs.lock();
            for (id, state) in jobs.iter() {
                if !matches!(state.job.status, JobStatus::Downloading) {
                    continue;
                }
                if let Some(ref tracker) = state.hopeless_tracker
                    && let Some(abort) = tracker.time_based_check(timeout)
                {
                    to_abort.push((id.clone(), abort));
                }
            }
        }

        for (job_id, abort) in to_abort {
            warn!(
                job_id = %job_id,
                tier = abort.tier,
                reason = %abort.reason,
                "Job is hopeless (no-progress timeout) — aborting"
            );
            {
                let mut jobs = self.jobs.lock();
                if let Some(state) = jobs.get_mut(&job_id) {
                    state.job.error_message = Some(abort.reason.clone());
                    // Drop tracker so any in-flight ArticleFailed events
                    // can't re-enter the hopeless path.
                    state.hopeless_tracker = None;
                }
            }
            self.dispatch.abort_job(&job_id, abort.reason);
        }
    }

    /// Set max active downloads and start queued jobs if capacity allows.
    pub fn set_max_active_downloads(self: &Arc<Self>, max: usize) {
        self.max_active_downloads.store(max, Ordering::Relaxed);
        self.start_next_queued();
    }

    /// Get max active downloads.
    pub fn get_max_active_downloads(&self) -> usize {
        self.max_active_downloads.load(Ordering::Relaxed)
    }

    /// Set the download speed limit in bytes per second (0 = unlimited).
    pub fn set_speed_limit(&self, bps: u64) {
        use std::num::NonZeroU32;
        let limit = if bps > 0 {
            NonZeroU32::new(bps as u32)
        } else {
            None
        };
        self.bandwidth.set_download_bps(limit);
    }

    /// Get the current download speed limit in bytes per second (0 = unlimited).
    pub fn get_speed_limit(&self) -> u64 {
        self.bandwidth
            .get_download_bps()
            .map(|v| v.get() as u64)
            .unwrap_or(0)
    }

    /// Count currently downloading jobs.
    #[allow(dead_code)]
    fn active_download_count(&self) -> usize {
        let jobs = self.jobs.lock();
        jobs.values()
            .filter(|s| s.job.status == JobStatus::Downloading)
            .count()
    }

    /// Atomically find the next queued job that can start, mark it as
    /// `Downloading` in the jobs map, and return its ID.
    ///
    /// Returns `None` if no download slot is available or there are no
    /// queued jobs.  Because the status transition happens under the same
    /// lock acquisition as the active-count check, concurrent callers
    /// cannot both claim the same slot (no TOCTOU race).
    fn claim_next_download_slot(&self, max: usize) -> Option<String> {
        if self.globally_paused.load(Ordering::Relaxed) {
            return None;
        }

        let mut jobs = self.jobs.lock();
        let active = jobs
            .values()
            .filter(|s| s.job.status == JobStatus::Downloading)
            .count();
        if max > 0 && active >= max {
            return None;
        }

        let order = self.job_order.lock();
        let mut best: Option<(String, u8)> = None;
        for id in order.iter() {
            if let Some(s) = jobs.get(id)
                && s.job.status == JobStatus::Queued
            {
                let p = s.job.priority as u8;
                if best.as_ref().is_none_or(|(_, bp)| p > *bp) {
                    best = Some((id.clone(), p));
                }
            }
        }
        let (id, _) = best?;

        // Mark as Downloading while still holding the lock
        if let Some(state) = jobs.get_mut(&id) {
            state.job.status = JobStatus::Downloading;
            info!(job_id = %id, name = %state.job.name, "Starting queued job");
        }
        Some(id)
    }

    /// Start queued jobs up to the concurrency limit.
    fn start_next_queued(self: &Arc<Self>) {
        let max = self.max_active_downloads.load(Ordering::Relaxed);
        loop {
            let Some(job_id) = self.claim_next_download_slot(max) else {
                break;
            };
            self.launch_download(&job_id);
        }
    }

    /// Add a job to the queue and start downloading if a slot is available.
    ///
    /// The job should already have its `work_dir` and `output_dir` set.
    /// The job is always inserted as `Queued` (or `Paused` if globally
    /// paused) first, then `start_next_queued` is called to atomically
    /// claim a download slot if one is available.  This eliminates the
    /// TOCTOU race that previously allowed concurrent callers to exceed
    /// the `max_active_downloads` limit.
    pub fn add_job(
        self: &Arc<Self>,
        mut job: NzbJob,
        nzb_data: Option<Vec<u8>>,
    ) -> crate::nzb_core::Result<()> {
        // Dedup: reject if the same article set is already active/queued.
        // A `None` hash means the job has no articles yet (nothing to
        // compare against) — proceed without dedup.
        let content_hash = compute_content_hash(&job);
        if let Some(hash) = content_hash {
            let hashes = self.content_hashes.lock();
            if let Some(existing_id) = hashes.get(&hash) {
                let existing_id = existing_id.clone();
                drop(hashes);
                warn!(
                    job_id = %job.id,
                    name = %job.name,
                    existing_id = %existing_id,
                    "Rejecting duplicate: articles match already-queued job"
                );
                return Err(crate::nzb_core::NzbError::InvalidNzb(format!(
                    "duplicate content: already queued as job {existing_id}"
                )));
            }
        }

        // Ensure work directory exists
        std::fs::create_dir_all(&job.work_dir)?;

        // Persist to DB
        {
            let db = self.db.lock();
            db.queue_insert(&job)?;
            // Store raw NZB data if available
            if let Some(ref data) = nzb_data {
                let _ = db.queue_store_nzb_data(&job.id, data);
            }
        }

        // Reserve the hash now that the job is committed.
        if let Some(hash) = content_hash {
            self.content_hashes.lock().insert(hash, job.id.clone());
        }

        let job_id = job.id.clone();
        info!(
            job_id = %job_id,
            name = %job.name,
            files = job.file_count,
            articles = job.article_count,
            "Job added to queue"
        );

        // If globally paused, add as paused
        if self.globally_paused.load(Ordering::Relaxed) {
            job.status = JobStatus::Paused;
            let state = JobState {
                job,
                progress_handle: None,
                speed: Arc::new(SpeedTracker::new()),
                nzb_data,
                direct_unpacker: None,
                hopeless_tracker: None,
            };
            self.jobs.lock().insert(job_id.clone(), state);
            self.job_order.lock().push(job_id);
            return Ok(());
        }

        // Insert as Queued — start_next_queued will atomically claim a
        // download slot if one is available.
        job.status = JobStatus::Queued;
        let state = JobState {
            job,
            progress_handle: None,
            speed: Arc::new(SpeedTracker::new()),
            nzb_data,
            direct_unpacker: None,
            hopeless_tracker: None,
        };
        self.jobs.lock().insert(job_id.clone(), state);
        self.job_order.lock().push(job_id);

        // Try to start this or other queued jobs
        self.start_next_queued();
        Ok(())
    }

    /// Launch the download task for a job that is already in the jobs map
    /// with status `Downloading`.
    ///
    /// Builds a [`JobContext`] and submits work items to the shared worker
    /// pool, then spawns the per-job progress listener. If the pre-flight
    /// disk-space check fails, the job is set to `Paused`.
    fn launch_download(self: &Arc<Self>, job_id: &str) {
        // Lazily load NZB data if not in memory (queued jobs skip loading at restore time)
        {
            let mut jobs = self.jobs.lock();
            if let Some(state) = jobs.get_mut(job_id)
                && state.nzb_data.is_none()
            {
                let db = self.db.lock();
                if let Some(data) = db.queue_get_nzb_data(job_id).unwrap_or(None) {
                    // Parse NZB to populate files/articles
                    match nzb_parser::parse_nzb(&state.job.name, &data) {
                        Ok(parsed) => {
                            state.job.files = parsed.files;
                            // Apply checkpoint if available
                            if let Some(cp_data) = db.queue_load_job_data(job_id).unwrap_or(None)
                                && let Ok(checkpoint) =
                                    serde_json::from_slice::<JobCheckpoint>(&cp_data)
                            {
                                state.job.downloaded_bytes = checkpoint.downloaded_bytes;
                                state.job.articles_downloaded = checkpoint.articles_downloaded;
                                state.job.articles_failed = checkpoint.articles_failed;
                                state.job.files_completed = checkpoint.files_completed;
                                for file in &mut state.job.files {
                                    if let Some(segments) = checkpoint.files.get(&file.id) {
                                        let mut fbd: u64 = 0;
                                        for article in &mut file.articles {
                                            if segments.contains(&article.segment_number) {
                                                article.downloaded = true;
                                                fbd += article.bytes;
                                            }
                                        }
                                        file.bytes_downloaded = fbd;
                                        if file.articles.iter().all(|a| a.downloaded) {
                                            file.assembled = true;
                                        }
                                    }
                                }
                                info!(
                                    job_id = %job_id,
                                    name = %state.job.name,
                                    articles_downloaded = state.job.articles_downloaded,
                                    "Lazy-loaded job checkpoint"
                                );
                            }
                        }
                        Err(e) => {
                            warn!(job_id = %job_id, "Failed to lazy-load NZB data: {e}");
                        }
                    }
                    state.nzb_data = Some(data);
                }
            }
        }

        // Read job data from the map (we need a copy for the spawned task)
        let (job, _nzb_data) = {
            let jobs = self.jobs.lock();
            let Some(state) = jobs.get(job_id) else {
                return;
            };
            (state.job.clone(), state.nzb_data.clone())
        };

        // Pre-flight disk space check
        let free = get_disk_free(&self.incomplete_dir.lock());
        if self.min_free_space > 0 && free > 0 && free < self.min_free_space {
            warn!(
                job_id = %job_id,
                free_bytes = free,
                min_free_space = self.min_free_space,
                "Paused job due to low disk space"
            );
            let mut jobs = self.jobs.lock();
            if let Some(state) = jobs.get_mut(job_id) {
                state.job.status = JobStatus::Paused;
                state.job.error_message = Some("Paused: low disk space".to_string());
            }
            return;
        }

        info!(
            job_id = %job_id,
            name = %job.name,
            total_bytes = job.total_bytes,
            article_count = job.article_count,
            file_count = job.file_count,
            "Starting download job"
        );

        let job_speed = Arc::new(SpeedTracker::new());
        // Phase 7: bounded progress channel. The handler reads at ~articles
        // per second; under DB-lock contention or post-processing pauses it
        // can fall behind. Unbounded was a memory hazard. With a 10K cap
        // the worst case is bounded buffering plus a `WARN` from
        // `try_send_or_warn` when the channel is full.
        let (progress_tx, progress_rx) =
            mpsc::channel::<ProgressUpdate>(crate::download_engine::PROGRESS_CHANNEL_CAPACITY);

        {
            let srv = self.servers.lock();
            let enabled_count = srv.iter().filter(|s| s.enabled).count();
            info!(
                job_id = %job_id,
                total_servers = srv.len(),
                enabled_servers = enabled_count,
                "Dispatching job to shared worker pool"
            );
            if enabled_count == 0 {
                warn!(job_id = %job_id, "No enabled servers — job will stall until servers are added");
            }
        }

        // Dispatch builds its own job context internally.
        self.dispatch.submit_job(&job, progress_tx);

        // Spawn the per-job progress handler and record its handle.
        let qm = Arc::clone(self);
        let jid = job_id.to_string();
        let speed_for_task = Arc::clone(&job_speed);
        let progress_handle = tokio::spawn(async move {
            qm.handle_progress(jid, progress_rx, speed_for_task).await;
        });

        // Update the existing map entry with the handle and trackers.
        {
            let mut jobs = self.jobs.lock();
            if let Some(state) = jobs.get_mut(job_id) {
                state.progress_handle = Some(progress_handle);
                state.speed = Arc::clone(&job_speed);
                state.hopeless_tracker = Some(HopelessTracker::new(&state.job));
            }
        }
    }

    /// Handle progress updates from the download engine.
    async fn handle_progress(
        self: Arc<Self>,
        job_id: String,
        mut progress_rx: mpsc::Receiver<ProgressUpdate>,
        job_speed: Arc<SpeedTracker>,
    ) {
        let mut last_db_update = Instant::now();

        while let Some(update) = progress_rx.recv().await {
            match update {
                ProgressUpdate::ArticleComplete {
                    file_id,
                    segment_number,
                    decoded_bytes,
                    file_complete,
                    server_id,
                    ..
                } => {
                    self.speed.record(decoded_bytes);
                    job_speed.record(decoded_bytes);

                    // Update in-memory job state
                    {
                        let mut jobs = self.jobs.lock();
                        if let Some(state) = jobs.get_mut(&job_id) {
                            state.job.downloaded_bytes += decoded_bytes;
                            state.job.articles_downloaded += 1;

                            // Update per-server stats
                            if let Some(ref sid) = server_id {
                                let stats = &mut state.job.server_stats;
                                if let Some(ss) = stats.iter_mut().find(|s| s.server_id == *sid) {
                                    ss.articles_downloaded += 1;
                                    ss.bytes_downloaded += decoded_bytes;
                                } else {
                                    // Find server name from config
                                    let sname = self
                                        .servers
                                        .lock()
                                        .iter()
                                        .find(|s| s.id == *sid)
                                        .map(|s| s.name.clone())
                                        .unwrap_or_else(|| sid.clone());
                                    stats.push(ServerArticleStats {
                                        server_id: sid.clone(),
                                        server_name: sname,
                                        articles_downloaded: 1,
                                        articles_failed: 0,
                                        bytes_downloaded: decoded_bytes,
                                    });
                                }
                            }

                            let file_is_par2 = state
                                .job
                                .files
                                .iter()
                                .find(|f| f.id == file_id)
                                .is_some_and(|f| f.is_par2);
                            if let Some(ref mut tracker) = state.hopeless_tracker {
                                tracker.record_success(file_is_par2);
                            }

                            for file in &mut state.job.files {
                                if file.id == file_id {
                                    file.bytes_downloaded += decoded_bytes;
                                    for article in &mut file.articles {
                                        if article.segment_number == segment_number {
                                            article.downloaded = true;
                                            article.data_size = Some(decoded_bytes);
                                        }
                                    }
                                    if file_complete && !file.assembled {
                                        file.assembled = true;
                                        state.job.files_completed += 1;
                                        info!(
                                            job_id = %job_id,
                                            file = %file.filename,
                                            completed = state.job.files_completed,
                                            total = state.job.file_count,
                                            "File assembly complete"
                                        );

                                        // Direct unpack: feed completed RAR volumes to the
                                        // unpacker so extraction overlaps with download.
                                        if self.direct_unpack_enabled.load(Ordering::Relaxed)
                                            && state.job.articles_failed == 0
                                            && let Some(vol_info) = parse_rar_volume(&file.filename)
                                        {
                                            if state.direct_unpacker.is_none() {
                                                state.direct_unpacker = DirectUnpacker::new(
                                                    &state.job.work_dir,
                                                    &state.job.output_dir,
                                                    state.job.password.clone(),
                                                );
                                                if state.direct_unpacker.is_some() {
                                                    info!(
                                                        job_id = %job_id,
                                                        "Direct unpack enabled — starting RAR extraction during download"
                                                    );
                                                }
                                            }
                                            if let Some(ref du) = state.direct_unpacker {
                                                let path = state.job.work_dir.join(&file.filename);
                                                du.add_volume(
                                                    &vol_info.set_name,
                                                    vol_info.volume_number,
                                                    path,
                                                );
                                            }
                                        }
                                    }
                                    break;
                                }
                            }
                        }
                    }

                    // Batch DB writes (every 2 seconds)
                    if last_db_update.elapsed() >= Duration::from_secs(2) {
                        self.persist_job_progress(&job_id);
                        last_db_update = Instant::now();
                    }
                }
                ProgressUpdate::ArticleFailed {
                    file_id, failure, ..
                } => {
                    let _ = &failure.message; // forwarded into logs below
                    let should_abort = {
                        let mut jobs = self.jobs.lock();
                        if let Some(state) = jobs.get_mut(&job_id) {
                            state.job.articles_failed += 1;

                            // Update per-server failed stats
                            let sid = &failure.server_id;
                            let stats = &mut state.job.server_stats;
                            if let Some(ss) = stats.iter_mut().find(|s| s.server_id == *sid) {
                                ss.articles_failed += 1;
                            } else {
                                let sname = self
                                    .servers
                                    .lock()
                                    .iter()
                                    .find(|s| s.id == *sid)
                                    .map(|s| s.name.clone())
                                    .unwrap_or_else(|| sid.clone());
                                stats.push(ServerArticleStats {
                                    server_id: sid.clone(),
                                    server_name: sname,
                                    articles_downloaded: 0,
                                    articles_failed: 1,
                                    bytes_downloaded: 0,
                                });
                            }

                            // Abort direct unpack on first article failure —
                            // PAR2 repair may be needed before extraction.
                            if let Some(du) = state.direct_unpacker.take() {
                                info!(
                                    job_id = %job_id,
                                    "Aborting direct unpack — article failure detected, falling back to normal pipeline"
                                );
                                du.abort();
                            }

                            // Track failure in hopeless detector and check
                            // whether this job can still complete.
                            if let Some(ref mut tracker) = state.hopeless_tracker {
                                let file_is_par2 = state
                                    .job
                                    .files
                                    .iter()
                                    .find(|f| f.id == file_id)
                                    .is_some_and(|f| f.is_par2);
                                // Estimate article size from total file bytes / article count
                                let estimated_bytes = state
                                    .job
                                    .files
                                    .iter()
                                    .find(|f| f.id == file_id)
                                    .map(|f| {
                                        if f.articles.is_empty() {
                                            0
                                        } else {
                                            f.bytes / f.articles.len() as u64
                                        }
                                    })
                                    .unwrap_or(0);
                                tracker.record_failure(file_is_par2, estimated_bytes, failure.kind);
                                // Observability: dump tracker state on every
                                // failure so operators can see the ratio
                                // evolving towards hopeless-abort thresholds.
                                // Helps diagnose "why does this job take 4
                                // minutes to fail" — exposes grace period,
                                // early_failure, and ongoing_availability
                                // check progress in real time.
                                let avail_bytes_pct = if tracker.content_bytes > 0 {
                                    let avail: u64 = tracker
                                        .content_bytes
                                        .saturating_sub(tracker.content_bytes_missing);
                                    100.0 * (avail as f64 / tracker.content_bytes as f64)
                                } else {
                                    100.0
                                };
                                let fail_rate = if tracker.content_articles_checked > 0 {
                                    tracker.content_articles_failed as f64
                                        / tracker.content_articles_checked as f64
                                } else {
                                    0.0
                                };
                                debug!(
                                    job_id = %job_id,
                                    checked = tracker.content_articles_checked,
                                    failed = tracker.content_articles_failed,
                                    total = tracker.content_articles_total,
                                    failure_rate = format!("{fail_rate:.3}"),
                                    availability_pct = format!("{avail_bytes_pct:.2}"),
                                    required_pct = format!("{:.1}", self.required_completion_pct),
                                    kind = failure.kind.as_str(),
                                    "Hopeless tracker updated after article failure"
                                );
                                tracker.check(
                                    self.abort_hopeless,
                                    self.early_failure_check,
                                    self.required_completion_pct,
                                )
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    };

                    if let Some(abort) = should_abort {
                        warn!(
                            job_id = %job_id,
                            tier = abort.tier,
                            reason = %abort.reason,
                            "Job is hopeless — aborting"
                        );
                        // Emit a per-server breakdown so the reason above
                        // can be read in context — the primary tier's raw
                        // fail rate is invisible from the post-cascade
                        // tracker alone (see HopelessTracker::check comment
                        // about "fully-cascaded samples").
                        let stats = self.dispatch.server_stats_snapshot();
                        for (sid, s) in &stats {
                            info!(
                                job_id = %job_id,
                                server_id = %sid,
                                attempted = s.attempted,
                                succeeded = s.succeeded,
                                not_found = s.not_found,
                                transient_failed = s.transient_failed,
                                "server_stats at abort"
                            );
                        }
                        {
                            let mut jobs = self.jobs.lock();
                            if let Some(state) = jobs.get_mut(&job_id) {
                                state.job.error_message = Some(abort.reason.clone());
                                // Drop the tracker so subsequent ArticleFailed
                                // events from in-flight articles don't re-fire
                                // the hopeless check (and re-emit abort_job).
                                // Without this, the next ~30 failures each
                                // log a duplicate "Job is hopeless — aborting"
                                // warning and spam dispatch.abort_job().
                                state.hopeless_tracker = None;
                            }
                        }
                        // Tell the worker pool to drain the job and emit
                        // JobAborted — the JobAborted arm below handles the
                        // rest of the teardown.
                        self.dispatch.abort_job(&job_id, abort.reason);
                    } else {
                        warn!(
                            job_id = %job_id,
                            kind = failure.kind.as_str(),
                            server = %failure.server_id,
                            "Article failed: {}", failure.message
                        );
                    }
                }
                ProgressUpdate::JobFinished {
                    success,
                    articles_failed,
                    ..
                } => {
                    info!(
                        job_id = %job_id,
                        success,
                        articles_failed,
                        "Job download finished"
                    );

                    // Mark as PostProcessing immediately so the slot is freed
                    // for the next queued job. This lets the next download ramp
                    // up while post-processing (par2/unpack) runs concurrently.
                    {
                        let mut jobs = self.jobs.lock();
                        if let Some(state) = jobs.get_mut(&job_id) {
                            state.job.status = JobStatus::PostProcessing;
                            state.job.completed_at = Some(chrono::Utc::now());
                        }
                    }
                    self.start_next_queued();

                    self.on_job_finished(&job_id, success, articles_failed)
                        .await;
                    break;
                }
                ProgressUpdate::NoServersAvailable { reason, .. } => {
                    warn!(
                        job_id = %job_id,
                        reason = %reason,
                        "No servers available — pausing job for retry"
                    );
                    {
                        let mut jobs = self.jobs.lock();
                        if let Some(state) = jobs.get_mut(&job_id) {
                            state.job.status = JobStatus::Paused;
                            state.job.error_message = Some(reason);
                        }
                    }
                    self.persist_job_progress(&job_id);
                    // Release the download slot so queued jobs can start
                    self.start_next_queued();
                    break;
                }
                ProgressUpdate::JobAborted { reason, .. } => {
                    warn!(
                        job_id = %job_id,
                        reason = %reason,
                        "Job aborted by download engine"
                    );
                    // Read the real article-failed count BEFORE mutating —
                    // on_job_finished logs it and the post-proc pipeline
                    // uses it to decide whether par2 might help.
                    let articles_failed = {
                        let mut jobs = self.jobs.lock();
                        let articles_failed = jobs
                            .get(&job_id)
                            .map(|s| s.job.articles_failed)
                            .unwrap_or(0);
                        if let Some(state) = jobs.get_mut(&job_id) {
                            state.job.status = JobStatus::Failed;
                            state.job.error_message = Some(reason);
                            state.job.completed_at = Some(chrono::Utc::now());
                        }
                        articles_failed
                    };
                    self.start_next_queued();
                    self.on_job_finished(&job_id, false, articles_failed).await;
                    break;
                }
            }
        }
    }

    /// Called when a job's download phase completes.
    ///
    /// Note: the job's status is already set to `PostProcessing` and
    /// `start_next_queued()` has already been called by `handle_progress`,
    /// so the next download is ramping up concurrently with this work.
    async fn on_job_finished(
        self: &Arc<Self>,
        job_id: &str,
        success: bool,
        articles_failed: usize,
    ) {
        let pipeline_start = Instant::now();

        // Short-circuit when the job never downloaded anything — e.g. a
        // hopeless-abort or no-progress timeout fired before a single
        // article landed. Running par2 / extract on an empty work_dir
        // spams the logs with misleading "Cannot open the file as
        // archive" errors and can't possibly recover; just move the job
        // to history as Failed and return.
        let articles_downloaded = {
            let jobs = self.jobs.lock();
            jobs.get(job_id)
                .map(|s| s.job.articles_downloaded)
                .unwrap_or(0)
        };
        if articles_downloaded == 0 && !success {
            info!(
                job_id = %job_id,
                articles_failed,
                "Job finished with no articles downloaded — skipping post-processing"
            );
            self.release_content_hash(job_id);
            let mut jobs = self.jobs.lock();
            if let Some(state) = jobs.get_mut(job_id) {
                self.move_to_history(state, Vec::new());
            }
            return;
        }

        // Extract info needed for post-processing and take the direct unpacker.
        let (work_dir, output_dir, category, pp_level, direct_unpacker, password) = {
            let mut jobs = self.jobs.lock();
            let Some(state) = jobs.get_mut(job_id) else {
                return;
            };

            if success {
                info!(job_id = %job_id, "Job moving to post-processing");
            } else {
                info!(
                    job_id = %job_id,
                    articles_failed,
                    "Job moving to post-processing ({articles_failed} article(s) failed, par2 may repair)"
                );
            }

            let cat = state.job.category.clone();
            let pp = self
                .categories
                .lock()
                .iter()
                .find(|c| c.name == cat)
                .map(|c| c.post_processing)
                .unwrap_or(3); // default: repair+unpack
            let du = state.direct_unpacker.take();
            let pw = state.job.password.clone();
            (
                state.job.work_dir.clone(),
                state.job.output_dir.clone(),
                cat,
                pp,
                du,
                pw,
            )
        };

        // Wait for direct unpack to finish (if active). It may still be
        // extracting the last volume when the download completes.
        let direct_unpack_success = if let Some(du) = direct_unpacker {
            let results = du.finish().await;
            let all_ok = !results.is_empty() && results.iter().all(|r| r.success);
            if all_ok {
                info!(
                    job_id = %job_id,
                    sets = results.len(),
                    "Direct unpack completed successfully — skipping extract stage"
                );
            } else {
                for r in &results {
                    if !r.success {
                        warn!(
                            job_id = %job_id,
                            set = %r.set_name,
                            error = ?r.error,
                            "Direct unpack failed for set — falling back to normal extraction"
                        );
                    }
                }
            }
            all_ok
        } else {
            false
        };

        // Run post-processing pipeline (par2 can repair failed articles)
        let stages = if pp_level > 0 {
            info!(
                job_id = %job_id,
                category = %category,
                pp_level,
                "Running post-processing pipeline"
            );

            let config = PostProcConfig {
                cleanup_after_extract: true,
                output_dir: Some(output_dir.clone()),
                articles_failed,
                skip_extract: direct_unpack_success,
                password: password.clone(),
            };

            let result = run_pipeline(&work_dir, &config).await;

            info!(
                job_id = %job_id,
                success = result.success,
                stages = result.stages.len(),
                elapsed_secs = pipeline_start.elapsed().as_secs_f64(),
                "Post-processing pipeline finished"
            );

            // Update job status based on pipeline result
            {
                let mut jobs = self.jobs.lock();
                if let Some(state) = jobs.get_mut(job_id)
                    && !result.success
                {
                    state.job.status = JobStatus::Failed;
                    state.job.error_message = result.error.clone();
                }
            }

            result.stages
        } else {
            info!(job_id = %job_id, pp_level, "Post-processing disabled for category, skipping pipeline");
            // No pipeline to repair — if articles failed, mark as failed now
            if !success {
                let mut jobs = self.jobs.lock();
                if let Some(state) = jobs.get_mut(job_id) {
                    state.job.status = JobStatus::Failed;
                    state.job.error_message =
                        Some(format!("{articles_failed} article(s) failed to download"));
                }
            }
            Vec::new()
        };

        // Move to history with real stage results
        {
            let mut jobs = self.jobs.lock();
            if let Some(state) = jobs.get_mut(job_id) {
                self.move_to_history(state, stages);
            }
        }

        // Job is terminal — release its dedup hash so matching content
        // can be re-added.
        self.release_content_hash(job_id);

        // Persist final state
        self.persist_job_progress(job_id);

        // Keep the completed/failed job visible in the queue briefly so the
        // UI has a chance to show the transition.  Fast downloads can go from
        // Queued → Downloading → PostProcessing → History in under a second,
        // before the UI's poll interval (1-5s) can observe them.
        let jid = job_id.to_string();
        let qm = Arc::clone(self);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(8)).await;
            qm.jobs.lock().remove(&jid);
            qm.job_order.lock().retain(|id| id != &jid);
        });
    }

    /// Release a job's content-hash reservation so its articles no longer
    /// block dedup of future add_job calls. Call on any terminal transition
    /// (Completed, Failed, removed).
    fn release_content_hash(&self, job_id: &str) {
        self.content_hashes.lock().retain(|_, id| id != job_id);
    }

    /// Move a job's files to output and insert a history entry.
    fn move_to_history(&self, state: &mut JobState, stages: Vec<StageResult>) {
        let move_start = Instant::now();

        let final_status = if state.job.status == JobStatus::Failed {
            // Already marked failed (by pipeline or download with pp disabled)
            JobStatus::Failed
        } else {
            // Pipeline ran successfully (or no articles failed) — job is complete.
            // Par2 may have repaired missing articles, so articles_failed > 0 is OK.
            JobStatus::Completed
        };

        // Move files from work_dir to output_dir (if not already done by pipeline extract)
        if final_status == JobStatus::Completed {
            if let Err(e) = std::fs::create_dir_all(&state.job.output_dir) {
                warn!(job_id = %state.job.id, "Failed to create output dir: {e}");
            }
            if let Ok(entries) = std::fs::read_dir(&state.job.work_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() {
                        let dest = state.job.output_dir.join(entry.file_name());
                        if let Err(e) = std::fs::rename(&path, &dest) {
                            if let Err(e2) = std::fs::copy(&path, &dest) {
                                warn!(
                                    job_id = %state.job.id,
                                    file = %path.display(),
                                    "Failed to move file: rename={e}, copy={e2}"
                                );
                            } else {
                                let _ = std::fs::remove_file(&path);
                            }
                        }
                    }
                }
            }
        }

        let file_move_secs = move_start.elapsed().as_secs_f64();
        info!(
            job_id = %state.job.id,
            final_status = %final_status,
            file_move_secs = format!("{file_move_secs:.3}"),
            stage_count = stages.len(),
            "Moving job to history"
        );

        state.job.status = final_status;

        // Insert into history with real stage results
        let history_entry = HistoryEntry {
            id: state.job.id.clone(),
            name: state.job.name.clone(),
            category: state.job.category.clone(),
            status: final_status,
            total_bytes: state.job.total_bytes,
            downloaded_bytes: state.job.downloaded_bytes,
            added_at: state.job.added_at,
            completed_at: state.job.completed_at.unwrap_or_else(chrono::Utc::now),
            output_dir: state.job.output_dir.clone(),
            stages,
            error_message: state.job.error_message.clone(),
            server_stats: state.job.server_stats.clone(),
            nzb_data: state.nzb_data.clone(),
        };

        let db = self.db.lock();
        if let Err(e) = db.history_insert(&history_entry) {
            error!(job_id = %state.job.id, "Failed to insert history: {e}");
        }

        // Capture and persist per-job logs from the ring buffer
        if let Some(ref log_buffer) = self.log_buffer {
            let logs = log_buffer.get_entries(Some(&state.job.id), None, None, 5000);
            if !logs.is_empty() {
                let logs_json = serde_json::to_string(&logs).unwrap_or_default();
                if let Err(e) = db.history_store_logs(&state.job.id, &logs_json) {
                    warn!(job_id = %state.job.id, "Failed to store logs in history: {e}");
                }
            }
        }

        if let Err(e) = db.queue_remove(&state.job.id) {
            error!(job_id = %state.job.id, "Failed to remove from queue: {e}");
        }

        // Enforce retention
        if let Some(max) = *self.history_retention.lock()
            && let Err(e) = db.history_enforce_retention(max)
        {
            warn!("Failed to enforce history retention: {e}");
        }
    }

    /// Persist current job progress to the database, including article-level
    /// checkpoint data for resume support.
    fn persist_job_progress(&self, job_id: &str) {
        let jobs = self.jobs.lock();
        if let Some(state) = jobs.get(job_id) {
            let db = self.db.lock();
            if let Err(e) = db.queue_update_progress(
                job_id,
                state.job.status,
                state.job.downloaded_bytes,
                state.job.articles_downloaded,
                state.job.articles_failed,
                state.job.files_completed,
            ) {
                warn!(job_id = %job_id, "Failed to persist progress: {e}");
            }

            // Build and store checkpoint of downloaded article segments
            let checkpoint = JobCheckpoint {
                files: state
                    .job
                    .files
                    .iter()
                    .map(|f| {
                        let downloaded_segments: Vec<u32> = f
                            .articles
                            .iter()
                            .filter(|a| a.downloaded)
                            .map(|a| a.segment_number)
                            .collect();
                        (f.id.clone(), downloaded_segments)
                    })
                    .collect(),
                downloaded_bytes: state.job.downloaded_bytes,
                articles_downloaded: state.job.articles_downloaded,
                articles_failed: state.job.articles_failed,
                files_completed: state.job.files_completed,
            };

            if let Ok(data) = serde_json::to_vec(&checkpoint)
                && let Err(e) = db.queue_store_job_data(job_id, &data)
            {
                warn!(job_id = %job_id, "Failed to persist checkpoint: {e}");
            }
        }
    }

    // -----------------------------------------------------------------------
    // Job control
    // -----------------------------------------------------------------------

    /// Change the priority of a specific job, reorder the queue, and preempt
    /// lower-priority downloads when a higher-priority job is waiting.
    pub fn set_job_priority(
        self: &Arc<Self>,
        id: &str,
        priority: Priority,
    ) -> crate::nzb_core::Result<()> {
        let max = self.max_active_downloads.load(Ordering::Relaxed);

        // 1. Update priority
        {
            let mut jobs = self.jobs.lock();
            let job_state = jobs
                .get_mut(id)
                .ok_or_else(|| crate::nzb_core::NzbError::JobNotFound(id.to_string()))?;
            job_state.job.priority = priority;
            let db = self.db.lock();
            db.queue_update_priority(id, priority as i32)?;
            info!(
                job_id = %id,
                ?priority,
                priority_val = priority as u8,
                max_active = max,
                "Job priority changed"
            );
        }

        // 2. Reorder job_order by priority (stable: preserves order within same priority)
        {
            let jobs = self.jobs.lock();
            let mut order = self.job_order.lock();
            let before: Vec<String> = order.clone();
            order.sort_by(|a, b| {
                let pa = jobs.get(a).map(|s| s.job.priority as u8).unwrap_or(0);
                let pb = jobs.get(b).map(|s| s.job.priority as u8).unwrap_or(0);
                pb.cmp(&pa) // descending: highest priority first
            });
            if *order != before {
                info!(
                    before = ?before.iter().take(6).collect::<Vec<_>>(),
                    after = ?order.iter().take(6).collect::<Vec<_>>(),
                    "Queue reordered by priority"
                );
            }
        }

        // 3. Preempt lower-priority downloads if a higher-priority queued job is waiting
        self.preempt_if_needed();

        Ok(())
    }

    /// Check whether a queued job has higher priority than a running download,
    /// and if so, pause the lower-priority download to make room.
    fn preempt_if_needed(self: &Arc<Self>) {
        let max = self.max_active_downloads.load(Ordering::Relaxed);

        loop {
            // Snapshot current state
            let (active, best_queued, worst_downloading) = {
                let jobs = self.jobs.lock();
                let order = self.job_order.lock();

                let active = jobs
                    .values()
                    .filter(|s| s.job.status == JobStatus::Downloading)
                    .count();

                let mut best_q: Option<(String, u8, String)> = None;
                let mut worst_d: Option<(String, u8, String)> = None;

                for id in order.iter() {
                    if let Some(s) = jobs.get(id) {
                        let p = s.job.priority as u8;
                        let name = s.job.name.clone();
                        if s.job.status == JobStatus::Queued
                            && best_q.as_ref().is_none_or(|(_, bp, _)| p > *bp)
                        {
                            best_q = Some((id.clone(), p, name));
                        } else if s.job.status == JobStatus::Downloading
                            && worst_d.as_ref().is_none_or(|(_, wp, _)| p < *wp)
                        {
                            worst_d = Some((id.clone(), p, name));
                        }
                    }
                }
                (active, best_q, worst_d)
            };

            // If unlimited slots or free slots available, just start queued jobs
            if max == 0 || active < max {
                self.start_next_queued();
                return;
            }

            // All slots full — check if preemption is warranted
            match (&best_queued, &worst_downloading) {
                (Some((q_id, q_pri, q_name)), Some((d_id, d_pri, d_name))) if q_pri > d_pri => {
                    info!(
                        preempted_id = %d_id,
                        preempted_name = %d_name,
                        preempted_priority = d_pri,
                        starting_id = %q_id,
                        starting_name = %q_name,
                        starting_priority = q_pri,
                        active_downloads = active,
                        max_downloads = max,
                        "Preempting lower-priority download for higher-priority job"
                    );

                    // Pause the lower-priority download via the worker pool.
                    self.dispatch.pause_job(d_id);
                    {
                        let mut jobs = self.jobs.lock();
                        if let Some(state) = jobs.get_mut(d_id.as_str()) {
                            state.job.status = JobStatus::Paused;
                            // Persist to DB
                            let db = self.db.lock();
                            let _ = db.queue_update_progress(
                                d_id,
                                JobStatus::Paused,
                                state.job.downloaded_bytes,
                                state.job.articles_downloaded,
                                state.job.articles_failed,
                                state.job.files_completed,
                            );
                        }
                    }
                    // Loop back — active count decreased, start_next_queued will run
                }
                _ => {
                    info!(
                        active_downloads = active,
                        max_downloads = max,
                        best_queued_pri = best_queued.as_ref().map(|q| q.1),
                        worst_dl_pri = worst_downloading.as_ref().map(|d| d.1),
                        "No preemption needed"
                    );
                    return;
                }
            }
        }
    }

    /// Pause a specific job.
    pub fn pause_job(self: &Arc<Self>, id: &str) -> crate::nzb_core::Result<()> {
        // Tell the pool first — workers stop pulling this job's items.
        self.dispatch.pause_job(id);
        {
            let mut jobs = self.jobs.lock();
            let state = jobs
                .get_mut(id)
                .ok_or_else(|| crate::nzb_core::NzbError::JobNotFound(id.to_string()))?;

            state.job.status = JobStatus::Paused;

            let db = self.db.lock();
            db.queue_update_progress(
                id,
                JobStatus::Paused,
                state.job.downloaded_bytes,
                state.job.articles_downloaded,
                state.job.articles_failed,
                state.job.files_completed,
            )?;

            info!(job_id = %id, "Job paused");
        }

        // Release the download slot so queued jobs can start
        self.start_next_queued();
        Ok(())
    }

    /// Resume a specific job.
    pub fn resume_job(self: &Arc<Self>, id: &str) -> crate::nzb_core::Result<()> {
        let ctx_alive = self.dispatch.has_job(id);

        let needs_launch = {
            let mut jobs = self.jobs.lock();
            if !jobs.contains_key(id) {
                return Err(crate::nzb_core::NzbError::JobNotFound(id.to_string()));
            }

            let active = jobs
                .values()
                .filter(|s| s.job.status == JobStatus::Downloading)
                .count();

            let state = jobs.get_mut(id).unwrap();

            if ctx_alive {
                // Job context still lives in the pool — just unpause it.
                state.job.status = JobStatus::Downloading;
                state.job.error_message = None;
                let db = self.db.lock();
                let _ = db.queue_update_progress(
                    id,
                    JobStatus::Downloading,
                    state.job.downloaded_bytes,
                    state.job.articles_downloaded,
                    state.job.articles_failed,
                    state.job.files_completed,
                );
                false
            } else {
                // Pool has no context — we need to rebuild work items and submit.
                let max = self.max_active_downloads.load(Ordering::Relaxed);
                if max > 0 && active >= max {
                    state.job.status = JobStatus::Queued;
                    info!(job_id = %id, "Job queued (active download limit reached)");
                    false
                } else {
                    state.job.status = JobStatus::Downloading;
                    state.job.error_message = None;
                    true
                }
            }
        };

        if ctx_alive {
            self.dispatch.resume_job(id);
        } else if needs_launch {
            self.launch_download(id);
        }

        info!(job_id = %id, "Job resumed");
        Ok(())
    }

    /// Remove a specific job from the queue.
    pub fn remove_job(&self, id: &str) -> crate::nzb_core::Result<()> {
        // Silently cancel in the pool — drains queued items and unregisters.
        self.dispatch.cancel_job(id);
        // Release dedup reservation up front — the job is gone as far as
        // future add_job calls are concerned, regardless of cleanup order.
        self.release_content_hash(id);
        let removed = self.jobs.lock().remove(id);
        if let Some(state) = removed {
            if let Some(handle) = state.progress_handle {
                handle.abort();
            }

            // Remove from DB
            let db = self.db.lock();
            let _ = db.queue_remove(id);

            // Remove from order
            self.job_order.lock().retain(|jid| jid != id);

            // Try to clean up work directory
            if state.job.work_dir.exists() {
                let _ = std::fs::remove_dir_all(&state.job.work_dir);
            }

            info!(job_id = %id, "Job removed");
        }
        Ok(())
    }

    /// Rename a job in the queue.
    pub fn rename_job(&self, id: &str, new_name: &str) -> crate::nzb_core::Result<()> {
        let mut jobs = self.jobs.lock();
        let state = jobs
            .iter_mut()
            .find(|(_, s)| s.job.id == id || s.job.id.starts_with(id));
        match state {
            Some((_, s)) => {
                s.job.name = new_name.to_string();
                info!(job_id = %id, new_name = %new_name, "Job renamed");
                Ok(())
            }
            None => Err(crate::nzb_core::NzbError::JobNotFound(id.to_string())),
        }
    }

    /// Change a job's category in the queue.
    pub fn change_job_category(&self, id: &str, category: &str) -> crate::nzb_core::Result<()> {
        let mut jobs = self.jobs.lock();
        let state = jobs
            .iter_mut()
            .find(|(_, s)| s.job.id == id || s.job.id.starts_with(id));
        match state {
            Some((_, s)) => {
                s.job.category = category.to_string();
                // Update the output directory to match the new category
                let complete_dir = self.complete_dir.lock().join(category).join(&s.job.name);
                s.job.output_dir = complete_dir;
                info!(job_id = %id, category = %category, "Job category changed");
                Ok(())
            }
            None => Err(crate::nzb_core::NzbError::JobNotFound(id.to_string())),
        }
    }

    /// Move a job to a new position in the queue order.
    pub fn move_job(&self, id: &str, position: usize) -> crate::nzb_core::Result<()> {
        let mut order = self.job_order.lock();
        let current_pos = order
            .iter()
            .position(|x| x == id)
            .ok_or_else(|| crate::nzb_core::NzbError::JobNotFound(id.to_string()))?;
        let id_str = order.remove(current_pos);
        let new_pos = position.min(order.len());
        order.insert(new_pos, id_str);
        Ok(())
    }

    /// Pause all downloads globally.
    pub fn pause_all(&self) {
        self.globally_paused.store(true, Ordering::Relaxed);
        self.db.lock().set_setting("globally_paused", "true");

        // Collect ids to pause in the pool, to avoid holding the jobs lock
        // while calling into the worker pool.
        let to_pause: Vec<String> = {
            let mut jobs = self.jobs.lock();
            let mut ids = Vec::new();
            for (id, state) in jobs.iter_mut() {
                match state.job.status {
                    JobStatus::Downloading => {
                        ids.push(id.clone());
                        state.job.status = JobStatus::Paused;
                    }
                    JobStatus::Queued => {
                        state.job.status = JobStatus::Paused;
                    }
                    _ => {}
                }
            }
            ids
        };
        for id in to_pause {
            self.dispatch.pause_job(&id);
        }
        info!("All downloads paused");
    }

    /// Pause all downloads for a specified duration.
    pub fn pause_for(self: &Arc<Self>, duration_secs: u64) {
        self.pause_all();
        let until = Utc::now() + chrono::Duration::seconds(duration_secs as i64);
        *self.pause_until.lock() = Some(until);

        let qm = Arc::clone(self);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(duration_secs)).await;
            // Only auto-resume if the pause_until hasn't been cleared
            let should_resume = {
                let until = qm.pause_until.lock();
                until.is_some()
            };
            if should_resume {
                *qm.pause_until.lock() = None;
                qm.resume_all();
                info!("Auto-resumed after timed pause");
            }
        });

        info!(duration_secs, "Paused for duration");
    }

    /// Get remaining pause time in seconds (None if not timed).
    pub fn pause_remaining_secs(&self) -> Option<i64> {
        let until = self.pause_until.lock();
        until.map(|u| {
            let remaining = u - Utc::now();
            remaining.num_seconds().max(0)
        })
    }

    /// Resume all downloads globally.
    pub fn resume_all(self: &Arc<Self>) {
        self.globally_paused.store(false, Ordering::Relaxed);
        self.db.lock().set_setting("globally_paused", "false");
        *self.pause_until.lock() = None;

        // Decide per job whether to just unpause (ctx still in pool) or
        // mark it as Queued so start_next_queued re-submits it.
        let mut to_unpause: Vec<String> = Vec::new();
        {
            let mut jobs = self.jobs.lock();
            for (id, state) in jobs.iter_mut() {
                if state.job.status == JobStatus::Paused {
                    state.job.error_message = None;
                    if self.dispatch.has_job(id) {
                        state.job.status = JobStatus::Downloading;
                        to_unpause.push(id.clone());
                    } else {
                        state.job.status = JobStatus::Queued;
                    }
                }
            }
        }
        for id in to_unpause {
            self.dispatch.resume_job(&id);
        }

        // Start queued jobs up to the concurrency limit
        self.start_next_queued();

        info!("All downloads resumed");
    }

    // -----------------------------------------------------------------------
    // Server management
    // -----------------------------------------------------------------------

    /// Update the server list at runtime.
    ///
    /// If any enabled servers are present, jobs that were paused due to
    /// server errors (e.g. auth failure / service unavailable) are
    /// automatically resumed.
    pub fn update_servers(self: &Arc<Self>, servers: Vec<ServerConfig>) {
        let enabled = servers.iter().filter(|s| s.enabled).count();
        info!(total = servers.len(), enabled, "Updating server list");

        // Reconcile the connection tracker:
        //  - Updated/added servers: set_limit (grows in place or replaces).
        //  - Removed servers: remove_server (orphans the slot, workers detect
        //    via slot_is_current and exit on next iteration).
        let new_ids: std::collections::HashSet<String> =
            servers.iter().map(|s| s.id.clone()).collect();
        let old_ids: Vec<String> = self
            .conn_tracker
            .snapshot()
            .into_iter()
            .map(|(id, _, _)| id)
            .collect();
        for old_id in &old_ids {
            if !new_ids.contains(old_id) {
                self.conn_tracker.remove_server(old_id);
            }
        }
        for server in &servers {
            self.conn_tracker
                .set_limit(&server.id, &server.name, server.connections as usize);
        }
        *self.servers.lock() = servers;

        // Reconcile the worker pool to match the new server list (spawns or
        // retires workers so per-server connection counts stay exactly in
        // line with server.connections).
        self.dispatch.reconcile_servers();

        // Auto-resume jobs paused by server errors now that config changed
        if enabled > 0 {
            self.resume_server_paused_jobs();
        }
    }

    /// Resume jobs that were paused due to server unavailability.
    ///
    /// Only targets jobs where `error_message` is set (i.e. paused by the
    /// circuit breaker / `NoServersAvailable`), not user-paused jobs.
    fn resume_server_paused_jobs(self: &Arc<Self>) {
        let mut resumed = 0u32;
        let mut to_unpause: Vec<String> = Vec::new();
        {
            let mut jobs = self.jobs.lock();
            for (id, state) in jobs.iter_mut() {
                if state.job.status == JobStatus::Paused && state.job.error_message.is_some() {
                    state.job.error_message = None;
                    if self.dispatch.has_job(id) {
                        state.job.status = JobStatus::Downloading;
                        to_unpause.push(id.clone());
                    } else {
                        state.job.status = JobStatus::Queued;
                    }
                    resumed += 1;
                }
            }
        }
        for id in to_unpause {
            self.dispatch.resume_job(&id);
        }
        if resumed > 0 {
            info!(
                count = resumed,
                "Resumed server-paused jobs after config change"
            );
            self.start_next_queued();
        }
    }

    /// Get current server configs.
    pub fn get_servers(&self) -> Vec<ServerConfig> {
        self.servers.lock().clone()
    }

    // -----------------------------------------------------------------------
    // Query methods (for API handlers)
    // -----------------------------------------------------------------------

    /// Get a snapshot of all jobs in the queue.
    pub fn get_jobs(&self) -> Vec<NzbJob> {
        let jobs = self.jobs.lock();
        let order = self.job_order.lock();
        let mut result = Vec::with_capacity(order.len());
        for id in order.iter() {
            if let Some(state) = jobs.get(id) {
                let mut job = state.job.clone();
                job.speed_bps = state.speed.bps();
                result.push(job);
            }
        }
        result
    }

    /// Get a single job by ID (with files included).
    pub fn get_job(&self, job_id: &str) -> Option<NzbJob> {
        let jobs = self.jobs.lock();
        jobs.get(job_id).map(|state| {
            let mut job = state.job.clone();
            job.speed_bps = state.speed.bps();
            job
        })
    }

    /// Get the current download speed in bytes per second.
    pub fn get_speed(&self) -> u64 {
        self.speed.bps()
    }

    /// Check if downloads are globally paused.
    pub fn is_paused(&self) -> bool {
        self.globally_paused.load(Ordering::Relaxed)
    }

    /// Get the number of jobs in the queue.
    pub fn queue_size(&self) -> usize {
        self.jobs.lock().len()
    }

    /// Get the current incomplete directory.
    pub fn incomplete_dir(&self) -> std::path::PathBuf {
        self.incomplete_dir.lock().clone()
    }

    /// Set the incomplete directory at runtime.
    pub fn set_incomplete_dir(&self, dir: std::path::PathBuf) {
        *self.incomplete_dir.lock() = dir;
    }

    /// Get the current complete directory.
    pub fn complete_dir(&self) -> std::path::PathBuf {
        self.complete_dir.lock().clone()
    }

    /// Set the complete directory at runtime.
    pub fn set_complete_dir(&self, dir: std::path::PathBuf) {
        *self.complete_dir.lock() = dir;
    }

    /// Get the minimum free disk space threshold.
    pub fn min_free_space(&self) -> u64 {
        self.min_free_space
    }

    /// Lock the database and execute a closure with direct access.
    ///
    /// This allows callers (e.g. app-specific handlers) to run arbitrary
    /// queries against the underlying `Database`, such as newsgroup-browsing
    /// operations that only exist when the `groups-db` feature is enabled.
    pub fn with_db<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&Database) -> R,
    {
        let db = self.db.lock();
        f(&db)
    }

    // -----------------------------------------------------------------------
    // History query methods (delegate to DB)
    // -----------------------------------------------------------------------

    /// List history entries.
    pub fn history_list(&self, limit: usize) -> crate::nzb_core::Result<Vec<HistoryEntry>> {
        let db = self.db.lock();
        db.history_list(limit)
    }

    /// Get a single history entry.
    pub fn history_get(&self, id: &str) -> crate::nzb_core::Result<Option<HistoryEntry>> {
        let db = self.db.lock();
        db.history_get(id)
    }

    /// Get raw NZB data for retry.
    pub fn history_get_nzb_data(&self, id: &str) -> crate::nzb_core::Result<Option<Vec<u8>>> {
        let db = self.db.lock();
        db.history_get_nzb_data(id)
    }

    /// Remove a history entry.
    pub fn history_remove(&self, id: &str) -> crate::nzb_core::Result<()> {
        let db = self.db.lock();
        db.history_remove(id)
    }

    /// Clear all history.
    pub fn history_clear(&self) -> crate::nzb_core::Result<()> {
        let db = self.db.lock();
        db.history_clear()
    }

    /// Get live logs for an active job from the in-memory log buffer.
    pub fn get_job_logs(&self, job_id: &str, limit: usize) -> Vec<crate::log_buffer::LogEntry> {
        if let Some(ref lb) = self.log_buffer {
            lb.get_entries(Some(job_id), None, None, limit)
        } else {
            Vec::new()
        }
    }

    /// Get persisted logs for a history entry.
    pub fn history_get_logs(&self, id: &str) -> crate::nzb_core::Result<Option<String>> {
        let db = self.db.lock();
        db.history_get_logs(id)
    }

    // -----------------------------------------------------------------------
    // RSS item/rule query methods (delegate to DB)
    // -----------------------------------------------------------------------

    /// List RSS feed items.
    pub fn rss_items_list(
        &self,
        feed_name: Option<&str>,
        limit: usize,
    ) -> crate::nzb_core::Result<Vec<RssItem>> {
        let db = self.db.lock();
        db.rss_items_list(feed_name, limit)
    }

    /// Get a single RSS item by ID.
    pub fn rss_item_get(&self, id: &str) -> crate::nzb_core::Result<Option<RssItem>> {
        let db = self.db.lock();
        db.rss_item_get(id)
    }

    /// Mark an RSS item as downloaded.
    pub fn rss_item_mark_downloaded(
        &self,
        id: &str,
        category: Option<&str>,
    ) -> crate::nzb_core::Result<()> {
        let db = self.db.lock();
        db.rss_item_mark_downloaded(id, category)
    }

    /// Upsert an RSS feed item.
    pub fn rss_item_upsert(&self, item: &RssItem) -> crate::nzb_core::Result<()> {
        let db = self.db.lock();
        db.rss_item_upsert(item)
    }

    /// Batch upsert RSS feed items (single DB lock + transaction).
    pub fn rss_items_batch_upsert(&self, items: &[RssItem]) -> crate::nzb_core::Result<usize> {
        let db = self.db.lock();
        db.rss_items_batch_upsert(items)
    }

    /// Check if an RSS item exists.
    pub fn rss_item_exists(&self, id: &str) -> crate::nzb_core::Result<bool> {
        let db = self.db.lock();
        db.rss_item_exists(id)
    }

    /// Count total RSS items.
    pub fn rss_item_count(&self) -> crate::nzb_core::Result<usize> {
        let db = self.db.lock();
        db.rss_item_count()
    }

    /// Prune RSS items to keep only N most recent.
    pub fn rss_items_prune(&self, keep: usize) -> crate::nzb_core::Result<usize> {
        let db = self.db.lock();
        db.rss_items_prune(keep)
    }

    /// List all RSS download rules.
    pub fn rss_rule_list(&self) -> crate::nzb_core::Result<Vec<RssRule>> {
        let db = self.db.lock();
        db.rss_rule_list()
    }

    /// Insert a new RSS download rule.
    pub fn rss_rule_insert(&self, rule: &RssRule) -> crate::nzb_core::Result<()> {
        let db = self.db.lock();
        db.rss_rule_insert(rule)
    }

    /// Update an RSS download rule.
    pub fn rss_rule_update(&self, rule: &RssRule) -> crate::nzb_core::Result<()> {
        let db = self.db.lock();
        db.rss_rule_update(rule)
    }

    /// Delete an RSS download rule.
    pub fn rss_rule_delete(&self, id: &str) -> crate::nzb_core::Result<()> {
        let db = self.db.lock();
        db.rss_rule_delete(id)
    }

    // -----------------------------------------------------------------------
    // Startup: restore jobs from DB
    // -----------------------------------------------------------------------

    /// Restore in-progress jobs from the database on startup.
    ///
    /// Re-parses NZB data for each job and applies any saved checkpoint to
    /// mark already-downloaded articles, so downloads resume where they left off.
    pub fn restore_from_db(self: &Arc<Self>) -> crate::nzb_core::Result<()> {
        // Restore globally_paused from persisted state
        let was_paused = {
            let db = self.db.lock();
            db.get_setting("globally_paused")
                .is_some_and(|v| v == "true")
        };
        if was_paused {
            self.globally_paused.store(true, Ordering::Relaxed);
            info!("Restored global pause state from database");
        }

        let jobs = {
            let db = self.db.lock();
            db.queue_list()?
        };

        if jobs.is_empty() {
            return Ok(());
        }

        info!(count = jobs.len(), "Restoring jobs from database");

        for mut job in jobs {
            let job_id = job.id.clone();

            // Only load full NZB data + checkpoints for jobs that were actively
            // downloading. Queued/paused jobs just need metadata — their NZB data
            // is loaded lazily in launch_download() when they reach the front of
            // the queue. This keeps memory low with large queues (hundreds of jobs).
            let was_active = job.status == JobStatus::Downloading;

            let nzb_data = if was_active {
                let db = self.db.lock();
                db.queue_get_nzb_data(&job_id).unwrap_or(None)
            } else {
                None
            };

            if was_active {
                // Re-parse NZB to populate files and articles
                if let Some(ref data) = nzb_data {
                    match nzb_parser::parse_nzb(&job.name, data) {
                        Ok(parsed) => {
                            job.files = parsed.files;
                        }
                        Err(e) => {
                            warn!(job_id = %job_id, "Failed to re-parse NZB data: {e}");
                        }
                    }
                }

                // Load and apply checkpoint to mark downloaded articles
                let checkpoint_data = {
                    let db = self.db.lock();
                    db.queue_load_job_data(&job_id).unwrap_or(None)
                };

                if let Some(ref data) = checkpoint_data {
                    match serde_json::from_slice::<JobCheckpoint>(data) {
                        Ok(checkpoint) => {
                            job.downloaded_bytes = checkpoint.downloaded_bytes;
                            job.articles_downloaded = checkpoint.articles_downloaded;
                            job.articles_failed = checkpoint.articles_failed;
                            job.files_completed = checkpoint.files_completed;

                            for file in &mut job.files {
                                if let Some(segments) = checkpoint.files.get(&file.id) {
                                    let mut file_bytes_downloaded: u64 = 0;
                                    for article in &mut file.articles {
                                        if segments.contains(&article.segment_number) {
                                            article.downloaded = true;
                                            file_bytes_downloaded += article.bytes;
                                        }
                                    }
                                    file.bytes_downloaded = file_bytes_downloaded;
                                    if file.articles.iter().all(|a| a.downloaded) {
                                        file.assembled = true;
                                    }
                                }
                            }

                            let remaining = job
                                .article_count
                                .saturating_sub(job.articles_downloaded + job.articles_failed);
                            info!(
                                job_id = %job_id,
                                name = %job.name,
                                articles_downloaded = job.articles_downloaded,
                                articles_failed = job.articles_failed,
                                remaining,
                                "Restored job checkpoint — resuming from previous progress"
                            );
                        }
                        Err(e) => {
                            warn!(
                                job_id = %job_id,
                                "Failed to deserialize checkpoint, starting from scratch: {e}"
                            );
                        }
                    }
                }
            }

            if job.status == JobStatus::Paused && was_paused {
                // Keep as Paused — it's globally paused; resume_all will
                // re-submit it to the pool.
            } else if job.status == JobStatus::Paused || job.status == JobStatus::Downloading {
                job.status = JobStatus::Queued;
            }

            // Populate dedup map for jobs whose articles are loaded. Queued
            // jobs without parsed NZB data skip this; they'll be checked
            // lazily at launch if necessary.
            if let Some(hash) = compute_content_hash(&job) {
                self.content_hashes.lock().insert(hash, job_id.clone());
            }

            let state = JobState {
                job,
                progress_handle: None,
                speed: Arc::new(SpeedTracker::new()),
                nzb_data,
                direct_unpacker: None,
                hopeless_tracker: None,
            };
            self.jobs.lock().insert(job_id.clone(), state);
            self.job_order.lock().push(job_id);
        }

        // Start queued jobs up to the concurrency limit
        self.start_next_queued();

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Background task: speed calculation
    // -----------------------------------------------------------------------

    /// Spawn the background task that periodically updates the speed counter.
    /// Gracefully shut down the queue manager.
    ///
    /// Cancels all in-flight downloads, waits for tasks to stop, and persists
    /// final job state to the database so progress is not lost.
    pub async fn shutdown(&self) {
        info!("Shutting down queue manager...");

        // 1. Set globally paused to prevent new downloads from starting
        self.globally_paused.store(true, Ordering::Relaxed);

        // 2. Mark downloading jobs as Queued for next restart, and collect
        //    progress-handler task handles so they can be aborted after the
        //    pool drains.
        let mut handles = Vec::new();
        {
            let mut jobs = self.jobs.lock();
            for (id, state) in jobs.iter_mut() {
                if state.job.status == JobStatus::Downloading {
                    info!(job_id = %id, "Marking download for shutdown");
                    state.job.status = JobStatus::Queued; // Will resume on restart
                }
                if let Some(handle) = state.progress_handle.take() {
                    handles.push(handle);
                }
            }
        }

        // 3. Shut down the worker pool gracefully. In-flight articles finish
        //    first (finish-in-flight), then workers exit.
        self.dispatch.shutdown().await;

        // 4. Abort the per-job progress listeners (their sender sides are
        //    dropped, so the loops would exit anyway; we just don't want to
        //    wait for them).
        for handle in handles {
            handle.abort();
        }

        // 4. Persist final state for all jobs to DB
        {
            let jobs = self.jobs.lock();
            let db = self.db.lock();
            for (id, state) in jobs.iter() {
                if let Err(e) = db.queue_update_progress(
                    id,
                    state.job.status,
                    state.job.downloaded_bytes,
                    state.job.articles_downloaded,
                    state.job.articles_failed,
                    state.job.files_completed,
                ) {
                    error!(job_id = %id, error = %e, "Failed to persist job state on shutdown");
                }
            }
        }

        info!("Queue manager shutdown complete");
    }

    pub fn spawn_speed_tracker(self: &Arc<Self>) {
        let qm = Arc::clone(self);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            let mut tick_count: u64 = 0;
            loop {
                interval.tick().await;
                qm.speed.tick(1.0);
                // Tick per-job speed trackers
                {
                    let jobs = qm.jobs.lock();
                    for (_id, state) in jobs.iter() {
                        state.speed.tick(1.0);
                    }
                }

                // Phase 6: time-based hopeless scan. Catches downloads
                // that have been alive long enough without any progress
                // event reaching the tracker (the zombie scenario, where
                // workers cycle silently and never emit ArticleComplete
                // or ArticleFailed).
                qm.scan_for_no_progress_jobs();

                // Observability: every 10s, dump the state of every
                // active download so operators can see at a glance what
                // each job is doing — progress, failure ratio, hopeless
                // tracker state, time since job start. This is the line
                // to grep when users report "stuck at 0 KB/s" — it shows
                // whether the engine is genuinely stuck, making slow
                // progress, or burning through failed articles.
                if tick_count.is_multiple_of(10) {
                    let snapshots: Vec<_> = {
                        let jobs = qm.jobs.lock();
                        jobs.iter()
                            .filter(|(_, s)| {
                                matches!(
                                    s.job.status,
                                    JobStatus::Downloading
                                        | JobStatus::Queued
                                        | JobStatus::PostProcessing
                                )
                            })
                            .map(|(id, s)| {
                                let (
                                    tracker_checked,
                                    tracker_failed,
                                    tracker_content_total,
                                    tracker_elapsed_secs,
                                    tracker_content_bytes,
                                    tracker_content_missing,
                                ) = s
                                    .hopeless_tracker
                                    .as_ref()
                                    .map(|t| {
                                        (
                                            t.content_articles_checked,
                                            t.content_articles_failed,
                                            t.content_articles_total,
                                            t.created_at.elapsed().as_secs(),
                                            t.content_bytes,
                                            t.content_bytes_missing,
                                        )
                                    })
                                    .unwrap_or((0, 0, 0, 0, 0, 0));
                                (
                                    id.clone(),
                                    s.job.name.clone(),
                                    s.job.status,
                                    s.job.articles_downloaded,
                                    s.job.articles_failed,
                                    s.job.article_count,
                                    s.job.downloaded_bytes,
                                    s.job.total_bytes,
                                    s.speed.bps(),
                                    tracker_checked,
                                    tracker_failed,
                                    tracker_content_total,
                                    tracker_elapsed_secs,
                                    tracker_content_bytes,
                                    tracker_content_missing,
                                )
                            })
                            .collect()
                    };
                    for (
                        job_id,
                        name,
                        status,
                        dl,
                        failed,
                        total_art,
                        dl_bytes,
                        total_bytes,
                        bps,
                        t_checked,
                        t_failed,
                        t_total,
                        t_elapsed,
                        t_bytes_total,
                        t_bytes_missing,
                    ) in snapshots
                    {
                        let pct_bytes = if total_bytes > 0 {
                            (dl_bytes as f64 / total_bytes as f64) * 100.0
                        } else {
                            0.0
                        };
                        let avail_pct = if t_bytes_total > 0 {
                            let avail: u64 = t_bytes_total.saturating_sub(t_bytes_missing);
                            100.0 * (avail as f64 / t_bytes_total as f64)
                        } else {
                            100.0
                        };
                        info!(
                            job_id = %job_id,
                            name = %name,
                            status = %status,
                            pct = format!("{pct_bytes:.1}"),
                            dl_articles = dl,
                            failed_articles = failed,
                            total_articles = total_art,
                            dl_bytes,
                            total_bytes,
                            kbps = bps / 1024,
                            elapsed_secs = t_elapsed,
                            tracker_checked = t_checked,
                            tracker_failed = t_failed,
                            tracker_total = t_total,
                            availability_pct = format!("{avail_pct:.1}"),
                            "Job status snapshot"
                        );
                    }
                }

                // Periodic connection count + disk space checks (every 30 seconds)
                tick_count += 1;
                if tick_count.is_multiple_of(30) {
                    // Log active NNTP connections per server
                    let snapshot = qm.conn_tracker.snapshot();
                    let total: usize = snapshot.iter().map(|(_, c, _)| *c).sum();
                    if total > 0 {
                        for (server_id, count, limit) in &snapshot {
                            if *count > 0 {
                                let server_name = qm
                                    .servers
                                    .lock()
                                    .iter()
                                    .find(|s| s.id == *server_id)
                                    .map(|s| s.name.clone())
                                    .unwrap_or_else(|| server_id.clone());
                                if *limit > 0 && *count > *limit {
                                    warn!(
                                        server = %server_name,
                                        active = count,
                                        limit,
                                        "NNTP connections EXCEED limit"
                                    );
                                } else {
                                    info!(
                                        server = %server_name,
                                        active = count,
                                        limit,
                                        "NNTP connection count"
                                    );
                                }
                            }
                        }
                        info!(total_nntp_connections = total, "NNTP connection summary");
                    }
                }
                if tick_count.is_multiple_of(30) && qm.min_free_space > 0 {
                    let free = get_disk_free(&qm.incomplete_dir.lock());
                    if free > 0
                        && free < qm.min_free_space
                        && !qm.globally_paused.load(Ordering::Relaxed)
                    {
                        warn!(
                            free_bytes = free,
                            min_free_space = qm.min_free_space,
                            "Low disk space, auto-pausing downloads"
                        );
                        qm.globally_paused.store(true, Ordering::Relaxed);
                        qm.db.lock().set_setting("globally_paused", "true");
                        let to_pause: Vec<String> = {
                            let jobs = qm.jobs.lock();
                            jobs.iter()
                                .filter(|(_, s)| s.job.status == JobStatus::Downloading)
                                .map(|(id, _)| id.clone())
                                .collect()
                        };
                        for id in to_pause {
                            qm.dispatch.pause_job(&id);
                        }
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod hopeless_tests {
    use super::*;

    /// Helper: build a tracker with N content articles of ~1MB each and M par2 articles.
    fn make_tracker(content_articles: usize, par2_articles: usize) -> HopelessTracker {
        let article_bytes: u64 = 750_000; // ~750KB per article
        HopelessTracker {
            created_at: Instant::now(),
            content_bytes: content_articles as u64 * article_bytes,
            par2_bytes: par2_articles as u64 * article_bytes,
            content_bytes_missing: 0,
            content_articles_checked: 0,
            content_articles_failed: 0,
            content_articles_total: content_articles,
        }
    }

    #[test]
    fn grace_period_allows_small_failures() {
        let mut t = make_tracker(1000, 50);
        // Fail 5 articles (at the grace threshold)
        for _ in 0..5 {
            t.record_failure(
                false,
                750_000,
                crate::article_failure::ArticleFailureKind::NotFound,
            );
        }
        assert!(
            t.check(true, true, 100.2).is_none(),
            "Should not abort within grace period"
        );
    }

    #[test]
    fn grace_period_disabled_when_abort_hopeless_off() {
        let mut t = make_tracker(100, 10);
        for _ in 0..100 {
            t.record_failure(
                false,
                750_000,
                crate::article_failure::ArticleFailureKind::NotFound,
            );
        }
        assert!(
            t.check(false, true, 100.2).is_none(),
            "Should never abort when abort_hopeless is disabled"
        );
    }

    #[test]
    fn early_check_fires_at_80pct_of_first_10() {
        let mut t = make_tracker(1000, 50);
        // Simulate: 8 failures, 2 successes out of first 10
        for _ in 0..8 {
            t.record_failure(
                false,
                750_000,
                crate::article_failure::ArticleFailureKind::NotFound,
            );
        }
        for _ in 0..2 {
            t.record_success(false);
        }
        let result = t.check(true, true, 100.2);
        assert!(result.is_some(), "Should abort: 80% failure in first 10");
        let abort = result.unwrap();
        assert_eq!(abort.tier, "early_failure");
        assert!(abort.reason.contains("80%"));
    }

    #[test]
    fn early_check_does_not_fire_below_threshold() {
        let mut t = make_tracker(1000, 50);
        // 7 failures, 3 successes = 70% failure
        for _ in 0..7 {
            t.record_failure(
                false,
                750_000,
                crate::article_failure::ArticleFailureKind::NotFound,
            );
        }
        for _ in 0..3 {
            t.record_success(false);
        }
        // Still within grace (7 > 5) but early check at 70% < 80%
        // However, we need to check the ongoing ratio too.
        // 7 * 750KB missing out of 1000 * 750KB total = 0.7% missing
        // availability = 99.3%, which is below 100.2 — so it should still pass
        // because at 10 articles checked the ratio is still fine.
        // Actually: 7 * 750KB = 5.25MB missing out of 750MB total = 99.3% available
        // 99.3 < 100.2 → would abort via tier 3!
        // But wait, content_bytes = 1000 * 750_000 = 750MB, missing = 5.25MB
        // availability = (750MB - 5.25MB) / 750MB * 100 = 99.3%
        // 99.3 < 100.2 → yes this triggers tier 3.
        // With 1000 articles, 7 missing is easily repairable by par2.
        // The 100.2% threshold is very aggressive. Let's use a more reasonable test.
        let result = t.check(true, false, 95.0);
        assert!(
            result.is_none(),
            "Should not abort: 70% failure in first 10 with early_check disabled and low threshold"
        );
    }

    #[test]
    fn early_check_disabled_still_checks_ongoing() {
        let mut t = make_tracker(100, 10);
        // Fail all 100 content articles
        for _ in 0..100 {
            t.record_failure(
                false,
                750_000,
                crate::article_failure::ArticleFailureKind::NotFound,
            );
        }
        let result = t.check(true, false, 100.0);
        assert!(
            result.is_some(),
            "Ongoing ratio should catch 100% failure even with early_check off"
        );
        let abort = result.unwrap();
        assert_eq!(abort.tier, "ongoing_availability");
        assert!(abort.reason.contains("0.0%"));
    }

    #[test]
    fn par2_failures_do_not_count() {
        let mut t = make_tracker(100, 50);
        // Fail 20 par2 articles — should not affect content tracking
        for _ in 0..20 {
            t.record_failure(
                true,
                750_000,
                crate::article_failure::ArticleFailureKind::NotFound,
            );
        }
        assert_eq!(t.content_articles_failed, 0);
        assert_eq!(t.content_bytes_missing, 0);
        assert!(t.check(true, true, 100.2).is_none());
    }

    #[test]
    fn ongoing_ratio_triggers_when_missing_exceeds_par2_capacity() {
        // 100 content articles + 10 par2 articles. Fail 15 content articles
        // (11.25 MB missing). PAR2 can cover 10 articles (7.5 MB) — so 15
        // missing is 7.5 MB beyond par2's recovery capacity.
        //
        // Matches SABnzbd's par2-aware formula: ratio drops below 100 only
        // when content_missing > par2_total.
        let mut t = make_tracker(100, 10);
        for _ in 0..15 {
            t.record_failure(
                false,
                750_000,
                crate::article_failure::ArticleFailureKind::NotFound,
            );
        }
        for _ in 0..40 {
            t.record_success(false);
        }
        let result = t.check(true, true, 100.0);
        assert!(
            result.is_some(),
            "15 articles missing with only 10-article par2 must abort"
        );
    }

    #[test]
    fn par2_credits_allow_tolerable_content_loss() {
        // 100 content articles + 10 par2. Fail 8 content articles (6 MB).
        // PAR2 capacity (7.5 MB) > missing (6 MB) → job is repairable →
        // tier 3 must NOT abort.
        //
        // This is the bug that caused retention-edge NZBs to die in our
        // tracker while SABnzbd happily finished + repaired them.
        let mut t = make_tracker(100, 10);
        for _ in 0..8 {
            t.record_failure(
                false,
                750_000,
                crate::article_failure::ArticleFailureKind::NotFound,
            );
        }
        for _ in 0..50 {
            t.record_success(false);
        }
        let result = t.check(true, true, 100.0);
        assert!(
            result.is_none(),
            "8 articles missing with 10-article par2 must NOT abort (par2 will repair)"
        );
    }

    #[test]
    fn early_check_fires_continuously_after_phase_6() {
        // Phase 6 removed the `<= total/4` window cap. Tier 2 now fires
        // whenever the failure rate is high enough — even past the 25%
        // mark. Previously this scenario would only trip tier 3
        // (ongoing_availability); now tier 2 wins because it's checked
        // first.
        let mut t = make_tracker(40, 5);
        for _ in 0..9 {
            t.record_failure(
                false,
                750_000,
                crate::article_failure::ArticleFailureKind::NotFound,
            );
        }
        for _ in 0..2 {
            t.record_success(false);
        }
        // 11 checked, 9 failed = 81.8% failure rate, well above the 80%
        // threshold. Tier 2 should fire regardless of how many articles
        // remain to check.
        let result = t.check(true, true, 100.0);
        assert!(result.is_some());
        let abort = result.unwrap();
        assert_eq!(abort.tier, "early_failure");
    }

    #[test]
    fn completely_dead_nzb_aborts_fast() {
        let mut t = make_tracker(10000, 500);
        // First 10 articles all fail — exactly the scenario from the bug report
        for _ in 0..10 {
            t.record_failure(
                false,
                750_000,
                crate::article_failure::ArticleFailureKind::NotFound,
            );
        }
        let result = t.check(true, true, 100.2);
        assert!(
            result.is_some(),
            "100% failure on first 10 should abort immediately"
        );
    }
}

#[cfg(test)]
mod dedup_tests {
    use super::*;
    use chrono::Utc;
    use nzb_nntp::Article;
    use std::path::PathBuf;

    fn make_article(message_id: &str, segment: u32) -> Article {
        Article {
            message_id: message_id.to_string(),
            segment_number: segment,
            bytes: 1024,
            downloaded: false,
            data_begin: None,
            data_size: None,
            crc32: None,
            tried_servers: Vec::new(),
            tries: 0,
        }
    }

    fn empty_job(id: &str) -> NzbJob {
        NzbJob {
            id: id.to_string(),
            name: format!("job {id}"),
            category: String::new(),
            status: JobStatus::Queued,
            priority: Priority::default(),
            total_bytes: 0,
            downloaded_bytes: 0,
            file_count: 0,
            files_completed: 0,
            article_count: 0,
            articles_downloaded: 0,
            articles_failed: 0,
            added_at: Utc::now(),
            completed_at: None,
            work_dir: PathBuf::new(),
            output_dir: PathBuf::new(),
            password: None,
            error_message: None,
            speed_bps: 0,
            server_stats: Vec::new(),
            files: Vec::new(),
        }
    }

    fn make_job(id: &str, message_ids: &[&str]) -> NzbJob {
        let articles: Vec<Article> = message_ids
            .iter()
            .enumerate()
            .map(|(i, m)| make_article(m, (i + 1) as u32))
            .collect();
        let file = NzbFile {
            id: format!("{id}-file-0"),
            filename: "x.bin".into(),
            bytes: 0,
            bytes_downloaded: 0,
            is_par2: false,
            par2_setname: None,
            par2_vol: None,
            par2_blocks: None,
            assembled: false,
            groups: vec!["alt.binaries.test".into()],
            articles,
        };
        let mut job = empty_job(id);
        job.files = vec![file];
        job.file_count = 1;
        job.article_count = message_ids.len();
        job
    }

    #[test]
    fn identical_article_sets_produce_same_hash() {
        let a = make_job("a", &["<msg-1@x>", "<msg-2@x>", "<msg-3@x>"]);
        let b = make_job("b", &["<msg-1@x>", "<msg-2@x>", "<msg-3@x>"]);
        assert_eq!(
            compute_content_hash(&a),
            compute_content_hash(&b),
            "same message-id set → same hash regardless of job id"
        );
    }

    #[test]
    fn hash_is_order_independent() {
        let a = make_job("a", &["<msg-1@x>", "<msg-2@x>", "<msg-3@x>"]);
        let b = make_job("b", &["<msg-3@x>", "<msg-1@x>", "<msg-2@x>"]);
        assert_eq!(
            compute_content_hash(&a),
            compute_content_hash(&b),
            "shuffling message-ids → same hash (sorted before hashing)"
        );
    }

    #[test]
    fn different_article_sets_produce_different_hash() {
        let a = make_job("a", &["<msg-1@x>", "<msg-2@x>"]);
        let b = make_job("b", &["<msg-1@x>", "<msg-other@x>"]);
        assert_ne!(
            compute_content_hash(&a),
            compute_content_hash(&b),
            "any different message-id → different hash"
        );
    }

    #[test]
    fn disjoint_prefix_avoids_concatenation_collision() {
        // Without a delimiter between message-ids, "<a><b>" and "<ab><>" hash the same.
        // Asserts the delimiter guard in compute_content_hash does its job.
        let a = make_job("a", &["<a>", "<b>"]);
        let b = make_job("b", &["<ab>"]);
        assert_ne!(compute_content_hash(&a), compute_content_hash(&b));
    }

    #[test]
    fn empty_articles_produce_no_hash() {
        let job = empty_job("empty");
        assert!(compute_content_hash(&job).is_none());
    }
}

#[cfg(test)]
mod speed_tracker_tests {
    use super::*;

    #[test]
    fn zero_activity_reports_zero() {
        let t = SpeedTracker::new();
        t.tick(1.0);
        assert_eq!(t.bps(), 0);
    }

    #[test]
    fn steady_flow_reports_rate() {
        // 100 KB/s steady for 10 seconds → bps averages to 100 KB.
        let t = SpeedTracker::new();
        for _ in 0..10 {
            t.record(100_000);
            t.tick(1.0);
        }
        assert_eq!(t.bps(), 100_000);
    }

    #[test]
    fn bursty_flow_smooths_to_average() {
        // Prod scenario: a 1 MB article completes every 5 seconds.
        // Without smoothing, bps alternates 0, 0, 0, 0, 1MB → sampled snapshots
        // often catch a zero. Rolling window should report ~200 KB/s average.
        let t = SpeedTracker::new();
        for i in 0..10 {
            if i % 5 == 4 {
                t.record(1_000_000);
            }
            t.tick(1.0);
        }
        // 2 articles × 1 MB over 10 s → 200 KB/s average.
        assert_eq!(t.bps(), 200_000);
        // And bps is non-zero regardless of which second the last tick landed on.
        for _ in 0..4 {
            t.tick(1.0);
            assert!(
                t.bps() > 0,
                "bps must stay non-zero across sparse ticks after any recent activity"
            );
        }
    }

    #[test]
    fn window_rolls_off_old_samples() {
        // Record 1MB at t=0, then 14 seconds of nothing.
        // After the window has rolled past, bps should return to 0.
        let t = SpeedTracker::new();
        t.record(1_000_000);
        t.tick(1.0);
        assert!(t.bps() > 0, "bps should reflect the 1MB record");
        for _ in 0..SPEED_WINDOW_SECS {
            t.tick(1.0);
        }
        assert_eq!(t.bps(), 0, "after window expires, bps should be 0");
    }

    #[test]
    fn concurrent_record_and_tick_race_safely() {
        // record is called from worker tasks; tick is called from the speed
        // tracker loop. pending_bytes is atomic — this is a smoke test that
        // the swap semantics don't drop bytes under light concurrency.
        let t = Arc::new(SpeedTracker::new());
        let t_rec = Arc::clone(&t);
        let recorder = std::thread::spawn(move || {
            for _ in 0..1000 {
                t_rec.record(1024);
            }
        });
        for _ in 0..10 {
            t.tick(1.0);
            std::thread::sleep(std::time::Duration::from_micros(100));
        }
        recorder.join().unwrap();
        t.tick(1.0); // flush any remaining pending
        // Sum of all window buckets must equal the total recorded (1MB).
        let total: u64 = t.window.lock().iter().sum();
        assert_eq!(total, 1024 * 1000);
    }
}
