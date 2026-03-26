pub mod naming;

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Serialize;
use sqlx::PgPool;

use naming::{build_episode_filename, build_movie_filename, build_season_folder, sanitize_filename};

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
    pub imported_files: Vec<ImportedFile>,
    pub skipped_files: Vec<String>,
    pub errors: Vec<String>,
}

/// A single file that was successfully imported.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedFile {
    pub source_path: String,
    pub dest_path: String,
    pub media_file_id: i64,
    pub quality: String,
    pub size: i64,
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

/// Context needed to process a completed download.
pub struct ImportContext {
    pub pool: PgPool,
    /// Download ID from the queue record.
    pub download_id: String,
    /// Path where the download client placed the files.
    pub output_path: PathBuf,
    /// `"series"` or `"movie"` from the queue record.
    pub media_type: String,
    /// The series_id or movie_id from the queue record.
    pub media_id: i64,
    /// The episode_id from the queue record (for TV).
    pub episode_id: Option<i64>,
}

// ── Naming config loaded from DB ────────────────────────────────────────────

struct NamingConfig {
    rename_files: bool,
    standard_format: Option<String>,
    season_folder_format: Option<String>,
    movie_format: Option<String>,
    colon_replacement: String,
}

async fn load_naming_config(pool: &PgPool, media_type: &str) -> Result<NamingConfig> {
    let row: Option<(bool, Option<String>, Option<String>, Option<String>, String)> =
        sqlx::query_as(
            "SELECT rename_files, standard_format, season_folder_format, movie_format, colon_replacement \
             FROM naming_config WHERE media_type = $1",
        )
        .bind(media_type)
        .fetch_optional(pool)
        .await?;

    match row {
        Some((rename_files, standard_format, season_folder_format, movie_format, colon_replacement)) => {
            Ok(NamingConfig {
                rename_files,
                standard_format,
                season_folder_format,
                movie_format,
                colon_replacement,
            })
        }
        None => {
            // Sensible defaults if no config exists
            Ok(NamingConfig {
                rename_files: true,
                standard_format: Some(
                    "{Series Title} - S{season:00}E{episode:00} - {Episode Title} [{Quality Title}]"
                        .to_string(),
                ),
                season_folder_format: Some("Season {season:00}".to_string()),
                movie_format: Some(
                    "{Movie Title} ({Release Year}) [{Quality Title}]".to_string(),
                ),
                colon_replacement: "smart".to_string(),
            })
        }
    }
}

// ── Media extensions ─────────────────────────────────────────────────────────

fn is_media_extension(ext: &str) -> bool {
    matches!(
        ext,
        "mkv" | "mp4" | "avi" | "wmv" | "ts" | "m4v" | "flv" | "mov" | "webm"
    )
}

/// Minimum size (in bytes) for a file to not be considered a sample.
const SAMPLE_SIZE_THRESHOLD: u64 = 50 * 1024 * 1024; // 50 MB

/// Returns true if the file is likely a sample (small + "sample" in name).
fn is_sample(path: &Path, size: u64) -> bool {
    if size >= SAMPLE_SIZE_THRESHOLD {
        return false;
    }
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();
    name.contains("sample")
}

// ── Import pipeline ─────────────────────────────────────────────────────────

