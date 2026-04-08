use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{DefaultBodyLimit, Multipart, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EnabledModulesResponse {
    tv_management: bool,
    movie_management: bool,
    torrent_embedded: bool,
    usenet_embedded: bool,
    torrent_external: bool,
    usenet_external: bool,
    indexarr_sidecar: bool,
    external_indexers: bool,
    plex_integration: bool,
    notifications: bool,
    streaming: bool,
    remote_access: bool,
    stremio_addon: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusResponse {
    version: &'static str,
    instance_name: String,
    first_boot: bool,
    auth_method: String,
    modules: EnabledModulesResponse,
    /// Whether the Indexarr container is deployed (STACKARR_INDEXARR_ENABLED env var).
    indexarr_available: bool,
    start_time: String,
}

async fn get_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let pool = state.db.pool();

    // Check if any modules are enabled — if none, it's first boot
    let enabled_count: i64 = match sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM enabled_modules WHERE enabled = true",
    )
    .fetch_one(pool)
    .await
    {
        Ok(count) => count,
        Err(e) => {
            tracing::error!(error = %e, "failed to query enabled modules count");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response();
        }
    };

    let first_boot = enabled_count == 0;

    // Load module states
    let mut modules = EnabledModulesResponse {
        tv_management: false,
        movie_management: false,
        torrent_embedded: false,
        usenet_embedded: false,
        torrent_external: false,
        usenet_external: false,
        indexarr_sidecar: false,
        external_indexers: false,
        plex_integration: false,
        notifications: false,
        streaming: false,
        remote_access: false,
        stremio_addon: false,
    };

    if let Ok(rows) =
        sqlx::query_as::<_, (String, bool)>("SELECT module, enabled FROM enabled_modules")
            .fetch_all(pool)
            .await
    {
        for (module, enabled) in rows {
            match module.as_str() {
                "tv_management" => modules.tv_management = enabled,
                "movie_management" => modules.movie_management = enabled,
                "torrent_embedded" => modules.torrent_embedded = enabled,
                "usenet_embedded" => modules.usenet_embedded = enabled,
                "torrent_external" => modules.torrent_external = enabled,
                "usenet_external" => modules.usenet_external = enabled,
                "indexarr_sidecar" => modules.indexarr_sidecar = enabled,
                "external_indexers" => modules.external_indexers = enabled,
                "plex_integration" => modules.plex_integration = enabled,
                "notifications" => modules.notifications = enabled,
                "streaming" => modules.streaming = enabled,
                "remote_access" => modules.remote_access = enabled,
                "stremio_addon" => modules.stremio_addon = enabled,
                _ => {}
            }
        }
    }

    // Get instance name and auth method from app_config
    let config_rows = sqlx::query_as::<_, (String, serde_json::Value)>(
        "SELECT key, value FROM app_config WHERE key IN ('instance_name', 'auth_method')",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let mut instance_name = state.config.load().general.instance_name.clone();
    let mut auth_method = "none".to_string();

    for (key, value) in config_rows {
        let s = value.as_str().map(String::from).unwrap_or_default();
        match key.as_str() {
            "instance_name" => {
                if !s.is_empty() {
                    instance_name = s;
                }
            }
            "auth_method" => {
                if !s.is_empty() {
                    auth_method = s;
                }
            }
            _ => {}
        }
    }

    let start_time = chrono::Utc::now().to_rfc3339();

    Json(StatusResponse {
        version: env!("CARGO_PKG_VERSION"),
        instance_name,
        first_boot,
        auth_method,
        modules,
        indexarr_available: state.indexarr_available,
        start_time,
    })
    .into_response()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EnabledModulesRequest {
    tv_management: Option<bool>,
    movie_management: Option<bool>,
    torrent_embedded: Option<bool>,
    usenet_embedded: Option<bool>,
    torrent_external: Option<bool>,
    usenet_external: Option<bool>,
    indexarr_sidecar: Option<bool>,
    external_indexers: Option<bool>,
    plex_integration: Option<bool>,
    notifications: Option<bool>,
    streaming: Option<bool>,
    remote_access: Option<bool>,
    stremio_addon: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MediaLibraryFolderRequest {
    path: String,
    media_type: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct IndexarrSetupRequest {
    url: String,
    api_key: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PathMappingRequest {
    from: String,
    to: String,
    /// Optional media type scope: "tv"/"series" or "movie".
    /// When set, the mapping only applies to the corresponding table(s).
    #[serde(default)]
    media_type: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetupRequest {
    modules: EnabledModulesRequest,
    media_library_folders: Option<Vec<MediaLibraryFolderRequest>>,
    path_mappings: Option<Vec<PathMappingRequest>>,
    instance_name: Option<String>,
    indexarr: Option<IndexarrSetupRequest>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SetupResponse {
    success: bool,
    message: String,
    api_key: String,
    modules_configured: Vec<String>,
    media_library_folders_added: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    recovery_phrase: Option<String>,
}

async fn init_setup(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SetupRequest>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    // Check if already set up
    let enabled_count: i64 = match sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM enabled_modules WHERE enabled = true",
    )
    .fetch_one(pool)
    .await
    {
        Ok(count) => count,
        Err(e) => {
            tracing::error!(error = %e, "failed to check setup status");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response();
        }
    };

    if enabled_count > 0 {
        return (
            StatusCode::CONFLICT,
            Json(json!({"error": "Setup has already been completed. Use the settings API to modify configuration."})),
        )
            .into_response();
    }

    // Upsert each module into enabled_modules
    let module_entries = [
        ("tv_management", body.modules.tv_management.unwrap_or(false)),
        (
            "movie_management",
            body.modules.movie_management.unwrap_or(false),
        ),
        (
            "torrent_embedded",
            body.modules.torrent_embedded.unwrap_or(false),
        ),
        (
            "usenet_embedded",
            body.modules.usenet_embedded.unwrap_or(false),
        ),
        (
            "torrent_external",
            body.modules.torrent_external.unwrap_or(false),
        ),
        (
            "usenet_external",
            body.modules.usenet_external.unwrap_or(false),
        ),
        (
            "indexarr_sidecar",
            body.modules.indexarr_sidecar.unwrap_or(false),
        ),
        (
            "external_indexers",
            body.modules.external_indexers.unwrap_or(false),
        ),
        (
            "plex_integration",
            body.modules.plex_integration.unwrap_or(false),
        ),
        ("notifications", body.modules.notifications.unwrap_or(false)),
        ("streaming", body.modules.streaming.unwrap_or(false)),
        ("remote_access", body.modules.remote_access.unwrap_or(false)),
    ];

    let mut modules_configured = Vec::new();

    for (module, enabled) in &module_entries {
        if let Err(e) = sqlx::query(
            "INSERT INTO enabled_modules (module, enabled) VALUES ($1, $2)
             ON CONFLICT (module) DO UPDATE SET enabled = $2",
        )
        .bind(module)
        .bind(enabled)
        .execute(pool)
        .await
        {
            tracing::error!(error = %e, module, "failed to upsert module");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response();
        }
        if *enabled {
            modules_configured.push(module.to_string());
        }
    }

    // Insert media library folders if provided.
    // The migration may have written raw *arr paths (e.g. /TV1/) to the table.
    // The user's folder list is the source of truth — delete any migration-imported
    // folders that the user did NOT include (unless they're referenced by series/movies).
    let mut media_library_folders_added = 0;
    if let Some(folders) = &body.media_library_folders {
        let user_paths: Vec<&str> = folders.iter().map(|f| f.path.as_str()).collect();

        // Delete unreferenced folders not in the user's list
        if let Err(e) = sqlx::query(
            "DELETE FROM media_library_folders
             WHERE path != ALL($1)
               AND id NOT IN (SELECT DISTINCT media_library_folder_id FROM series WHERE media_library_folder_id IS NOT NULL)
               AND id NOT IN (SELECT DISTINCT media_library_folder_id FROM movies WHERE media_library_folder_id IS NOT NULL)",
        )
        .bind(&user_paths)
        .execute(pool)
        .await
        {
            tracing::warn!(error = %e, "failed to clean up migration-imported folders");
        }

        for folder in folders {
            if let Err(e) = sqlx::query(
                "INSERT INTO media_library_folders (path, media_type) VALUES ($1, $2)
                 ON CONFLICT (path) DO UPDATE SET media_type = $2",
            )
            .bind(&folder.path)
            .bind(&folder.media_type)
            .execute(pool)
            .await
            {
                tracing::error!(error = %e, path = %folder.path, "failed to insert media library folder");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "internal server error"})),
                )
                    .into_response();
            }
            media_library_folders_added += 1;
        }
    }

    // Apply path mappings if provided (remap imported Sonarr/Radarr paths to container paths)
    if let Some(mappings) = &body.path_mappings {
        let valid_mappings: Vec<_> = mappings
            .iter()
            .filter(|m| !m.from.is_empty() && !m.to.is_empty() && m.from != m.to)
            .collect();

        if !valid_mappings.is_empty() {
            for m in &valid_mappings {
                let scope = m.media_type.as_deref().unwrap_or("all");
                tracing::info!(from = %m.from, to = %m.to, scope, "applying path mapping");

                let is_tv = matches!(scope, "tv" | "series" | "all");
                let is_movie = matches!(scope, "movie" | "all");

                // Remap media_library_folders paths.
                // Use ON CONFLICT to handle cases where the target path already exists
                // (e.g. user already specified the correct path in their folder list).
                let folder_query = if is_tv && is_movie {
                    "UPDATE media_library_folders SET path = $2 || substring(path from length($1) + 1)
                     WHERE path LIKE $1 || '%'
                       AND NOT EXISTS (SELECT 1 FROM media_library_folders mf2 WHERE mf2.path = $2 || substring(media_library_folders.path from length($1) + 1))"
                } else if is_tv {
                    "UPDATE media_library_folders SET path = $2 || substring(path from length($1) + 1)
                     WHERE path LIKE $1 || '%' AND media_type IN ('tv', 'series')
                       AND NOT EXISTS (SELECT 1 FROM media_library_folders mf2 WHERE mf2.path = $2 || substring(media_library_folders.path from length($1) + 1))"
                } else {
                    "UPDATE media_library_folders SET path = $2 || substring(path from length($1) + 1)
                     WHERE path LIKE $1 || '%' AND media_type = 'movie'
                       AND NOT EXISTS (SELECT 1 FROM media_library_folders mf2 WHERE mf2.path = $2 || substring(media_library_folders.path from length($1) + 1))"
                };
                if let Err(e) = sqlx::query(folder_query)
                    .bind(&m.from)
                    .bind(&m.to)
                    .execute(pool)
                    .await
                {
                    tracing::error!(error = %e, "failed to remap media_library_folders paths");
                }
                // Reassign FK references from old folders to their remapped equivalents,
                // then delete the now-unreferenced old folder rows.
                {
                    // Reassign series.media_library_folder_id
                    let _ = sqlx::query(
                        "UPDATE series SET media_library_folder_id = new_f.id
                         FROM media_library_folders old_f, media_library_folders new_f
                         WHERE series.media_library_folder_id = old_f.id
                           AND old_f.path LIKE $1 || '%'
                           AND new_f.path = $2 || substring(old_f.path from length($1) + 1)",
                    )
                    .bind(&m.from)
                    .bind(&m.to)
                    .execute(pool)
                    .await;
                    // Reassign movies.media_library_folder_id
                    let _ = sqlx::query(
                        "UPDATE movies SET media_library_folder_id = new_f.id
                         FROM media_library_folders old_f, media_library_folders new_f
                         WHERE movies.media_library_folder_id = old_f.id
                           AND old_f.path LIKE $1 || '%'
                           AND new_f.path = $2 || substring(old_f.path from length($1) + 1)",
                    )
                    .bind(&m.from)
                    .bind(&m.to)
                    .execute(pool)
                    .await;
                    // Now delete old folders (should be unreferenced after reassignment)
                    let _ = sqlx::query(
                        "DELETE FROM media_library_folders WHERE path LIKE $1 || '%'
                           AND id NOT IN (SELECT DISTINCT media_library_folder_id FROM series WHERE media_library_folder_id IS NOT NULL)
                           AND id NOT IN (SELECT DISTINCT media_library_folder_id FROM movies WHERE media_library_folder_id IS NOT NULL)",
                    )
                    .bind(&m.from)
                    .execute(pool)
                    .await;
                }

                // Update series paths (only if scope includes TV)
                if is_tv
                    && let Err(e) = sqlx::query(
                        "UPDATE series SET path = $2 || substring(path from length($1) + 1)
                         WHERE path LIKE $1 || '%'",
                    )
                    .bind(&m.from)
                    .bind(&m.to)
                    .execute(pool)
                    .await
                {
                    tracing::error!(error = %e, "failed to remap series paths");
                }

                // Update movie paths (only if scope includes movies)
                if is_movie
                    && let Err(e) = sqlx::query(
                        "UPDATE movies SET path = $2 || substring(path from length($1) + 1)
                         WHERE path LIKE $1 || '%'",
                    )
                    .bind(&m.from)
                    .bind(&m.to)
                    .execute(pool)
                    .await
                {
                    tracing::error!(error = %e, "failed to remap movie paths");
                }
            }

            // Store mappings in app_config for runtime use by streaming
            let maps_json: serde_json::Value = valid_mappings
                .iter()
                .map(|m| serde_json::json!([m.from, m.to]))
                .collect::<Vec<_>>()
                .into();
            if let Err(e) = sqlx::query(
                "INSERT INTO app_config (key, value) VALUES ('path_maps', $1)
                 ON CONFLICT (key) DO UPDATE SET value = $1",
            )
            .bind(&maps_json)
            .execute(pool)
            .await
            {
                tracing::error!(error = %e, "failed to store path_maps");
            }
        }
    }

    // Clean up media library folders that point to non-existent paths
    // This handles folders imported from backups where the original path no longer exists
    // and the user didn't create a mapping for them.
    {
        let orphaned: Vec<(i32, String)> =
            sqlx::query_as("SELECT id, path FROM media_library_folders")
                .fetch_all(pool)
                .await
                .unwrap_or_default();

        for (id, path) in &orphaned {
            if !std::path::Path::new(path).is_dir() {
                tracing::info!(
                    path,
                    "removing orphaned media library folder (path does not exist)"
                );
                let _ = sqlx::query("DELETE FROM media_library_folders WHERE id = $1")
                    .bind(id)
                    .execute(pool)
                    .await;
            }
        }
    }

    // Update instance_name in app_config if provided
    if let Some(name) = &body.instance_name {
        let name_json = serde_json::Value::String(name.clone());
        if let Err(e) = sqlx::query(
            "INSERT INTO app_config (key, value) VALUES ('instance_name', $1)
             ON CONFLICT (key) DO UPDATE SET value = $1",
        )
        .bind(&name_json)
        .execute(pool)
        .await
        {
            tracing::error!(error = %e, "failed to set instance name");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response();
        }
    }

    // Store Indexarr config if provided
    if let Some(indexarr) = &body.indexarr {
        for (key, val) in [
            ("indexarr_url", &indexarr.url),
            ("indexarr_api_key", &indexarr.api_key),
        ] {
            let val_json = serde_json::Value::String(val.clone());
            if let Err(e) = sqlx::query(
                "INSERT INTO app_config (key, value) VALUES ($1, $2)
                 ON CONFLICT (key) DO UPDATE SET value = $2",
            )
            .bind(key)
            .bind(&val_json)
            .execute(pool)
            .await
            {
                tracing::error!(error = %e, "failed to store indexarr config");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "internal server error"})),
                )
                    .into_response();
            }
        }
    }

    // Generate and store API key
    let api_key = uuid::Uuid::new_v4().to_string();
    let api_key_json = serde_json::Value::String(api_key.clone());
    if let Err(e) = sqlx::query(
        "INSERT INTO app_config (key, value) VALUES ('api_key', $1)
         ON CONFLICT (key) DO UPDATE SET value = $1",
    )
    .bind(&api_key_json)
    .execute(pool)
    .await
    {
        tracing::error!(error = %e, "failed to store API key");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "internal server error"})),
        )
            .into_response();
    }
    state.set_cached_api_key(Some(api_key.clone()));

    // When streaming is enabled, also enable remote_access (bootstrap) so
    // remote clients can discover this server.
    let streaming_enabled = body.modules.streaming.unwrap_or(false);
    if streaming_enabled && !body.modules.remote_access.unwrap_or(false) {
        let _ = sqlx::query(
            "INSERT INTO enabled_modules (module, enabled) VALUES ('remote_access', true)
             ON CONFLICT (module) DO UPDATE SET enabled = true",
        )
        .execute(pool)
        .await;
        if !modules_configured.contains(&"remote_access".to_string()) {
            modules_configured.push("remote_access".to_string());
        }
    }

    // Generate a BIP39 recovery phrase when streaming is enabled.
    // The phrase is shown once to the user and only the hash is stored.
    let recovery_phrase = if streaming_enabled {
        match stackarr_core::generate_recovery_phrase() {
            Ok((phrase, hex_hash)) => {
                // Store hash so bootstrap can verify it later
                let hash_json = serde_json::Value::String(hex_hash);
                let _ = sqlx::query(
                    "INSERT INTO app_config (key, value) VALUES ('recovery_key_hash', $1)
                     ON CONFLICT (key) DO UPDATE SET value = $1",
                )
                .bind(&hash_json)
                .execute(pool)
                .await;
                Some(phrase)
            }
            Err(e) => {
                tracing::error!(error = %e, "failed to generate recovery phrase");
                None
            }
        }
    } else {
        None
    };

    // Initialize embedded engines that were just enabled
    if body.modules.torrent_embedded.unwrap_or(false) {
        state.init_torrent_engine().await;
    }
    if body.modules.usenet_embedded.unwrap_or(false) {
        state.init_usenet_engine().await;
    }

    // Trigger initial library scan in the background so the user sees
    // activity immediately after first-boot setup.
    if media_library_folders_added > 0 {
        let scan_pool = pool.clone();
        tokio::spawn(async move {
            let db = stackarr_core::Database::from_pool(scan_pool.clone());
            let folders: Vec<(String, String)> =
                match sqlx::query_as("SELECT path, media_type FROM media_library_folders")
                    .fetch_all(&scan_pool)
                    .await
                {
                    Ok(f) => f,
                    Err(e) => {
                        tracing::error!(error = %e, "initial library scan: failed to load folders");
                        return;
                    }
                };

            let activity = db
                .create_activity(
                    "disk_scan",
                    "Library Scan",
                    Some("Initial library scan after setup..."),
                )
                .await
                .ok();
            let activity_id = activity.as_ref().map(|a| a.id);

            let mut total_found = 0usize;
            let mut total_matched = 0usize;
            let folder_count = folders.len();

            for (i, (path, media_type)) in folders.iter().enumerate() {
                let scan_path = std::path::Path::new(path);
                if !scan_path.exists() {
                    continue;
                }

                if let Some(aid) = activity_id {
                    let _ = db
                        .update_activity_progress(
                            aid,
                            Some(&format!(
                                "Scanning folder {}/{}: {}",
                                i + 1,
                                folder_count,
                                path
                            )),
                            Some(serde_json::json!({
                                "folders_total": folder_count,
                                "folders_done": i,
                                "files_found": total_found,
                                "files_matched": total_matched,
                            })),
                        )
                        .await;
                }

                match stackarr_import::disk_scan(&scan_pool, scan_path, media_type).await {
                    Ok(result) => {
                        total_found += result.files_found;
                        total_matched += result.files_matched;
                    }
                    Err(e) => {
                        tracing::error!(path, error = %e, "initial library scan: folder failed");
                    }
                }
            }

            if let Some(aid) = activity_id {
                let detail = format!("{total_found} files found, {total_matched} matched");
                let _ = db
                    .complete_activity(
                        aid,
                        "completed",
                        Some(&detail),
                        Some(serde_json::json!({
                            "files_found": total_found,
                            "files_matched": total_matched,
                            "folders_scanned": folder_count,
                        })),
                        None,
                    )
                    .await;
            }

            tracing::info!(
                files_found = total_found,
                files_matched = total_matched,
                "initial library scan complete"
            );
        });
    }

    (
        StatusCode::CREATED,
        Json(json!(SetupResponse {
            success: true,
            message: "Setup complete".to_string(),
            api_key,
            modules_configured,
            media_library_folders_added,
            recovery_phrase,
        })),
    )
        .into_response()
}

