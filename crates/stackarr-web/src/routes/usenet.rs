use std::io::Read as _;
use std::sync::Arc;

use axum::extract::{Multipart, Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{error, info};

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
    retention: Option<u32>,
    pipelining: Option<u8>,
    proxy_url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SpeedLimitRequest {
    /// Speed limit in bytes per second (0 = unlimited).
    bytes_per_second: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PauseDurationRequest {
    /// Optional pause duration in seconds. If absent or 0, pause indefinitely.
    #[serde(default)]
    duration_secs: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UsenetSettingsResponse {
    max_active_downloads: usize,
    speed_limit: u64,
    history_retention: Option<usize>,
    incomplete_dir: String,
    complete_dir: String,
    /// Backup-server probe policy. `probeEnabled=false` disables probing and
    /// reverts to cascade-everything routing. Changing probe fields requires
    /// an engine restart to take effect (the current downloader instance
    /// reads the policy only at startup).
    probe_enabled: bool,
    probe_count: u32,
    probe_min_hit_rate_pct: f32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateUsenetSettingsRequest {
    max_active_downloads: Option<usize>,
    speed_limit: Option<u64>,
    history_retention: Option<Option<usize>>,
    incomplete_dir: Option<String>,
    complete_dir: Option<String>,
    probe_enabled: Option<bool>,
    probe_count: Option<u32>,
    probe_min_hit_rate_pct: Option<f32>,
}

// ---------------------------------------------------------------------------
// DB row for download_clients
// ---------------------------------------------------------------------------

#[derive(Debug, sqlx::FromRow)]
struct DownloadClientRow {
    id: i32,
    name: String,
    #[allow(dead_code)]
    client_type: String,
    #[allow(dead_code)]
    protocol: String,
    config: serde_json::Value,
    enabled: bool,
    priority: i32,
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

fn nzb_error_response(e: nzb_web::nzb_core::NzbError) -> impl IntoResponse {
    let status = match &e {
        nzb_web::nzb_core::NzbError::JobNotFound(_)
        | nzb_web::nzb_core::NzbError::ServerNotFound(_) => StatusCode::NOT_FOUND,
        _ => StatusCode::BAD_REQUEST,
    };
    (status, Json(json!({ "error": e.to_string() })))
}

/// Query all `download_clients` with `client_type = 'embedded_usenet'`,
/// deserialize each config as `ServerConfig`, and push them into the
/// running nzb engine via `update_servers()`.
async fn refresh_engine_servers(
    state: &AppState,
) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    let pool = state.db.pool();

    let rows = sqlx::query_as::<_, DownloadClientRow>(
        "SELECT id, name, client_type, protocol, config, enabled, priority
         FROM download_clients
         WHERE client_type = 'embedded_usenet'
         ORDER BY priority ASC",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| {
        error!("Failed to query usenet servers from DB: {e}");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("database error: {e}") })),
        )
    })?;

    let mut servers = Vec::with_capacity(rows.len());
    for row in &rows {
        match serde_json::from_value::<nzb_web::nzb_core::config::ServerConfig>(row.config.clone())
        {
            Ok(mut sc) => {
                // Override enabled/priority from the DB columns
                sc.enabled = row.enabled;
                sc.priority = row.priority as u8;
                servers.push(sc);
            }
            Err(e) => {
                error!(
                    id = row.id,
                    name = %row.name,
                    "Failed to deserialize usenet server config: {e}"
                );
            }
        }
    }

    let uq = state.usenet_queue.load_full();
    if let Some(qm) = &uq {
        qm.update_servers(servers);
        info!(
            "Refreshed usenet engine with {} servers from DB",
            rows.len()
        );
    }

    // Rebuild DAV streaming pools if the module is active
    let dav = state.dav_manager.load();
    if let Some(dav) = dav.as_ref() {
        let dav_pools = crate::dav_manager::build_dav_pools(state.db.pool()).await;
        dav.provider.replace_pools(dav_pools);
        info!("Refreshed DAV streaming pools from DB");
    }

    Ok(())
}

/// Build a `ServerConfig` from a request, filling defaults for missing fields.
fn server_config_from_request(req: &NntpServerRequest) -> nzb_web::nzb_core::config::ServerConfig {
    let host = req.host.clone().unwrap_or_default();
    let name = req.name.clone().unwrap_or_else(|| host.clone());
    let mut c = nzb_web::nzb_core::config::ServerConfig::default();
    c.name = name;
    c.host = host;
    c.port = req.port.unwrap_or(563);
    c.ssl = req.ssl.unwrap_or(true);
    c.username = req.username.clone();
    c.password = req.password.clone();
    c.connections = req.connections.unwrap_or(8) as u16;
    c.priority = req.priority.unwrap_or(0) as u8;
    c.enabled = req.enabled.unwrap_or(true);
    c.retention = req.retention.unwrap_or(0);
    c.pipelining = req.pipelining.unwrap_or(15);
    c.recv_buffer_size = 0;
    c.proxy_url = req.proxy_url.clone();
    c
}

