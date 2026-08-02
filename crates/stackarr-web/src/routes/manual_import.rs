use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::AppState;

// ── Request / response types ────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzeRequest {
    pub path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkAnalyzeRequest {
    pub paths: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkAnalyzeResponse {
    pub results: Vec<AnalyzeResponse>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzedFile {
    pub name: String,
    pub path: String,
    pub size: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySeriesMatch {
    pub id: i64,
    pub title: String,
    pub year: Option<i32>,
    pub poster_url: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryMovieMatch {
    pub id: i64,
    pub title: String,
    pub year: Option<i32>,
    pub poster_url: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoMatch {
    /// "series" or "movie"
    pub media_type: String,
    pub media_id: i64,
    pub media_title: String,
    pub media_year: Option<i32>,
    /// For series: which episode in the DB this resolved to, if any.
    pub episode_id: Option<i64>,
    pub episode_label: Option<String>,
    /// "high", "medium", "low".
    pub confidence: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzeResponse {
    pub path: String,
    pub folder_name: String,
    pub files: Vec<AnalyzedFile>,
    pub parsed_title: Option<String>,
    pub parsed_year: Option<i32>,
    pub parsed_season: Option<i32>,
    pub parsed_episodes: Vec<i32>,
    pub parsed_quality: String,
    /// Heuristic: "series" if S/E parsed, "movie" otherwise.
    pub suggested_media_type: String,
    pub series_matches: Vec<LibrarySeriesMatch>,
    pub movie_matches: Vec<LibraryMovieMatch>,
    pub auto_match: Option<AutoMatch>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualImportBody {
    /// Path to the file or folder to import.
    pub path: String,
    /// "series" (default for backwards-compat) or "movie".
    #[serde(default)]
    pub media_type: Option<String>,
    /// Series to import into (when media_type is "series").
    #[serde(default)]
    pub series_id: Option<i64>,
    /// Movie to import into (when media_type is "movie").
    #[serde(default)]
    pub movie_id: Option<i64>,
    /// Specific episode to target. When omitted the episode is inferred from
    /// the parsed filename (season + episode numbers).
    pub episode_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkImportItem {
    pub path: String,
    /// "series" or "movie".
    pub media_type: String,
    /// Series id when media_type is "series", movie id when "movie".
    pub media_id: i64,
    pub episode_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkImportRequest {
    pub items: Vec<BulkImportItem>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkImportItemResult {
    pub path: String,
    pub ok: bool,
    pub imported: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
    /// Top-level failure (e.g. media_id not found, IO error) distinct from per-file errors.
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkImportResponse {
    pub results: Vec<BulkImportItemResult>,
    pub total_imported: usize,
    pub total_failed: usize,
}

// ── Handlers ────────────────────────────────────────────────────────────────

pub async fn analyze(
    State(state): State<Arc<AppState>>,
    Json(body): Json<AnalyzeRequest>,
) -> impl IntoResponse {
    let result = analyze_path(&state, &body.path).await;
    Json(result).into_response()
}

pub async fn bulk_analyze(
    State(state): State<Arc<AppState>>,
    Json(body): Json<BulkAnalyzeRequest>,
) -> impl IntoResponse {
    let mut results = Vec::with_capacity(body.paths.len());
    for p in &body.paths {
        results.push(analyze_path(&state, p).await);
    }
    Json(BulkAnalyzeResponse { results }).into_response()
}

pub async fn import(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ManualImportBody>,
) -> impl IntoResponse {
    let media_type = body
        .media_type
        .as_deref()
        .unwrap_or("series")
        .to_ascii_lowercase();

    let media_id = match media_type.as_str() {
        "series" => match body.series_id {
            Some(id) => id,
            None => {
                return super::api_error(
                    StatusCode::BAD_REQUEST,
                    "seriesId required when mediaType is 'series'",
                );
            }
        },
        "movie" => match body.movie_id {
            Some(id) => id,
            None => {
                return super::api_error(
                    StatusCode::BAD_REQUEST,
                    "movieId required when mediaType is 'movie'",
                );
            }
        },
        other => {
            return super::api_error(
                StatusCode::BAD_REQUEST,
                format!("unsupported mediaType '{other}'"),
            );
        }
    };

    match run_manual_import(&state, &body.path, &media_type, media_id, body.episode_id).await {
        Ok(result) => Json(serde_json::json!({
            "imported": result.imported_files.len(),
            "skipped": result.skipped_files.len(),
            "errors": result.errors,
            "logLines": result.log_lines,
            "files": result.imported_files,
        }))
        .into_response(),
        Err(e) => {
            tracing::error!(error = %e, path = %body.path, "manual import failed");
            super::api_error(StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}"))
        }
    }
}

pub async fn bulk_import(
    State(state): State<Arc<AppState>>,
    Json(body): Json<BulkImportRequest>,
) -> impl IntoResponse {
    let mut results = Vec::with_capacity(body.items.len());
    let mut total_imported = 0usize;
    let mut total_failed = 0usize;

    for item in body.items {
        let media_type = item.media_type.to_ascii_lowercase();
        if media_type != "series" && media_type != "movie" {
            total_failed += 1;
            results.push(BulkImportItemResult {
                path: item.path,
                ok: false,
                imported: 0,
                skipped: 0,
                errors: Vec::new(),
                error: Some(format!("unsupported mediaType '{}'", item.media_type)),
            });
            continue;
        }

        match run_manual_import(
            &state,
            &item.path,
            &media_type,
            item.media_id,
            item.episode_id,
        )
        .await
        {
            Ok(result) => {
                let imported = result.imported_files.len();
                let ok = !result.imported_files.is_empty() && result.errors.is_empty();
                total_imported += imported;
                if !ok {
                    total_failed += 1;
                }
                results.push(BulkImportItemResult {
                    path: item.path,
                    ok,
                    imported,
                    skipped: result.skipped_files.len(),
                    errors: result.errors,
                    error: None,
                });
            }
            Err(e) => {
                tracing::error!(error = %e, path = %item.path, "bulk import item failed");
                total_failed += 1;
                results.push(BulkImportItemResult {
                    path: item.path,
                    ok: false,
                    imported: 0,
                    skipped: 0,
                    errors: Vec::new(),
                    error: Some(format!("{e:#}")),
                });
            }
        }
    }

    Json(BulkImportResponse {
        results,
        total_imported,
        total_failed,
    })
    .into_response()
}

// ── Shared import logic ─────────────────────────────────────────────────────

async fn run_manual_import(
    state: &Arc<AppState>,
    path: &str,
    media_type: &str,
    media_id: i64,
    episode_id: Option<i64>,
) -> anyhow::Result<stackarr_import::ImportResult> {
    let pool = state.db.pool();

    // Verify target exists
    let exists: Option<(i64,)> = match media_type {
        "series" => {
            sqlx::query_as("SELECT id FROM series WHERE id = ?")
                .bind(media_id)
                .fetch_optional(pool)
                .await?
        }
        "movie" => {
            sqlx::query_as("SELECT id FROM movies WHERE id = ?")
                .bind(media_id)
                .fetch_optional(pool)
                .await?
        }
        _ => anyhow::bail!("unsupported media_type '{media_type}'"),
    };
    if exists.is_none() {
        anyhow::bail!("{media_type} {media_id} not found");
    }

    let ffprobe_path = Some(state.config.load().streaming.ffprobe_path.clone());

    let ctx = stackarr_import::ImportContext {
        pool: pool.clone(),
        download_id: format!("manual-{}", chrono::Utc::now().timestamp_millis()),
        output_path: PathBuf::from(path),
        media_type: media_type.to_string(),
        media_id,
        episode_id,
        ffprobe_path,
    };

    stackarr_import::process_completed_download(ctx).await
}

// ── Analysis helpers ────────────────────────────────────────────────────────

async fn analyze_path(state: &Arc<AppState>, raw_path: &str) -> AnalyzeResponse {
    let path = PathBuf::from(raw_path);
    let pool = state.db.pool();

    // Parse release name from the leaf folder / filename
    let name_to_parse = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(raw_path);
    let parsed = stackarr_parser::parse_release(name_to_parse);
    let quality_str = format!("{:?}", parsed.quality.quality);

    let files = find_video_files(&path).await;

    let clean = stackarr_parser::clean_title(&parsed.title);
    let has_episode_info = parsed.episode_info.season_number.is_some()
        && !parsed.episode_info.episode_numbers.is_empty();
    let suggested_media_type = if has_episode_info { "series" } else { "movie" };

    // Series matches — always look (e.g. may be an anime with absolute numbering
    // where season wasn't parsed cleanly, user can still pick).
    let series_rows: Vec<(i64, String, Option<i32>, Option<serde_json::Value>)> =
        if clean.is_empty() {
            Vec::new()
        } else {
            sqlx::query_as(
                "SELECT id, title, year, images FROM series \
             WHERE clean_title LIKE CONCAT('%', ?, '%') \
             ORDER BY \
               CASE WHEN clean_title = ? THEN 0 ELSE 1 END, \
               title \
             LIMIT 10",
            )
            .bind(&clean)
            .bind(&clean)
            .fetch_all(pool)
            .await
            .unwrap_or_default()
        };

    let series_matches: Vec<LibrarySeriesMatch> = series_rows
        .iter()
        .map(|(id, title, year, images)| LibrarySeriesMatch {
            id: *id,
            title: title.clone(),
            year: *year,
            poster_url: super::extract_image_url(images, "poster"),
        })
        .collect();

    // Movie matches.
    let movie_rows: Vec<(i64, String, Option<i32>, Option<serde_json::Value>)> = if clean.is_empty()
    {
        Vec::new()
    } else {
        sqlx::query_as(
            "SELECT id, title, year, images FROM movies \
             WHERE clean_title LIKE CONCAT('%', ?, '%') \
             ORDER BY \
               CASE WHEN clean_title = ? THEN 0 ELSE 1 END, \
               CASE WHEN year = ? THEN 0 ELSE 1 END, \
               title \
             LIMIT 10",
        )
        .bind(&clean)
        .bind(&clean)
        .bind(parsed.year)
        .fetch_all(pool)
        .await
        .unwrap_or_default()
    };

    let movie_matches: Vec<LibraryMovieMatch> = movie_rows
        .iter()
        .map(|(id, title, year, images)| LibraryMovieMatch {
            id: *id,
            title: title.clone(),
            year: *year,
            poster_url: super::extract_image_url(images, "poster"),
        })
        .collect();

    // Auto-match: pick the best candidate given the suggested media type.
    let auto_match = build_auto_match(
        pool,
        &clean,
        parsed.year,
        suggested_media_type,
        &series_rows,
        &movie_rows,
        parsed.episode_info.season_number,
        parsed.episode_info.episode_numbers.first().copied(),
    )
    .await;

    let folder_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();

    AnalyzeResponse {
        path: raw_path.to_string(),
        folder_name,
        files,
        parsed_title: if parsed.title.is_empty() {
            None
        } else {
            Some(parsed.title)
        },
        parsed_year: parsed.year,
        parsed_season: parsed.episode_info.season_number,
        parsed_episodes: parsed.episode_info.episode_numbers,
        parsed_quality: quality_str,
        suggested_media_type: suggested_media_type.to_string(),
        series_matches,
        movie_matches,
        auto_match,
    }
}

#[allow(clippy::too_many_arguments)]
async fn build_auto_match(
    pool: &sqlx::MySqlPool,
    clean_title: &str,
    parsed_year: Option<i32>,
    suggested_media_type: &str,
    series_rows: &[(i64, String, Option<i32>, Option<serde_json::Value>)],
    movie_rows: &[(i64, String, Option<i32>, Option<serde_json::Value>)],
    season: Option<i32>,
    episode_number: Option<i32>,
) -> Option<AutoMatch> {
    if clean_title.is_empty() {
        return None;
    }

    if suggested_media_type == "series" {
        let (id, title, year, _) = series_rows.first()?;
        let clean_match = stackarr_parser::clean_title(title) == clean_title;
        let mut confidence = if clean_match { "high" } else { "medium" };

        // Resolve the episode row (if season + episode parsed)
        let (episode_id, episode_label) = match (season, episode_number) {
            (Some(s), Some(e)) => {
                let row: Option<(i64, Option<String>)> = sqlx::query_as(
                    "SELECT id, title FROM episodes \
                     WHERE series_id = ? AND season_number = ? AND episode_number = ?",
                )
                .bind(id)
                .bind(s)
                .bind(e)
                .fetch_optional(pool)
                .await
                .unwrap_or(None);
                match row {
                    Some((eid, etitle)) => {
                        let label = format!(
                            "S{:02}E{:02}{}",
                            s,
                            e,
                            etitle
                                .as_deref()
                                .map(|t| format!(" — {t}"))
                                .unwrap_or_default()
                        );
                        (Some(eid), Some(label))
                    }
                    None => {
                        // Series matched but episode not found — drop confidence one tier.
                        confidence = if confidence == "high" {
                            "medium"
                        } else {
                            "low"
                        };
                        (None, None)
                    }
                }
            }
            _ => {
                // No season/episode info — full-series or unknown. Lower confidence.
                confidence = if confidence == "high" {
                    "medium"
                } else {
                    "low"
                };
                (None, None)
            }
        };

        Some(AutoMatch {
            media_type: "series".into(),
            media_id: *id,
            media_title: title.clone(),
            media_year: *year,
            episode_id,
            episode_label,
            confidence: confidence.into(),
        })
    } else {
        // movie
        let (id, title, year, _) = movie_rows.first()?;
        let clean_match = stackarr_parser::clean_title(title) == clean_title;
        let year_match = match (parsed_year, year) {
            (Some(a), Some(b)) => a == *b,
            _ => false,
        };
        let confidence = match (clean_match, year_match) {
            (true, true) => "high",
            (true, false) => "medium",
            _ => "low",
        };
        Some(AutoMatch {
            media_type: "movie".into(),
            media_id: *id,
            media_title: title.clone(),
            media_year: *year,
            episode_id: None,
            episode_label: None,
            confidence: confidence.into(),
        })
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

const VIDEO_EXTENSIONS: &[&str] = &[
    "mkv", "mp4", "avi", "ts", "m4v", "wmv", "mov", "webm", "flv",
];

async fn find_video_files(root: &std::path::Path) -> Vec<AnalyzedFile> {
    let mut files = Vec::new();

    if root.is_file() {
        if is_video(root) {
            let size = tokio::fs::metadata(root)
                .await
                .map(|m| m.len() as i64)
                .unwrap_or(0);
            files.push(AnalyzedFile {
                name: root
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string(),
                path: root.display().to_string(),
                size,
            });
        }
        return files;
    }

    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let mut rd = match tokio::fs::read_dir(&dir).await {
            Ok(r) => r,
            Err(_) => continue,
        };
        while let Ok(Some(entry)) = rd.next_entry().await {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else if is_video(&p) {
                let size = entry.metadata().await.map(|m| m.len() as i64).unwrap_or(0);
                files.push(AnalyzedFile {
                    name: p
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_string(),
                    path: p.display().to_string(),
                    size,
                });
            }
        }
    }
    files
}

fn is_video(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| VIDEO_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

// ── Router ───────────────────────────────────────────────────────────────────

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/manual-import/analyze", post(analyze))
        .route("/api/v1/manual-import/bulk-analyze", post(bulk_analyze))
        .route("/api/v1/manual-import/import", post(import))
        .route("/api/v1/manual-import/bulk-import", post(bulk_import))
}