// ── Migration endpoint ──────────────────────────────────────────────────────

async fn post_migrate(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let pool = state.db.pool();
    let tmp_dir = match tempfile::tempdir() {
        Ok(d) => d,
        Err(e) => {
            tracing::error!(error = %e, "failed to create temp directory");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response();
        }
    };

    let mut sonarr_path: Option<PathBuf> = None;
    let mut radarr_path: Option<PathBuf> = None;
    let mut prowlarr_path: Option<PathBuf> = None;
    let mut path_mappings: Vec<stackarr_migrate::PathMapping> = Vec::new();

    // Process multipart fields
    while let Ok(Some(field)) = multipart.next_field().await {
        let field_name = field.name().unwrap_or("").to_string();
        let data = match field.bytes().await {
            Ok(d) => d,
            Err(e) => {
                tracing::error!(error = %e, field_name, "failed to read multipart field");
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "failed to read upload field"})),
                )
                    .into_response();
            }
        };

        match field_name.as_str() {
            "path_mappings" => {
                match serde_json::from_slice::<Vec<stackarr_migrate::PathMapping>>(&data) {
                    Ok(mappings) => path_mappings = mappings,
                    Err(e) => {
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(json!({"error": format!("invalid path_mappings JSON: {e}")})),
                        )
                            .into_response();
                    }
                }
                continue;
            }
            name @ ("sonarr_db" | "radarr_db" | "prowlarr_db") => {
                // Validate upload size (500 MB max)
                const MAX_UPLOAD_SIZE: usize = 500 * 1024 * 1024;
                if data.len() > MAX_UPLOAD_SIZE {
                    return (
                        StatusCode::PAYLOAD_TOO_LARGE,
                        Json(json!({"error": "upload exceeds 500 MB limit"})),
                    )
                        .into_response();
                }
                // Validate SQLite header
                if data.len() < 16 || &data[..16] != b"SQLite format 3\0" {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(json!({"error": format!("{field_name} is not a valid SQLite database")})),
                    )
                        .into_response();
                }

                let dest = match name {
                    "sonarr_db" => {
                        let p = tmp_dir.path().join("sonarr.db");
                        sonarr_path = Some(p.clone());
                        p
                    }
                    "radarr_db" => {
                        let p = tmp_dir.path().join("radarr.db");
                        radarr_path = Some(p.clone());
                        p
                    }
                    _ => {
                        let p = tmp_dir.path().join("prowlarr.db");
                        prowlarr_path = Some(p.clone());
                        p
                    }
                };

                if let Err(e) = tokio::fs::write(&dest, &data).await {
                    tracing::error!(error = %e, "failed to write temp file");
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"error": "internal server error"})),
                    )
                        .into_response();
                }
            }
            _ => continue,
        }
    }

    if sonarr_path.is_none() && radarr_path.is_none() && prowlarr_path.is_none() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "at least one of sonarr_db, radarr_db, or prowlarr_db must be uploaded"})),
        )
            .into_response();
    }

    match stackarr_migrate::run_migration(
        pool,
        sonarr_path.as_deref(),
        radarr_path.as_deref(),
        prowlarr_path.as_deref(),
        &path_mappings,
        false,
    )
    .await
    {
        Ok(report) => Json(json!(report)).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "migration failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "migration failed"})),
            )
                .into_response()
        }
    }
    // tmp_dir is dropped here, cleaning up temp files
}

