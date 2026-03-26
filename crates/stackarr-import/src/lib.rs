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

/// Result of a disk scan operation.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskScanResult {
    pub files_found: usize,
    pub files_matched: usize,
    pub files_unmatched: usize,
    pub files_already_tracked: usize,
    pub unmatched_files: Vec<String>,
}

// ── Media extensions ─────────────────────────────────────────────────────────

fn is_media_extension(ext: &str) -> bool {
    matches!(
        ext,
        "mkv" | "mp4" | "avi" | "wmv" | "ts" | "m4v" | "flv" | "mov" | "webm"
    )
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
            if is_media_extension(&ext) {
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

// ── Disk scan ────────────────────────────────────────────────────────────────

/// Scan a root folder for media files on disk, matching them to series/movies
/// already in the database. Creates `media_file` records for matched files.
///
/// `media_type` should be `"series"` or `"movie"`.
pub async fn disk_scan(
    pool: &PgPool,
    root_path: &Path,
    media_type: &str,
) -> Result<DiskScanResult> {
    tracing::info!(
        path = %root_path.display(),
        media_type,
        "starting disk scan"
    );

    if !root_path.exists() {
        anyhow::bail!("root folder does not exist: {}", root_path.display());
    }

    match media_type {
        "series" => scan_series(pool, root_path).await,
        "movie" => scan_movies(pool, root_path).await,
        other => anyhow::bail!("unknown media_type: {other}"),
    }
}

/// Scan for series: expects `{root}/{Series Name}/Season XX/file.mkv`
async fn scan_series(pool: &PgPool, root_path: &Path) -> Result<DiskScanResult> {
    let mut result = DiskScanResult {
        files_found: 0,
        files_matched: 0,
        files_unmatched: 0,
        files_already_tracked: 0,
        unmatched_files: Vec::new(),
    };

    // Walk for media files
    for entry in walkdir::WalkDir::new(root_path)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        if !is_media_extension(&ext) {
            continue;
        }

        result.files_found += 1;
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0) as i64;

        // Extract series name from first-level directory under root
        let relative = match path.strip_prefix(root_path) {
            Ok(r) => r,
            Err(_) => {
                result.files_unmatched += 1;
                result.unmatched_files.push(path.display().to_string());
                continue;
            }
        };

        let components: Vec<_> = relative.components().collect();
        if components.is_empty() {
            result.files_unmatched += 1;
            result.unmatched_files.push(path.display().to_string());
            continue;
        }

        // First component is the series directory name
        let series_dir_name = components[0].as_os_str().to_string_lossy().to_string();
        let clean_dir = stackarr_parser::clean_title(&series_dir_name);

        // Match to series in DB by clean_title
        let series_row: Option<(i64,)> = sqlx::query_as(
            "SELECT id FROM series WHERE clean_title = $1",
        )
        .bind(&clean_dir)
        .fetch_optional(pool)
        .await?;

        let series_id = match series_row {
            Some((id,)) => id,
            None => {
                tracing::debug!(
                    series_dir = %series_dir_name,
                    clean_title = %clean_dir,
                    "no matching series found in DB"
                );
                result.files_unmatched += 1;
                result.unmatched_files.push(path.display().to_string());
                continue;
            }
        };

        // Check if this file is already tracked
        let relative_path_str = relative.display().to_string();
        let existing: Option<(i64,)> = sqlx::query_as(
            "SELECT id FROM media_files WHERE relative_path = $1 AND media_type = 'series'",
        )
        .bind(&relative_path_str)
        .fetch_optional(pool)
        .await?;

        if existing.is_some() {
            result.files_already_tracked += 1;
            continue;
        }

        // Parse the filename for quality/episode info
        let filename = path
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("");
        let parsed = stackarr_parser::parse_release(filename);
        let quality_json = serde_json::to_value(&parsed.quality)?;
        let languages_json = serde_json::to_value(&parsed.languages)?;

        // Insert media_file record
        let media_file_row: (i64,) = sqlx::query_as(
            "INSERT INTO media_files (media_type, relative_path, size, quality, languages, scene_name, release_group, release_hash, edition)
             VALUES ('series', $1, $2, $3, $4, $5, $6, $7, $8)
             RETURNING id",
        )
        .bind(&relative_path_str)
        .bind(size)
        .bind(&quality_json)
        .bind(&languages_json)
        .bind(filename)
        .bind(&parsed.release_group)
        .bind(&parsed.release_hash)
        .bind(&parsed.edition)
        .fetch_one(pool)
        .await?;

        let media_file_id = media_file_row.0;

        // Try to match to specific episode
        let season = parsed.episode_info.season_number;
        let episodes = &parsed.episode_info.episode_numbers;

        if let Some(season_num) = season {
            for &ep_num in episodes {
                // Find the episode
                let episode_row: Option<(i64,)> = sqlx::query_as(
                    "SELECT id FROM episodes WHERE series_id = $1 AND season_number = $2 AND episode_number = $3",
                )
                .bind(series_id)
                .bind(season_num)
                .bind(ep_num)
                .fetch_optional(pool)
                .await?;

                if let Some((episode_id,)) = episode_row {
                    // Link episode to media file via episode_files join table
                    sqlx::query(
                        "INSERT INTO episode_files (episode_id, media_file_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
                    )
                    .bind(episode_id)
                    .bind(media_file_id)
                    .execute(pool)
                    .await?;

                    // Also update the episode's episode_file_id pointer
                    sqlx::query(
                        "UPDATE episodes SET episode_file_id = $1 WHERE id = $2 AND episode_file_id IS NULL",
                    )
                    .bind(media_file_id)
                    .bind(episode_id)
                    .execute(pool)
                    .await?;
                }
            }
        }

        result.files_matched += 1;
        tracing::debug!(
            file = %relative_path_str,
            series_id,
            "matched series file"
        );
    }

    tracing::info!(
        found = result.files_found,
        matched = result.files_matched,
        unmatched = result.files_unmatched,
        already_tracked = result.files_already_tracked,
        "series disk scan complete"
    );

    Ok(result)
}