/// Merge an `NntpServerRequest` (partial update) on top of an existing `ServerConfig`.
fn merge_server_config(
    existing: &mut nzb_web::nzb_core::config::ServerConfig,
    req: &NntpServerRequest,
) {
    if let Some(name) = &req.name {
        existing.name = name.clone();
    }
    if let Some(host) = &req.host {
        existing.host = host.clone();
    }
    if let Some(port) = req.port {
        existing.port = port;
    }
    if let Some(ssl) = req.ssl {
        existing.ssl = ssl;
    }
    if let Some(username) = &req.username {
        existing.username = Some(username.clone());
    }
    if let Some(password) = &req.password {
        // Skip empty and the mask sentinel — both mean "keep existing password".
        if !password.is_empty() && password != "********" {
            existing.password = Some(password.clone());
        }
    }
    if let Some(connections) = req.connections {
        existing.connections = connections as u16;
    }
    if let Some(priority) = req.priority {
        existing.priority = priority as u8;
    }
    if let Some(enabled) = req.enabled {
        existing.enabled = enabled;
    }
    if let Some(retention) = req.retention {
        existing.retention = retention;
    }
    if let Some(pipelining) = req.pipelining {
        existing.pipelining = pipelining;
    }
    if req.proxy_url.is_some() {
        existing.proxy_url = req.proxy_url.clone();
    }
}

/// Serialize a `DownloadClientRow` to the JSON shape the API returns.
fn server_row_to_json(row: &DownloadClientRow) -> serde_json::Value {
    let mut server = row.config.clone();
    // Inject the DB id so the frontend can reference it for CRUD
    if let Some(obj) = server.as_object_mut() {
        obj.insert("dbId".to_string(), json!(row.id));
        // Ensure enabled/priority reflect the DB columns
        obj.insert("enabled".to_string(), json!(row.enabled));
        obj.insert("priority".to_string(), json!(row.priority));
        // Mask the password in the response
        if obj.contains_key("password") {
            obj.insert("password".to_string(), json!("********"));
        }
    }
    server
}

// ---------------------------------------------------------------------------
// Status / queue handlers
// ---------------------------------------------------------------------------

/// GET /api/v1/usenet/status
async fn usenet_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let uq = state.usenet_queue.load_full();
    match &uq {
        Some(qm) => Json(json!({
            "enabled": true,
            "downloadSpeed": qm.get_speed(),
            "queueSize": qm.queue_size(),
            "activeDownloads": qm.get_jobs().iter().filter(|j| j.status == nzb_web::nzb_core::models::JobStatus::Downloading).count(),
            "paused": qm.is_paused(),
            "maxActiveDownloads": qm.get_max_active_downloads(),
            "speedLimit": qm.get_speed_limit(),
            "pauseRemainingSecs": qm.pause_remaining_secs(),
        }))
        .into_response(),
        None => Json(json!({
            "enabled": false,
            "downloadSpeed": 0,
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
    let uq = state.usenet_queue.load_full();
    match &uq {
        Some(qm) => {
            let jobs = qm.get_jobs();
            // Transform to match UI's expected field names
            let items: Vec<serde_json::Value> = jobs
                .into_iter()
                .map(|j| {
                    let total = j.total_bytes as f64;
                    let downloaded = j.downloaded_bytes as f64;
                    let progress = if total > 0.0 {
                        ((downloaded / total) * 100.0).min(100.0)
                    } else {
                        0.0
                    };
                    let speed = j.speed_bps;
                    let remaining = (total - downloaded).max(0.0);
                    let eta = if speed > 0 {
                        (remaining / speed as f64) as u64
                    } else {
                        0
                    };
                    json!({
                        "id": j.id,
                        "name": j.name,
                        "size": j.total_bytes,
                        "progress": progress,
                        "speed": speed,
                        "status": j.status,
                        "eta": eta,
                        "errorMessage": j.error_message,
                        "category": j.category,
                        "priority": j.priority,
                        "totalArticles": j.article_count,
                        "downloadedArticles": j.articles_downloaded,
                    })
                })
                .collect();
            Json(json!({ "jobs": items })).into_response()
        }
        None => Json(json!({ "jobs": [] })).into_response(),
    }
}

/// POST /api/v1/usenet/add — accepts JSON (url-based) requests.
async fn usenet_add(
    State(state): State<Arc<AppState>>,
    Json(body): Json<AddNzbRequest>,
) -> impl IntoResponse {
    let uq = state.usenet_queue.load_full();
    let Some(qm) = &uq else {
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
        Ok(resp) => {
            if !resp.status().is_success() {
                return (
                    StatusCode::BAD_GATEWAY,
                    Json(json!({ "error": format!("indexer returned HTTP {}", resp.status()) })),
                )
                    .into_response();
            }
            match resp.bytes().await {
                Ok(b) => {
                    let raw = b.to_vec();
                    // Decompress gzip if needed
                    if raw.len() >= 2 && raw[0] == 0x1f && raw[1] == 0x8b {
                        let mut decoder = GzDecoder::new(&raw[..]);
                        let mut decompressed = Vec::new();
                        match decoder.read_to_end(&mut decompressed) {
                            Ok(_) => decompressed,
                            Err(e) => {
                                return (
                                    StatusCode::BAD_REQUEST,
                                    Json(json!({ "error": format!("failed to decompress gzip NZB: {e}") })),
                                )
                                    .into_response();
                            }
                        }
                    } else {
                        raw
                    }
                }
                Err(e) => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(json!({ "error": format!("failed to read NZB data: {e}") })),
                    )
                        .into_response();
                }
            }
        }
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": format!("failed to fetch NZB: {e}") })),
            )
                .into_response();
        }
    };

    let name = body.name.unwrap_or_else(|| "download".to_string());
    let mut job = match nzb_web::nzb_core::nzb_parser::parse_nzb(&name, &nzb_bytes) {
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

/// POST /api/v1/usenet/add/upload — accepts multipart file uploads.
async fn usenet_add_upload(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let uq = state.usenet_queue.load_full();
    let Some(qm) = &uq else {
        return engine_not_initialized().into_response();
    };

    let mut nzb_bytes: Option<Vec<u8>> = None;
    let mut file_name: Option<String> = None;
    let mut category: Option<String> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "file" => {
                file_name = field.file_name().map(|s| s.to_string());
                match field.bytes().await {
                    Ok(b) => nzb_bytes = Some(b.to_vec()),
                    Err(e) => {
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(json!({ "error": format!("failed to read uploaded file: {e}") })),
                        )
                            .into_response();
                    }
                }
            }
            "category" => {
                if let Ok(val) = field.text().await
                    && !val.is_empty()
                {
                    category = Some(val);
                }
            }
            _ => { /* ignore unknown fields like priority for now */ }
        }
    }

    let nzb_bytes = match nzb_bytes {
        Some(b) if !b.is_empty() => b,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "no NZB file uploaded" })),
            )
                .into_response();
        }
    };

    let name = file_name
        .as_deref()
        .unwrap_or("upload")
        .trim_end_matches(".nzb")
        .to_string();

    let mut job = match nzb_web::nzb_core::nzb_parser::parse_nzb(&name, &nzb_bytes) {
        Ok(j) => j,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": format!("failed to parse NZB: {e}") })),
            )
                .into_response();
        }
    };

    job.work_dir = qm.incomplete_dir().join(&job.id);
    job.output_dir = qm.complete_dir().join(&job.name);
    if let Some(cat) = category {
        job.category = cat;
    }

    match qm.add_job(job, Some(nzb_bytes)) {
        Ok(()) => Json(json!({ "success": true })).into_response(),
        Err(e) => nzb_error_response(e).into_response(),
    }
}