// ── Command endpoint ────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommandRequest {
    name: String,
    series_id: Option<i64>,
    movie_id: Option<i64>,
    #[serde(default)]
    episode_ids: Option<Vec<i64>>,
    #[serde(default)]
    movie_ids: Option<Vec<i64>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CommandResponse {
    name: String,
    status: String,
    result: Option<serde_json::Value>,
    error: Option<String>,
}

async fn post_command(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CommandRequest>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    match body.name.as_str() {
        "DiskScan" => {
            // If a specific series_id is given, scan just that series' path
            if let Some(series_id) = body.series_id {
                let series_row: Option<(String,)> =
                    match sqlx::query_as("SELECT path FROM series WHERE id = $1")
                        .bind(series_id)
                        .fetch_optional(pool)
                        .await
                    {
                        Ok(r) => r,
                        Err(e) => {
                            tracing::error!(error = %e, series_id, "failed to query series path");
                            return (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(json!(CommandResponse {
                                    name: body.name,
                                    status: "error".to_string(),
                                    result: None,
                                    error: Some("internal server error".to_string()),
                                })),
                            )
                                .into_response();
                        }
                    };

                if let Some((path,)) = series_row {
                    let scan_path = std::path::Path::new(&path);
                    // For a specific series, we scan its parent (media library folder) or the series path directly
                    // Since the series path itself IS the series dir, we scan it directly as a "series" root
                    // But disk_scan expects the root to contain series dirs, so we use the parent
                    let root_path = scan_path.parent().unwrap_or(scan_path);
                    match stackarr_import::disk_scan(pool, root_path, "series").await {
                        Ok(scan_result) => {
                            return Json(json!(CommandResponse {
                                name: body.name,
                                status: "completed".to_string(),
                                result: Some(serde_json::to_value(scan_result).unwrap_or_default()),
                                error: None,
                            }))
                            .into_response();
                        }
                        Err(e) => {
                            tracing::error!(error = %e, "disk scan failed for series");
                            return (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(json!(CommandResponse {
                                    name: body.name,
                                    status: "error".to_string(),
                                    result: None,
                                    error: Some("disk scan failed".to_string()),
                                })),
                            )
                                .into_response();
                        }
                    }
                } else {
                    return (
                        StatusCode::NOT_FOUND,
                        Json(json!(CommandResponse {
                            name: body.name,
                            status: "error".to_string(),
                            result: None,
                            error: Some(format!("series {series_id} not found")),
                        })),
                    )
                        .into_response();
                }
            }

            // No specific series — scan all media library folders

            // Prevent concurrent full scans
            if let Ok(Some(_)) = state.db.get_running_activity_by_type("disk_scan").await {
                return (
                    StatusCode::CONFLICT,
                    Json(json!(CommandResponse {
                        name: body.name,
                        status: "error".to_string(),
                        result: None,
                        error: Some("a library scan is already running".to_string()),
                    })),
                )
                    .into_response();
            }

            let media_library_folders: Vec<(String, String)> =
                match sqlx::query_as("SELECT path, media_type FROM media_library_folders")
                    .fetch_all(pool)
                    .await
                {
                    Ok(rows) => rows,
                    Err(e) => {
                        tracing::error!(error = %e, "failed to query media library folders");
                        return (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!(CommandResponse {
                                name: body.name,
                                status: "error".to_string(),
                                result: None,
                                error: Some("internal server error".to_string()),
                            })),
                        )
                            .into_response();
                    }
                };

            if media_library_folders.is_empty() {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!(CommandResponse {
                        name: body.name,
                        status: "error".to_string(),
                        result: None,
                        error: Some("no media library folders configured".to_string()),
                    })),
                )
                    .into_response();
            }

            // Create activity record
            let activity = state
                .db
                .create_activity("disk_scan", "Library Scan", Some("Manual scan started"))
                .await
                .ok();
            let activity_id = activity.as_ref().map(|a| a.id);

            // Aggregate results from all media library folders
            let mut total = stackarr_import::DiskScanResult {
                files_found: 0,
                files_matched: 0,
                files_unmatched: 0,
                files_already_tracked: 0,
                unmatched_files: Vec::new(),
            };
            let mut errors = Vec::new();
            let folder_count = media_library_folders.len();

            for (i, (path, media_type)) in media_library_folders.iter().enumerate() {
                if let Some(aid) = activity_id {
                    let progress_detail = if total.files_found > 0 {
                        format!(
                            "Scanning folder {}/{}: {} ({} files so far, {} matched)",
                            i + 1,
                            folder_count,
                            path,
                            total.files_found,
                            total.files_matched
                        )
                    } else {
                        format!("Scanning folder {}/{}: {}", i + 1, folder_count, path)
                    };
                    let _ = state
                        .db
                        .update_activity_progress(
                            aid,
                            Some(&progress_detail),
                            Some(serde_json::json!({
                                "folders_total": folder_count,
                                "folders_done": i,
                                "files_found": total.files_found,
                                "files_matched": total.files_matched,
                            })),
                        )
                        .await;
                }

                let scan_path = std::path::Path::new(path);
                match stackarr_import::disk_scan(pool, scan_path, media_type).await {
                    Ok(r) => {
                        total.files_found += r.files_found;
                        total.files_matched += r.files_matched;
                        total.files_unmatched += r.files_unmatched;
                        total.files_already_tracked += r.files_already_tracked;
                        total.unmatched_files.extend(r.unmatched_files);
                    }
                    Err(e) => {
                        errors.push(format!("scan of '{path}' failed: {e}"));
                    }
                }
            }

            // Complete the activity
            if let Some(aid) = activity_id {
                let result_json = serde_json::json!({
                    "files_found": total.files_found,
                    "files_matched": total.files_matched,
                    "folders_scanned": folder_count,
                });
                let detail = if total.files_found > 0 {
                    format!(
                        "{} files found, {} matched",
                        total.files_found, total.files_matched
                    )
                } else {
                    "No new files found".to_string()
                };
                if errors.is_empty() {
                    let _ = state
                        .db
                        .complete_activity(aid, "completed", Some(&detail), Some(result_json), None)
                        .await;
                } else {
                    let err = errors.join("; ");
                    let _ = state
                        .db
                        .complete_activity(
                            aid,
                            "failed",
                            Some(&detail),
                            Some(result_json),
                            Some(&err),
                        )
                        .await;
                }
                let _ = state
                    .db
                    .update_activity_progress(
                        aid,
                        None,
                        Some(serde_json::json!({
                            "folders_total": folder_count,
                            "folders_done": folder_count,
                            "files_found": total.files_found,
                            "files_matched": total.files_matched,
                        })),
                    )
                    .await;
            }

            let error_msg = if errors.is_empty() {
                None
            } else {
                Some(errors.join("; "))
            };

            Json(json!(CommandResponse {
                name: body.name,
                status: if error_msg.is_some() {
                    "completedWithErrors".to_string()
                } else {
                    "completed".to_string()
                },
                result: Some(serde_json::to_value(total).unwrap_or_default()),
                error: error_msg,
            }))
            .into_response()
        }
        "RefreshSeries" => {
            if let Some(series_id) = body.series_id {
                // Mark a specific series as needing refresh by updating last_info_sync
                match sqlx::query("UPDATE series SET last_info_sync = NOW() WHERE id = $1")
                    .bind(series_id)
                    .execute(pool)
                    .await
                {
                    Ok(r) if r.rows_affected() == 0 => (
                        StatusCode::NOT_FOUND,
                        Json(json!(CommandResponse {
                            name: body.name,
                            status: "error".to_string(),
                            result: None,
                            error: Some(format!("series {series_id} not found")),
                        })),
                    )
                        .into_response(),
                    Ok(_) => Json(json!(CommandResponse {
                        name: body.name,
                        status: "completed".to_string(),
                        result: Some(json!({"seriesId": series_id})),
                        error: None,
                    }))
                    .into_response(),
                    Err(e) => {
                        tracing::error!(error = %e, series_id, "failed to refresh series");
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!(CommandResponse {
                                name: body.name,
                                status: "error".to_string(),
                                result: None,
                                error: Some("internal server error".to_string()),
                            })),
                        )
                            .into_response()
                    }
                }
            } else {
                // Refresh all series
                match sqlx::query("UPDATE series SET last_info_sync = NOW()")
                    .execute(pool)
                    .await
                {
                    Ok(r) => Json(json!(CommandResponse {
                        name: body.name,
                        status: "completed".to_string(),
                        result: Some(json!({"seriesUpdated": r.rows_affected()})),
                        error: None,
                    }))
                    .into_response(),
                    Err(e) => {
                        tracing::error!(error = %e, "failed to refresh all series");
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!(CommandResponse {
                                name: body.name,
                                status: "error".to_string(),
                                result: None,
                                error: Some("internal server error".to_string()),
                            })),
                        )
                            .into_response()
                    }
                }
            }
        }
        "RefreshMovie" => {
            if let Some(movie_id) = body.movie_id {
                match sqlx::query("UPDATE movies SET last_info_sync = NOW() WHERE id = $1")
                    .bind(movie_id)
                    .execute(pool)
                    .await
                {
                    Ok(r) if r.rows_affected() == 0 => (
                        StatusCode::NOT_FOUND,
                        Json(json!(CommandResponse {
                            name: body.name,
                            status: "error".to_string(),
                            result: None,
                            error: Some(format!("movie {movie_id} not found")),
                        })),
                    )
                        .into_response(),
                    Ok(_) => Json(json!(CommandResponse {
                        name: body.name,
                        status: "completed".to_string(),
                        result: Some(json!({"movieId": movie_id})),
                        error: None,
                    }))
                    .into_response(),
                    Err(e) => {
                        tracing::error!(error = %e, movie_id, "failed to refresh movie");
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!(CommandResponse {
                                name: body.name,
                                status: "error".to_string(),
                                result: None,
                                error: Some("internal server error".to_string()),
                            })),
                        )
                            .into_response()
                    }
                }
            } else {
                // Refresh all movies
                match sqlx::query("UPDATE movies SET last_info_sync = NOW()")
                    .execute(pool)
                    .await
                {
                    Ok(r) => Json(json!(CommandResponse {
                        name: body.name,
                        status: "completed".to_string(),
                        result: Some(json!({"moviesUpdated": r.rows_affected()})),
                        error: None,
                    }))
                    .into_response(),
                    Err(e) => {
                        tracing::error!(error = %e, "failed to refresh all movies");
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!(CommandResponse {
                                name: body.name,
                                status: "error".to_string(),
                                result: None,
                                error: Some("internal server error".to_string()),
                            })),
                        )
                            .into_response()
                    }
                }
            }
        }
        "RefreshAll" => {
            // Set last_info_sync to NULL for all series and movies so the scheduler
            // picks them up for a full metadata refresh
            let series_result = sqlx::query("UPDATE series SET last_info_sync = NULL")
                .execute(pool)
                .await;
            let movies_result = sqlx::query("UPDATE movies SET last_info_sync = NULL")
                .execute(pool)
                .await;

            match (series_result, movies_result) {
                (Ok(sr), Ok(mr)) => Json(json!(CommandResponse {
                    name: body.name,
                    status: "completed".to_string(),
                    result: Some(json!({
                        "seriesMarked": sr.rows_affected(),
                        "moviesMarked": mr.rows_affected(),
                    })),
                    error: None,
                }))
                .into_response(),
                (Err(e), _) | (_, Err(e)) => {
                    tracing::error!(error = %e, "failed to refresh all media");
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!(CommandResponse {
                            name: body.name,
                            status: "error".to_string(),
                            result: None,
                            error: Some("internal server error".to_string()),
                        })),
                    )
                        .into_response()
                }
            }
        }
        "EpisodeSearch" => {
            let episode_ids = match body.episode_ids.as_deref() {
                Some(ids) if !ids.is_empty() => ids.to_vec(),
                _ => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(json!(CommandResponse {
                            name: body.name,
                            status: "error".to_string(),
                            result: None,
                            error: Some("episodeIds is required".to_string()),
                        })),
                    )
                        .into_response();
                }
            };

            // Look up episode + series info
            let episodes: Vec<(i64, i64, String, i32, i32, Option<i64>)> = match sqlx::query_as(
                "SELECT e.id, e.series_id, s.title, e.season_number, e.episode_number, s.tvdb_id \
                 FROM episodes e JOIN series s ON e.series_id = s.id \
                 WHERE e.id = ANY($1)",
            )
            .bind(&episode_ids)
            .fetch_all(pool)
            .await
            {
                Ok(rows) => rows,
                Err(e) => {
                    tracing::error!(error = %e, "failed to query episodes for search");
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!(CommandResponse {
                            name: body.name,
                            status: "error".to_string(),
                            result: None,
                            error: Some("failed to query episodes".to_string()),
                        })),
                    )
                        .into_response();
                }
            };

            if episodes.is_empty() {
                return (
                    StatusCode::NOT_FOUND,
                    Json(json!(CommandResponse {
                        name: body.name,
                        status: "error".to_string(),
                        result: None,
                        error: Some("no matching episodes found".to_string()),
                    })),
                )
                    .into_response();
            }

            // Spawn background task for search
            let state_clone = state.clone();
            let cmd_name = body.name.clone();
            tokio::spawn(async move {
                let db = &state_clone.db;
                let total = episodes.len();
                let title_hint = if total == 1 {
                    let (_, _, ref series_title, season, ep, _) = episodes[0];
                    format!("{series_title} S{season:02}E{ep:02}")
                } else {
                    format!("{total} episodes")
                };

                let activity = db
                    .create_activity(
                        "episode_search",
                        "Episode Search",
                        Some(&format!("Searching: {title_hint}")),
                    )
                    .await
                    .ok();
                let activity_id = activity.as_ref().map(|a| a.id);

                let mut searched = 0usize;
                let mut grabbed = 0usize;

                for (ep_id, series_id, series_title, season, episode_num, tvdb_id) in &episodes {
                    searched += 1;
                    if let Some(aid) = activity_id {
                        let detail = format!(
                            "Searching: {series_title} S{season:02}E{episode_num:02} ({searched}/{total})"
                        );
                        let _ = db
                            .update_activity_progress(
                                aid,
                                Some(&detail),
                                Some(json!({ "total": total, "searched": searched, "grabbed": grabbed })),
                            )
                            .await;
                    }

                    match super::releases::search_and_grab(
                        &state_clone,
                        series_title,
                        false,
                        *series_id,
                        Some(*ep_id),
                        Some(*series_id),
                        None,
                        *tvdb_id,
                        None,
                        None,
                        Some(*season),
                        Some(*episode_num),
                    )
                    .await
                    {
                        Ok(Some(result)) => {
                            grabbed += 1;
                            tracing::info!(
                                episode_id = ep_id,
                                title = %result.title,
                                "auto-grabbed release for episode"
                            );
                        }
                        Ok(None) => {
                            tracing::debug!(episode_id = ep_id, "no approved releases found");
                        }
                        Err(e) => {
                            tracing::warn!(episode_id = ep_id, error = %e, "episode search failed");
                        }
                    }
                }

                if let Some(aid) = activity_id {
                    let detail = if grabbed > 0 {
                        format!("{grabbed}/{total} episodes grabbed")
                    } else {
                        format!("No approved releases found for {total} episode(s)")
                    };
                    let _ = db
                        .complete_activity(
                            aid,
                            "completed",
                            Some(&detail),
                            Some(
                                json!({ "total": total, "searched": searched, "grabbed": grabbed }),
                            ),
                            None,
                        )
                        .await;
                }
            });

            Json(json!(CommandResponse {
                name: cmd_name,
                status: "started".to_string(),
                result: None,
                error: None,
            }))
            .into_response()
        }
        "MovieSearch" => {
            let movie_ids = match body.movie_ids.as_deref() {
                Some(ids) if !ids.is_empty() => ids.to_vec(),
                _ => match body.movie_id {
                    Some(id) => vec![id],
                    None => {
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(json!(CommandResponse {
                                name: body.name,
                                status: "error".to_string(),
                                result: None,
                                error: Some("movieIds or movieId is required".to_string()),
                            })),
                        )
                            .into_response();
                    }
                },
            };

            let movies: Vec<(i64, String, Option<i64>, Option<String>)> = match sqlx::query_as(
                "SELECT id, title, tmdb_id, imdb_id FROM movies WHERE id = ANY($1)",
            )
            .bind(&movie_ids)
            .fetch_all(pool)
            .await
            {
                Ok(rows) => rows,
                Err(e) => {
                    tracing::error!(error = %e, "failed to query movies for search");
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!(CommandResponse {
                            name: body.name,
                            status: "error".to_string(),
                            result: None,
                            error: Some("failed to query movies".to_string()),
                        })),
                    )
                        .into_response();
                }
            };

            if movies.is_empty() {
                return (
                    StatusCode::NOT_FOUND,
                    Json(json!(CommandResponse {
                        name: body.name,
                        status: "error".to_string(),
                        result: None,
                        error: Some("no matching movies found".to_string()),
                    })),
                )
                    .into_response();
            }

            let state_clone = state.clone();
            let cmd_name = body.name.clone();
            tokio::spawn(async move {
                let db = &state_clone.db;
                let total = movies.len();
                let title_hint = if total == 1 {
                    movies[0].1.clone()
                } else {
                    format!("{total} movies")
                };

                let activity = db
                    .create_activity(
                        "movie_search",
                        "Movie Search",
                        Some(&format!("Searching: {title_hint}")),
                    )
                    .await
                    .ok();
                let activity_id = activity.as_ref().map(|a| a.id);

                let mut searched = 0usize;
                let mut grabbed = 0usize;

                for (movie_id, title, tmdb_id, imdb_id) in &movies {
                    searched += 1;
                    if let Some(aid) = activity_id {
                        let detail = format!("Searching: {title} ({searched}/{total})");
                        let _ = db
                            .update_activity_progress(
                                aid,
                                Some(&detail),
                                Some(json!({ "total": total, "searched": searched, "grabbed": grabbed })),
                            )
                            .await;
                    }

                    match super::releases::search_and_grab(
                        &state_clone,
                        title,
                        true,
                        *movie_id,
                        None,
                        None,
                        Some(*movie_id),
                        None,
                        *tmdb_id,
                        imdb_id.clone(),
                        None,
                        None,
                    )
                    .await
                    {
                        Ok(Some(result)) => {
                            grabbed += 1;
                            tracing::info!(movie_id, title = %result.title, "auto-grabbed release for movie");
                        }
                        Ok(None) => {
                            tracing::debug!(movie_id, "no approved releases found");
                        }
                        Err(e) => {
                            tracing::warn!(movie_id, error = %e, "movie search failed");
                        }
                    }
                }

                if let Some(aid) = activity_id {
                    let detail = if grabbed > 0 {
                        format!("{grabbed}/{total} movies grabbed")
                    } else {
                        format!("No approved releases found for {total} movie(s)")
                    };
                    let _ = db
                        .complete_activity(
                            aid,
                            "completed",
                            Some(&detail),
                            Some(
                                json!({ "total": total, "searched": searched, "grabbed": grabbed }),
                            ),
                            None,
                        )
                        .await;
                }
            });

            Json(json!(CommandResponse {
                name: cmd_name,
                status: "started".to_string(),
                result: None,
                error: None,
            }))
            .into_response()
        }
        "MissingSearch" => {
            // Prevent concurrent missing searches
            if let Ok(Some(_)) = state
                .db
                .get_running_activity_by_type("missing_search")
                .await
            {
                return (
                    StatusCode::CONFLICT,
                    Json(json!(CommandResponse {
                        name: body.name,
                        status: "error".to_string(),
                        result: None,
                        error: Some("a missing search is already running".to_string()),
                    })),
                )
                    .into_response();
            }

            let state_clone = state.clone();
            let cmd_name = body.name.clone();
            tokio::spawn(async move {
                let db = &state_clone.db;
                let pool = db.pool();

                let activity = db
                    .create_activity(
                        "missing_search",
                        "Search Missing",
                        Some("Gathering missing items..."),
                    )
                    .await
                    .ok();
                let activity_id = activity.as_ref().map(|a| a.id);

                // Run the actual search in a block so we always complete the activity
                let (total, searched, grabbed) = async {
                    // Query missing episodes
                    let episodes: Vec<(i64, i64, String, i32, i32, Option<i64>)> = sqlx::query_as(
                        "SELECT e.id, e.series_id, s.title, e.season_number, e.episode_number, s.tvdb_id \
                         FROM episodes e JOIN series s ON e.series_id = s.id \
                         WHERE e.monitored = true AND s.monitored = true \
                         AND e.episode_file_id IS NULL \
                         AND e.season_number > 0 \
                         AND (e.air_date IS NULL OR e.air_date <= CURRENT_DATE) \
                         ORDER BY e.air_date DESC NULLS LAST",
                    )
                    .fetch_all(pool)
                    .await
                    .unwrap_or_default();

                    // Query missing movies
                    let movies: Vec<(i64, String, Option<i64>, Option<String>)> = sqlx::query_as(
                        "SELECT id, title, tmdb_id, imdb_id FROM movies \
                         WHERE monitored = true AND movie_file_id IS NULL \
                         ORDER BY title",
                    )
                    .fetch_all(pool)
                    .await
                    .unwrap_or_default();

                    let total = episodes.len() + movies.len();
                    if total == 0 {
                        return (0usize, 0usize, 0usize);
                    }

                    let searched = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
                    let grabbed = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
                    let search_timeout = std::time::Duration::from_secs(90);

                    // Maximum concurrent searches — keeps DB pool and indexer
                    // load bounded so the rest of the app stays responsive.
                    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(5));

                    // Build all search futures (episodes + movies)
                    let mut search_tasks = tokio::task::JoinSet::new();

                    for (ep_id, series_id, series_title, season, episode_num, tvdb_id) in episodes {
                        let state_ref = state_clone.clone();
                        let sem = semaphore.clone();
                        let grabbed = grabbed.clone();
                        let searched = searched.clone();

                        let db_ref = state_clone.db.clone();
                        search_tasks.spawn(async move {
                            let _permit = sem.acquire().await.unwrap();
                            let idx = searched.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                            if let Some(aid) = activity_id {
                                let detail = format!(
                                    "Searching: {series_title} S{season:02}E{episode_num:02} ({idx}/{total})"
                                );
                                let _ = db_ref
                                    .update_activity_progress(
                                        aid,
                                        Some(&detail),
                                        Some(json!({ "total": total, "searched": idx, "grabbed": grabbed.load(std::sync::atomic::Ordering::Relaxed) })),
                                    )
                                    .await;
                            }

                            let result = tokio::time::timeout(
                                search_timeout,
                                super::releases::search_and_grab(
                                    &state_ref,
                                    &series_title,
                                    false,
                                    series_id,
                                    Some(ep_id),
                                    Some(series_id),
                                    None,
                                    tvdb_id,
                                    None,
                                    None,
                                    Some(season),
                                    Some(episode_num),
                                ),
                            )
                            .await;

                            match result {
                                Ok(Ok(Some(_))) => { grabbed.fetch_add(1, std::sync::atomic::Ordering::Relaxed); }
                                Ok(Ok(None)) => {}
                                Ok(Err(e)) => {
                                    tracing::warn!(
                                        series = %series_title,
                                        season,
                                        episode = episode_num,
                                        error = %e,
                                        "missing search: episode search failed"
                                    );
                                }
                                Err(_) => {
                                    tracing::warn!(
                                        series = %series_title,
                                        season,
                                        episode = episode_num,
                                        "search_and_grab timed out after {}s",
                                        search_timeout.as_secs()
                                    );
                                }
                            }
                        });
                    }

                    for (movie_id, title, tmdb_id, imdb_id) in movies {
                        let state_ref = state_clone.clone();
                        let sem = semaphore.clone();
                        let grabbed = grabbed.clone();
                        let searched = searched.clone();

                        let db_ref = state_clone.db.clone();
                        search_tasks.spawn(async move {
                            let _permit = sem.acquire().await.unwrap();
                            let idx = searched.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                            if let Some(aid) = activity_id {
                                let detail = format!("Searching: {title} ({idx}/{total})");
                                let _ = db_ref
                                    .update_activity_progress(
                                        aid,
                                        Some(&detail),
                                        Some(json!({ "total": total, "searched": idx, "grabbed": grabbed.load(std::sync::atomic::Ordering::Relaxed) })),
                                    )
                                    .await;
                            }

                            let result = tokio::time::timeout(
                                search_timeout,
                                super::releases::search_and_grab(
                                    &state_ref,
                                    &title,
                                    true,
                                    movie_id,
                                    None,
                                    None,
                                    Some(movie_id),
                                    None,
                                    tmdb_id,
                                    imdb_id,
                                    None,
                                    None,
                                ),
                            )
                            .await;

                            match result {
                                Ok(Ok(Some(_))) => { grabbed.fetch_add(1, std::sync::atomic::Ordering::Relaxed); }
                                Ok(Ok(None)) => {}
                                Ok(Err(e)) => {
                                    tracing::warn!(
                                        movie = %title,
                                        error = %e,
                                        "missing search: movie search failed"
                                    );
                                }
                                Err(_) => {
                                    tracing::warn!(
                                        movie = %title,
                                        "search_and_grab timed out after {}s",
                                        search_timeout.as_secs()
                                    );
                                }
                            }
                        });
                    }

                    // Wait for all searches to complete
                    while search_tasks.join_next().await.is_some() {}

                    let searched = searched.load(std::sync::atomic::Ordering::Relaxed);
                    let grabbed = grabbed.load(std::sync::atomic::Ordering::Relaxed);
                    (total, searched, grabbed)
                }
                .await;

                // Always complete the activity — even on early return or panic recovery
                if let Some(aid) = activity_id {
                    let detail = if total == 0 {
                        "No missing items found".to_string()
                    } else {
                        format!("{total} items searched, {grabbed} grabbed")
                    };
                    let _ = db
                        .complete_activity(
                            aid,
                            "completed",
                            Some(&detail),
                            Some(
                                json!({ "total": total, "searched": searched, "grabbed": grabbed }),
                            ),
                            None,
                        )
                        .await;
                }

                tracing::info!(total, grabbed, "missing search completed");
            });

            Json(json!(CommandResponse {
                name: cmd_name,
                status: "started".to_string(),
                result: None,
                error: None,
            }))
            .into_response()
        }
        "CutoffSearch" => {
            // Prevent concurrent cutoff searches
            if let Ok(Some(_)) = state.db.get_running_activity_by_type("cutoff_search").await {
                return (
                    StatusCode::CONFLICT,
                    Json(json!(CommandResponse {
                        name: body.name,
                        status: "error".to_string(),
                        result: None,
                        error: Some("a cutoff search is already running".to_string()),
                    })),
                )
                    .into_response();
            }

            let state_clone = state.clone();
            let cmd_name = body.name.clone();
            tokio::spawn(async move {
                let db = &state_clone.db;
                let pool = db.pool();

                let activity = db
                    .create_activity(
                        "cutoff_search",
                        "Search Cutoff Unmet",
                        Some("Gathering cutoff-unmet items..."),
                    )
                    .await
                    .ok();
                let activity_id = activity.as_ref().map(|a| a.id);

                // Query episodes below cutoff
                let episodes: Vec<(i64, i64, String, i32, i32, Option<i64>)> = sqlx::query_as(
                    "SELECT e.id, e.series_id, s.title, e.season_number, e.episode_number, s.tvdb_id \
                     FROM episodes e \
                     JOIN series s ON e.series_id = s.id \
                     JOIN media_files mf ON e.episode_file_id = mf.id \
                     JOIN quality_profiles qp ON s.quality_profile_id = qp.id \
                     WHERE e.monitored = true AND s.monitored = true \
                     AND e.episode_file_id IS NOT NULL \
                     AND (mf.quality->>'quality')::int < qp.cutoff \
                     ORDER BY e.air_date DESC NULLS LAST",
                )
                .fetch_all(pool)
                .await
                .unwrap_or_default();

                // Query movies below cutoff
                let movies: Vec<(i64, String, Option<i64>, Option<String>)> = sqlx::query_as(
                    "SELECT m.id, m.title, m.tmdb_id, m.imdb_id \
                     FROM movies m \
                     JOIN media_files mf ON m.movie_file_id = mf.id \
                     JOIN quality_profiles qp ON m.quality_profile_id = qp.id \
                     WHERE m.monitored = true AND m.movie_file_id IS NOT NULL \
                     AND (mf.quality->>'quality')::int < qp.cutoff \
                     ORDER BY m.title",
                )
                .fetch_all(pool)
                .await
                .unwrap_or_default();

                let total = episodes.len() + movies.len();
                if total == 0 {
                    if let Some(aid) = activity_id {
                        let _ = db
                            .complete_activity(
                                aid,
                                "completed",
                                Some("No cutoff-unmet items found"),
                                None,
                                None,
                            )
                            .await;
                    }
                    return;
                }

                let mut searched = 0usize;
                let mut grabbed = 0usize;

                for (ep_id, series_id, series_title, season, episode_num, tvdb_id) in &episodes {
                    searched += 1;
                    if let Some(aid) = activity_id {
                        let detail = format!(
                            "Searching: {series_title} S{season:02}E{episode_num:02} ({searched}/{total})"
                        );
                        let _ = db
                            .update_activity_progress(
                                aid,
                                Some(&detail),
                                Some(json!({ "total": total, "searched": searched, "grabbed": grabbed })),
                            )
                            .await;
                    }

                    match super::releases::search_and_grab(
                        &state_clone,
                        series_title,
                        false,
                        *series_id,
                        Some(*ep_id),
                        Some(*series_id),
                        None,
                        *tvdb_id,
                        None,
                        None,
                        Some(*season),
                        Some(*episode_num),
                    )
                    .await
                    {
                        Ok(Some(_)) => {
                            grabbed += 1;
                        }
                        Ok(None) => {}
                        Err(e) => {
                            tracing::warn!(
                                series = %series_title, season, episode = episode_num, error = %e,
                                "cutoff search: episode search failed"
                            );
                        }
                    }
                }

                for (movie_id, title, tmdb_id, imdb_id) in &movies {
                    searched += 1;
                    if let Some(aid) = activity_id {
                        let detail = format!("Searching: {title} ({searched}/{total})");
                        let _ = db
                            .update_activity_progress(
                                aid,
                                Some(&detail),
                                Some(json!({ "total": total, "searched": searched, "grabbed": grabbed })),
                            )
                            .await;
                    }

                    match super::releases::search_and_grab(
                        &state_clone,
                        title,
                        true,
                        *movie_id,
                        None,
                        None,
                        Some(*movie_id),
                        None,
                        *tmdb_id,
                        imdb_id.clone(),
                        None,
                        None,
                    )
                    .await
                    {
                        Ok(Some(_)) => {
                            grabbed += 1;
                        }
                        Ok(None) => {}
                        Err(e) => {
                            tracing::warn!(
                                movie = %title, error = %e,
                                "cutoff search: movie search failed"
                            );
                        }
                    }
                }

                if let Some(aid) = activity_id {
                    let detail = format!("{total} items searched, {grabbed} grabbed");
                    let _ = db
                        .complete_activity(
                            aid,
                            "completed",
                            Some(&detail),
                            Some(
                                json!({ "total": total, "searched": searched, "grabbed": grabbed }),
                            ),
                            None,
                        )
                        .await;
                }

                tracing::info!(total, grabbed, "cutoff search completed");
            });

            Json(json!(CommandResponse {
                name: cmd_name,
                status: "started".to_string(),
                result: None,
                error: None,
            }))
            .into_response()
        }
        "SeriesMissingSearch" => {
            let series_id = match body.series_id {
                Some(id) => id,
                None => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(json!(CommandResponse {
                            name: body.name,
                            status: "error".to_string(),
                            result: None,
                            error: Some("seriesId is required".to_string()),
                        })),
                    )
                        .into_response();
                }
            };

            let state_clone = state.clone();
            let cmd_name = body.name.clone();
            tokio::spawn(async move {
                let db = &state_clone.db;
                let pool = db.pool();

                // Fetch series title for activity display
                let series_title: String =
                    sqlx::query_scalar("SELECT title FROM series WHERE id = $1")
                        .bind(series_id)
                        .fetch_optional(pool)
                        .await
                        .ok()
                        .flatten()
                        .unwrap_or_else(|| format!("Series {series_id}"));

                let activity = db
                    .create_activity(
                        "series_missing_search",
                        "Series Missing Search",
                        Some(&format!("Searching missing: {series_title}")),
                    )
                    .await
                    .ok();
                let activity_id = activity.as_ref().map(|a| a.id);

                // Query missing episodes for this specific series
                let episodes: Vec<(i64, i64, String, i32, i32, Option<i64>)> = sqlx::query_as(
                    "SELECT e.id, e.series_id, s.title, e.season_number, e.episode_number, s.tvdb_id \
                     FROM episodes e JOIN series s ON e.series_id = s.id \
                     WHERE e.series_id = $1 \
                     AND e.monitored = true AND s.monitored = true \
                     AND e.episode_file_id IS NULL \
                     AND e.season_number > 0 \
                     AND (e.air_date IS NULL OR e.air_date <= CURRENT_DATE) \
                     ORDER BY e.season_number, e.episode_number",
                )
                .bind(series_id)
                .fetch_all(pool)
                .await
                .unwrap_or_default();

                let total = episodes.len();
                if total == 0 {
                    if let Some(aid) = activity_id {
                        let _ = db
                            .complete_activity(
                                aid,
                                "completed",
                                Some(&format!("No missing episodes for {series_title}")),
                                None,
                                None,
                            )
                            .await;
                    }
                    return;
                }

                let mut searched = 0usize;
                let mut grabbed = 0usize;

                for (ep_id, sid, s_title, season, episode_num, tvdb_id) in &episodes {
                    searched += 1;
                    if let Some(aid) = activity_id {
                        let detail = format!(
                            "Searching: {s_title} S{season:02}E{episode_num:02} ({searched}/{total})"
                        );
                        let _ = db
                            .update_activity_progress(
                                aid,
                                Some(&detail),
                                Some(json!({ "total": total, "searched": searched, "grabbed": grabbed })),
                            )
                            .await;
                    }

                    // Use "Series Name S##E##" as query term so text-based
                    // indexers can match even without TVDB ID support
                    let search_term = format!("{s_title} S{season:02}E{episode_num:02}");
                    match super::releases::search_and_grab(
                        &state_clone,
                        &search_term,
                        false,
                        *sid,
                        Some(*ep_id),
                        Some(*sid),
                        None,
                        *tvdb_id,
                        None,
                        None,
                        Some(*season),
                        Some(*episode_num),
                    )
                    .await
                    {
                        Ok(Some(_)) => {
                            grabbed += 1;
                        }
                        Ok(None) => {}
                        Err(e) => {
                            tracing::warn!(
                                series = %s_title, season, episode = episode_num, error = %e,
                                "search: episode search failed"
                            );
                        }
                    }
                }

                if let Some(aid) = activity_id {
                    let detail = if grabbed > 0 {
                        format!("{series_title}: {grabbed}/{total} episodes grabbed")
                    } else {
                        format!("{series_title}: no approved releases for {total} episode(s)")
                    };
                    let _ = db
                        .complete_activity(
                            aid,
                            "completed",
                            Some(&detail),
                            Some(
                                json!({ "total": total, "searched": searched, "grabbed": grabbed }),
                            ),
                            None,
                        )
                        .await;
                }

                tracing::info!(series_id, total, grabbed, "series missing search completed");
            });

            Json(json!(CommandResponse {
                name: cmd_name,
                status: "started".to_string(),
                result: None,
                error: None,
            }))
            .into_response()
        }
        "SeriesCutoffSearch" => {
            let series_id = match body.series_id {
                Some(id) => id,
                None => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(json!(CommandResponse {
                            name: body.name,
                            status: "error".to_string(),
                            result: None,
                            error: Some("seriesId is required".to_string()),
                        })),
                    )
                        .into_response();
                }
            };

            let state_clone = state.clone();
            let cmd_name = body.name.clone();
            tokio::spawn(async move {
                let db = &state_clone.db;
                let pool = db.pool();

                // Fetch series title for activity display
                let series_title: String =
                    sqlx::query_scalar("SELECT title FROM series WHERE id = $1")
                        .bind(series_id)
                        .fetch_optional(pool)
                        .await
                        .ok()
                        .flatten()
                        .unwrap_or_else(|| format!("Series {series_id}"));

                let activity = db
                    .create_activity(
                        "series_cutoff_search",
                        "Series Cutoff Search",
                        Some(&format!("Searching upgrades: {series_title}")),
                    )
                    .await
                    .ok();
                let activity_id = activity.as_ref().map(|a| a.id);

                // Query episodes below cutoff for this specific series
                let episodes: Vec<(i64, i64, String, i32, i32, Option<i64>)> = sqlx::query_as(
                    "SELECT e.id, e.series_id, s.title, e.season_number, e.episode_number, s.tvdb_id \
                     FROM episodes e \
                     JOIN series s ON e.series_id = s.id \
                     JOIN media_files mf ON e.episode_file_id = mf.id \
                     JOIN quality_profiles qp ON s.quality_profile_id = qp.id \
                     WHERE e.series_id = $1 \
                     AND e.monitored = true AND s.monitored = true \
                     AND e.episode_file_id IS NOT NULL \
                     AND (mf.quality->>'quality')::int < qp.cutoff \
                     ORDER BY e.season_number, e.episode_number",
                )
                .bind(series_id)
                .fetch_all(pool)
                .await
                .unwrap_or_default();

                let total = episodes.len();
                if total == 0 {
                    if let Some(aid) = activity_id {
                        let _ = db
                            .complete_activity(
                                aid,
                                "completed",
                                Some(&format!("No upgradeable episodes for {series_title}")),
                                None,
                                None,
                            )
                            .await;
                    }
                    return;
                }

                let mut searched = 0usize;
                let mut grabbed = 0usize;

                for (ep_id, sid, s_title, season, episode_num, tvdb_id) in &episodes {
                    searched += 1;
                    if let Some(aid) = activity_id {
                        let detail = format!(
                            "Searching: {s_title} S{season:02}E{episode_num:02} ({searched}/{total})"
                        );
                        let _ = db
                            .update_activity_progress(
                                aid,
                                Some(&detail),
                                Some(json!({ "total": total, "searched": searched, "grabbed": grabbed })),
                            )
                            .await;
                    }

                    // Use "Series Name S##E##" as query term so text-based
                    // indexers can match even without TVDB ID support
                    let search_term = format!("{s_title} S{season:02}E{episode_num:02}");
                    match super::releases::search_and_grab(
                        &state_clone,
                        &search_term,
                        false,
                        *sid,
                        Some(*ep_id),
                        Some(*sid),
                        None,
                        *tvdb_id,
                        None,
                        None,
                        Some(*season),
                        Some(*episode_num),
                    )
                    .await
                    {
                        Ok(Some(_)) => {
                            grabbed += 1;
                        }
                        Ok(None) => {}
                        Err(e) => {
                            tracing::warn!(
                                series = %s_title, season, episode = episode_num, error = %e,
                                "search: episode search failed"
                            );
                        }
                    }
                }

                if let Some(aid) = activity_id {
                    let detail = if grabbed > 0 {
                        format!("{series_title}: {grabbed}/{total} episodes upgraded")
                    } else {
                        format!("{series_title}: no upgrades found for {total} episode(s)")
                    };
                    let _ = db
                        .complete_activity(
                            aid,
                            "completed",
                            Some(&detail),
                            Some(
                                json!({ "total": total, "searched": searched, "grabbed": grabbed }),
                            ),
                            None,
                        )
                        .await;
                }

                tracing::info!(series_id, total, grabbed, "series cutoff search completed");
            });

            Json(json!(CommandResponse {
                name: cmd_name,
                status: "started".to_string(),
                result: None,
                error: None,
            }))
            .into_response()
        }
        other => (
            StatusCode::BAD_REQUEST,
            Json(json!(CommandResponse {
                name: other.to_string(),
                status: "error".to_string(),
                result: None,
                error: Some(format!("unknown command: {other}")),
            })),
        )
            .into_response(),
    }
}

