use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::Column;

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
        indexers: fetch_table!("SELECT id, name, indexer_type, base_url, api_key, protocol, categories, enabled, priority, supports_search, supports_rss, config FROM indexers ORDER BY id"),
        download_clients: fetch_table!("SELECT id, name, client_type, protocol, config, enabled, priority FROM download_clients ORDER BY id"),
        media_library_folders: fetch_table!("SELECT id, path, media_type FROM media_library_folders ORDER BY id"),
        naming_config: fetch_table!("SELECT id, media_type, rename_files, standard_format, daily_format, anime_format, season_folder_format, movie_format, movie_folder_format FROM naming_config ORDER BY id"),
        notification_providers: fetch_table!("SELECT id, name, provider_type, config, on_grab, on_import, on_upgrade, on_health_issue, on_failure, enabled FROM notification_providers ORDER BY id"),
        import_lists: fetch_table!("SELECT id, name, list_type, media_type, config, quality_profile_id, media_library_folder_id, monitored, enabled FROM import_lists ORDER BY id"),
        enabled_modules: fetch_table!("SELECT id, module, enabled FROM enabled_modules ORDER BY id"),
    };

    Json(resp).into_response()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RestoreRequest {
    quality_profiles: Option<Vec<serde_json::Value>>,
    tags: Option<Vec<serde_json::Value>>,
    indexers: Option<Vec<serde_json::Value>>,
    download_clients: Option<Vec<serde_json::Value>>,
    media_library_folders: Option<Vec<serde_json::Value>>,
    naming_config: Option<Vec<serde_json::Value>>,
    notification_providers: Option<Vec<serde_json::Value>>,
    import_lists: Option<Vec<serde_json::Value>>,
    enabled_modules: Option<Vec<serde_json::Value>>,
}

