use std::sync::Arc;

use axum::extract::{Multipart, Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::AppState;

// ---------------------------------------------------------------------------
// Request types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddNzbRequest {
    url: Option<String>,
    name: Option<String>,
    category: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NntpServerRequest {
    name: Option<String>,
    host: Option<String>,
    port: Option<u16>,
    username: Option<String>,
    password: Option<String>,
    ssl: Option<bool>,
    connections: Option<u32>,
    priority: Option<i32>,
    enabled: Option<bool>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn engine_not_initialized() -> impl IntoResponse {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({
            "error": "usenet engine not initialized"
        })),
    )
}

fn nzb_error_response(e: nzb_core::NzbError) -> impl IntoResponse {
    let status = match &e {
        nzb_core::NzbError::JobNotFound(_) | nzb_core::NzbError::ServerNotFound(_) => {
            StatusCode::NOT_FOUND
        }
        _ => StatusCode::BAD_REQUEST,
    };
    (status, Json(json!({ "error": e.to_string() })))
}

// ---------------------------------------------------------------------------
// Status / queue handlers
// ---------------------------------------------------------------------------

/// GET /api/v1/usenet/status
async fn usenet_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match &state.usenet_queue {
        Some(qm) => Json(json!({
            "enabled": true,
            "speed": qm.get_speed(),
            "queueSize": qm.queue_size(),
            "activeDownloads": qm.get_jobs().iter().filter(|j| j.status == nzb_core::models::JobStatus::Downloading).count(),
            "paused": qm.is_paused(),
            "maxActiveDownloads": qm.get_max_active_downloads(),
            "speedLimit": qm.get_speed_limit(),
            "pauseRemainingSecs": qm.pause_remaining_secs(),
        }))
        .into_response(),
        None => Json(json!({
            "enabled": false,
            "speed": 0,
            "queueSize": 0,
            "activeDownloads": 0,
            "paused": false,
            "message": "Usenet engine not initialized. Configure NNTP servers in settings."
        }))
        .into_response(),
    }
}

/// GET /api/v1/usenet/queue
async fn usenet_queue(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match &state.usenet_queue {
        Some(qm) => {
            let jobs = qm.get_jobs();
            Json(json!({ "jobs": jobs })).into_response()
        }
        None => Json(json!({ "jobs": [] })).into_response(),
    }
}

/// POST /api/v1/usenet/add
async fn usenet_add(
    State(state): State<Arc<AppState>>,
    Json(body): Json<AddNzbRequest>,
) -> impl IntoResponse {
    let Some(qm) = &state.usenet_queue else {
        return engine_not_initialized().into_response();
    };

    let url = match body.url {
        Some(u) => u,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "missing 'url' field" })),
            )
                .into_response();
        }
    };

    // Fetch the NZB data from the URL
    let nzb_bytes = match reqwest::get(&url).await {
        Ok(resp) => match resp.bytes().await {
            Ok(b) => b.to_vec(),
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({ "error": format!("failed to read NZB data: {e}") })),
                )
                    .into_response();
            }
        },
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": format!("failed to fetch NZB: {e}") })),
            )
                .into_response();
        }
    };

    let name = body.name.unwrap_or_else(|| "download".to_string());
    let mut job = match nzb_core::nzb_parser::parse_nzb(&name, &nzb_bytes) {
        Ok(j) => j,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": format!("failed to parse NZB: {e}") })),
            )
                .into_response();
        }
    };

    // Set directories
    job.work_dir = qm.incomplete_dir().join(&job.id);
    job.output_dir = qm.complete_dir().join(&job.name);
    if let Some(cat) = body.category {
        job.category = cat;
    }

    match qm.add_job(job, Some(nzb_bytes)) {
        Ok(()) => Json(json!({ "success": true })).into_response(),
        Err(e) => nzb_error_response(e).into_response(),
    }
}

/// POST /api/v1/usenet/queue/{id}/pause
async fn usenet_queue_pause(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Some(qm) = &state.usenet_queue else {
        return engine_not_initialized().into_response();
    };

    match qm.pause_job(&id) {
        Ok(()) => Json(json!({ "success": true })).into_response(),
        Err(e) => nzb_error_response(e).into_response(),
    }
}

/// POST /api/v1/usenet/queue/{id}/resume
async fn usenet_queue_resume(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Some(qm) = &state.usenet_queue else {
        return engine_not_initialized().into_response();
    };

    match qm.resume_job(&id) {
        Ok(()) => Json(json!({ "success": true })).into_response(),
        Err(e) => nzb_error_response(e).into_response(),
    }
}