/// GET /api/v1/usenet/queue/{id} — single job detail with files
async fn usenet_queue_detail(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let uq = state.usenet_queue.load_full();
    let Some(qm) = &uq else {
        return engine_not_initialized().into_response();
    };

    match qm.get_job(&id) {
        Some(j) => {
            let total = j.total_bytes as f64;
            let downloaded = j.downloaded_bytes as f64;
            let progress = if total > 0.0 {
                ((downloaded / total) * 100.0).min(100.0)
            } else {
                0.0
            };
            let speed = j.speed_bps;
            let remaining = (total - downloaded).max(0.0);
            let eta = if speed > 0 {
                (remaining / speed as f64) as u64
            } else {
                0
            };
            let files: Vec<serde_json::Value> = j
                .files
                .iter()
                .map(|f| {
                    let status = if f.assembled {
                        "completed"
                    } else if f.bytes_downloaded > 0 {
                        "downloading"
                    } else {
                        "queued"
                    };
                    json!({
                        "name": f.filename,
                        "size": f.bytes,
                        "status": status,
                    })
                })
                .collect();
            let mut logs: Vec<String> = qm
                .get_job_logs(&id, 500)
                .iter()
                .map(|e| {
                    format!(
                        "[{}] {} {}",
                        e.timestamp.format("%H:%M:%S"),
                        e.level,
                        e.message
                    )
                })
                .collect();
            let import_lines = import_log_lines_for_download(state.db.pool(), &id).await;
            if !import_lines.is_empty() {
                logs.push(
                    "── Import ─────────────────────────────────────────────────────".to_string(),
                );
                logs.extend(import_lines);
            }
            Json(json!({
                "id": j.id,
                "name": j.name,
                "size": j.total_bytes,
                "progress": progress,
                "speed": speed,
                "status": j.status,
                "eta": eta,
                "errorMessage": j.error_message,
                "category": j.category,
                "priority": j.priority,
                "totalArticles": j.article_count,
                "downloadedArticles": j.articles_downloaded,
                "files": files,
                "logs": logs,
            }))
            .into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": format!("job not found: {id}") })),
        )
            .into_response(),
    }
}

