//! Embedded Usenet download client backed by rustnzbd (nzb-web / nzb-core).
//!
//! Only compiled when the `usenet-embedded` feature is enabled.

use std::sync::Arc;

use anyhow::{Context, bail};
use async_trait::async_trait;
use tracing::{debug, info};

use nzb_core::models::{JobStatus, NzbJob};
use nzb_web::QueueManager;

use crate::client::{
    ClientStatus, DownloadClient, DownloadItem, DownloadItemStatus, DownloadProtocol, GrabRequest,
};

/// An embedded Usenet download client powered by rustnzbd.
///
/// Runs the NZB download engine in-process -- no external SABnzbd / NZBGet
/// installation required.
pub struct EmbeddedUsenetClient {
    queue_manager: Arc<QueueManager>,
}

impl EmbeddedUsenetClient {
    /// Wrap an already-initialised `QueueManager`.
    ///
    /// The caller is responsible for constructing the `QueueManager` (via
    /// `nzb_web::StartupConfig` / `nzb_web::run`) and passing it here.
    /// This keeps the embedded client thin and avoids duplicating complex
    /// startup logic.
    pub fn from_queue_manager(queue_manager: Arc<QueueManager>) -> Self {
        info!("embedded usenet client attached to queue manager");
        Self { queue_manager }
    }
}

/// Map a rustnzbd `JobStatus` to our unified `DownloadItemStatus`.
fn map_job_status(status: JobStatus) -> DownloadItemStatus {
    match status {
        JobStatus::Queued => DownloadItemStatus::Queued,
        JobStatus::Downloading => DownloadItemStatus::Downloading,
        JobStatus::Paused => DownloadItemStatus::Paused,
        JobStatus::Verifying | JobStatus::Repairing => DownloadItemStatus::Verifying,
        JobStatus::Extracting => DownloadItemStatus::Extracting,
        JobStatus::PostProcessing => DownloadItemStatus::Extracting,
        JobStatus::Completed => DownloadItemStatus::Completed,
        JobStatus::Failed => DownloadItemStatus::Failed,
    }
}

/// Convert an `NzbJob` into our generic `DownloadItem`.
fn job_to_item(job: &NzbJob) -> DownloadItem {
    let remaining = job.total_bytes.saturating_sub(job.downloaded_bytes);
    DownloadItem {
        download_id: job.id.clone(),
        title: job.name.clone(),
        status: map_job_status(job.status),
        total_size: job.total_bytes,
        remaining_size: remaining,
        output_path: Some(job.output_dir.clone()),
        category: Some(job.category.clone()),
        can_move_files: job.status == JobStatus::Completed,
        can_be_removed: true,
        protocol: DownloadProtocol::Usenet,
    }
}

#[async_trait]
impl DownloadClient for EmbeddedUsenetClient {
    fn name(&self) -> &str {
        "Embedded Usenet (rustnzbd)"
    }

    fn protocol(&self) -> DownloadProtocol {
        DownloadProtocol::Usenet
    }

    async fn add(&self, request: &GrabRequest) -> anyhow::Result<String> {
        // Download the NZB file from the provided URL.
        let resp = reqwest::get(&request.download_url)
            .await
            .context("embedded usenet: failed to fetch NZB URL")?;

        if !resp.status().is_success() {
            bail!(
                "embedded usenet: NZB download returned HTTP {}",
                resp.status()
            );
        }

        let nzb_bytes = resp
            .bytes()
            .await
            .context("embedded usenet: failed to read NZB response body")?;

        // Parse the NZB XML into a job.
        let mut job = nzb_core::nzb_parser::parse_nzb(&request.title, &nzb_bytes)
            .map_err(|e| anyhow::anyhow!("embedded usenet: NZB parse error: {e}"))?;

        // Apply category from the grab request.
        if let Some(ref cat) = request.category {
            job.category = cat.clone();
        }

        // Set working and output directories relative to the queue manager dirs.
        let incomplete = self.queue_manager.incomplete_dir().join(&job.id);
        let complete = self
            .queue_manager
            .complete_dir()
            .join(&job.category)
            .join(&job.name);
        job.work_dir = incomplete;
        job.output_dir = complete;

        let job_id = job.id.clone();
        let nzb_data = nzb_bytes.to_vec();

        self.queue_manager
            .add_job(job, Some(nzb_data))
            .map_err(|e| anyhow::anyhow!("embedded usenet: failed to add job: {e}"))?;

        debug!(job_id = %job_id, title = %request.title, "NZB job added to embedded client");
        Ok(job_id)
    }

    async fn get_items(&self) -> anyhow::Result<Vec<DownloadItem>> {
        let jobs = self.queue_manager.get_jobs();
        Ok(jobs.iter().map(job_to_item).collect())
    }

    async fn remove(&self, id: &str, _delete_data: bool) -> anyhow::Result<()> {
        self.queue_manager
            .remove_job(id)
            .map_err(|e| anyhow::anyhow!("embedded usenet: remove failed: {e}"))?;

        debug!(id = %id, "NZB job removed from embedded client");
        Ok(())
    }

    async fn pause(&self, id: &str) -> anyhow::Result<()> {
        self.queue_manager
            .pause_job(id)
            .map_err(|e| anyhow::anyhow!("embedded usenet: pause failed: {e}"))?;

        debug!(id = %id, "NZB job paused");
        Ok(())
    }

    async fn resume(&self, id: &str) -> anyhow::Result<()> {
        self.queue_manager
            .resume_job(id)
            .map_err(|e| anyhow::anyhow!("embedded usenet: resume failed: {e}"))?;

        debug!(id = %id, "NZB job resumed");
        Ok(())
    }

    async fn test(&self) -> anyhow::Result<()> {
        // If we can query the queue, the engine is alive.
        let _jobs = self.queue_manager.get_jobs();
        Ok(())
    }

    async fn status(&self) -> anyhow::Result<ClientStatus> {
        let queue_size = self.queue_manager.queue_size();
        let speed_bps = self.queue_manager.get_speed();
        let paused = self.queue_manager.is_paused();

        Ok(ClientStatus {
            name: "Embedded Usenet (rustnzbd)".to_string(),
            protocol: DownloadProtocol::Usenet,
            version: format!(
                "rustnzbd ({} jobs, {:.1} KB/s{})",
                queue_size,
                speed_bps as f64 / 1024.0,
                if paused { ", paused" } else { "" },
            ),
            is_connected: true,
        })
    }
}