/// POST /api/v1/usenet/queue/{id}/delete
async fn usenet_queue_delete(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Some(qm) = &state.usenet_queue else {
        return engine_not_initialized().into_response();
    };

    match qm.remove_job(&id) {
        Ok(()) => Json(json!({ "success": true })).into_response(),
        Err(e) => nzb_error_response(e).into_response(),
    }
}

// ---------------------------------------------------------------------------
// History handlers
// ---------------------------------------------------------------------------

/// GET /api/v1/usenet/history
async fn usenet_history(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match &state.usenet_queue {
        Some(qm) => match qm.history_list(500) {
            Ok(records) => Json(json!({ "records": records })).into_response(),
            Err(e) => nzb_error_response(e).into_response(),
        },
        None => Json(json!({ "records": [] })).into_response(),
    }
}

/// POST /api/v1/usenet/history/{id}/retry
async fn usenet_history_retry(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Some(qm) = &state.usenet_queue else {
        return engine_not_initialized().into_response();
    };

    // Get the history entry's NZB data
    let nzb_data = match qm.history_get_nzb_data(&id) {
        Ok(Some(data)) => data,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "no NZB data available for retry" })),
            )
                .into_response();
        }
        Err(e) => return nzb_error_response(e).into_response(),
    };

    // Get the entry for its name/category
    let entry = match qm.history_get(&id) {
        Ok(Some(entry)) => entry,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "history entry not found" })),
            )
                .into_response();
        }
        Err(e) => return nzb_error_response(e).into_response(),
    };

    // Re-parse and re-add
    let mut job = match nzb_core::nzb_parser::parse_nzb(&entry.name, &nzb_data) {
        Ok(j) => j,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": format!("failed to parse NZB for retry: {e}") })),
            )
                .into_response();
        }
    };

    job.work_dir = qm.incomplete_dir().join(&job.id);
    job.output_dir = qm.complete_dir().join(&job.name);
    job.category = entry.category;

    match qm.add_job(job, Some(nzb_data)) {
        Ok(()) => Json(json!({ "success": true })).into_response(),
        Err(e) => nzb_error_response(e).into_response(),
    }
}

// ---------------------------------------------------------------------------
// NNTP server management handlers
//
// These operate on StackArr's config, not the usenet engine directly.
// ---------------------------------------------------------------------------

/// GET /api/v1/usenet/servers
async fn usenet_servers_list(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let cfg = state.config.load();
    Json(json!({
        "servers": cfg.usenet.servers
    }))
}

/// POST /api/v1/usenet/servers
async fn usenet_servers_add(
    State(_state): State<Arc<AppState>>,
    Json(_body): Json<NntpServerRequest>,
) -> impl IntoResponse {
    // Server CRUD requires config persistence — not yet wired
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({ "error": "server management not yet implemented" })),
    )
}

/// PUT /api/v1/usenet/servers/{id}
async fn usenet_servers_update(
    State(_state): State<Arc<AppState>>,
    Path(_id): Path<String>,
    Json(_body): Json<NntpServerRequest>,
) -> impl IntoResponse {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({ "error": "server management not yet implemented" })),
    )
}

/// DELETE /api/v1/usenet/servers/{id}
async fn usenet_servers_delete(
    State(_state): State<Arc<AppState>>,
    Path(_id): Path<String>,
) -> impl IntoResponse {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({ "error": "server management not yet implemented" })),
    )
}

/// POST /api/v1/usenet/servers/{id}/test
async fn usenet_servers_test(
    State(_state): State<Arc<AppState>>,
    Path(_id): Path<String>,
) -> impl IntoResponse {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({ "error": "server test not yet implemented" })),
    )
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// SABnzbd import
// ---------------------------------------------------------------------------

/// POST /api/v1/usenet/import-sabnzbd — upload sabnzbd.ini file, return preview
async fn import_sabnzbd_ini(mut multipart: Multipart) -> impl IntoResponse {
    let mut ini_content: Option<String> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("file") || field.name() == Some("sabnzbd_ini") {
            match field.text().await {
                Ok(text) => ini_content = Some(text),
                Err(e) => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(json!({"error": format!("failed to read file: {e}")})),
                    )
                        .into_response();
                }
            }
        }
    }

    let content = match ini_content {
        Some(c) => c,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "no sabnzbd.ini file uploaded (field name: 'file' or 'sabnzbd_ini')"})),
            )
                .into_response();
        }
    };

    let preview = nzb_core::sabnzbd_import::parse_sabnzbd_ini(&content);
    Json(json!(preview)).into_response()
}