/// POST /api/v1/usenet/queue/{id}/pause
async fn usenet_queue_pause(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let uq = state.usenet_queue.load_full();
    let Some(qm) = &uq else {
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
    let uq = state.usenet_queue.load_full();
    let Some(qm) = &uq else {
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
    let uq = state.usenet_queue.load_full();
    let Some(qm) = &uq else {
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
    let uq = state.usenet_queue.load_full();
    match &uq {
        Some(qm) => match qm.history_list(500) {
            Ok(records) => {
                // Transform to camelCase field names matching the frontend HistoryItem interface
                let items: Vec<serde_json::Value> = records
                    .into_iter()
                    .map(|r| {
                        // Extract stage statuses for the UI
                        let par2_status = r
                            .stages
                            .iter()
                            .find(|s| s.name == "par2_verify")
                            .map(|s| &s.status);
                        let repair_status = r
                            .stages
                            .iter()
                            .find(|s| s.name == "par2_repair")
                            .map(|s| &s.status);
                        let extract_status = r
                            .stages
                            .iter()
                            .find(|s| s.name == "extract")
                            .map(|s| &s.status);
                        json!({
                            "id": r.id,
                            "name": r.name,
                            "size": r.total_bytes,
                            "status": r.status,
                            "completedAt": r.completed_at.to_rfc3339(),
                            "addedAt": r.added_at.to_rfc3339(),
                            "par2Status": par2_status,
                            "repairStatus": repair_status,
                            "extractStatus": extract_status,
                            "errorMessage": r.error_message,
                            "serverStats": r.server_stats,
                        })
                    })
                    .collect();
                Json(json!({ "records": items })).into_response()
            }
            Err(e) => nzb_error_response(e).into_response(),
        },
        None => Json(json!({ "records": [] })).into_response(),
    }
}

/// GET /api/v1/usenet/history/{id}
async fn usenet_history_detail(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let uq = state.usenet_queue.load_full();
    let Some(qm) = &uq else {
        return engine_not_initialized().into_response();
    };

    let entry = match qm.history_get(&id) {
        Ok(Some(e)) => e,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": format!("history entry not found: {id}") })),
            )
                .into_response();
        }
        Err(e) => return nzb_error_response(e).into_response(),
    };

    let mut logs: Vec<String> = match qm.history_get_logs(&id) {
        Ok(Some(text)) => text.lines().map(String::from).collect(),
        Ok(None) => Vec::new(),
        Err(e) => {
            error!("Failed to get history logs for {id}: {e}");
            Vec::new()
        }
    };
    // Append import log lines stored in the stackarr history record for this download
    let import_lines = import_log_lines_for_download(state.db.pool(), &id).await;
    if !import_lines.is_empty() {
        logs.push("── Import ─────────────────────────────────────────────────────".to_string());
        logs.extend(import_lines);
    }

    let par2_status = entry
        .stages
        .iter()
        .find(|s| s.name == "par2_verify")
        .map(|s| &s.status);
    let repair_status = entry
        .stages
        .iter()
        .find(|s| s.name == "par2_repair")
        .map(|s| &s.status);
    let extract_status = entry
        .stages
        .iter()
        .find(|s| s.name == "extract")
        .map(|s| &s.status);

    Json(json!({
        "id": entry.id,
        "name": entry.name,
        "size": entry.total_bytes,
        "status": entry.status,
        "completedAt": entry.completed_at.to_rfc3339(),
        "addedAt": entry.added_at.to_rfc3339(),
        "par2Status": par2_status,
        "repairStatus": repair_status,
        "extractStatus": extract_status,
        "errorMessage": entry.error_message,
        "serverStats": entry.server_stats,
        "files": [],
        "logs": logs,
    }))
    .into_response()
}

/// POST /api/v1/usenet/history/{id}/retry
async fn usenet_history_retry(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let uq = state.usenet_queue.load_full();
    let Some(qm) = &uq else {
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
    let mut job = match nzb_web::nzb_core::nzb_parser::parse_nzb(&entry.name, &nzb_data) {
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
// Servers are persisted in the `download_clients` table and hot-reloaded
// into the running nzb engine after every mutation.
// ---------------------------------------------------------------------------

/// GET /api/v1/usenet/servers
async fn usenet_servers_list(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let pool = state.db.pool();

    let rows = match sqlx::query_as::<_, DownloadClientRow>(
        "SELECT id, name, client_type, protocol, config, enabled, priority
         FROM download_clients
         WHERE client_type = 'embedded_usenet'
         ORDER BY priority ASC",
    )
    .fetch_all(pool)
    .await
    {
        Ok(rows) => rows,
        Err(e) => {
            error!("Failed to list usenet servers: {e}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("database error: {e}") })),
            )
                .into_response();
        }
    };

    let servers: Vec<serde_json::Value> = rows.iter().map(server_row_to_json).collect();
    Json(json!({ "servers": servers })).into_response()
}

/// POST /api/v1/usenet/servers
async fn usenet_servers_add(
    State(state): State<Arc<AppState>>,
    Json(body): Json<NntpServerRequest>,
) -> impl IntoResponse {
    // Require at least a host
    let host = match &body.host {
        Some(h) if !h.is_empty() => h.clone(),
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "'host' is required" })),
            )
                .into_response();
        }
    };

    let server_config = server_config_from_request(&body);
    let config_json = match serde_json::to_value(&server_config) {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("failed to serialize config: {e}") })),
            )
                .into_response();
        }
    };

    let display_name = body.name.clone().unwrap_or_else(|| host.clone());
    let enabled = body.enabled.unwrap_or(true);
    let priority = body.priority.unwrap_or(0);

    let pool = state.db.pool();
    let row = match sqlx::query_as::<_, DownloadClientRow>(
        "INSERT INTO download_clients (name, client_type, protocol, config, enabled, priority)
         VALUES ($1, 'embedded_usenet', 'usenet', $2, $3, $4)
         RETURNING id, name, client_type, protocol, config, enabled, priority",
    )
    .bind(&display_name)
    .bind(&config_json)
    .bind(enabled)
    .bind(priority)
    .fetch_one(pool)
    .await
    {
        Ok(row) => row,
        Err(e) => {
            error!("Failed to insert usenet server: {e}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("database error: {e}") })),
            )
                .into_response();
        }
    };

    // Start the engine if it wasn't running, otherwise refresh server list
    if state.usenet_queue.load().is_none() {
        info!("usenet engine not running — starting now with new server");
        state.init_usenet_engine().await;
    } else if let Err(resp) = refresh_engine_servers(&state).await {
        return resp.into_response();
    }

    info!(id = row.id, name = %display_name, "Added usenet server");
    (StatusCode::CREATED, Json(server_row_to_json(&row))).into_response()
}

