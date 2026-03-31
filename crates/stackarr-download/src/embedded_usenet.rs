use std::sync::Arc;

use anyhow::{Context, bail};
use async_trait::async_trait;

use crate::client::{
    ClientStatus, DownloadClient, DownloadItem, DownloadItemStatus, DownloadProtocol, GrabRequest,
};

/// Wrapper around the embedded nzb usenet download engine.
pub struct EmbeddedUsenetClient {
    queue: Arc<nzb_web::QueueManager>,
}

impl EmbeddedUsenetClient {
    pub fn new(queue: Arc<nzb_web::QueueManager>) -> Self {
        Self { queue }
    }
}

#[async_trait]
impl DownloadClient for EmbeddedUsenetClient {
    fn name(&self) -> &str {
        "Embedded Usenet Client"
    }

    fn protocol(&self) -> DownloadProtocol {
        DownloadProtocol::Usenet
    }

    async fn add(&self, request: &GrabRequest) -> anyhow::Result<String> {
        // Fetch the NZB data from the URL
        let nzb_bytes = reqwest::get(&request.download_url)
            .await
            .context("failed to fetch NZB")?
            .bytes()
            .await
            .context("failed to read NZB data")?
            .to_vec();

        let name = &request.title;
        let mut job = nzb_web::nzb_core::nzb_parser::parse_nzb(name, &nzb_bytes)
            .context("failed to parse NZB")?;

        job.work_dir = self.queue.incomplete_dir().join(&job.id);
        job.output_dir = self.queue.complete_dir().join(&job.name);
        if let Some(ref cat) = request.category {
            job.category = cat.clone();
        }

        let job_id = job.id.clone();
        self.queue
            .add_job(job, Some(nzb_bytes))
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        Ok(job_id)
    }

    async fn get_items(&self) -> anyhow::Result<Vec<DownloadItem>> {
        let jobs = self.queue.get_jobs();
        let mut seen_ids: std::collections::HashSet<String> =
            jobs.iter().map(|j| j.id.clone()).collect();

        let mut items: Vec<DownloadItem> = jobs
            .into_iter()
            .map(|j| {
                let remaining = j.total_bytes.saturating_sub(j.downloaded_bytes);
                let status = match j.status {
                    nzb_web::nzb_core::models::JobStatus::Queued => DownloadItemStatus::Queued,
                    nzb_web::nzb_core::models::JobStatus::Downloading => DownloadItemStatus::Downloading,
                    nzb_web::nzb_core::models::JobStatus::Paused => DownloadItemStatus::Paused,
                    nzb_web::nzb_core::models::JobStatus::Verifying => DownloadItemStatus::Verifying,
                    nzb_web::nzb_core::models::JobStatus::Repairing
                    | nzb_web::nzb_core::models::JobStatus::Extracting => DownloadItemStatus::Extracting,
                    nzb_web::nzb_core::models::JobStatus::PostProcessing => DownloadItemStatus::Extracting,
                    nzb_web::nzb_core::models::JobStatus::Completed => DownloadItemStatus::Completed,
                    nzb_web::nzb_core::models::JobStatus::Failed => DownloadItemStatus::Failed,
                };
                DownloadItem {
                    download_id: j.id,
                    title: j.name,
                    status,
                    total_size: j.total_bytes,
                    remaining_size: remaining,
                    output_path: Some(j.output_dir),
                    category: if j.category.is_empty() {
                        None
                    } else {
                        Some(j.category)
                    },
                    can_move_files: true,
                    can_be_removed: true,
                    protocol: DownloadProtocol::Usenet,
                }
            })
            .collect();

        // Include recently completed/failed items from history so the scheduler
        // always sees them, even after the in-memory queue removes them (~8s).
        if let Ok(history) = self.queue.history_list(50) {
            let cutoff = chrono::Utc::now() - chrono::Duration::minutes(10);
            for h in history {
                if h.completed_at < cutoff {
                    continue;
                }
                if seen_ids.contains(&h.id) {
                    continue;
                }
                seen_ids.insert(h.id.clone());
                let status = match h.status {
                    nzb_web::nzb_core::models::JobStatus::Completed => DownloadItemStatus::Completed,
                    nzb_web::nzb_core::models::JobStatus::Failed => DownloadItemStatus::Failed,
                    _ => continue,
                };
                items.push(DownloadItem {
                    download_id: h.id,
                    title: h.name,
                    status,
                    total_size: h.total_bytes,
                    remaining_size: 0,
                    output_path: Some(h.output_dir),
                    category: if h.category.is_empty() {
                        None
                    } else {
                        Some(h.category)
                    },
                    can_move_files: true,
                    can_be_removed: true,
                    protocol: DownloadProtocol::Usenet,
                });
            }
        }

        Ok(items)
    }

    async fn remove(&self, id: &str, _delete_data: bool) -> anyhow::Result<()> {
        self.queue
            .remove_job(id)
            .map_err(|e| anyhow::anyhow!("{e}"))
    }

    async fn pause(&self, id: &str) -> anyhow::Result<()> {
        self.queue
            .pause_job(id)
            .map_err(|e| anyhow::anyhow!("{e}"))
    }

    async fn resume(&self, id: &str) -> anyhow::Result<()> {
        self.queue
            .resume_job(id)
            .map_err(|e| anyhow::anyhow!("{e}"))
    }

    async fn test(&self) -> anyhow::Result<()> {
        if self.queue.get_servers().is_empty() {
            bail!("no usenet servers configured");
        }
        Ok(())
    }

    async fn status(&self) -> anyhow::Result<ClientStatus> {
        Ok(ClientStatus {
            name: "Embedded Usenet Client".into(),
            protocol: DownloadProtocol::Usenet,
            version: env!("CARGO_PKG_VERSION").into(),
            is_connected: !self.queue.get_servers().is_empty(),
        })
    }
}
