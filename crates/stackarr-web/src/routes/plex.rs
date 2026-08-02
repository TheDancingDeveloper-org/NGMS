use std::sync::Arc;

use axum::extract::{Multipart, Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use stackarr_plex::types::*;
use stackarr_plex::{PlexApi, PlexScanner, PlexTvApi};

use crate::AppState;
use crate::middleware::redact_sensitive_fields;

// ── Plex Server CRUD ───────────────────────────────────────────────────────

async fn load_server(
    pool: &sqlx::MySqlPool,
    server_id: i32,
) -> Result<Option<PlexServer>, sqlx::Error> {
    sqlx::query_as::<_, PlexServer>(
        "SELECT id, name, machine_id, ip, port, use_ssl, verify_tls, auth_token, web_app_url, webhook_secret, created_at, updated_at \
         FROM plex_servers WHERE id = ?",
    )
    .bind(server_id)
    .fetch_optional(pool)
    .await
}

async fn load_library(
    pool: &sqlx::MySqlPool,
    library_id: i32,
) -> Result<Option<PlexLibrary>, sqlx::Error> {
    sqlx::query_as::<_, PlexLibrary>(
        "SELECT id, plex_server_id, section_id, name, enabled, library_type, last_scan \
         FROM plex_libraries WHERE id = ?",
    )
    .bind(library_id)
    .fetch_optional(pool)
    .await
}

async fn list_servers(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let pool = state.db.pool();
    match sqlx::query_as::<_, PlexServer>(
        "SELECT id, name, machine_id, ip, port, use_ssl, verify_tls, auth_token, web_app_url, webhook_secret, created_at, updated_at \
         FROM plex_servers ORDER BY id",
    )
    .fetch_all(pool)
    .await
    {
        Ok(servers) => {
            let mut value = serde_json::to_value(&servers).unwrap_or_default();
            redact_sensitive_fields(&mut value);
            Json(value).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to list plex servers");
            super::api_error(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
        }
    }
}

async fn create_server(
    State(state): State<Arc<AppState>>,
    Json(input): Json<CreatePlexServerInput>,
) -> impl IntoResponse {
    let pool = state.db.pool();
    let name = input.name.unwrap_or_else(|| "Plex".to_string());
    let port = input.port.unwrap_or(32400);
    let use_ssl = input.use_ssl.unwrap_or(false);
    let verify_tls = input.verify_tls.unwrap_or(true);

    // Validate connection by fetching server info
    let api = PlexApi::with_tls_verify(&input.ip, port, use_ssl, &input.auth_token, verify_tls);
    let machine_id = match api.get_status().await {
        Ok(info) => info.machine_identifier,
        Err(e) => {
            tracing::error!(error = %e, "failed to connect to plex server during setup");
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "failed to connect to Plex server"})),
            )
                .into_response();
        }
    };

    let webhook_secret = uuid::Uuid::new_v4().to_string();

    let created = async {
        let result = sqlx::query(
        "INSERT INTO plex_servers (name, machine_id, ip, port, use_ssl, verify_tls, auth_token, web_app_url, webhook_secret) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&name)
        .bind(&machine_id)
        .bind(&input.ip)
        .bind(port)
        .bind(use_ssl)
        .bind(verify_tls)
        .bind(&input.auth_token)
        .bind(&input.web_app_url)
        .bind(&webhook_secret)
        .execute(pool)
        .await?;
        let id = i32::try_from(result.last_insert_id())
            .map_err(|error| sqlx::Error::Protocol(format!("Plex server id overflow: {error}")))?;
        load_server(pool, id)
            .await?
            .ok_or(sqlx::Error::RowNotFound)
    }
    .await;

    match created {
        Ok(server) => {
            let mut value = serde_json::to_value(&server).unwrap_or_default();
            redact_sensitive_fields(&mut value);
            (StatusCode::CREATED, Json(value)).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to insert plex server");
            super::api_error(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
        }
    }
}

async fn update_server(
    State(state): State<Arc<AppState>>,
    Path(server_id): Path<i32>,
    Json(input): Json<UpdatePlexServerInput>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    let existing = load_server(pool, server_id).await;

    let existing = match existing {
        Ok(Some(s)) => s,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            tracing::error!(error = %e, server_id, "failed to fetch plex server for update");
            return super::api_error(StatusCode::INTERNAL_SERVER_ERROR, "internal server error");
        }
    };

    let name = input.name.unwrap_or(existing.name);
    let ip = input.ip.unwrap_or(existing.ip);
    let port = input.port.unwrap_or(existing.port);
    let use_ssl = input.use_ssl.unwrap_or(existing.use_ssl);
    let verify_tls = input.verify_tls.unwrap_or(existing.verify_tls);
    let auth_token = input.auth_token.or(existing.auth_token);
    let web_app_url = input.web_app_url.or(existing.web_app_url);

    let updated = async {
        sqlx::query(
        "UPDATE plex_servers SET name = ?, ip = ?, port = ?, use_ssl = ?, verify_tls = ?, auth_token = ?, \
         web_app_url = ?, updated_at = NOW() WHERE id = ?",
        )
        .bind(&name)
        .bind(&ip)
        .bind(port)
        .bind(use_ssl)
        .bind(verify_tls)
        .bind(&auth_token)
        .bind(&web_app_url)
        .bind(server_id)
        .execute(pool)
        .await?;
        load_server(pool, server_id)
            .await?
            .ok_or(sqlx::Error::RowNotFound)
    }
    .await;

    match updated {
        Ok(server) => {
            let mut value = serde_json::to_value(&server).unwrap_or_default();
            redact_sensitive_fields(&mut value);
            Json(value).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, server_id, "failed to update plex server");
            super::api_error(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
        }
    }
}