/// POST /api/v1/usenet/import-sabnzbd-api — fetch config from a running SABnzbd instance
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportSabnzbdApiRequest {
    url: String,
    api_key: String,
}

async fn import_sabnzbd_api(Json(body): Json<ImportSabnzbdApiRequest>) -> impl IntoResponse {
    let config_url = format!(
        "{}/sabnzbd/api?mode=get_config&output=json&apikey={}",
        body.url.trim_end_matches('/'),
        body.api_key
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_default();

    let resp = match client.get(&config_url).send().await {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": format!("failed to connect to SABnzbd: {e}")})),
            )
                .into_response();
        }
    };

    let json_val: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": format!("invalid JSON from SABnzbd: {e}")})),
            )
                .into_response();
        }
    };

    let preview = nzb_core::sabnzbd_import::parse_sabnzbd_api_response(&json_val);
    Json(json!(preview)).into_response()
}

/// POST /api/v1/usenet/import-sabnzbd/apply — apply a previewed import to the usenet engine
async fn import_sabnzbd_apply(
    State(state): State<Arc<AppState>>,
    Json(preview): Json<nzb_core::sabnzbd_import::SabnzbdImportPreview>,
) -> impl IntoResponse {
    // Check for masked passwords
    for server in &preview.servers {
        if server.password_masked {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": format!("server '{}' has a masked password — please enter it manually before applying", server.name)})),
            )
                .into_response();
        }
    }

    // Convert imported servers to nzb-core ServerConfig and store in DB
    let pool = state.db.pool();
    let mut servers_added = 0;
    let mut categories_added = 0;

    for imported in &preview.servers {
        let server_config = imported.to_server_config();
        let config_json = serde_json::to_value(&server_config).unwrap_or_default();
        let protocol = "usenet";

        let result = sqlx::query(
            "INSERT INTO download_clients (name, client_type, protocol, config, enabled, priority)
             VALUES ($1, 'embedded_usenet', $2, $3, $4, $5)"
        )
        .bind(&imported.name)
        .bind(protocol)
        .bind(&config_json)
        .bind(imported.enabled)
        .bind(imported.priority as i32)
        .execute(pool)
        .await;

        if result.is_ok() {
            servers_added += 1;
        }
    }

    // Store categories in app_config
    if !preview.categories.is_empty() {
        let cats_json = serde_json::to_value(&preview.categories).unwrap_or_default();
        let _ = sqlx::query(
            "INSERT INTO app_config (key, value) VALUES ('usenet_categories', $1)
             ON CONFLICT (key) DO UPDATE SET value = $1"
        )
        .bind(&cats_json)
        .execute(pool)
        .await;
        categories_added = preview.categories.len();
    }

    Json(json!({
        "success": true,
        "serversAdded": servers_added,
        "categoriesAdded": categories_added,
        "rssFeedsAdded": preview.rss_feeds.len(),
        "warnings": preview.warnings,
        "skippedFields": preview.skipped_fields,
    }))
    .into_response()
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        // Status & queue
        .route("/api/v1/usenet/status", get(usenet_status))
        .route("/api/v1/usenet/queue", get(usenet_queue))
        .route("/api/v1/usenet/add", post(usenet_add))
        .route("/api/v1/usenet/queue/{id}/pause", post(usenet_queue_pause))
        .route(
            "/api/v1/usenet/queue/{id}/resume",
            post(usenet_queue_resume),
        )
        .route(
            "/api/v1/usenet/queue/{id}/delete",
            post(usenet_queue_delete),
        )
        // History
        .route("/api/v1/usenet/history", get(usenet_history))
        .route(
            "/api/v1/usenet/history/{id}/retry",
            post(usenet_history_retry),
        )
        // NNTP server management
        .route(
            "/api/v1/usenet/servers",
            get(usenet_servers_list).post(usenet_servers_add),
        )
        .route(
            "/api/v1/usenet/servers/{id}",
            put(usenet_servers_update).delete(usenet_servers_delete),
        )
        .route(
            "/api/v1/usenet/servers/{id}/test",
            post(usenet_servers_test),
        )
        // SABnzbd import
        .route(
            "/api/v1/usenet/import-sabnzbd",
            post(import_sabnzbd_ini),
        )
        .route(
            "/api/v1/usenet/import-sabnzbd-api",
            post(import_sabnzbd_api),
        )
        .route(
            "/api/v1/usenet/import-sabnzbd/apply",
            post(import_sabnzbd_apply),
        )
}
