use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::AppState;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BrowseRoot {
    name: String,
    path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BrowseEntry {
    name: String,
    path: String,
    is_dir: bool,
    size: u64,
    #[serde(with = "chrono::serde::ts_milliseconds_option")]
    modified: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Deserialize)]
struct BrowseQuery {
    path: Option<String>,
    /// "media" = unrestricted browsing for media folder selection
    mode: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteRequest {
    path: String,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Collect filesystem root directories for media folder browsing.
/// Shows mount points and common media paths for unrestricted selection.
fn media_roots() -> Vec<(String, PathBuf)> {
    let mut roots = Vec::new();
    // Show actual filesystem mount points
    for dir in &["/", "/media", "/mnt", "/data", "/config", "/storage"] {
        let p = PathBuf::from(dir);
        if p.exists() {
            roots.push((dir.to_string(), p));
        }
    }
    roots
}

/// Collect allowed root directories from the live engines.
///
/// Queries the running usenet and torrent engines for their current
/// directories rather than reading from the TOML config snapshot, so
/// runtime changes (e.g. torrent settings API) are reflected.
fn allowed_roots(state: &AppState) -> Vec<(String, PathBuf)> {
    let mut roots = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Usenet — directories are fixed at QueueManager construction
    if let Some(qm) = state.usenet_queue.load_full() {
        let incomplete = qm.incomplete_dir().to_path_buf();
        let complete = qm.complete_dir().to_path_buf();
        if seen.insert(incomplete.clone()) {
            roots.push(("Usenet / Incomplete".into(), incomplete));
        }
        if seen.insert(complete.clone()) {
            roots.push(("Usenet / Complete".into(), complete));
        }
    }

    // Torrent — directories can be changed at runtime via settings API
    if let Some(api) = state.torrent_api.load_full() {
        let download = PathBuf::from(api.api_output_folder());
        if seen.insert(download.clone()) {
            roots.push(("Torrent / Download".into(), download));
        }
        if let Some(completed) = api.api_completed_folder() {
            let completed = PathBuf::from(completed);
            if seen.insert(completed.clone()) {
                roots.push(("Torrent / Complete".into(), completed));
            }
        }
    }

    roots
}

/// Verify that `requested` is inside one of the allowed roots.
/// Returns the canonicalized path on success.
async fn validate_path(requested: &str, roots: &[(String, PathBuf)]) -> Result<PathBuf, StatusCode> {
    let requested = PathBuf::from(requested);

    // Canonicalize if it exists; otherwise reject
    let canonical = tokio::fs::canonicalize(&requested)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    for (_label, root) in roots {
        if let Ok(root_canon) = tokio::fs::canonicalize(root).await {
            if canonical.starts_with(&root_canon) {
                return Ok(canonical);
            }
        }
    }

    // Path is outside all allowed roots
    Err(StatusCode::FORBIDDEN)
}

/// Calculate the total size of a directory recursively (blocking — call via spawn_blocking).
fn dir_size(path: &Path) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_dir() {
                    total += dir_size(&entry.path());
                } else {
                    total += meta.len();
                }
            }
        }
    }
    total
}