async fn delete_server(
    State(state): State<Arc<AppState>>,
    Path(server_id): Path<i32>,
) -> impl IntoResponse {
    let pool = state.db.pool();
    match sqlx::query("DELETE FROM plex_servers WHERE id = ?")
        .bind(server_id)
        .execute(pool)
        .await
    {
        Ok(r) if r.rows_affected() > 0 => StatusCode::NO_CONTENT.into_response(),
        Ok(_) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            tracing::error!(error = %e, server_id, "failed to delete plex server");
            super::api_error(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
        }
    }
}

// ── Libraries ──────────────────────────────────────────────────────────────

/// Sync libraries from the Plex server and return the list.
async fn sync_libraries(
    State(state): State<Arc<AppState>>,
    Path(server_id): Path<i32>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    let server = match load_server(pool, server_id).await {
        Ok(Some(s)) => s,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            tracing::error!(error = %e, server_id, "failed to fetch plex server for library sync");
            return super::api_error(StatusCode::INTERNAL_SERVER_ERROR, "internal server error");
        }
    };

    let Some(api) = PlexApi::from_server(&server) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "No auth token configured"})),
        )
            .into_response();
    };

    let sections = match api.get_libraries().await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, server_id, "failed to fetch plex libraries from server");
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": "failed to fetch libraries from Plex server"})),
            )
                .into_response();
        }
    };

    // Upsert each library section
    for section in &sections {
        let lib_type = match section.section_type.as_str() {
            "movie" => "movie",
            "show" => "show",
            _ => continue, // skip music, photos, etc.
        };

        let _ = sqlx::query(
            "INSERT INTO plex_libraries (plex_server_id, section_id, name, library_type) \
             VALUES (?, ?, ?, ?) \
             ON DUPLICATE KEY UPDATE name = VALUES(name), library_type = VALUES(library_type)",
        )
        .bind(server_id)
        .bind(&section.key)
        .bind(&section.title)
        .bind(lib_type)
        .execute(pool)
        .await;
    }

    // Return the updated list
    match sqlx::query_as::<_, PlexLibrary>(
        "SELECT id, plex_server_id, section_id, name, enabled, library_type, last_scan \
         FROM plex_libraries WHERE plex_server_id = ? ORDER BY id",
    )
    .bind(server_id)
    .fetch_all(pool)
    .await
    {
        Ok(libs) => Json(libs).into_response(),
        Err(e) => {
            tracing::error!(error = %e, server_id, "failed to list plex libraries");
            super::api_error(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
        }
    }
}

