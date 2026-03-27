use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::Column;

use crate::middleware::RequireApiKey;
use crate::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BackupResponse {
    series: Vec<serde_json::Value>,
    movies: Vec<serde_json::Value>,
    quality_profiles: Vec<serde_json::Value>,
    tags: Vec<serde_json::Value>,
    indexers: Vec<serde_json::Value>,
    download_clients: Vec<serde_json::Value>,
    media_library_folders: Vec<serde_json::Value>,
    naming_config: Vec<serde_json::Value>,
    notification_providers: Vec<serde_json::Value>,
    import_lists: Vec<serde_json::Value>,
    enabled_modules: Vec<serde_json::Value>,
}

/// GET /api/v1/system/backup — export the database as JSON.
async fn export_backup(
    _auth: RequireApiKey,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    macro_rules! fetch_table {
        ($q:expr) => {
            match sqlx::query($q).fetch_all(pool).await {
                Ok(rows) => {
                    use sqlx::Row;
                    rows.iter()
                        .map(|r| {
                            let cols = r.columns();
                            let mut obj = serde_json::Map::new();
                            for col in cols {
                                let name = col.name();
                                // Try common types
                                if let Ok(v) = r.try_get::<serde_json::Value, _>(name) {
                                    obj.insert(name.to_string(), v);
                                } else if let Ok(v) = r.try_get::<String, _>(name) {
                                    obj.insert(name.to_string(), json!(v));
                                } else if let Ok(v) = r.try_get::<i64, _>(name) {
                                    obj.insert(name.to_string(), json!(v));
                                } else if let Ok(v) = r.try_get::<i32, _>(name) {
                                    obj.insert(name.to_string(), json!(v));
                                } else if let Ok(v) = r.try_get::<bool, _>(name) {
                                    obj.insert(name.to_string(), json!(v));
                                } else {
                                    obj.insert(name.to_string(), serde_json::Value::Null);
                                }
                            }
                            serde_json::Value::Object(obj)
                        })
                        .collect::<Vec<_>>()
                }
                Err(e) => {
                    tracing::error!(table = $q, error = %e, "backup: failed to export table");
                    Vec::new()
                }
            }
        };
    }

    let resp = BackupResponse {
        series: fetch_table!("SELECT id, title, path, quality_profile_id, media_library_folder_id, monitored, tvdb_id, imdb_id, tmdb_id FROM series ORDER BY id"),
        movies: fetch_table!("SELECT id, title, path, quality_profile_id, media_library_folder_id, monitored, tmdb_id, imdb_id, minimum_availability FROM movies ORDER BY id"),
        quality_profiles: fetch_table!("SELECT id, name, cutoff, upgrade_allowed, min_format_score, cutoff_format_score, items, media_type FROM quality_profiles ORDER BY id"),
        tags: fetch_table!("SELECT id, label FROM tags ORDER BY id"),
        indexers: fetch_table!("SELECT id, name, indexer_type, base_url, protocol, categories, enabled, priority, supports_search, supports_rss FROM indexers ORDER BY id"),
        download_clients: fetch_table!("SELECT id, name, client_type, protocol, enabled, priority FROM download_clients ORDER BY id"),
        media_library_folders: fetch_table!("SELECT id, path, media_type FROM media_library_folders ORDER BY id"),
        naming_config: fetch_table!("SELECT id, media_type, rename_files, standard_format, daily_format, anime_format, season_folder_format, movie_format, movie_folder_format FROM naming_config ORDER BY id"),
        notification_providers: fetch_table!("SELECT id, name, provider_type, on_grab, on_import, on_upgrade, on_health_issue, on_failure, enabled FROM notification_providers ORDER BY id"),
        import_lists: fetch_table!("SELECT id, name, list_type, media_type, enabled FROM import_lists ORDER BY id"),
        enabled_modules: fetch_table!("SELECT id, module, enabled FROM enabled_modules ORDER BY id"),
    };

    Json(resp).into_response()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct RestoreRequest {
    quality_profiles: Option<Vec<serde_json::Value>>,
    tags: Option<Vec<serde_json::Value>>,
    media_library_folders: Option<Vec<serde_json::Value>>,
    naming_config: Option<Vec<serde_json::Value>>,
    enabled_modules: Option<Vec<serde_json::Value>>,
}

/// POST /api/v1/system/restore — import configuration from a backup JSON.
///
/// Only restores configuration tables (quality profiles, tags, folders, naming, modules).
/// Media (series/movies) must be re-imported via migration or TMDB lookup.
async fn import_restore(
    _auth: RequireApiKey,
    State(state): State<Arc<AppState>>,
    Json(body): Json<RestoreRequest>,
) -> impl IntoResponse {
    let pool = state.db.pool();
    let mut restored = Vec::new();
    let mut errors = Vec::new();

    // Restore tags
    if let Some(tags) = &body.tags {
        let mut count = 0;
        for tag in tags {
            let label = tag.get("label").and_then(|v| v.as_str()).unwrap_or("");
            if label.is_empty() {
                continue;
            }
            match sqlx::query("INSERT INTO tags (label) VALUES ($1) ON CONFLICT (label) DO NOTHING")
                .bind(label)
                .execute(pool)
                .await
            {
                Ok(_) => count += 1,
                Err(e) => {
                    tracing::error!(error = %e, "restore: failed to insert tag");
                    errors.push(format!("tag '{label}': insert failed"));
                }
            }
        }
        restored.push(format!("tags: {count}"));
    }

    // Restore media library folders
    if let Some(folders) = &body.media_library_folders {
        let mut count = 0;
        for folder in folders {
            let path = folder.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let media_type = folder.get("media_type").or_else(|| folder.get("mediaType"))
                .and_then(|v| v.as_str()).unwrap_or("series");
            if path.is_empty() {
                continue;
            }
            match sqlx::query(
                "INSERT INTO media_library_folders (path, media_type) VALUES ($1, $2) ON CONFLICT (path) DO NOTHING"
            )
                .bind(path)
                .bind(media_type)
                .execute(pool)
                .await
            {
                Ok(_) => count += 1,
                Err(e) => {
                    tracing::error!(error = %e, "restore: failed to insert folder");
                    errors.push(format!("folder '{path}': insert failed"));
                }
            }
        }
        restored.push(format!("media_library_folders: {count}"));
    }

    // Restore enabled modules
    if let Some(modules) = &body.enabled_modules {
        let mut count = 0;
        for m in modules {
            let module = m.get("module").and_then(|v| v.as_str()).unwrap_or("");
            let enabled = m.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);
            if module.is_empty() {
                continue;
            }
            match sqlx::query(
                "INSERT INTO enabled_modules (module, enabled) VALUES ($1, $2) ON CONFLICT (module) DO UPDATE SET enabled = $2"
            )
                .bind(module)
                .bind(enabled)
                .execute(pool)
                .await
            {
                Ok(_) => count += 1,
                Err(e) => {
                    tracing::error!(error = %e, "restore: failed to insert module");
                    errors.push(format!("module '{module}': insert failed"));
                }
            }
        }
        restored.push(format!("enabled_modules: {count}"));
    }

    let status = if errors.is_empty() {
        StatusCode::OK
    } else {
        StatusCode::MULTI_STATUS
    };

    (
        status,
        Json(json!({
            "success": errors.is_empty(),
            "restored": restored,
            "errors": errors,
        })),
    )
        .into_response()
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/system/backup", get(export_backup))
        .route("/api/v1/system/restore", post(import_restore))
}
