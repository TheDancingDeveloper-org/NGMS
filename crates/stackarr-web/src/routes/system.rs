use std::sync::Arc;

use axum::extract::State;
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

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/system/status", get(get_status))
        .route("/api/v1/setup/init", post(init_setup))
}