async fn update_library(
    State(state): State<Arc<AppState>>,
    Path(library_id): Path<i32>,
    Json(input): Json<UpdatePlexLibraryInput>,
) -> impl IntoResponse {
    let pool = state.db.pool();
    let updated = async {
        sqlx::query("UPDATE plex_libraries SET enabled = ? WHERE id = ?")
            .bind(input.enabled)
            .bind(library_id)
            .execute(pool)
            .await?;
        load_library(pool, library_id).await
    }
    .await;

    match updated {
        Ok(Some(lib)) => Json(lib).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            tracing::error!(error = %e, library_id, "failed to update plex library");
            super::api_error(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
        }
    }
}

// ── Scan triggers ──────────────────────────────────────────────────────────

async fn trigger_full_scan(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let pool = state.db.pool().clone();
    // Spawn as background task so we don't block the response
    tokio::spawn(async move {
        let scanner = PlexScanner::new(pool);
        match scanner.full_scan().await {
            Ok(report) => tracing::info!(?report, "plex full scan complete"),
            Err(e) => tracing::error!(error = %e, "plex full scan failed"),
        }
    });
    Json(json!({"status": "scan started"}))
}

async fn trigger_recent_scan(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let pool = state.db.pool().clone();
    tokio::spawn(async move {
        let scanner = PlexScanner::new(pool);
        match scanner.recent_scan().await {
            Ok(report) => tracing::info!(?report, "plex recent scan complete"),
            Err(e) => tracing::error!(error = %e, "plex recent scan failed"),
        }
    });
    Json(json!({"status": "recent scan started"}))
}

// ── OAuth / Auth ───────────────────────────────────────────────────────────

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlexAuthInput {
    auth_token: String,
}

/// Validate a Plex auth token and return user info.
async fn validate_plex_token(Json(input): Json<PlexAuthInput>) -> impl IntoResponse {
    let tv_api = PlexTvApi::new(&input.auth_token);
    match tv_api.get_user().await {
        Ok(user) => Json(json!({
            "valid": true,
            "user": user
        }))
        .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "plex token validation failed");
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({"valid": false, "error": "token validation failed"})),
            )
                .into_response()
        }
    }
}

/// Discover Plex servers for a given token.
async fn discover_servers(Json(input): Json<PlexAuthInput>) -> impl IntoResponse {
    let tv_api = PlexTvApi::new(&input.auth_token);
    match tv_api.get_servers().await {
        Ok(servers) => Json(servers).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "plex server discovery failed");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": "failed to discover plex servers"})),
            )
                .into_response()
        }
    }
}

// ── PIN-based OAuth ───────────────────────────────────────────────────────

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PinCreateInput {
    client_id: String,
}

/// Create a Plex OAuth PIN. The frontend opens a popup to app.plex.tv/auth with
/// the returned code, then polls check_pin until the user authorizes.
async fn create_pin(Json(input): Json<PinCreateInput>) -> impl IntoResponse {
    let tv_api = PlexTvApi::new("");
    match tv_api.create_pin(&input.client_id).await {
        Ok(pin) => Json(json!({
            "id": pin.id,
            "code": pin.code,
        }))
        .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to create plex PIN");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": "failed to create PIN"})),
            )
                .into_response()
        }
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PinCheckQuery {
    client_id: String,
}

/// Poll a PIN to check if the user has authorized. Returns authToken when authorized.
async fn check_pin(
    Path(pin_id): Path<i64>,
    Query(query): Query<PinCheckQuery>,
) -> impl IntoResponse {
    let tv_api = PlexTvApi::new("");
    match tv_api.check_pin(pin_id, &query.client_id).await {
        Ok(pin) => Json(json!({
            "id": pin.id,
            "code": pin.code,
            "authToken": pin.auth_token,
        }))
        .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to check plex PIN");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": "failed to check PIN"})),
            )
                .into_response()
        }
    }
}

// ── Watchlist ──────────────────────────────────────────────────────────────