/// POST /api/v1/system/restore — import configuration from a backup JSON.
///
/// Restores all configuration tables. Media (series/movies) must be
/// re-imported via migration or TMDB lookup.
async fn import_restore(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RestoreRequest>,
) -> impl IntoResponse {
    let pool = state.db.pool();
    let mut restored = Vec::new();
    let mut errors = Vec::new();

    // Helper: get a string field from JSON, trying both snake_case and camelCase.
    fn str_field<'a>(v: &'a serde_json::Value, snake: &str, camel: &str) -> &'a str {
        v.get(snake)
            .or_else(|| v.get(camel))
            .and_then(|v| v.as_str())
            .unwrap_or("")
    }

    fn json_field(v: &serde_json::Value, snake: &str, camel: &str) -> serde_json::Value {
        v.get(snake)
            .or_else(|| v.get(camel))
            .cloned()
            .unwrap_or(serde_json::Value::Null)
    }

    fn bool_field(v: &serde_json::Value, snake: &str, camel: &str, default: bool) -> bool {
        v.get(snake)
            .or_else(|| v.get(camel))
            .and_then(|v| v.as_bool())
            .unwrap_or(default)
    }

    fn i64_field(v: &serde_json::Value, snake: &str, camel: &str, default: i64) -> i64 {
        v.get(snake)
            .or_else(|| v.get(camel))
            .and_then(|v| v.as_i64())
            .unwrap_or(default)
    }

    // --- Quality Profiles ---
    if let Some(profiles) = &body.quality_profiles {
        let mut count = 0;
        for p in profiles {
            let name = str_field(p, "name", "name");
            if name.is_empty() { continue; }
            let cutoff = i64_field(p, "cutoff", "cutoff", 0) as i32;
            let upgrade_allowed = bool_field(p, "upgrade_allowed", "upgradeAllowed", true);
            let min_format_score = i64_field(p, "min_format_score", "minFormatScore", 0) as i32;
            let cutoff_format_score = i64_field(p, "cutoff_format_score", "cutoffFormatScore", 0) as i32;
            let items = json_field(p, "items", "items");
            let media_type = p.get("media_type").or_else(|| p.get("mediaType")).and_then(|v| v.as_str());

            match sqlx::query(
                "INSERT INTO quality_profiles (name, cutoff, upgrade_allowed, min_format_score, cutoff_format_score, items, media_type)
                 VALUES ($1, $2, $3, $4, $5, $6, $7)
                 ON CONFLICT DO NOTHING"
            )
            .bind(name)
            .bind(cutoff)
            .bind(upgrade_allowed)
            .bind(min_format_score)
            .bind(cutoff_format_score)
            .bind(&items)
            .bind(media_type)
            .execute(pool)
            .await
            {
                Ok(_) => count += 1,
                Err(e) => {
                    tracing::error!(error = %e, "restore: failed to insert quality profile");
                    errors.push(format!("quality_profile '{name}': {e}"));
                }
            }
        }
        restored.push(format!("quality_profiles: {count}"));
    }

    // --- Tags ---
    if let Some(tags) = &body.tags {
        let mut count = 0;
        for tag in tags {
            let label = str_field(tag, "label", "label");
            if label.is_empty() { continue; }
            match sqlx::query("INSERT INTO tags (label) VALUES ($1) ON CONFLICT (label) DO NOTHING")
                .bind(label)
                .execute(pool)
                .await
            {
                Ok(_) => count += 1,
                Err(e) => {
                    tracing::error!(error = %e, "restore: failed to insert tag");
                    errors.push(format!("tag '{label}': {e}"));
                }
            }
        }
        restored.push(format!("tags: {count}"));
    }

    // --- Media Library Folders ---
    if let Some(folders) = &body.media_library_folders {
        let mut count = 0;
        for folder in folders {
            let path = str_field(folder, "path", "path");
            let media_type = str_field(folder, "media_type", "mediaType");
            let media_type = if media_type.is_empty() { "series" } else { media_type };
            if path.is_empty() { continue; }
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
                    errors.push(format!("folder '{path}': {e}"));
                }
            }
        }
        restored.push(format!("media_library_folders: {count}"));
    }

    // --- Naming Config ---
    if let Some(configs) = &body.naming_config {
        let mut count = 0;
        for c in configs {
            let media_type = str_field(c, "media_type", "mediaType");
            if media_type.is_empty() { continue; }
            let rename_files = bool_field(c, "rename_files", "renameFiles", true);
            let standard = c.get("standard_format").or_else(|| c.get("standardFormat")).and_then(|v| v.as_str());
            let daily = c.get("daily_format").or_else(|| c.get("dailyFormat")).and_then(|v| v.as_str());
            let anime = c.get("anime_format").or_else(|| c.get("animeFormat")).and_then(|v| v.as_str());
            let season_folder = c.get("season_folder_format").or_else(|| c.get("seasonFolderFormat")).and_then(|v| v.as_str());
            let movie = c.get("movie_format").or_else(|| c.get("movieFormat")).and_then(|v| v.as_str());
            let movie_folder = c.get("movie_folder_format").or_else(|| c.get("movieFolderFormat")).and_then(|v| v.as_str());

            match sqlx::query(
                "INSERT INTO naming_config (media_type, rename_files, standard_format, daily_format, anime_format, season_folder_format, movie_format, movie_folder_format)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                 ON CONFLICT (media_type) DO UPDATE SET
                     rename_files = $2, standard_format = $3, daily_format = $4,
                     anime_format = $5, season_folder_format = $6, movie_format = $7, movie_folder_format = $8"
            )
            .bind(media_type)
            .bind(rename_files)
            .bind(standard)
            .bind(daily)
            .bind(anime)
            .bind(season_folder)
            .bind(movie)
            .bind(movie_folder)
            .execute(pool)
            .await
            {
                Ok(_) => count += 1,
                Err(e) => {
                    tracing::error!(error = %e, "restore: failed to insert naming config");
                    errors.push(format!("naming_config '{media_type}': {e}"));
                }
            }
        }
        restored.push(format!("naming_config: {count}"));
    }

    // --- Indexers ---
    if let Some(indexers) = &body.indexers {
        let mut count = 0;
        for idx in indexers {
            let name = str_field(idx, "name", "name");
            if name.is_empty() { continue; }
            let indexer_type = str_field(idx, "indexer_type", "indexerType");
            let base_url = str_field(idx, "base_url", "baseUrl");
            let api_key = idx.get("api_key").or_else(|| idx.get("apiKey")).and_then(|v| v.as_str());
            let protocol = str_field(idx, "protocol", "protocol");
            let categories = json_field(idx, "categories", "categories");
            let enabled = bool_field(idx, "enabled", "enabled", true);
            let priority = i64_field(idx, "priority", "priority", 25) as i32;
            let supports_search = bool_field(idx, "supports_search", "supportsSearch", true);
            let supports_rss = bool_field(idx, "supports_rss", "supportsRss", true);
            let config = json_field(idx, "config", "config");

            match sqlx::query(
                "INSERT INTO indexers (name, indexer_type, base_url, api_key, protocol, categories, enabled, priority, supports_search, supports_rss, config)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                 ON CONFLICT DO NOTHING"
            )
            .bind(name)
            .bind(indexer_type)
            .bind(base_url)
            .bind(api_key)
            .bind(protocol)
            .bind(&categories)
            .bind(enabled)
            .bind(priority)
            .bind(supports_search)
            .bind(supports_rss)
            .bind(&config)
            .execute(pool)
            .await
            {
                Ok(_) => count += 1,
                Err(e) => {
                    tracing::error!(error = %e, "restore: failed to insert indexer");
                    errors.push(format!("indexer '{name}': {e}"));
                }
            }
        }
        restored.push(format!("indexers: {count}"));
    }

    // --- Download Clients ---
    if let Some(clients) = &body.download_clients {
        let mut count = 0;
        for c in clients {
            let name = str_field(c, "name", "name");
            if name.is_empty() { continue; }
            let client_type = str_field(c, "client_type", "clientType");
            let protocol = str_field(c, "protocol", "protocol");
            let config = json_field(c, "config", "config");
            let enabled = bool_field(c, "enabled", "enabled", true);
            let priority = i64_field(c, "priority", "priority", 1) as i32;

            match sqlx::query(
                "INSERT INTO download_clients (name, client_type, protocol, config, enabled, priority)
                 VALUES ($1, $2, $3, $4, $5, $6)
                 ON CONFLICT DO NOTHING"
            )
            .bind(name)
            .bind(client_type)
            .bind(protocol)
            .bind(&config)
            .bind(enabled)
            .bind(priority)
            .execute(pool)
            .await
            {
                Ok(_) => count += 1,
                Err(e) => {
                    tracing::error!(error = %e, "restore: failed to insert download client");
                    errors.push(format!("download_client '{name}': {e}"));
                }
            }
        }
        restored.push(format!("download_clients: {count}"));
    }

    // --- Notification Providers ---
    if let Some(providers) = &body.notification_providers {
        let mut count = 0;
        for p in providers {
            let name = str_field(p, "name", "name");
            if name.is_empty() { continue; }
            let provider_type = str_field(p, "provider_type", "providerType");
            let config = json_field(p, "config", "config");
            let on_grab = bool_field(p, "on_grab", "onGrab", false);
            let on_import = bool_field(p, "on_import", "onImport", false);
            let on_upgrade = bool_field(p, "on_upgrade", "onUpgrade", false);
            let on_health_issue = bool_field(p, "on_health_issue", "onHealthIssue", false);
            let on_failure = bool_field(p, "on_failure", "onFailure", false);
            let enabled = bool_field(p, "enabled", "enabled", true);

            match sqlx::query(
                "INSERT INTO notification_providers (name, provider_type, config, on_grab, on_import, on_upgrade, on_health_issue, on_failure, enabled)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                 ON CONFLICT DO NOTHING"
            )
            .bind(name)
            .bind(provider_type)
            .bind(&config)
            .bind(on_grab)
            .bind(on_import)
            .bind(on_upgrade)
            .bind(on_health_issue)
            .bind(on_failure)
            .bind(enabled)
            .execute(pool)
            .await
            {
                Ok(_) => count += 1,
                Err(e) => {
                    tracing::error!(error = %e, "restore: failed to insert notification provider");
                    errors.push(format!("notification '{name}': {e}"));
                }
            }
        }
        restored.push(format!("notification_providers: {count}"));
    }

    // --- Import Lists ---
    if let Some(lists) = &body.import_lists {
        let mut count = 0;
        for l in lists {
            let name = str_field(l, "name", "name");
            if name.is_empty() { continue; }
            let list_type = str_field(l, "list_type", "listType");
            let media_type = str_field(l, "media_type", "mediaType");
            let config = json_field(l, "config", "config");
            let monitored = bool_field(l, "monitored", "monitored", true);
            let enabled = bool_field(l, "enabled", "enabled", true);

            match sqlx::query(
                "INSERT INTO import_lists (name, list_type, media_type, config, monitored, enabled)
                 VALUES ($1, $2, $3, $4, $5, $6)
                 ON CONFLICT DO NOTHING"
            )
            .bind(name)
            .bind(list_type)
            .bind(media_type)
            .bind(&config)
            .bind(monitored)
            .bind(enabled)
            .execute(pool)
            .await
            {
                Ok(_) => count += 1,
                Err(e) => {
                    tracing::error!(error = %e, "restore: failed to insert import list");
                    errors.push(format!("import_list '{name}': {e}"));
                }
            }
        }
        restored.push(format!("import_lists: {count}"));
    }

    // --- Enabled Modules ---
    if let Some(modules) = &body.enabled_modules {
        let mut count = 0;
        for m in modules {
            let module = str_field(m, "module", "module");
            let enabled = bool_field(m, "enabled", "enabled", false);
            if module.is_empty() { continue; }
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
                    errors.push(format!("module '{module}': {e}"));
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
