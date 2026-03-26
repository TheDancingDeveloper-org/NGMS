use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Multipart, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
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
    notifications: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusResponse {
    version: &'static str,
    instance_name: String,
    first_boot: bool,
    modules: EnabledModulesResponse,
    start_time: String,
}

async fn get_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let pool = state.db.pool();

    // Check if any modules are enabled — if none, it's first boot
    let enabled_count: i64 =
        match sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM enabled_modules WHERE enabled = true")
            .fetch_one(pool)
            .await
        {
            Ok(count) => count,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("database error: {e}")})),
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
        notifications: false,
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
                "notifications" => modules.notifications = enabled,
                _ => {}
            }
        }
    }

    // Get instance name from app_config, fall back to config file value
    let instance_name = sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT value FROM app_config WHERE key = 'instance_name'",
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .and_then(|v| v.as_str().map(String::from))
    .unwrap_or_else(|| state.config.load().general.instance_name.clone());

    let start_time = chrono::Utc::now().to_rfc3339();

    Json(StatusResponse {
        version: env!("CARGO_PKG_VERSION"),
        instance_name,
        first_boot,
        modules,
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
    notifications: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RootFolderRequest {
    path: String,
    media_type: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetupRequest {
    modules: EnabledModulesRequest,
    root_folders: Option<Vec<RootFolderRequest>>,
    instance_name: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SetupResponse {
    success: bool,
    message: String,
    api_key: String,
    modules_configured: Vec<String>,
    root_folders_added: usize,
}

async fn init_setup(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SetupRequest>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    // Check if already set up
    let enabled_count: i64 =
        match sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM enabled_modules WHERE enabled = true")
            .fetch_one(pool)
            .await
        {
            Ok(count) => count,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("database error: {e}")})),
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
            "notifications",
            body.modules.notifications.unwrap_or(false),
        ),
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
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("failed to upsert module {module}: {e}")})),
            )
                .into_response();
        }
        if *enabled {
            modules_configured.push(module.to_string());
        }
    }

    // Insert root folders if provided
    let mut root_folders_added = 0;
    if let Some(folders) = &body.root_folders {
        for folder in folders {
            if let Err(e) = sqlx::query(
                "INSERT INTO root_folders (path, media_type) VALUES ($1, $2)
                 ON CONFLICT (path) DO UPDATE SET media_type = $2",
            )
            .bind(&folder.path)
            .bind(&folder.media_type)
            .execute(pool)
            .await
            {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(
                        json!({"error": format!("failed to insert root folder '{}': {e}", folder.path)}),
                    ),
                )
                    .into_response();
            }
            root_folders_added += 1;
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
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("failed to set instance name: {e}")})),
            )
                .into_response();
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
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("failed to store API key: {e}")})),
        )
            .into_response();
    }

    (
        StatusCode::CREATED,
        Json(json!(SetupResponse {
            success: true,
            message: "Setup complete".to_string(),
            api_key,
            modules_configured,
            root_folders_added,
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
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("failed to create temp directory: {e}")})),
            )
                .into_response();
        }
    };

    let mut sonarr_path: Option<PathBuf> = None;
    let mut radarr_path: Option<PathBuf> = None;
    let mut prowlarr_path: Option<PathBuf> = None;

    // Process multipart fields
    while let Ok(Some(field)) = multipart.next_field().await {
        let field_name = field.name().unwrap_or("").to_string();
        let data = match field.bytes().await {
            Ok(d) => d,
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": format!("failed to read field '{field_name}': {e}")})),
                )
                    .into_response();
            }
        };

        let dest = match field_name.as_str() {
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
            "prowlarr_db" => {
                let p = tmp_dir.path().join("prowlarr.db");
                prowlarr_path = Some(p.clone());
                p
            }
            _ => continue,
        };

        if let Err(e) = tokio::fs::write(&dest, &data).await {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("failed to write temp file: {e}")})),
            )
                .into_response();
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
        false,
    )
    .await
    {
        Ok(report) => Json(json!(report)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("migration failed: {e}")})),
        )
            .into_response(),
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
                let series_row: Option<(String,)> = match sqlx::query_as(
                    "SELECT path FROM series WHERE id = $1",
                )
                .bind(series_id)
                .fetch_optional(pool)
                .await
                {
                    Ok(r) => r,
                    Err(e) => {
                        return (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!(CommandResponse {
                                name: body.name,
                                status: "error".to_string(),
                                result: None,
                                error: Some(format!("database error: {e}")),
                            })),
                        )
                            .into_response();
                    }
                };

                if let Some((path,)) = series_row {
                    let scan_path = std::path::Path::new(&path);
                    // For a specific series, we scan its parent (root folder) or the series path directly
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
                            return (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(json!(CommandResponse {
                                    name: body.name,
                                    status: "error".to_string(),
                                    result: None,
                                    error: Some(format!("disk scan failed: {e}")),
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

            // No specific series — scan all root folders
            let root_folders: Vec<(String, String)> = match sqlx::query_as(
                "SELECT path, media_type FROM root_folders",
            )
            .fetch_all(pool)
            .await
            {
                Ok(rows) => rows,
                Err(e) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!(CommandResponse {
                            name: body.name,
                            status: "error".to_string(),
                            result: None,
                            error: Some(format!("database error: {e}")),
                        })),
                    )
                        .into_response();
                }
            };

            if root_folders.is_empty() {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!(CommandResponse {
                        name: body.name,
                        status: "error".to_string(),
                        result: None,
                        error: Some("no root folders configured".to_string()),
                    })),
                )
                    .into_response();
            }

            // Aggregate results from all root folders
            let mut total = stackarr_import::DiskScanResult {
                files_found: 0,
                files_matched: 0,
                files_unmatched: 0,
                files_already_tracked: 0,
                unmatched_files: Vec::new(),
            };
            let mut errors = Vec::new();

            for (path, media_type) in &root_folders {
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
                        errors.push(format!("scan of '{}' failed: {e}", path));
                    }
                }
            }

            let error_msg = if errors.is_empty() {
                None
            } else {
                Some(errors.join("; "))
            };

            Json(json!(CommandResponse {
                name: body.name,
                status: if error_msg.is_some() { "completedWithErrors".to_string() } else { "completed".to_string() },
                result: Some(serde_json::to_value(total).unwrap_or_default()),
                error: error_msg,
            }))
            .into_response()
        }
        "RefreshSeries" => {
            if let Some(series_id) = body.series_id {
                // Mark a specific series as needing refresh by updating last_info_sync
                match sqlx::query(
                    "UPDATE series SET last_info_sync = NOW() WHERE id = $1",
                )
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
                    Err(e) => (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!(CommandResponse {
                            name: body.name,
                            status: "error".to_string(),
                            result: None,
                            error: Some(format!("database error: {e}")),
                        })),
                    )
                        .into_response(),
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
                    Err(e) => (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!(CommandResponse {
                            name: body.name,
                            status: "error".to_string(),
                            result: None,
                            error: Some(format!("database error: {e}")),
                        })),
                    )
                        .into_response(),
                }
            }
        }
        "RefreshMovie" => {
            if let Some(movie_id) = body.movie_id {
                match sqlx::query(
                    "UPDATE movies SET last_info_sync = NOW() WHERE id = $1",
                )
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
                    Err(e) => (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!(CommandResponse {
                            name: body.name,
                            status: "error".to_string(),
                            result: None,
                            error: Some(format!("database error: {e}")),
                        })),
                    )
                        .into_response(),
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
                    Err(e) => (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!(CommandResponse {
                            name: body.name,
                            status: "error".to_string(),
                            result: None,
                            error: Some(format!("database error: {e}")),
                        })),
                    )
                        .into_response(),
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
                (Err(e), _) | (_, Err(e)) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!(CommandResponse {
                        name: body.name,
                        status: "error".to_string(),
                        result: None,
                        error: Some(format!("database error: {e}")),
                    })),
                )
                    .into_response(),
            }
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

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/system/status", get(get_status))
        .route("/api/v1/setup/init", post(init_setup))
        .route("/api/v1/system/migrate", post(post_migrate))
        .route("/api/v1/command", post(post_command))
}