async fn list_watchlist(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let pool = state.db.pool();
    match sqlx::query_as::<_, WatchlistEntry>(
        "SELECT id, tmdb_id, media_type, plex_rating_key, auto_requested, created_at \
         FROM watchlist ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await
    {
        Ok(entries) => Json(entries).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to list watchlist entries");
            super::api_error(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
        }
    }
}

async fn trigger_watchlist_sync(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let pool = state.db.pool().clone();
    tokio::spawn(async move {
        let sync = stackarr_plex::WatchlistSync::new(pool);
        match sync.run().await {
            Ok(report) => tracing::info!(?report, "watchlist sync complete"),
            Err(e) => tracing::error!(error = %e, "watchlist sync failed"),
        }
    });
    Json(json!({"status": "watchlist sync started"}))
}

// ── Watchlist auto-request config ──────────────────────────────────────────

/// GET /api/v1/plex/watchlist/config
async fn get_watchlist_config(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let pool = state.db.pool();
    let config: Option<serde_json::Value> = sqlx::query_scalar(
        "SELECT value FROM app_config WHERE key = 'plex_watchlist_auto_request'",
    )
    .fetch_optional(pool)
    .await
    .unwrap_or(None);

    Json(config.unwrap_or_else(|| json!({"mode": "disabled"}))).into_response()
}

/// PUT /api/v1/plex/watchlist/config
async fn update_watchlist_config(
    State(state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    let result = sqlx::query(
        "INSERT INTO app_config (key, value) VALUES ('plex_watchlist_auto_request', ?) \
         ON DUPLICATE KEY UPDATE value = VALUES(value)",
    )
    .bind(&body)
    .execute(pool)
    .await;

    match result {
        Ok(_) => Json(body).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to update watchlist config");
            super::api_error(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
        }
    }
}

// ── Webhook receiver (public, no auth) ────────────────────────────────────

/// POST /api/v1/plex/webhook/{secret} — receives Plex webhook events.
/// Validates the secret against plex_servers.webhook_secret.
async fn receive_webhook(
    State(state): State<Arc<AppState>>,
    Path(secret): Path<String>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let pool = state.db.pool();

    // Validate secret matches a configured server
    let server_id: Option<i32> =
        sqlx::query_scalar("SELECT id FROM plex_servers WHERE webhook_secret = ?")
            .bind(&secret)
            .fetch_optional(pool)
            .await
            .unwrap_or(None);

    if server_id.is_none() {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    // Extract JSON payload from multipart form
    let mut payload_json: Option<String> = None;
    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        if name == "payload"
            && let Ok(text) = field.text().await
        {
            payload_json = Some(text);
        }
    }

    let Some(payload_str) = payload_json else {
        return super::api_error(StatusCode::BAD_REQUEST, "missing payload field");
    };

    let payload: PlexWebhookPayload = match serde_json::from_str(&payload_str) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(error = %e, "failed to parse plex webhook payload");
            return super::api_error(StatusCode::BAD_REQUEST, "invalid payload");
        }
    };

    // Build title from metadata
    let title = payload.metadata.as_ref().map(|m| {
        if let Some(ref gp) = m.grandparent_title {
            format!("{} - {}", gp, m.title)
        } else {
            m.title.clone()
        }
    });

    let user_name = payload.account.as_ref().map(|a| a.title.clone());
    let rating_key = payload.metadata.as_ref().and_then(|m| m.rating_key.clone());
    let raw_payload: serde_json::Value = serde_json::from_str(&payload_str).unwrap_or_default();

    // Insert event
    let _ = sqlx::query(
        "INSERT INTO plex_events (event_type, plex_server_id, user_name, title, rating_key, metadata, received_at) \
         VALUES (?, ?, ?, ?, ?, ?, NOW())",
    )
    .bind(&payload.event)
    .bind(server_id)
    .bind(&user_name)
    .bind(&title)
    .bind(&rating_key)
    .bind(&raw_payload)
    .execute(pool)
    .await;

    tracing::info!(event = %payload.event, user = ?user_name, title = ?title, "plex webhook received");
    StatusCode::OK.into_response()
}