/// PUT /api/v1/usenet/servers/{id}
async fn usenet_servers_update(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
    Json(body): Json<NntpServerRequest>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    // Fetch existing row
    let existing = match sqlx::query_as::<_, DownloadClientRow>(
        "SELECT id, name, client_type, protocol, config, enabled, priority
         FROM download_clients
         WHERE id = $1 AND client_type = 'embedded_usenet'",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(row)) => row,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": format!("usenet server {id} not found") })),
            )
                .into_response();
        }
        Err(e) => {
            error!("Failed to fetch usenet server {id}: {e}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("database error: {e}") })),
            )
                .into_response();
        }
    };

    // Merge request fields into existing config
    let mut server_config = match serde_json::from_value::<nzb_web::nzb_core::config::ServerConfig>(
        existing.config.clone(),
    ) {
        Ok(sc) => sc,
        Err(e) => {
            error!("Failed to deserialize existing server config for {id}: {e}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("corrupted server config: {e}") })),
            )
                .into_response();
        }
    };

    merge_server_config(&mut server_config, &body);

    let config_json = match serde_json::to_value(&server_config) {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("failed to serialize config: {e}") })),
            )
                .into_response();
        }
    };

    let display_name = body.name.clone().unwrap_or(existing.name);
    let enabled = body.enabled.unwrap_or(existing.enabled);
    let priority = body.priority.unwrap_or(existing.priority);

    let row = match sqlx::query_as::<_, DownloadClientRow>(
        "UPDATE download_clients
         SET name = $1, config = $2, enabled = $3, priority = $4
         WHERE id = $5
         RETURNING id, name, client_type, protocol, config, enabled, priority",
    )
    .bind(&display_name)
    .bind(&config_json)
    .bind(enabled)
    .bind(priority)
    .bind(id)
    .fetch_one(pool)
    .await
    {
        Ok(row) => row,
        Err(e) => {
            error!("Failed to update usenet server {id}: {e}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("database error: {e}") })),
            )
                .into_response();
        }
    };

    // Refresh the nzb engine
    if let Err(resp) = refresh_engine_servers(&state).await {
        return resp.into_response();
    }

    info!(id, name = %display_name, "Updated usenet server");
    Json(server_row_to_json(&row)).into_response()
}

/// DELETE /api/v1/usenet/servers/{id}
async fn usenet_servers_delete(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    let result = match sqlx::query(
        "DELETE FROM download_clients
         WHERE id = $1 AND client_type = 'embedded_usenet'",
    )
    .bind(id)
    .execute(pool)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to delete usenet server {id}: {e}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("database error: {e}") })),
            )
                .into_response();
        }
    };

    if result.rows_affected() == 0 {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": format!("usenet server {id} not found") })),
        )
            .into_response();
    }

    // Refresh the nzb engine
    if let Err(resp) = refresh_engine_servers(&state).await {
        return resp.into_response();
    }

    info!(id, "Deleted usenet server");
    StatusCode::NO_CONTENT.into_response()
}

