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
pub struct AnalyzeResponse {
    pub path: String,
    pub folder_name: String,
    pub files: Vec<AnalyzedFile>,
    pub parsed_title: Option<String>,
    pub parsed_season: Option<i32>,
    pub parsed_episodes: Vec<i32>,
    pub parsed_quality: String,
    pub series_matches: Vec<LibrarySeriesMatch>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualImportBody {
    /// Path to the file or folder to import.
    pub path: String,
    /// Series to import into.
    pub series_id: i64,
    /// Specific episode to target. When omitted the episode is inferred from
    /// the parsed filename (season + episode numbers).
    pub episode_id: Option<i64>,
}

// ── Handlers ────────────────────────────────────────────────────────────────

pub async fn analyze(
    State(state): State<Arc<AppState>>,
    Json(body): Json<AnalyzeRequest>,
) -> impl IntoResponse {
    let path = PathBuf::from(&body.path);

    // Parse release name from the leaf folder / filename
    let name_to_parse = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(body.path.as_str());
    let parsed = stackarr_parser::parse_release(name_to_parse);
    let quality_str = format!("{:?}", parsed.quality.quality);

    // Collect video files under the path
    let files = find_video_files(&path).await;

    // Search DB for series whose clean_title contains the parsed title
    let clean = stackarr_parser::clean_title(&parsed.title);
    let pool = state.db.pool();

    let rows: Vec<(i64, String, Option<i32>, Option<serde_json::Value>)> = sqlx::query_as(
        "SELECT id, title, year, images FROM series \
         WHERE clean_title ILIKE '%' || $1 || '%' \
         ORDER BY \
           CASE WHEN clean_title = $1 THEN 0 ELSE 1 END, \
           title \
         LIMIT 10",
    )
    .bind(&clean)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let series_matches = rows
        .into_iter()
        .map(|(id, title, year, images)| {
            let poster_url = super::extract_image_url(&images, "poster");
            LibrarySeriesMatch { id, title, year, poster_url }
        })
        .collect();

    let folder_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();

    Json(AnalyzeResponse {
        path: body.path,
        folder_name,
        files,
        parsed_title: if parsed.title.is_empty() { None } else { Some(parsed.title) },
        parsed_season: parsed.episode_info.season_number,
        parsed_episodes: parsed.episode_info.episode_numbers,
        parsed_quality: quality_str,
        series_matches,
    })
    .into_response()
}

pub async fn import(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ManualImportBody>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    // Verify series exists
    let exists: Option<(i64,)> = sqlx::query_as("SELECT id FROM series WHERE id = $1")
        .bind(body.series_id)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);
    if exists.is_none() {
        return super::api_error(
            StatusCode::NOT_FOUND,
            format!("series {} not found", body.series_id),
        );
    }

    let ffprobe_path = Some(state.config.load().streaming.ffprobe_path.clone());

    let ctx = stackarr_import::ImportContext {
        pool: pool.clone(),
        download_id: format!("manual-{}", chrono::Utc::now().timestamp_millis()),
        output_path: PathBuf::from(&body.path),
        media_type: "series".to_string(),
        media_id: body.series_id,
        episode_id: body.episode_id,
        ffprobe_path,
    };

    match stackarr_import::process_completed_download(ctx).await {
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

// ── Helpers ─────────────────────────────────────────────────────────────────

const VIDEO_EXTENSIONS: &[&str] = &["mkv", "mp4", "avi", "ts", "m4v", "wmv", "mov", "webm", "flv"];

async fn find_video_files(root: &std::path::Path) -> Vec<AnalyzedFile> {
    let mut files = Vec::new();

    if root.is_file() {
        if is_video(root) {
            let size = tokio::fs::metadata(root).await.map(|m| m.len() as i64).unwrap_or(0);
            files.push(AnalyzedFile {
                name: root.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string(),
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
                    name: p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string(),
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
        .route("/api/v1/manual-import/import", post(import))
}
