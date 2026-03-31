use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::AppState;

#[derive(Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
struct MediaLibraryFolderRow {
    id: i32,
    path: String,
    media_type: String,
    free_space: Option<i64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MediaLibraryFolderResponse {
    id: i32,
    path: String,
    media_type: String,
    free_space: Option<i64>,
    total_space: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateMediaLibraryFolderRequest {
    path: String,
    media_type: String,
}

/// Get free and total space for a path by parsing `df` output.
fn get_disk_space(path: &str) -> (Option<i64>, Option<i64>) {
    let metadata = match std::fs::metadata(path) {
        Ok(m) if m.is_dir() => m,
        _ => return (None, None),
    };
    let _ = metadata;

    // Use `df --output=avail,size -B1 <path>` which outputs available and total bytes
    let output = match std::process::Command::new("df")
        .args(["--output=avail,size", "-B1", path])
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => return (None, None),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = match stdout.lines().nth(1) {
        Some(l) => l,
        None => return (None, None),
    };

    let parts: Vec<&str> = line.split_whitespace().collect();
    let avail = parts.first().and_then(|s| s.parse::<i64>().ok());
    let total = parts.get(1).and_then(|s| s.parse::<i64>().ok());
    (avail, total)
}

async fn list_media_library_folders(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let pool = state.db.pool();

    match sqlx::query_as::<_, MediaLibraryFolderRow>(
        "SELECT id, path, media_type, free_space FROM media_library_folders ORDER BY id",
    )
    .fetch_all(pool)
    .await
    {
        Ok(rows) => {
            let folders = tokio::task::spawn_blocking(move || {
                rows.into_iter()
                    .map(|row| {
                        let (avail, total) = get_disk_space(&row.path);
                        MediaLibraryFolderResponse {
                            id: row.id,
                            path: row.path,
                            media_type: row.media_type,
                            free_space: avail.or(row.free_space),
                            total_space: total,
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .await
            .unwrap_or_default();
            Json(folders).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to list media library folders");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

async fn create_media_library_folder(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateMediaLibraryFolderRequest>,
) -> impl IntoResponse {
    // Canonicalize and validate path to prevent traversal attacks
    let canonical = match tokio::fs::canonicalize(&body.path).await {
        Ok(p) if p.is_dir() => p,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "path does not exist or is not a directory"})),
            )
                .into_response();
        }
    };
    let canonical_str = canonical.to_string_lossy().to_string();

    // Validate media_type
    if body.media_type != "series" && body.media_type != "movie" {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "media_type must be 'series' or 'movie'"})),
        )
            .into_response();
    }

    let pool = state.db.pool();
    let disk_path = canonical_str.clone();
    let (free_space, total_space) = tokio::task::spawn_blocking(move || get_disk_space(&disk_path))
        .await
        .unwrap_or((None, None));

    match sqlx::query_as::<_, MediaLibraryFolderRow>(
        "INSERT INTO media_library_folders (path, media_type, free_space, last_checked)
         VALUES ($1, $2, $3, NOW())
         RETURNING id, path, media_type, free_space",
    )
    .bind(&canonical_str)
    .bind(&body.media_type)
    .bind(free_space)
    .fetch_one(pool)
    .await
    {
        Ok(row) => (
            StatusCode::CREATED,
            Json(json!(MediaLibraryFolderResponse {
                id: row.id,
                path: row.path,
                media_type: row.media_type,
                free_space: row.free_space,
                total_space,
            })),
        )
            .into_response(),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("duplicate key") || msg.contains("unique") {
                (
                    StatusCode::CONFLICT,
                    Json(json!({"error": "media library folder path already exists"})),
                )
                    .into_response()
            } else {
                tracing::error!(error = %e, "failed to create media library folder");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "failed to create media library folder"})),
                )
                    .into_response()
            }
        }
    }
}

async fn delete_media_library_folder(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let pool = state.db.pool();
    let id_i32 = id as i32;

    // Unlink any series, movies, and import lists referencing this folder
    let _ = sqlx::query("UPDATE series SET media_library_folder_id = NULL WHERE media_library_folder_id = $1")
        .bind(id_i32)
        .execute(pool)
        .await;
    let _ = sqlx::query("UPDATE movies SET media_library_folder_id = NULL WHERE media_library_folder_id = $1")
        .bind(id_i32)
        .execute(pool)
        .await;
    let _ = sqlx::query("UPDATE import_lists SET media_library_folder_id = NULL WHERE media_library_folder_id = $1")
        .bind(id_i32)
        .execute(pool)
        .await;

    match sqlx::query("DELETE FROM media_library_folders WHERE id = $1")
        .bind(id_i32)
        .execute(pool)
        .await
    {
        Ok(result) => {
            if result.rows_affected() == 0 {
                (
                    StatusCode::NOT_FOUND,
                    Json(json!({"error": "media library folder not found"})),
                )
                    .into_response()
            } else {
                StatusCode::NO_CONTENT.into_response()
            }
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to delete media library folder");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response()
        }
    }
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/v1/medialibraryfolder",
            get(list_media_library_folders).post(create_media_library_folder),
        )
        .route("/api/v1/medialibraryfolder/{id}", axum::routing::delete(delete_media_library_folder))
}