/// POST /api/v1/usenet/servers/test — test from request body (before save)
async fn usenet_servers_test_body(Json(body): Json<NntpServerRequest>) -> impl IntoResponse {
    let host = match body.host {
        Some(ref h) if !h.is_empty() => h.clone(),
        _ => {
            return Json(json!({ "success": false, "message": "host is required" }))
                .into_response();
        }
    };
    let port = body.port.unwrap_or(563);
    let ssl = body.ssl.unwrap_or(true);

    let mut server_config = nzb_web::nzb_core::config::ServerConfig::new("test", &host);
    server_config.name = body.name.unwrap_or_default();
    server_config.port = port;
    server_config.ssl = ssl;
    server_config.ssl_verify = ssl;
    server_config.username = body.username;
    server_config.password = body.password;
    server_config.connections = 1;
    server_config.ramp_up_delay_ms = 0;
    server_config.recv_buffer_size = 0;
    server_config.proxy_url = body.proxy_url;

    let mut conn = nzb_web::nzb_core::nzb_nntp::NntpConnection::new("test".to_string());
    let test_result = tokio::time::timeout(
        std::time::Duration::from_secs(15),
        conn.connect(&server_config),
    )
    .await;

    match test_result {
        Ok(Ok(())) => {
            let _ = conn.quit().await;
            Json(json!({ "success": true, "message": format!("Successfully connected to {host}") })).into_response()
        }
        Ok(Err(e)) => {
            Json(json!({ "success": false, "message": e.to_string() })).into_response()
        }
        Err(_) => {
            Json(json!({ "success": false, "message": format!("Connection to {host} timed out after 15 seconds") })).into_response()
        }
    }
}

/// POST /api/v1/usenet/servers/{id}/test
///
/// Accepts an optional JSON body with overrides (e.g. updated password from
/// the edit form). Fields present in the body are merged over the DB config
/// so the user can test unsaved changes before committing them.
async fn usenet_servers_test(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
    body: Option<Json<NntpServerRequest>>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    // Fetch the server config from DB
    let row = match sqlx::query_as::<_, DownloadClientRow>(
        "SELECT id, name, client_type, protocol, config, enabled, priority
         FROM download_clients
         WHERE id = $1 AND client_type = 'embedded_usenet'",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(row)) => row,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": format!("usenet server {id} not found") })),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("database error: {e}") })),
            )
                .into_response();
        }
    };

    let mut server_config =
        match serde_json::from_value::<nzb_web::nzb_core::config::ServerConfig>(row.config.clone())
        {
            Ok(sc) => sc,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": format!("corrupted server config: {e}") })),
                )
                    .into_response();
            }
        };

    // Merge any overrides from the request body (e.g. updated password)
    if let Some(Json(overrides)) = body {
        merge_server_config(&mut server_config, &overrides);
    }

    // Test connectivity by creating an NNTP connection, authenticating, then quitting
    let mut conn = nzb_web::nzb_core::nzb_nntp::NntpConnection::new(server_config.id.clone());

    let test_result = tokio::time::timeout(
        std::time::Duration::from_secs(15),
        conn.connect(&server_config),
    )
    .await;

    match test_result {
        Ok(Ok(())) => {
            // Connection and auth succeeded — send QUIT
            let _ = conn.quit().await;
            Json(json!({
                "success": true,
                "message": format!("Successfully connected to {}", server_config.host)
            }))
            .into_response()
        }
        Ok(Err(e)) => {
            let msg = e.to_string();
            Json(json!({
                "success": false,
                "message": msg
            }))
            .into_response()
        }
        Err(_) => Json(json!({
            "success": false,
            "message": format!("Connection to {} timed out after 15 seconds", server_config.host)
        }))
        .into_response(),
    }
}

// ---------------------------------------------------------------------------
// Queue-wide control handlers
// ---------------------------------------------------------------------------

/// POST /api/v1/usenet/pause-all
async fn usenet_pause_all(
    State(state): State<Arc<AppState>>,
    body: Option<Json<PauseDurationRequest>>,
) -> impl IntoResponse {
    let uq = state.usenet_queue.load_full();
    let Some(qm) = &uq else {
        return engine_not_initialized().into_response();
    };

    let duration_secs = body.and_then(|b| b.duration_secs).unwrap_or(0);
    if duration_secs > 0 {
        qm.pause_for(duration_secs);
        info!(duration_secs, "Paused usenet queue for duration");
        Json(json!({ "success": true, "message": format!("Paused for {duration_secs} seconds") }))
            .into_response()
    } else {
        qm.pause_all();
        info!("Paused usenet queue indefinitely");
        Json(json!({ "success": true, "message": "Paused indefinitely" })).into_response()
    }
}

/// POST /api/v1/usenet/resume-all
async fn usenet_resume_all(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let uq = state.usenet_queue.load_full();
    let Some(qm) = &uq else {
        return engine_not_initialized().into_response();
    };

    qm.resume_all();
    info!("Resumed usenet queue");
    Json(json!({ "success": true, "message": "Resumed" })).into_response()
}

/// POST /api/v1/usenet/speed-limit
async fn usenet_speed_limit(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SpeedLimitRequest>,
) -> impl IntoResponse {
    let uq = state.usenet_queue.load_full();
    let Some(qm) = &uq else {
        return engine_not_initialized().into_response();
    };

    qm.set_speed_limit(body.bytes_per_second);
    info!(bps = body.bytes_per_second, "Set usenet speed limit");
    Json(json!({
        "success": true,
        "speedLimit": body.bytes_per_second
    }))
    .into_response()
}