// ── Filesystem browse endpoint ──────────────────────────────────────────────

#[derive(Deserialize)]
struct FilesystemBrowseQuery {
    path: Option<String>,
}

#[derive(Serialize)]
struct FilesystemDirectory {
    name: String,
    path: String,
}

#[derive(Serialize)]
struct FilesystemBrowseResponse {
    current: String,
    parent: Option<String>,
    directories: Vec<FilesystemDirectory>,
}

async fn get_filesystem_browse(Query(params): Query<FilesystemBrowseQuery>) -> impl IntoResponse {
    let raw_path = params.path.unwrap_or_else(|| "/".to_string());
    let browse_path = std::path::Path::new(&raw_path);

    let parent = browse_path
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .filter(|p| p != &raw_path);

    let mut directories = Vec::new();

    if let Ok(mut entries) = tokio::fs::read_dir(&browse_path).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let Ok(file_type) = entry.file_type().await else {
                continue;
            };
            if file_type.is_dir() {
                let name = entry.file_name().to_string_lossy().into_owned();
                let full_path = entry.path().to_string_lossy().into_owned();
                directories.push(FilesystemDirectory {
                    name,
                    path: full_path,
                });
            }
        }
    }

    directories.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    Json(FilesystemBrowseResponse {
        current: raw_path,
        parent,
        directories,
    })
}