/// Process a completed download: scan for media files, match to the library
/// item identified in the queue, rename with naming config tokens, move into
/// the library path, and update the database.
pub async fn process_completed_download(ctx: ImportContext) -> Result<ImportResult> {
    tracing::info!(
        download_id = %ctx.download_id,
        path = %ctx.output_path.display(),
        media_type = %ctx.media_type,
        media_id = ctx.media_id,
        "processing completed download"
    );

    let mut result = ImportResult {
        imported_files: Vec::new(),
        skipped_files: Vec::new(),
        errors: Vec::new(),
    };

    // 1. Scan output_path for media files
    let files = scan_folder_for_media(&ctx.output_path)?;
    if files.is_empty() {
        result.errors.push(format!(
            "no media files found in {}",
            ctx.output_path.display()
        ));
        return Ok(result);
    }

    tracing::info!(count = files.len(), "discovered media files");

    // 2. Filter out samples
    let media_files: Vec<_> = files
        .into_iter()
        .filter(|f| {
            if is_sample(&f.path, f.size) {
                tracing::debug!(path = %f.path.display(), "skipping sample file");
                result.skipped_files.push(f.path.display().to_string());
                false
            } else {
                true
            }
        })
        .collect();

    // 3. Process each media file
    for file in &media_files {
        match process_single_file(&ctx, file, &mut result).await {
            Ok(()) => {}
            Err(e) => {
                let msg = format!("error importing {}: {e}", file.path.display());
                tracing::error!("{msg}");
                result.errors.push(msg);
            }
        }
    }

    tracing::info!(
        imported = result.imported_files.len(),
        skipped = result.skipped_files.len(),
        errors = result.errors.len(),
        "import complete for download {}",
        ctx.download_id
    );

    Ok(result)
}

/// Process a single media file from a completed download.
async fn process_single_file(
    ctx: &ImportContext,
    file: &LocalFile,
    result: &mut ImportResult,
) -> Result<()> {
    // Parse the filename for quality/episode info
    let filename = file
        .path
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("");
    let parsed = stackarr_parser::parse_release(filename);

    match ctx.media_type.as_str() {
        "series" => {
            import_series_file(ctx, file, &parsed, result).await?;
        }
        "movie" => {
            import_movie_file(ctx, file, &parsed, result).await?;
        }
        other => {
            result
                .errors
                .push(format!("unknown media_type '{other}' for {filename}"));
        }
    }

    Ok(())
}