// ---------------------------------------------------------------------------
// Settings handlers
// ---------------------------------------------------------------------------

/// Read the current probe policy from `app_config`, falling back to the TOML
/// config on absence. Kept in sync with the loader in `state.rs` so the
/// settings API always reports the same values the engine would pick up on
/// the next restart.
async fn load_probe_policy(state: &AppState) -> (bool, u32, f32) {
    let cfg = state.config.load();
    let mut enabled = cfg.usenet.probe.enabled;
    let mut count = cfg.usenet.probe.probe_count;
    let mut rate = cfg.usenet.probe.min_hit_rate_pct;

    if let Ok(Some(v)) = sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT value FROM app_config WHERE key = 'usenet_probe_enabled'",
    )
    .fetch_optional(state.db.pool())
    .await
        && let Some(b) = v.as_bool()
    {
        enabled = b;
    }
    if let Ok(Some(v)) = sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT value FROM app_config WHERE key = 'usenet_probe_count'",
    )
    .fetch_optional(state.db.pool())
    .await
        && let Some(n) = v.as_u64()
    {
        count = n as u32;
    }
    if let Ok(Some(v)) = sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT value FROM app_config WHERE key = 'usenet_probe_min_hit_rate_pct'",
    )
    .fetch_optional(state.db.pool())
    .await
        && let Some(f) = v.as_f64()
    {
        rate = f as f32;
    }
    (enabled, count, rate)
}

/// GET /api/v1/usenet/settings
async fn usenet_settings_get(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let uq = state.usenet_queue.load_full();
    let Some(qm) = &uq else {
        return engine_not_initialized().into_response();
    };

    let (probe_enabled, probe_count, probe_min_hit_rate_pct) = load_probe_policy(&state).await;

    Json(UsenetSettingsResponse {
        max_active_downloads: qm.get_max_active_downloads(),
        speed_limit: qm.get_speed_limit(),
        history_retention: qm.get_history_retention(),
        incomplete_dir: qm.incomplete_dir().to_string_lossy().to_string(),
        complete_dir: qm.complete_dir().to_string_lossy().to_string(),
        probe_enabled,
        probe_count,
        probe_min_hit_rate_pct,
    })
    .into_response()
}

/// PUT /api/v1/usenet/settings
async fn usenet_settings_update(
    State(state): State<Arc<AppState>>,
    Json(body): Json<UpdateUsenetSettingsRequest>,
) -> impl IntoResponse {
    let uq = state.usenet_queue.load_full();
    let Some(qm) = &uq else {
        return engine_not_initialized().into_response();
    };

    if let Some(max) = body.max_active_downloads {
        qm.set_max_active_downloads(max);
        // Persist to DB so it survives restarts
        let _ = sqlx::query(
            "INSERT INTO app_config (key, value) VALUES ('usenet_max_active_downloads', $1::jsonb) \
             ON CONFLICT (key) DO UPDATE SET value = $1::jsonb",
        )
        .bind(serde_json::json!(max))
        .execute(state.db.pool())
        .await;
    }
    if let Some(bps) = body.speed_limit {
        qm.set_speed_limit(bps);
    }
    if let Some(retention) = body.history_retention {
        qm.set_history_retention(retention);
    }
    if let Some(ref dir) = body.incomplete_dir {
        let path = std::path::PathBuf::from(dir);
        let _ = tokio::fs::create_dir_all(&path).await;
        qm.set_incomplete_dir(path);
        let _ = sqlx::query(
            "INSERT INTO app_config (key, value) VALUES ('usenet_incomplete_dir', $1::jsonb) \
             ON CONFLICT (key) DO UPDATE SET value = $1::jsonb",
        )
        .bind(serde_json::json!(dir))
        .execute(state.db.pool())
        .await;
    }
    if let Some(ref dir) = body.complete_dir {
        let path = std::path::PathBuf::from(dir);
        let _ = tokio::fs::create_dir_all(&path).await;
        qm.set_complete_dir(path);
        let _ = sqlx::query(
            "INSERT INTO app_config (key, value) VALUES ('usenet_complete_dir', $1::jsonb) \
             ON CONFLICT (key) DO UPDATE SET value = $1::jsonb",
        )
        .bind(serde_json::json!(dir))
        .execute(state.db.pool())
        .await;
    }

    // Probe policy — persisted to DB only, applied on next engine start.
    if let Some(enabled) = body.probe_enabled {
        let _ = sqlx::query(
            "INSERT INTO app_config (key, value) VALUES ('usenet_probe_enabled', $1::jsonb) \
             ON CONFLICT (key) DO UPDATE SET value = $1::jsonb",
        )
        .bind(serde_json::json!(enabled))
        .execute(state.db.pool())
        .await;
    }
    if let Some(n) = body.probe_count {
        let _ = sqlx::query(
            "INSERT INTO app_config (key, value) VALUES ('usenet_probe_count', $1::jsonb) \
             ON CONFLICT (key) DO UPDATE SET value = $1::jsonb",
        )
        .bind(serde_json::json!(n))
        .execute(state.db.pool())
        .await;
    }
    if let Some(r) = body.probe_min_hit_rate_pct {
        // Clamp to 0..=100 to avoid storing nonsense.
        let r = r.clamp(0.0, 100.0);
        let _ = sqlx::query(
            "INSERT INTO app_config (key, value) VALUES ('usenet_probe_min_hit_rate_pct', $1::jsonb) \
             ON CONFLICT (key) DO UPDATE SET value = $1::jsonb",
        )
        .bind(serde_json::json!(r))
        .execute(state.db.pool())
        .await;
    }

    info!("Updated usenet engine settings");
    let (probe_enabled, probe_count, probe_min_hit_rate_pct) = load_probe_policy(&state).await;
    Json(UsenetSettingsResponse {
        max_active_downloads: qm.get_max_active_downloads(),
        speed_limit: qm.get_speed_limit(),
        history_retention: qm.get_history_retention(),
        incomplete_dir: qm.incomplete_dir().to_string_lossy().to_string(),
        complete_dir: qm.complete_dir().to_string_lossy().to_string(),
        probe_enabled,
        probe_count,
        probe_min_hit_rate_pct,
    })
    .into_response()
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

    let preview = nzb_web::nzb_core::sabnzbd_import::parse_sabnzbd_ini(&content);
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

    let preview = nzb_web::nzb_core::sabnzbd_import::parse_sabnzbd_api_response(&json_val);
    Json(json!(preview)).into_response()
}

