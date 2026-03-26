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
struct RootFolderRow {
    id: i32,
    path: String,
    media_type: String,
    free_space: Option<i64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RootFolderResponse {
    id: i32,
    path: String,
    media_type: String,
    free_space: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateRootFolderRequest {
    path: String,
    media_type: String,
}

/// Get free space for a path using statvfs on unix, or return None.
fn get_free_space(path: &str) -> Option<i64> {
    let metadata = std::fs::metadata(path).ok()?;
    if !metadata.is_dir() {
        return None;
    }

    #[cfg(unix)]
    {
        use std::ffi::CString;
        use std::mem::MaybeUninit;

        let c_path = CString::new(path).ok()?;
        unsafe {
            let mut stat = MaybeUninit::<libc::statvfs>::uninit();
            if libc::statvfs(c_path.as_ptr(), stat.as_mut_ptr()) == 0 {
                let stat = stat.assume_init();
                let free = stat.f_bavail as i64 * stat.f_frsize as i64;
                Some(free)
            } else {
                None
            }
        }
    }

    #[cfg(not(unix))]
    {
        None
    }
}

async fn list_root_folders(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let pool = state.db.pool();

    match sqlx::query_as::<_, RootFolderRow>(
        "SELECT id, path, media_type, free_space FROM root_folders ORDER BY id",
    )
    .fetch_all(pool)
    .await
    {
        Ok(rows) => {
            let folders: Vec<RootFolderResponse> = rows
                .into_iter()
                .map(|row| {
                    let free_space = get_free_space(&row.path).or(row.free_space);
                    RootFolderResponse {
                        id: row.id,
                        path: row.path,
                        media_type: row.media_type,
                        free_space,
                    }
                })
                .collect();
            Json(folders).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("database error: {e}")})),
        )
            .into_response(),
    }
}

async fn create_root_folder(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateRootFolderRequest>,
) -> impl IntoResponse {
    // Validate path exists on disk
    let path_meta = std::fs::metadata(&body.path);
    if path_meta.is_err() || !path_meta.unwrap().is_dir() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("path '{}' does not exist or is not a directory", body.path)})),
        )
            .into_response();
    }

    // Validate media_type
    if body.media_type != "series" && body.media_type != "movie" {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "media_type must be 'series' or 'movie'"})),
        )
            .into_response();
    }

    let pool = state.db.pool();
    let free_space = get_free_space(&body.path);

    match sqlx::query_as::<_, RootFolderRow>(
        "INSERT INTO root_folders (path, media_type, free_space, last_checked)
         VALUES ($1, $2, $3, NOW())
         RETURNING id, path, media_type, free_space",
    )
    .bind(&body.path)
    .bind(&body.media_type)
    .bind(free_space)
    .fetch_one(pool)
    .await
    {
        Ok(row) => (
            StatusCode::CREATED,
            Json(json!(RootFolderResponse {
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
                    Json(json!({"error": "root folder path already exists"})),
                )
                    .into_response()
            } else {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("database error: {e}")})),
                )
                    .into_response()
            }
        }
    }
}

async fn delete_root_folder(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    match sqlx::query("DELETE FROM root_folders WHERE id = $1")
        .bind(id as i32)
        .execute(pool)
        .await
    {
        Ok(result) => {
            if result.rows_affected() == 0 {
                (
                    StatusCode::NOT_FOUND,
                    Json(json!({"error": "root folder not found"})),
                )
                    .into_response()
            } else {
                StatusCode::NO_CONTENT.into_response()
            }
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("database error: {e}")})),
        )
            .into_response(),
    }
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/v1/rootfolder",
            get(list_root_folders).post(create_root_folder),
        )
        .route("/api/v1/rootfolder/{id}", axum::routing::delete(delete_root_folder))
}
