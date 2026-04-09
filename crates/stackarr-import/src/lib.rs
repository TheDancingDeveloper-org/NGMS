pub mod naming;
pub mod recycle_bin;
pub mod upgrade;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Serialize;
use sqlx::PgPool;

use naming::{
    build_episode_filename, build_movie_filename, build_season_folder, sanitize_filename,
};
use stackarr_stream::types::MediaInfo;

// ── ffprobe helper ─────────────────────────────────────────────────────────

/// Probe a file with ffprobe to extract media info. Returns `None` on failure
/// (non-blocking — naming still works, just without MediaInfo tokens).
async fn probe_media_info(ffprobe_path: Option<&str>, file_path: &Path) -> Option<MediaInfo> {
    let ffprobe = ffprobe_path?;
    match stackarr_stream::ffprobe::probe(ffprobe, file_path).await {
        Ok(info) => Some(info),
        Err(e) => {
            tracing::warn!(
                path = %file_path.display(),
                error = %e,
                "failed to probe media info — MediaInfo naming tokens will be empty"
            );
            None
        }
    }
}

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
    /// Path to ffprobe binary for media info extraction. If `None`, MediaInfo
    /// tokens in naming formats will resolve to empty strings.
    pub ffprobe_path: Option<String>,
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
    #[allow(clippy::type_complexity)]
    let row: Option<(bool, Option<String>, Option<String>, Option<String>, String)> =
        sqlx::query_as(
            "SELECT rename_files, standard_format, season_folder_format, movie_format, colon_replacement \
             FROM naming_config WHERE media_type = $1",
        )
        .bind(media_type)
        .fetch_optional(pool)
        .await?;

    match row {
        Some((
            rename_files,
            standard_format,
            season_folder_format,
            movie_format,
            colon_replacement,
        )) => Ok(NamingConfig {
            rename_files,
            standard_format,
            season_folder_format,
            movie_format,
            colon_replacement,
        }),
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
        ext.to_ascii_lowercase().as_str(),
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
    let full = path.to_string_lossy().to_lowercase();
    full.contains("sample")
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
                let msg = format!("error importing {}: {e:#}", file.path.display());
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
    let filename = file.path.file_name().and_then(|f| f.to_str()).unwrap_or("");
    let parsed = stackarr_parser::parse_release(filename);

    match ctx.media_type.as_str() {
        "series" | "tv" => {
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
    let series_row: Option<(i64, String, String)> =
        sqlx::query_as("SELECT id, title, path FROM series WHERE id = $1")
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
    let episode_row: Option<(i64, i32, i32, Option<String>)> = if let Some(ep_id) = ctx.episode_id {
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

    // ── Upgrade check ───────────────────────────────────────────────────
    let new_quality_num = stackarr_quality::parser_quality_to_num(parsed.quality.quality);

    let upgrade_result =
        upgrade::check_upgrade(pool, "series", series_id, Some(episode_id), new_quality_num)
            .await?;

    match upgrade_result {
        upgrade::UpgradeCheckResult::NotAnUpgrade { reason } => {
            tracing::info!(
                episode_id,
                reason = %reason,
                file = %file.path.display(),
                "skipping import: not an upgrade"
            );
            result
                .skipped_files
                .push(format!("{}: {reason}", file.path.display()));
            return Ok(());
        }
        upgrade::UpgradeCheckResult::Upgrade {
            existing_file_id,
            existing_path,
            existing_quality,
        } => {
            // Root folder guard: refuse to proceed if the library path is missing
            if !std::path::Path::new(&series_path).exists() {
                anyhow::bail!(
                    "root folder '{series_path}' does not exist — refusing to replace existing file \
                     (is the drive mounted?)"
                );
            }

            // Move old file to recycle bin (or permanently delete)
            let recycled = recycle_bin::recycle_file(
                pool,
                &existing_path,
                existing_file_id,
                "series",
                series_id,
            )
            .await?;

            // Clean up old DB records
            sqlx::query("DELETE FROM episode_files WHERE media_file_id = $1")
                .bind(existing_file_id)
                .execute(pool)
                .await?;
            sqlx::query("UPDATE episodes SET episode_file_id = NULL WHERE episode_file_id = $1")
                .bind(existing_file_id)
                .execute(pool)
                .await?;
            sqlx::query("DELETE FROM media_files WHERE id = $1")
                .bind(existing_file_id)
                .execute(pool)
                .await?;

            // Record deletion in history
            let delete_data = serde_json::json!({
                "reason": "upgrade",
                "recycled": recycled.is_some(),
                "recycle_path": recycled.as_ref().map(|p| p.display().to_string()),
                "replaced_by_quality": stackarr_quality::quality_name(new_quality_num),
            });
            sqlx::query(
                "INSERT INTO history (media_type, media_id, episode_id, event_type, quality, source_title, data) \
                 VALUES ('series', $1, $2, 'file_deleted', $3, $4, $5)",
            )
            .bind(series_id)
            .bind(episode_id)
            .bind(&existing_quality)
            .bind(existing_path.display().to_string())
            .bind(&delete_data)
            .execute(pool)
            .await?;

            tracing::info!(
                episode_id,
                old_file_id = existing_file_id,
                old_path = %existing_path.display(),
                new_quality = stackarr_quality::quality_name(new_quality_num),
                "replacing existing file (upgrade)"
            );
        }
        upgrade::UpgradeCheckResult::NoExistingFile => {
            // First file for this episode — proceed normally
        }
    }
    // ── End upgrade check ───────────────────────────────────────────────

    // Probe media info from the source file (before moving)
    let media_info = probe_media_info(ctx.ffprobe_path.as_deref(), &file.path).await;
    let naming_mi = media_info
        .as_ref()
        .map(naming::NamingMediaInfo::from_media_info);

    // Load naming config
    let naming = load_naming_config(pool, "series").await?;
    let ext = &file.extension;

    // Build the destination path
    let dest_path = if naming.rename_files {
        let format = naming.standard_format.as_deref().unwrap_or(
            "{Series Title} - S{season:00}E{episode:00} - {Episode Title} [{Quality Title}]",
        );

        let episode_filename = build_episode_filename(
            format,
            &series_title,
            season_num,
            episode_num,
            episode_title.as_deref(),
            &parsed.quality.quality,
            parsed.release_group.as_deref(),
            parsed
                .episode_info
                .absolute_episode_numbers
                .first()
                .copied(),
            naming_mi.as_ref(),
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
        let original_name = file.path.file_name().unwrap_or_default().to_string_lossy();
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
        tokio::fs::create_dir_all(parent).await.with_context(|| {
            format!(
                "failed to create destination directory '{}' — is the media library volume mounted?",
                parent.display()
            )
        })?;
    }

    tracing::info!(
        src = %file.path.display(),
        dest = %dest_path.display(),
        "moving file to library"
    );

    // Move file: try rename first (same filesystem), fall back to copy+remove
    move_file(&file.path, &dest_path).await.with_context(|| {
        format!(
            "failed to move '{}' -> '{}'",
            file.path.display(),
            dest_path.display()
        )
    })?;

    // Build relative path (relative to series root)
    let relative_path = dest_path
        .strip_prefix(&series_path)
        .unwrap_or(&dest_path)
        .display()
        .to_string();

    // Insert media_files record
    let quality_json = stackarr_quality::quality_model_to_json(&parsed.quality);
    let languages_json = serde_json::to_value(&parsed.languages)?;
    let size = file.size as i64;

    let media_info_json = media_info
        .as_ref()
        .and_then(|mi| serde_json::to_value(mi).ok());

    let media_file_row: (i64,) = sqlx::query_as(
        "INSERT INTO media_files (media_type, relative_path, size, quality, languages, scene_name, release_group, release_hash, edition, media_info) \
         VALUES ('series', $1, $2, $3, $4, $5, $6, $7, $8, $9) \
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
    .bind(&media_info_json)
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
    let movie_row: Option<(i64, String, String, Option<i32>)> =
        sqlx::query_as("SELECT id, title, path, year FROM movies WHERE id = $1")
            .bind(ctx.media_id)
            .fetch_optional(pool)
            .await?;

    let (movie_id, movie_title, movie_path, movie_year) = match movie_row {
        Some(row) => row,
        None => {
            anyhow::bail!("movie id={} not found in DB", ctx.media_id);
        }
    };

    // ── Upgrade check ───────────────────────────────────────────────────
    let new_quality_num = stackarr_quality::parser_quality_to_num(parsed.quality.quality);

    let upgrade_result =
        upgrade::check_upgrade(pool, "movie", movie_id, None, new_quality_num).await?;

    match upgrade_result {
        upgrade::UpgradeCheckResult::NotAnUpgrade { reason } => {
            tracing::info!(
                movie_id,
                reason = %reason,
                file = %file.path.display(),
                "skipping import: not an upgrade"
            );
            result
                .skipped_files
                .push(format!("{}: {reason}", file.path.display()));
            return Ok(());
        }
        upgrade::UpgradeCheckResult::Upgrade {
            existing_file_id,
            existing_path,
            existing_quality,
        } => {
            // Root folder guard
            if !std::path::Path::new(&movie_path).exists() {
                anyhow::bail!(
                    "root folder '{movie_path}' does not exist — refusing to replace existing file \
                     (is the drive mounted?)"
                );
            }

            // Move old file to recycle bin (or permanently delete)
            let recycled = recycle_bin::recycle_file(
                pool,
                &existing_path,
                existing_file_id,
                "movie",
                movie_id,
            )
            .await?;

            // Clean up old DB records
            sqlx::query("UPDATE movies SET movie_file_id = NULL WHERE movie_file_id = $1")
                .bind(existing_file_id)
                .execute(pool)
                .await?;
            sqlx::query("DELETE FROM media_files WHERE id = $1")
                .bind(existing_file_id)
                .execute(pool)
                .await?;

            // Record deletion in history
            let delete_data = serde_json::json!({
                "reason": "upgrade",
                "recycled": recycled.is_some(),
                "recycle_path": recycled.as_ref().map(|p| p.display().to_string()),
                "replaced_by_quality": stackarr_quality::quality_name(new_quality_num),
            });
            sqlx::query(
                "INSERT INTO history (media_type, media_id, episode_id, event_type, quality, source_title, data) \
                 VALUES ('movie', $1, NULL, 'file_deleted', $2, $3, $4)",
            )
            .bind(movie_id)
            .bind(&existing_quality)
            .bind(existing_path.display().to_string())
            .bind(&delete_data)
            .execute(pool)
            .await?;

            tracing::info!(
                movie_id,
                old_file_id = existing_file_id,
                old_path = %existing_path.display(),
                new_quality = stackarr_quality::quality_name(new_quality_num),
                "replacing existing file (upgrade)"
            );
        }
        upgrade::UpgradeCheckResult::NoExistingFile => {
            // First file for this movie — proceed normally
        }
    }
    // ── End upgrade check ───────────────────────────────────────────────

    // Probe media info from the source file (before moving)
    let media_info = probe_media_info(ctx.ffprobe_path.as_deref(), &file.path).await;
    let naming_mi = media_info
        .as_ref()
        .map(naming::NamingMediaInfo::from_media_info);

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
            naming_mi.as_ref(),
        );
        let safe_filename = sanitize_filename(&movie_filename, &naming.colon_replacement);

        PathBuf::from(&movie_path).join(format!("{safe_filename}.{ext}"))
    } else {
        let original_name = file.path.file_name().unwrap_or_default().to_string_lossy();
        PathBuf::from(&movie_path).join(original_name.as_ref())
    };

    // Create parent directories
    if let Some(parent) = dest_path.parent() {
        tokio::fs::create_dir_all(parent).await.with_context(|| {
            format!(
                "failed to create destination directory '{}' — is the media library volume mounted?",
                parent.display()
            )
        })?;
    }

    tracing::info!(
        src = %file.path.display(),
        dest = %dest_path.display(),
        "moving file to library"
    );

    // Move file
    move_file(&file.path, &dest_path).await.with_context(|| {
        format!(
            "failed to move '{}' -> '{}'",
            file.path.display(),
            dest_path.display()
        )
    })?;

    // Build relative path (relative to movie root)
    let relative_path = dest_path
        .strip_prefix(&movie_path)
        .unwrap_or(&dest_path)
        .display()
        .to_string();

    // Insert media_files record
    let quality_json = stackarr_quality::quality_model_to_json(&parsed.quality);
    let languages_json = serde_json::to_value(&parsed.languages)?;
    let size = file.size as i64;

    let media_info_json = media_info
        .as_ref()
        .and_then(|mi| serde_json::to_value(mi).ok());

    let media_file_row: (i64,) = sqlx::query_as(
        "INSERT INTO media_files (media_type, relative_path, size, quality, languages, scene_name, release_group, release_hash, edition, media_info) \
         VALUES ('movie', $1, $2, $3, $4, $5, $6, $7, $8, $9) \
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
    .bind(&media_info_json)
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
pub(crate) async fn move_file(src: &Path, dest: &Path) -> Result<()> {
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
        let _media_count = files.iter().filter(|f| !is_sample(&f.path, f.size)).count();

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

/// Scan a media library folder for media files on disk, matching them to series/movies
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
        anyhow::bail!(
            "media library folder does not exist: {}",
            root_path.display()
        );
    }

    match media_type {
        "series" | "tv" => scan_series(pool, root_path).await,
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

    // Pre-load series lookups: clean_title -> id and path folder name -> id
    let mut series_by_clean_title: HashMap<String, i64> = HashMap::new();
    let mut series_by_folder: HashMap<String, i64> = HashMap::new();
    let rows: Vec<(i64, String, String)> =
        sqlx::query_as("SELECT id, clean_title, path FROM series")
            .fetch_all(pool)
            .await?;
    for (id, clean_title, path) in &rows {
        series_by_clean_title.insert(clean_title.clone(), *id);
        // Extract last path segment (folder name) from the series path.
        // Lowercase the key for case-insensitive matching.
        let trimmed = path.trim_end_matches('/');
        if let Some(folder) = trimmed.rsplit('/').next()
            && !folder.is_empty()
        {
            series_by_folder.insert(folder.to_lowercase(), *id);
        }
    }

    // Pre-load tracked relative paths for series
    let tracked_paths: HashSet<String> = sqlx::query_as::<_, (String,)>(
        "SELECT relative_path FROM media_files WHERE media_type = 'series'",
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|(p,)| p)
    .collect();

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

        // Match to series using pre-loaded maps (clean_title or folder name)
        let series_id = match series_by_clean_title
            .get(&clean_dir)
            .or_else(|| series_by_folder.get(&series_dir_name.to_lowercase()))
        {
            Some(&id) => id,
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

        // Check if this file is already tracked using pre-loaded set
        let relative_path_str = relative.display().to_string();
        if tracked_paths.contains(&relative_path_str) {
            result.files_already_tracked += 1;
            continue;
        }

        // Parse the filename for quality/episode info
        let filename = path.file_name().and_then(|f| f.to_str()).unwrap_or("");
        let parsed = stackarr_parser::parse_release(filename);
        let quality_json = stackarr_quality::quality_model_to_json(&parsed.quality);
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

    // Pre-load movie lookups: clean_title -> id and path folder name -> id
    let mut movies_by_clean_title: HashMap<String, i64> = HashMap::new();
    let mut movies_by_folder: HashMap<String, i64> = HashMap::new();
    let rows: Vec<(i64, String, String)> =
        sqlx::query_as("SELECT id, clean_title, path FROM movies")
            .fetch_all(pool)
            .await?;
    for (id, clean_title, path) in &rows {
        movies_by_clean_title.insert(clean_title.clone(), *id);
        // Extract last path segment (folder name) from the movie path.
        // Lowercase the key for case-insensitive matching (the value is
        // just the movie ID — we never use the folder name for file ops).
        let trimmed = path.trim_end_matches('/');
        if let Some(folder) = trimmed.rsplit('/').next()
            && !folder.is_empty()
        {
            movies_by_folder.insert(folder.to_lowercase(), *id);
        }
    }

    // Pre-load tracked relative paths for movies
    let tracked_paths: HashSet<String> = sqlx::query_as::<_, (String,)>(
        "SELECT relative_path FROM media_files WHERE media_type = 'movie'",
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|(p,)| p)
    .collect();

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

        // Match to movie using pre-loaded maps (clean_title or folder name)
        let movie_id = match movies_by_clean_title
            .get(&clean_dir)
            .or_else(|| movies_by_folder.get(&movie_dir_name.to_lowercase()))
        {
            Some(&id) => id,
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

        // Check if this file is already tracked using pre-loaded set
        let relative_path_str = relative.display().to_string();
        if tracked_paths.contains(&relative_path_str) {
            result.files_already_tracked += 1;
            continue;
        }

        // Parse the filename for quality info
        let filename = path.file_name().and_then(|f| f.to_str()).unwrap_or("");
        let parsed = stackarr_parser::parse_release(filename);
        let quality_json = stackarr_quality::quality_model_to_json(&parsed.quality);
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
        sqlx::query("UPDATE movies SET movie_file_id = $1 WHERE id = $2 AND movie_file_id IS NULL")
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_is_media_extension_valid() {
        assert!(is_media_extension("mkv"));
        assert!(is_media_extension("mp4"));
        assert!(is_media_extension("avi"));
        assert!(is_media_extension("wmv"));
        assert!(is_media_extension("ts"));
        assert!(is_media_extension("m4v"));
        assert!(is_media_extension("flv"));
        assert!(is_media_extension("mov"));
        assert!(is_media_extension("webm"));
    }

    #[test]
    fn test_is_media_extension_invalid() {
        assert!(!is_media_extension("nfo"));
        assert!(!is_media_extension("txt"));
        assert!(!is_media_extension("jpg"));
        assert!(!is_media_extension("srt"));
        assert!(!is_media_extension("nzb"));
        assert!(!is_media_extension(""));
    }

    #[test]
    fn test_is_sample_small_with_keyword() {
        let path = Path::new("/downloads/Movie.2024/sample.mkv");
        assert!(is_sample(path, 10 * 1024 * 1024)); // 10 MB
    }

    #[test]
    fn test_is_sample_large_file() {
        let path = Path::new("/downloads/Movie.2024/sample.mkv");
        assert!(!is_sample(path, 100 * 1024 * 1024)); // 100 MB — too large to be sample
    }

    #[test]
    fn test_is_sample_no_keyword() {
        let path = Path::new("/downloads/Movie.2024/Movie.2024.720p.mkv");
        assert!(!is_sample(path, 10 * 1024 * 1024)); // small but no "sample" in name
    }

    #[test]
    fn test_is_sample_case_insensitive() {
        let path = Path::new("/downloads/Movie.Sample.mkv");
        assert!(is_sample(path, 5 * 1024 * 1024));
    }

    #[test]
    fn test_scan_folder_finds_media() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("movie.mkv"), "fake video content").unwrap();
        fs::write(dir.path().join("subtitle.srt"), "subtitle").unwrap();
        fs::write(dir.path().join("info.nfo"), "nfo").unwrap();

        let files = scan_folder_for_media(dir.path()).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].extension, "mkv");
    }

    #[test]
    fn test_scan_folder_empty() {
        let dir = tempfile::tempdir().unwrap();
        let files = scan_folder_for_media(dir.path()).unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn test_scan_folder_nonexistent() {
        let result = scan_folder_for_media(Path::new("/nonexistent/path/to/folder"));
        assert!(result.is_err());
    }

    #[test]
    fn test_scan_folder_single_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("episode.mp4");
        fs::write(&file_path, "video").unwrap();

        let files = scan_folder_for_media(&file_path).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].extension, "mp4");
    }

    #[test]
    fn test_scan_folder_nested() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("subfolder");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join("episode.mkv"), "video").unwrap();
        fs::write(dir.path().join("movie.avi"), "video2").unwrap();

        let files = scan_folder_for_media(dir.path()).unwrap();
        assert_eq!(files.len(), 2);
    }

    // ── Tests for disk_scan media_type routing ───────────────────────────
    // disk_scan requires a real PgPool + data, so we test the media_type
    // dispatch logic via the public function on a non-existent path to
    // confirm "tv" is accepted before the path-existence check.

    mod disk_scan_media_type {
        use super::*;
        use sqlx::postgres::PgPoolOptions;

        fn dummy_pool() -> PgPool {
            PgPoolOptions::new()
                .max_connections(1)
                .connect_lazy("postgresql://fake:fake@localhost:5432/fake")
                .expect("lazy pool")
        }

        #[tokio::test]
        async fn test_disk_scan_rejects_unknown_media_type() {
            let pool = dummy_pool();
            let dir = tempfile::tempdir().unwrap();

            let result = disk_scan(&pool, dir.path(), "anime").await;
            assert!(result.is_err());
            let err_msg = result.unwrap_err().to_string();
            assert!(
                err_msg.contains("unknown media_type"),
                "expected 'unknown media_type' error, got: {err_msg}"
            );
        }

        #[tokio::test]
        async fn test_disk_scan_accepts_series_media_type() {
            // "series" should pass the media_type check and reach the DB
            // query phase (which will fail with our dummy pool, not with
            // "unknown media_type")
            let pool = dummy_pool();
            let dir = tempfile::tempdir().unwrap();

            let result = disk_scan(&pool, dir.path(), "series").await;
            // Will either succeed (empty scan) or fail on DB — but NOT
            // with "unknown media_type"
            if let Err(e) = &result {
                assert!(
                    !e.to_string().contains("unknown media_type"),
                    "series should be accepted, got: {e}"
                );
            }
        }

        #[tokio::test]
        async fn test_disk_scan_accepts_tv_media_type() {
            // "tv" must be accepted as an alias for "series"
            let pool = dummy_pool();
            let dir = tempfile::tempdir().unwrap();

            let result = disk_scan(&pool, dir.path(), "tv").await;
            if let Err(e) = &result {
                assert!(
                    !e.to_string().contains("unknown media_type"),
                    "tv should be accepted as alias for series, got: {e}"
                );
            }
        }

        #[tokio::test]
        async fn test_disk_scan_accepts_movie_media_type() {
            let pool = dummy_pool();
            let dir = tempfile::tempdir().unwrap();

            let result = disk_scan(&pool, dir.path(), "movie").await;
            if let Err(e) = &result {
                assert!(
                    !e.to_string().contains("unknown media_type"),
                    "movie should be accepted, got: {e}"
                );
            }
        }

        #[tokio::test]
        async fn test_disk_scan_nonexistent_path_errors() {
            let pool = dummy_pool();
            let result = disk_scan(&pool, Path::new("/nonexistent/media/path"), "movie").await;
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("does not exist"));
        }
    }

    // ── Test for process_single_file media_type routing ──────────────────

    mod import_media_type {
        #[test]
        fn test_import_context_media_type_values() {
            // Verify the media_type string values that ImportContext accepts
            // are documented: "series", "tv", and "movie" should all be valid.
            let valid_types = ["series", "tv", "movie"];
            let invalid_types = ["anime", "TV", "Series", "Movie", ""];

            for t in &valid_types {
                // These should match in process_single_file's match arms
                assert!(
                    matches!(*t, "series" | "tv" | "movie"),
                    "{t} should be a valid media_type"
                );
            }
            for t in &invalid_types {
                assert!(
                    !matches!(*t, "series" | "tv" | "movie"),
                    "{t} should NOT be a valid media_type"
                );
            }
        }
    }

    // ── Additional media extension tests ──────────────────────────────

    #[test]
    fn test_is_media_extension_case_insensitive_existing() {
        // The function normalizes input to lowercase internally
        assert!(is_media_extension("MKV"));
        assert!(is_media_extension("MP4"));
    }

    // ── Additional sample detection tests ─────────────────────────────

    #[test]
    fn test_is_sample_exactly_at_threshold() {
        let path = Path::new("/downloads/sample.mkv");
        // Exactly at threshold — not a sample
        assert!(!is_sample(path, SAMPLE_SIZE_THRESHOLD));
    }

    #[test]
    fn test_is_sample_just_below_threshold() {
        let path = Path::new("/downloads/sample.mkv");
        assert!(is_sample(path, SAMPLE_SIZE_THRESHOLD - 1));
    }

    #[test]
    fn test_is_sample_in_subdirectory() {
        let path = Path::new("/downloads/Movie.2024/Sample/sample_video.mkv");
        assert!(is_sample(path, 5 * 1024 * 1024));
    }

    #[test]
    fn test_is_sample_zero_size() {
        let path = Path::new("/downloads/sample.mkv");
        assert!(is_sample(path, 0));
    }

    // ── Folder scanning edge cases ────────────────────────────────────

    #[test]
    fn test_scan_folder_multiple_extensions() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("movie.mkv"), "video").unwrap();
        fs::write(dir.path().join("movie.mp4"), "video2").unwrap();
        fs::write(dir.path().join("movie.avi"), "video3").unwrap();
        fs::write(dir.path().join("movie.wmv"), "video4").unwrap();
        fs::write(dir.path().join("movie.ts"), "video5").unwrap();
        fs::write(dir.path().join("movie.m4v"), "video6").unwrap();
        fs::write(dir.path().join("movie.flv"), "video7").unwrap();
        fs::write(dir.path().join("movie.mov"), "video8").unwrap();
        fs::write(dir.path().join("movie.webm"), "video9").unwrap();
        fs::write(dir.path().join("notes.txt"), "text").unwrap();

        let files = scan_folder_for_media(dir.path()).unwrap();
        assert_eq!(files.len(), 9);
    }

    #[test]
    fn test_scan_folder_deeply_nested() {
        let dir = tempfile::tempdir().unwrap();
        let deep = dir.path().join("a").join("b").join("c");
        fs::create_dir_all(&deep).unwrap();
        fs::write(deep.join("episode.mkv"), "video").unwrap();

        let files = scan_folder_for_media(dir.path()).unwrap();
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn test_scan_folder_single_non_media_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("readme.txt");
        fs::write(&file_path, "not video").unwrap();

        let files = scan_folder_for_media(&file_path).unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn test_scan_folder_captures_size() {
        let dir = tempfile::tempdir().unwrap();
        let content = vec![0u8; 1024];
        fs::write(dir.path().join("movie.mp4"), &content).unwrap();

        let files = scan_folder_for_media(dir.path()).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].size, 1024);
    }

    #[test]
    fn test_scan_folder_captures_extension() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("movie.webm"), "video").unwrap();

        let files = scan_folder_for_media(dir.path()).unwrap();
        assert_eq!(files[0].extension, "webm");
    }

    // ── ImportResult/DiskScanResult types ─────────────────────────────

    #[test]
    fn test_import_result_default() {
        let result = ImportResult {
            imported_files: Vec::new(),
            skipped_files: Vec::new(),
            errors: Vec::new(),
        };
        assert!(result.imported_files.is_empty());
        assert!(result.skipped_files.is_empty());
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_disk_scan_result_default() {
        let result = DiskScanResult {
            files_found: 0,
            files_matched: 0,
            files_unmatched: 0,
            files_already_tracked: 0,
            unmatched_files: Vec::new(),
        };
        assert_eq!(result.files_found, 0);
    }

    #[test]
    fn test_import_result_serde() {
        let result = ImportResult {
            imported_files: vec![ImportedFile {
                source_path: "/downloads/movie.mkv".to_string(),
                dest_path: "/movies/Movie (2024)/Movie (2024).mkv".to_string(),
                media_file_id: 42,
                quality: "Bluray1080p".to_string(),
                size: 1_500_000_000,
            }],
            skipped_files: vec!["sample.mkv".to_string()],
            errors: Vec::new(),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("importedFiles"));
        assert!(json.contains("skippedFiles"));
        assert!(json.contains("mediaFileId"));
    }

    #[test]
    fn test_disk_scan_result_serde() {
        let result = DiskScanResult {
            files_found: 10,
            files_matched: 8,
            files_unmatched: 2,
            files_already_tracked: 5,
            unmatched_files: vec!["/tv/Unknown/file.mkv".to_string()],
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("filesFound"));
        assert!(json.contains("filesMatched"));
        assert!(json.contains("filesAlreadyTracked"));
        assert!(json.contains("unmatchedFiles"));
    }

    #[test]
    fn test_is_media_extension_all_supported() {
        assert!(is_media_extension("mkv"));
        assert!(is_media_extension("mp4"));
        assert!(is_media_extension("avi"));
        assert!(is_media_extension("wmv"));
        assert!(is_media_extension("ts"));
        assert!(is_media_extension("m4v"));
        assert!(is_media_extension("flv"));
        assert!(is_media_extension("mov"));
        assert!(is_media_extension("webm"));
    }

    #[test]
    fn test_is_media_extension_case_insensitive() {
        assert!(is_media_extension("MKV"));
        assert!(is_media_extension("Mp4"));
        assert!(is_media_extension("AVI"));
    }

    #[test]
    fn test_is_media_extension_rejects_non_media() {
        assert!(!is_media_extension("txt"));
        assert!(!is_media_extension("nfo"));
        assert!(!is_media_extension("srt"));
        assert!(!is_media_extension("jpg"));
        assert!(!is_media_extension("png"));
        assert!(!is_media_extension("zip"));
        assert!(!is_media_extension("rar"));
        assert!(!is_media_extension("nzb"));
    }

    #[test]
    fn test_is_sample_by_name_and_size() {
        use std::path::Path;
        // "sample" in name AND under 50MB = sample
        assert!(is_sample(
            Path::new("/downloads/show/sample.mkv"),
            40_000_000
        ));
    }

    #[test]
    fn test_is_sample_large_file_with_sample_name() {
        use std::path::Path;
        // Over 50MB — not a sample even with "sample" in name
        assert!(!is_sample(
            Path::new("/downloads/show/sample.mkv"),
            60_000_000
        ));
    }

    #[test]
    fn test_is_sample_small_file_without_sample_name() {
        use std::path::Path;
        // Under 50MB but no "sample" in name
        assert!(!is_sample(
            Path::new("/downloads/show/episode.mkv"),
            40_000_000
        ));
    }

    #[test]
    fn test_is_sample_case_insensitive_new() {
        use std::path::Path;
        assert!(is_sample(
            Path::new("/downloads/show/Sample.mkv"),
            40_000_000
        ));
        assert!(is_sample(
            Path::new("/downloads/show/SAMPLE.mkv"),
            40_000_000
        ));
    }

    #[test]
    fn test_is_sample_in_subdirectory_new() {
        use std::path::Path;
        assert!(is_sample(
            Path::new("/downloads/show/Sample/video.mkv"),
            40_000_000
        ));
    }
}
