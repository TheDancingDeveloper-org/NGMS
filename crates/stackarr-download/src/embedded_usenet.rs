use std::sync::Arc;

use anyhow::{Context, bail};
use async_trait::async_trait;
use flate2::read::GzDecoder;
use std::io::Read as _;

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
        let response = reqwest::get(&request.download_url).await.with_context(|| {
            format!(
                "failed to fetch NZB for '{}' from {}",
                request.title, request.download_url
            )
        })?;

        let status = response.status();
        if !status.is_success() {
            bail!(
                "indexer returned HTTP {} when fetching NZB for '{}'",
                status,
                request.title
            );
        }

        let raw_bytes = response
            .bytes()
            .await
            .context("failed to read NZB data")?
            .to_vec();

        // Decompress gzip if the data starts with the gzip magic bytes (0x1f 0x8b).
        // Many Newznab APIs return gzip-compressed NZB files without setting
        // Content-Encoding, so reqwest won't auto-decompress them.
        let nzb_bytes = if raw_bytes.len() >= 2 && raw_bytes[0] == 0x1f && raw_bytes[1] == 0x8b {
            let mut decoder = GzDecoder::new(&raw_bytes[..]);
            let mut decompressed = Vec::new();
            decoder
                .read_to_end(&mut decompressed)
                .context("failed to decompress gzip NZB data")?;
            decompressed
        } else {
            raw_bytes
        };

        // Sanity check: NZB should be XML starting with '<'
        let first_non_ws = nzb_bytes.iter().find(|b| !b.is_ascii_whitespace());
        if first_non_ws != Some(&b'<') {
            let preview: String = nzb_bytes
                .iter()
                .take(200)
                .map(|&b| {
                    if b.is_ascii_graphic() || b == b' ' {
                        b as char
                    } else {
                        '.'
                    }
                })
                .collect();
            bail!(
                "NZB response is not XML for '{}' (first 200 bytes: {})",
                request.title,
                preview
            );
        }

        let name = &request.title;
        let mut job = nzb_web::nzb_core::nzb_parser::parse_nzb(name, &nzb_bytes)
            .context("failed to parse NZB")?;

        job.work_dir = self.queue.incomplete_dir().join(&job.id);
        job.output_dir = self.queue.complete_dir().join(&job.name);
        if let Some(ref cat) = request.category {
            job.category = cat.clone();
        }
        // API-provided password overrides NZB metadata password
        if let Some(ref pw) = request.password {
            job.password = Some(pw.clone());
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
                    nzb_web::nzb_core::models::JobStatus::Downloading => {
                        DownloadItemStatus::Downloading
                    }
                    nzb_web::nzb_core::models::JobStatus::Paused => DownloadItemStatus::Paused,
                    nzb_web::nzb_core::models::JobStatus::Verifying => {
                        DownloadItemStatus::Verifying
                    }
                    nzb_web::nzb_core::models::JobStatus::Repairing
                    | nzb_web::nzb_core::models::JobStatus::Extracting => {
                        DownloadItemStatus::Extracting
                    }
                    nzb_web::nzb_core::models::JobStatus::PostProcessing => {
                        DownloadItemStatus::Extracting
                    }
                    nzb_web::nzb_core::models::JobStatus::Completed => {
                        DownloadItemStatus::Completed
                    }
                    nzb_web::nzb_core::models::JobStatus::Failed => DownloadItemStatus::Failed,
                };
                let error_message = if j.status == nzb_web::nzb_core::models::JobStatus::Failed {
                    j.error_message.clone().or_else(|| {
                        if j.articles_failed > 0 {
                            Some(format!(
                                "{} article(s) missing — incomplete download",
                                j.articles_failed
                            ))
                        } else {
                            None
                        }
                    })
                } else {
                    None
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
                    error_message,
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
                    nzb_web::nzb_core::models::JobStatus::Completed => {
                        DownloadItemStatus::Completed
                    }
                    nzb_web::nzb_core::models::JobStatus::Failed => DownloadItemStatus::Failed,
                    _ => continue,
                };
                let error_message = if h.status == nzb_web::nzb_core::models::JobStatus::Failed {
                    // Prefer the explicit error, then check failed stages
                    h.error_message.clone().or_else(|| {
                        h.stages
                            .iter()
                            .find(|s| s.status == nzb_web::nzb_core::models::StageStatus::Failed)
                            .map(|s| {
                                let stage_name = &s.name;
                                let detail = s.message.as_deref().unwrap_or("unknown error");
                                format!("{stage_name} failed: {detail}")
                            })
                    })
                } else {
                    None
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
                    error_message,
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
        self.queue.pause_job(id).map_err(|e| anyhow::anyhow!("{e}"))
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