/// Import a media file for a TV series episode.
async fn import_series_file(
    ctx: &ImportContext,
    file: &LocalFile,
    parsed: &stackarr_parser::ParsedRelease,
    result: &mut ImportResult,
) -> Result<()> {
    let pool = &ctx.pool;

    // Load series from DB
    let series_row: Option<(i64, String, String)> = sqlx::query_as(
        "SELECT id, title, path FROM series WHERE id = $1",
    )
    .bind(ctx.media_id)
    .fetch_optional(pool)
    .await?;

    let (series_id, series_title, series_path) = match series_row {
        Some(row) => row,
        None => {
            anyhow::bail!("series id={} not found in DB", ctx.media_id);
        }
    };

    // Determine season + episode numbers from the parsed release or the queue
    let season = parsed.episode_info.season_number;
    let episodes = &parsed.episode_info.episode_numbers;

    // If the queue has a specific episode_id, load that episode
    let episode_row: Option<(i64, i32, i32, Option<String>)> = if let Some(ep_id) = ctx.episode_id
    {
        sqlx::query_as(
            "SELECT id, season_number, episode_number, title FROM episodes WHERE id = $1",
        )
        .bind(ep_id)
        .fetch_optional(pool)
        .await?
    } else if let (Some(s), Some(&e)) = (season, episodes.first()) {
        // Fall back to matching by season/episode from parsed name
        sqlx::query_as(
            "SELECT id, season_number, episode_number, title FROM episodes \
             WHERE series_id = $1 AND season_number = $2 AND episode_number = $3",
        )
        .bind(series_id)
        .bind(s)
        .bind(e)
        .fetch_optional(pool)
        .await?
    } else {
        None
    };

    let (episode_id, season_num, episode_num, episode_title) = match episode_row {
        Some(row) => row,
        None => {
            result.skipped_files.push(format!(
                "{}: could not match to episode",
                file.path.display()
            ));
            return Ok(());
        }
    };

    // Load naming config
    let naming = load_naming_config(pool, "series").await?;
    let ext = &file.extension;

    // Build the destination path
    let dest_path = if naming.rename_files {
        let format = naming
            .standard_format
            .as_deref()
            .unwrap_or("{Series Title} - S{season:00}E{episode:00} - {Episode Title} [{Quality Title}]");

        let episode_filename = build_episode_filename(
            format,
            &series_title,
            season_num,
            episode_num,
            episode_title.as_deref(),
            &parsed.quality.quality,
            parsed.release_group.as_deref(),
            parsed.episode_info.absolute_episode_numbers.first().copied(),
        );
        let safe_filename = sanitize_filename(&episode_filename, &naming.colon_replacement);

        let season_folder_fmt = naming
            .season_folder_format
            .as_deref()
            .unwrap_or("Season {season:00}");
        let season_folder = build_season_folder(season_folder_fmt, season_num);
        let safe_season = sanitize_filename(&season_folder, &naming.colon_replacement);

        PathBuf::from(&series_path)
            .join(&safe_season)
            .join(format!("{safe_filename}.{ext}"))
    } else {
        // Keep original filename, just place in series/season folder
        let original_name = file
            .path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        let season_folder_fmt = naming
            .season_folder_format
            .as_deref()
            .unwrap_or("Season {season:00}");
        let season_folder = build_season_folder(season_folder_fmt, season_num);
        let safe_season = sanitize_filename(&season_folder, &naming.colon_replacement);

        PathBuf::from(&series_path)
            .join(&safe_season)
            .join(original_name.as_ref())
    };

    // Create parent directories
    if let Some(parent) = dest_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    // Move file: try rename first (same filesystem), fall back to copy+remove
    move_file(&file.path, &dest_path).await?;

    // Build relative path (relative to series root)
    let relative_path = dest_path
        .strip_prefix(&series_path)
        .unwrap_or(&dest_path)
        .display()
        .to_string();

    // Insert media_files record
    let quality_json = serde_json::to_value(&parsed.quality)?;
    let languages_json = serde_json::to_value(&parsed.languages)?;
    let size = file.size as i64;

    let media_file_row: (i64,) = sqlx::query_as(
        "INSERT INTO media_files (media_type, relative_path, size, quality, languages, scene_name, release_group, release_hash, edition) \
         VALUES ('series', $1, $2, $3, $4, $5, $6, $7, $8) \
         RETURNING id",
    )
    .bind(&relative_path)
    .bind(size)
    .bind(&quality_json)
    .bind(&languages_json)
    .bind(file.path.file_name().and_then(|f| f.to_str()).unwrap_or(""))
    .bind(&parsed.release_group)
    .bind(&parsed.release_hash)
    .bind(&parsed.edition)
    .fetch_one(pool)
    .await?;

    let media_file_id = media_file_row.0;

    // Link episode to media file
    sqlx::query(
        "INSERT INTO episode_files (episode_id, media_file_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(episode_id)
    .bind(media_file_id)
    .execute(pool)
    .await?;

    // Update the episode's file pointer
    sqlx::query("UPDATE episodes SET episode_file_id = $1 WHERE id = $2")
        .bind(media_file_id)
        .bind(episode_id)
        .execute(pool)
        .await?;

    // Also link any additional episodes (multi-episode files)
    if episodes.len() > 1 {
        for &ep_num in episodes.iter().skip(1) {
            if let Some(s) = season {
                let extra_ep: Option<(i64,)> = sqlx::query_as(
                    "SELECT id FROM episodes WHERE series_id = $1 AND season_number = $2 AND episode_number = $3",
                )
                .bind(series_id)
                .bind(s)
                .bind(ep_num)
                .fetch_optional(pool)
                .await?;

                if let Some((extra_id,)) = extra_ep {
                    sqlx::query(
                        "INSERT INTO episode_files (episode_id, media_file_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
                    )
                    .bind(extra_id)
                    .bind(media_file_id)
                    .execute(pool)
                    .await?;

                    sqlx::query(
                        "UPDATE episodes SET episode_file_id = $1 WHERE id = $2 AND episode_file_id IS NULL",
                    )
                    .bind(media_file_id)
                    .bind(extra_id)
                    .execute(pool)
                    .await?;
                }
            }
        }
    }

    // Insert history record
    sqlx::query(
        "INSERT INTO history (media_type, media_id, episode_id, event_type, quality, languages, source_title, download_id) \
         VALUES ('series', $1, $2, 'imported', $3, $4, $5, $6)",
    )
    .bind(series_id)
    .bind(episode_id)
    .bind(&quality_json)
    .bind(&languages_json)
    .bind(file.path.file_name().and_then(|f| f.to_str()).unwrap_or(""))
    .bind(&ctx.download_id)
    .execute(pool)
    .await?;

    let quality_str = format!("{:?}", parsed.quality.quality);
    result.imported_files.push(ImportedFile {
        source_path: file.path.display().to_string(),
        dest_path: dest_path.display().to_string(),
        media_file_id,
        quality: quality_str,
        size,
    });

    tracing::info!(
        source = %file.path.display(),
        dest = %dest_path.display(),
        media_file_id,
        "imported series episode"
    );

    Ok(())
}

/// Import a media file for a movie.
async fn import_movie_file(
    ctx: &ImportContext,
    file: &LocalFile,
    parsed: &stackarr_parser::ParsedRelease,
    result: &mut ImportResult,
) -> Result<()> {
    let pool = &ctx.pool;

    // Load movie from DB
    let movie_row: Option<(i64, String, String, Option<i32>)> = sqlx::query_as(
        "SELECT id, title, path, year FROM movies WHERE id = $1",
    )
    .bind(ctx.media_id)
    .fetch_optional(pool)
    .await?;

    let (movie_id, movie_title, movie_path, movie_year) = match movie_row {
        Some(row) => row,
        None => {
            anyhow::bail!("movie id={} not found in DB", ctx.media_id);
        }
    };

    // Load naming config
    let naming = load_naming_config(pool, "movie").await?;
    let ext = &file.extension;

    // Build the destination path
    let dest_path = if naming.rename_files {
        let format = naming
            .movie_format
            .as_deref()
            .unwrap_or("{Movie Title} ({Release Year}) [{Quality Title}]");

        let movie_filename = build_movie_filename(
            format,
            &movie_title,
            movie_year,
            &parsed.quality.quality,
            parsed.edition.as_deref(),
            parsed.release_group.as_deref(),
        );
        let safe_filename = sanitize_filename(&movie_filename, &naming.colon_replacement);

        PathBuf::from(&movie_path).join(format!("{safe_filename}.{ext}"))
    } else {
        let original_name = file
            .path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        PathBuf::from(&movie_path).join(original_name.as_ref())
    };

    // Create parent directories
    if let Some(parent) = dest_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    // Move file
    move_file(&file.path, &dest_path).await?;

    // Build relative path (relative to movie root)
    let relative_path = dest_path
        .strip_prefix(&movie_path)
        .unwrap_or(&dest_path)
        .display()
        .to_string();

    // Insert media_files record
    let quality_json = serde_json::to_value(&parsed.quality)?;
    let languages_json = serde_json::to_value(&parsed.languages)?;
    let size = file.size as i64;

    let media_file_row: (i64,) = sqlx::query_as(
        "INSERT INTO media_files (media_type, relative_path, size, quality, languages, scene_name, release_group, release_hash, edition) \
         VALUES ('movie', $1, $2, $3, $4, $5, $6, $7, $8) \
         RETURNING id",
    )
    .bind(&relative_path)
    .bind(size)
    .bind(&quality_json)
    .bind(&languages_json)
    .bind(file.path.file_name().and_then(|f| f.to_str()).unwrap_or(""))
    .bind(&parsed.release_group)
    .bind(&parsed.release_hash)
    .bind(&parsed.edition)
    .fetch_one(pool)
    .await?;

    let media_file_id = media_file_row.0;

    // Link to movie
    sqlx::query("UPDATE movies SET movie_file_id = $1 WHERE id = $2")
        .bind(media_file_id)
        .bind(movie_id)
        .execute(pool)
        .await?;

    // Insert history record
    sqlx::query(
        "INSERT INTO history (media_type, media_id, event_type, quality, languages, source_title, download_id) \
         VALUES ('movie', $1, 'imported', $2, $3, $4, $5)",
    )
    .bind(movie_id)
    .bind(&quality_json)
    .bind(&languages_json)
    .bind(file.path.file_name().and_then(|f| f.to_str()).unwrap_or(""))
    .bind(&ctx.download_id)
    .execute(pool)
    .await?;

    let quality_str = format!("{:?}", parsed.quality.quality);
    result.imported_files.push(ImportedFile {
        source_path: file.path.display().to_string(),
        dest_path: dest_path.display().to_string(),
        media_file_id,
        quality: quality_str,
        size,
    });

    tracing::info!(
        source = %file.path.display(),
        dest = %dest_path.display(),
        media_file_id,
        "imported movie file"
    );

    Ok(())
}

// ── File operations ─────────────────────────────────────────────────────────

/// Move a file from `src` to `dest`. Tries `tokio::fs::rename` first (fast,
/// same-filesystem). On cross-device errors, falls back to copy + remove.
async fn move_file(src: &Path, dest: &Path) -> Result<()> {
    match tokio::fs::rename(src, dest).await {
        Ok(()) => {
            tracing::debug!(src = %src.display(), dest = %dest.display(), "renamed file");
            Ok(())
        }
        Err(e) if e.raw_os_error() == Some(18 /* EXDEV */) => {
            // Cross-device link — fall back to copy + remove
            tracing::debug!(
                src = %src.display(),
                dest = %dest.display(),
                "cross-device move, falling back to copy+remove"
            );
            tokio::fs::copy(src, dest).await?;
            tokio::fs::remove_file(src).await?;
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}

// ── Folder scanning ─────────────────────────────────────────────────────────

/// Recursively scan a folder for media files, returning all discovered files.
fn scan_folder_for_media(folder: &Path) -> Result<Vec<LocalFile>> {
    let mut files = Vec::new();

    if !folder.exists() {
        anyhow::bail!("folder does not exist: {}", folder.display());
    }

    // Handle case where output_path is a single file
    if folder.is_file() {
        let ext = folder
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if is_media_extension(&ext) {
            let size = std::fs::metadata(folder).map(|m| m.len()).unwrap_or(0);
            files.push(LocalFile {
                path: folder.to_path_buf(),
                size,
                extension: ext,
            });
        }
        return Ok(files);
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

// ── Import service (kept for backward compat) ───────────────────────────────

#[derive(Clone)]
pub struct ImportService {
    #[allow(dead_code)]
    pool: PgPool,
}

impl ImportService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Process a completed download via the new import pipeline.
    pub async fn process_completed_download(
        &self,
        download_folder: &Path,
        download_id: &str,
    ) -> Result<ImportResult> {
        // Legacy shim: without full context we cannot do the complete pipeline,
        // so just do a file scan and return counts.
        tracing::info!(
            download_id,
            path = %download_folder.display(),
            "processing completed download (legacy path)"
        );

        let files = scan_folder_for_media(download_folder)?;
        let _media_count = files
            .iter()
            .filter(|f| !is_sample(&f.path, f.size))
            .count();

        Ok(ImportResult {
            imported_files: Vec::new(),
            skipped_files: Vec::new(),
            errors: if files.is_empty() {
                vec![format!(
                    "no media files found in {}",
                    download_folder.display()
                )]
            } else {
                Vec::new()
            },
        })
    }

    /// Recursively scan a folder for media files.
    pub fn scan_folder(&self, folder: &Path) -> Result<Vec<LocalFile>> {
        scan_folder_for_media(folder)
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
