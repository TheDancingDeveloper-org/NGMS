use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Serialize;
use sqlx::PgPool;

// ── Types ───────────────────────────────────────────────────────────────────

/// A file discovered on disk during import scanning.
#[derive(Debug, Clone)]
pub struct LocalFile {
    pub path: PathBuf,
    pub size: u64,
    pub extension: String,
}

/// Result of processing a completed download or folder scan.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub imported: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
}

// ── Import service ──────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct ImportService {
    pool: PgPool,
}

impl ImportService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Process a completed download: identify media files, match to library,
    /// move/hardlink into the correct folder, and update the database.
    pub async fn process_completed_download(
        &self,
        download_folder: &Path,
        download_id: &str,
    ) -> Result<ImportResult> {
        tracing::info!(
            download_id,
            path = %download_folder.display(),
            "processing completed download"
        );

        let files = self.scan_folder(download_folder)?;
        tracing::info!(count = files.len(), "discovered local files");

        // TODO: for each file, parse the name, match to series/movie, apply
        // quality checks, move into library, insert media_file row, update
        // episode/movie file reference.
        let _pool = &self.pool;

        Ok(ImportResult {
            imported: files.len(),
            skipped: 0,
            errors: Vec::new(),
        })
    }

    /// Recursively scan a folder for media files.
    pub fn scan_folder(&self, folder: &Path) -> Result<Vec<LocalFile>> {
        let mut files = Vec::new();

        if !folder.exists() {
            anyhow::bail!("folder does not exist: {}", folder.display());
        }

        for entry in walkdir::WalkDir::new(folder)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path().to_path_buf();
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();

            // Only consider known media extensions
            if matches!(
                ext.as_str(),
                "mkv" | "mp4" | "avi" | "wmv" | "ts" | "m4v" | "flv" | "mov" | "webm"
            ) {
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                files.push(LocalFile {
                    path,
                    size,
                    extension: ext,
                });
            }
        }

        Ok(files)
    }
}
