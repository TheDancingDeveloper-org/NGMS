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
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateMediaLibraryFolderRequest {
    path: String,
    media_type: String,
}

/// Get free space for a path by parsing `df` output, or return None on error.
fn get_free_space(path: &str) -> Option<i64> {
    let metadata = std::fs::metadata(path).ok()?;
    if !metadata.is_dir() {
        return None;
    }

    // Use `df --output=avail -B1 <path>` which outputs available bytes
    let output = std::process::Command::new("df")
        .args(["--output=avail", "-B1", path])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Output is two lines: header + value
    let avail_str = stdout.lines().nth(1)?.trim();
    avail_str.parse::<i64>().ok()
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
            let folders: Vec<MediaLibraryFolderResponse> = rows
                .into_iter()
                .map(|row| {
                    let free_space = get_free_space(&row.path).or(row.free_space);
                    MediaLibraryFolderResponse {
                        id: row.id,
                        path: row.path,
                        media_type: row.media_type,
                        free_space,
                    }
                })
                .collect();
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
    let canonical = match std::fs::canonicalize(&body.path) {
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
    let free_space = get_free_space(&canonical_str);

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

    match sqlx::query("DELETE FROM media_library_folders WHERE id = $1")
        .bind(id as i32)
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