// ── Update modules endpoint ─────────────────────────────────────────────────

async fn put_modules(
    State(state): State<Arc<AppState>>,
    Json(body): Json<EnabledModulesRequest>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    let module_entries: Vec<(&str, Option<bool>)> = vec![
        ("tv_management", body.tv_management),
        ("movie_management", body.movie_management),
        ("torrent_embedded", body.torrent_embedded),
        ("usenet_embedded", body.usenet_embedded),
        ("torrent_external", body.torrent_external),
        ("usenet_external", body.usenet_external),
        ("indexarr_sidecar", body.indexarr_sidecar),
        ("external_indexers", body.external_indexers),
        ("plex_integration", body.plex_integration),
        ("notifications", body.notifications),
        ("streaming", body.streaming),
        ("remote_access", body.remote_access),
        ("stremio_addon", body.stremio_addon),
    ];

    let mut updated = Vec::new();

    for (module, value) in &module_entries {
        let Some(enabled) = value else { continue };
        if let Err(e) = sqlx::query(
            "INSERT INTO enabled_modules (module, enabled) VALUES ($1, $2)
             ON CONFLICT (module) DO UPDATE SET enabled = $2",
        )
        .bind(module)
        .bind(enabled)
        .execute(pool)
        .await
        {
            tracing::error!(error = %e, module, "failed to update module");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal server error"})),
            )
                .into_response();
        }
        updated.push(*module);
    }

    // When streaming is enabled, also enable remote_access (bootstrap) so
    // remote clients can discover this server.
    if body.streaming == Some(true) {
        let _ = sqlx::query(
            "INSERT INTO enabled_modules (module, enabled) VALUES ('remote_access', true)
             ON CONFLICT (module) DO UPDATE SET enabled = true",
        )
        .execute(pool)
        .await;
    }

    // Initialize engines if they were just enabled
    if body.torrent_embedded == Some(true) {
        state.init_torrent_engine().await;
    }
    if body.usenet_embedded == Some(true) {
        state.init_usenet_engine().await;
    }

    Json(json!({"updated": updated})).into_response()
}

/// Public routes (no auth required) — status + setup init.
pub fn public_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/system/status", get(get_status))
        .route("/api/v1/setup/init", post(init_setup))
}

/// Protected routes (require API key) — migration, commands, filesystem browse.
pub fn protected_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/system/migrate", post(post_migrate))
        .layer(DefaultBodyLimit::max(1024 * 1024 * 1024)) // 1 GB for DB uploads
        .route("/api/v1/command", post(post_command))
        .route("/api/v1/modules", put(put_modules))
        .route("/api/v1/filesystem/browse", get(get_filesystem_browse))
}