/// POST /api/v1/usenet/import-sabnzbd/apply — apply a previewed import to the usenet engine
async fn import_sabnzbd_apply(
    State(state): State<Arc<AppState>>,
    Json(preview): Json<nzb_web::nzb_core::sabnzbd_import::SabnzbdImportPreview>,
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
             VALUES ($1, 'embedded_usenet', $2, $3, $4, $5)",
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
             ON CONFLICT (key) DO UPDATE SET value = $1",
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

/// Query the stackarr `history` table for `download_imported` records matching
/// the given download_id and return the import log lines stored in their `data`
/// field. Returns an empty vec if nothing is found or data is absent.
async fn import_log_lines_for_download(pool: &sqlx::PgPool, download_id: &str) -> Vec<String> {
    let rows: Vec<(Option<serde_json::Value>,)> = match sqlx::query_as(
        "SELECT data FROM history WHERE download_id = $1 AND event_type = 'download_imported' \
         ORDER BY occurred_at DESC LIMIT 1",
    )
    .bind(download_id)
    .fetch_all(pool)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, download_id, "failed to fetch import log lines from history");
            return Vec::new();
        }
    };

    rows.into_iter()
        .filter_map(|(data,)| data)
        .filter_map(|v| {
            v.get("log_lines").and_then(|l| l.as_array()).map(|arr| {
                arr.iter()
                    .filter_map(|s| s.as_str().map(String::from))
                    .collect::<Vec<_>>()
            })
        })
        .next()
        .unwrap_or_default()
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        // Status & queue
        .route("/api/v1/usenet/status", get(usenet_status))
        .route("/api/v1/usenet/queue", get(usenet_queue))
        .route("/api/v1/usenet/queue/{id}", get(usenet_queue_detail))
        .route("/api/v1/usenet/add", post(usenet_add))
        .route("/api/v1/usenet/add/upload", post(usenet_add_upload))
        .route("/api/v1/usenet/queue/{id}/pause", post(usenet_queue_pause))
        .route(
            "/api/v1/usenet/queue/{id}/resume",
            post(usenet_queue_resume),
        )
        .route(
            "/api/v1/usenet/queue/{id}/delete",
            post(usenet_queue_delete),
        )
        // Queue-wide controls
        .route("/api/v1/usenet/pause-all", post(usenet_pause_all))
        .route("/api/v1/usenet/resume-all", post(usenet_resume_all))
        .route("/api/v1/usenet/speed-limit", post(usenet_speed_limit))
        // Engine settings
        .route(
            "/api/v1/usenet/settings",
            get(usenet_settings_get).put(usenet_settings_update),
        )
        // History
        .route("/api/v1/usenet/history", get(usenet_history))
        .route("/api/v1/usenet/history/{id}", get(usenet_history_detail))
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
            "/api/v1/usenet/servers/test",
            post(usenet_servers_test_body),
        )
        .route(
            "/api/v1/usenet/servers/{id}/test",
            post(usenet_servers_test),
        )
        // SABnzbd import
        .route("/api/v1/usenet/import-sabnzbd", post(import_sabnzbd_ini))
        .route(
            "/api/v1/usenet/import-sabnzbd-api",
            post(import_sabnzbd_api),
        )
        .route(
            "/api/v1/usenet/import-sabnzbd/apply",
            post(import_sabnzbd_apply),
        )
}