/// Read a directory listing on a blocking thread to avoid stalling the async runtime.
fn read_dir_blocking(path: &Path) -> std::io::Result<Vec<BrowseEntry>> {
    let mut items = Vec::new();
    for entry in std::fs::read_dir(path)?.flatten() {
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let modified = meta
            .modified()
            .ok()
            .map(chrono::DateTime::<chrono::Utc>::from);
        let size = if meta.is_dir() {
            dir_size(&entry.path())
        } else {
            meta.len()
        };
        items.push(BrowseEntry {
            name: entry.file_name().to_string_lossy().into_owned(),
            path: entry.path().to_string_lossy().into_owned(),
            is_dir: meta.is_dir(),
            size,
            modified,
        });
    }
    Ok(items)
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// GET /api/v1/filebrowser/roots — list allowed root directories
async fn list_roots(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let roots: Vec<BrowseRoot> = allowed_roots(&state)
        .into_iter()
        .filter(|(_, p)| p.exists())
        .map(|(name, path)| BrowseRoot {
            name,
            path: path.to_string_lossy().into_owned(),
        })
        .collect();

    Json(roots)
}

/// GET /api/v1/filebrowser/browse?path=... — list contents of a directory
async fn browse(
    State(state): State<Arc<AppState>>,
    Query(q): Query<BrowseQuery>,
) -> impl IntoResponse {
    let is_media_mode = q.mode.as_deref() == Some("media");
    let roots = if is_media_mode {
        media_roots()
    } else {
        allowed_roots(&state)
    };

    let path = match q.path {
        Some(ref p) if !p.is_empty() => match validate_path(p, &roots).await {
            Ok(p) => p,
            Err(status) => {
                let msg = if status == StatusCode::FORBIDDEN {
                    "Path is outside allowed directories"
                } else {
                    "Path not found"
                };
                return (status, Json(serde_json::json!({ "error": msg }))).into_response();
            }
        },
        _ => {
            // No path specified — return roots as top-level directories
            let entries: Vec<BrowseEntry> = roots
                .into_iter()
                .filter(|(_, p)| p.exists())
                .map(|(name, path)| BrowseEntry {
                    name,
                    path: path.to_string_lossy().into_owned(),
                    is_dir: true,
                    size: 0,
                    modified: None,
                })
                .collect();
            return Json(serde_json::json!({
                "path": "/",
                "entries": entries,
                "parent": serde_json::Value::Null,
            }))
            .into_response();
        }
    };

    if !path.is_dir() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Path is not a directory" })),
        )
            .into_response();
    }

    let browse_path = path.clone();
    let mut items = match tokio::task::spawn_blocking(move || read_dir_blocking(&browse_path)).await {
        Ok(Ok(items)) => items,
        Ok(Err(e)) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": format!("Failed to read directory: {e}") })),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": format!("Failed to read directory: {e}") })),
            )
                .into_response();
        }
    };

    // Sort: directories first, then alphabetically
    items.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));

    // Compute parent path — only if it's still within an allowed root
    let parent = if let Some(p) = path.parent() {
        let mut found = None;
        for (_label, root) in &roots {
            if let Ok(root_canon) = tokio::fs::canonicalize(root).await {
                if p.starts_with(&root_canon) && p != root_canon {
                    found = Some(p.to_string_lossy().into_owned());
                    break;
                }
            }
        }
        found
    } else {
        None
    };

    Json(serde_json::json!({
        "path": path.to_string_lossy(),
        "entries": items,
        "parent": parent,
    }))
    .into_response()
}

/// POST /api/v1/filebrowser/delete — delete a file or directory
async fn delete_entry(
    State(state): State<Arc<AppState>>,
    Json(req): Json<DeleteRequest>,
) -> impl IntoResponse {
    let roots = allowed_roots(&state);

    let path = match validate_path(&req.path, &roots).await {
        Ok(p) => p,
        Err(status) => {
            let msg = if status == StatusCode::FORBIDDEN {
                "Path is outside allowed directories"
            } else {
                "Path not found"
            };
            return (status, Json(serde_json::json!({ "error": msg }))).into_response();
        }
    };

    // Don't allow deleting root directories themselves
    for (_label, root) in &roots {
        if let Ok(root_canon) = tokio::fs::canonicalize(root).await {
            if path == root_canon {
                return (
                    StatusCode::FORBIDDEN,
                    Json(serde_json::json!({ "error": "Cannot delete a root directory" })),
                )
                    .into_response();
            }
        }
    }

    info!(path = %path.display(), "File browser: deleting");

    let delete_path = path.clone();
    let result = tokio::task::spawn_blocking(move || {
        if delete_path.is_dir() {
            std::fs::remove_dir_all(&delete_path)
        } else {
            std::fs::remove_file(&delete_path)
        }
    })
    .await;

    match result {
        Ok(Ok(())) => {
            info!(path = %path.display(), "File browser: deleted successfully");
            Json(serde_json::json!({ "success": true })).into_response()
        }
        Ok(Err(e)) => {
            warn!(path = %path.display(), error = %e, "File browser: delete failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": format!("Delete failed: {e}") })),
            )
                .into_response()
        }
        Err(e) => {
            warn!(path = %path.display(), error = %e, "File browser: delete task failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": format!("Delete failed: {e}") })),
            )
                .into_response()
        }
    }
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/filebrowser/roots", get(list_roots))
        .route("/api/v1/filebrowser/browse", get(browse))
        .route("/api/v1/filebrowser/delete", post(delete_entry))
}