/// Scan for movies: expects `{root}/{Movie Name (Year)}/file.mkv`
async fn scan_movies(pool: &PgPool, root_path: &Path) -> Result<DiskScanResult> {
    let mut result = DiskScanResult {
        files_found: 0,
        files_matched: 0,
        files_unmatched: 0,
        files_already_tracked: 0,
        unmatched_files: Vec::new(),
    };

    for entry in walkdir::WalkDir::new(root_path)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        if !is_media_extension(&ext) {
            continue;
        }

        result.files_found += 1;
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0) as i64;

        // Extract movie name from first-level directory under root
        let relative = match path.strip_prefix(root_path) {
            Ok(r) => r,
            Err(_) => {
                result.files_unmatched += 1;
                result.unmatched_files.push(path.display().to_string());
                continue;
            }
        };

        let components: Vec<_> = relative.components().collect();
        if components.is_empty() {
            result.files_unmatched += 1;
            result.unmatched_files.push(path.display().to_string());
            continue;
        }

        // First component is the movie directory name
        let movie_dir_name = components[0].as_os_str().to_string_lossy().to_string();
        let clean_dir = stackarr_parser::clean_title(&movie_dir_name);

        // Match to movie in DB by clean_title
        let movie_row: Option<(i64,)> = sqlx::query_as(
            "SELECT id FROM movies WHERE clean_title = $1",
        )
        .bind(&clean_dir)
        .fetch_optional(pool)
        .await?;

        let movie_id = match movie_row {
            Some((id,)) => id,
            None => {
                tracing::debug!(
                    movie_dir = %movie_dir_name,
                    clean_title = %clean_dir,
                    "no matching movie found in DB"
                );
                result.files_unmatched += 1;
                result.unmatched_files.push(path.display().to_string());
                continue;
            }
        };

        // Check if this file is already tracked
        let relative_path_str = relative.display().to_string();
        let existing: Option<(i64,)> = sqlx::query_as(
            "SELECT id FROM media_files WHERE relative_path = $1 AND media_type = 'movie'",
        )
        .bind(&relative_path_str)
        .fetch_optional(pool)
        .await?;

        if existing.is_some() {
            result.files_already_tracked += 1;
            continue;
        }

        // Parse the filename for quality info
        let filename = path
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("");
        let parsed = stackarr_parser::parse_release(filename);
        let quality_json = serde_json::to_value(&parsed.quality)?;
        let languages_json = serde_json::to_value(&parsed.languages)?;

        // Insert media_file record
        let media_file_row: (i64,) = sqlx::query_as(
            "INSERT INTO media_files (media_type, relative_path, size, quality, languages, scene_name, release_group, release_hash, edition)
             VALUES ('movie', $1, $2, $3, $4, $5, $6, $7, $8)
             RETURNING id",
        )
        .bind(&relative_path_str)
        .bind(size)
        .bind(&quality_json)
        .bind(&languages_json)
        .bind(filename)
        .bind(&parsed.release_group)
        .bind(&parsed.release_hash)
        .bind(&parsed.edition)
        .fetch_one(pool)
        .await?;

        let media_file_id = media_file_row.0;

        // Link to movie
        sqlx::query(
            "UPDATE movies SET movie_file_id = $1 WHERE id = $2 AND movie_file_id IS NULL",
        )
        .bind(media_file_id)
        .bind(movie_id)
        .execute(pool)
        .await?;

        result.files_matched += 1;
        tracing::debug!(
            file = %relative_path_str,
            movie_id,
            "matched movie file"
        );
    }

    tracing::info!(
        found = result.files_found,
        matched = result.files_matched,
        unmatched = result.files_unmatched,
        already_tracked = result.files_already_tracked,
        "movie disk scan complete"
    );

    Ok(result)
}