// ── Plex events (protected) ──────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EventsQuery {
    #[serde(default)]
    event_type: Option<String>,
    #[serde(default = "default_event_limit")]
    limit: i64,
}

fn default_event_limit() -> i64 {
    100
}

/// GET /api/v1/plex/events
async fn list_events(
    State(state): State<Arc<AppState>>,
    Query(query): Query<EventsQuery>,
) -> impl IntoResponse {
    let pool = state.db.pool();
    let limit = query.limit.min(500);

    let events = if let Some(ref event_type) = query.event_type {
        sqlx::query_as::<_, PlexEvent>(
            "SELECT id, event_type, plex_server_id, user_name, title, rating_key, metadata, thumb_url, received_at \
             FROM plex_events WHERE event_type = ? ORDER BY received_at DESC LIMIT ?",
        )
        .bind(event_type)
        .bind(limit)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query_as::<_, PlexEvent>(
            "SELECT id, event_type, plex_server_id, user_name, title, rating_key, metadata, thumb_url, received_at \
             FROM plex_events ORDER BY received_at DESC LIMIT ?",
        )
        .bind(limit)
        .fetch_all(pool)
        .await
    };

    match events {
        Ok(events) => Json(events).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to list plex events");
            super::api_error(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
        }
    }
}

/// DELETE /api/v1/plex/events
async fn clear_events(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let pool = state.db.pool();
    match sqlx::query("DELETE FROM plex_events").execute(pool).await {
        Ok(r) => Json(json!({"deleted": r.rows_affected()})).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to clear plex events");
            super::api_error(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
        }
    }
}

/// GET /api/v1/plex/servers/{server_id}/webhook-url
async fn get_webhook_url(
    State(state): State<Arc<AppState>>,
    Path(server_id): Path<i32>,
) -> impl IntoResponse {
    let pool = state.db.pool();
    let secret: Option<String> =
        sqlx::query_scalar("SELECT webhook_secret FROM plex_servers WHERE id = ?")
            .bind(server_id)
            .fetch_optional(pool)
            .await
            .unwrap_or(None);

    match secret {
        Some(s) => Json(json!({"webhookUrl": format!("/api/v1/plex/webhook/{s}")})).into_response(),
        None => super::api_error(
            StatusCode::NOT_FOUND,
            "server not found or no webhook secret",
        ),
    }
}

// ── Router ─────────────────────────────────────────────────────────────────

/// Public webhook router — mounted without auth middleware.
pub fn webhook_router() -> Router<Arc<AppState>> {
    Router::new().route("/api/v1/plex/webhook/{secret}", post(receive_webhook))
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        // Server management
        .route(
            "/api/v1/plex/servers",
            get(list_servers).post(create_server),
        )
        .route(
            "/api/v1/plex/servers/{server_id}",
            put(update_server).delete(delete_server),
        )
        // Library management
        .route(
            "/api/v1/plex/servers/{server_id}/libraries",
            get(sync_libraries),
        )
        .route("/api/v1/plex/libraries/{library_id}", put(update_library))
        // Scanning
        .route("/api/v1/plex/scan/full", post(trigger_full_scan))
        .route("/api/v1/plex/scan/recent", post(trigger_recent_scan))
        // Auth
        .route("/api/v1/plex/auth/validate", post(validate_plex_token))
        .route("/api/v1/plex/auth/servers", post(discover_servers))
        .route("/api/v1/plex/auth/pin", post(create_pin))
        .route("/api/v1/plex/auth/pin/{pin_id}", get(check_pin))
        // Watchlist
        .route("/api/v1/plex/watchlist", get(list_watchlist))
        .route("/api/v1/plex/watchlist/sync", post(trigger_watchlist_sync))
        .route(
            "/api/v1/plex/watchlist/config",
            get(get_watchlist_config).put(update_watchlist_config),
        )
        // Events
        .route("/api/v1/plex/events", get(list_events).delete(clear_events))
        // Webhook URL
        .route(
            "/api/v1/plex/servers/{server_id}/webhook-url",
            get(get_webhook_url),
        )
}
