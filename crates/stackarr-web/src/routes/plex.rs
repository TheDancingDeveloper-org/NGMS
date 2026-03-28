use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde_json::json;

use stackarr_plex::types::*;
use stackarr_plex::{PlexApi, PlexScanner, PlexTvApi};

use crate::middleware::redact_sensitive_fields;
use crate::AppState;

// ── Plex Server CRUD ───────────────────────────────────────────────────────

async fn list_servers(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let pool = state.db.pool();
    match sqlx::query_as::<_, PlexServer>(
        "SELECT id, name, machine_id, ip, port, use_ssl, verify_tls, auth_token, web_app_url, created_at, updated_at \
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
            (StatusCode::INTERNAL_SERVER_ERROR, "internal server error").into_response()
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

    match sqlx::query_as::<_, PlexServer>(
        "INSERT INTO plex_servers (name, machine_id, ip, port, use_ssl, verify_tls, auth_token, web_app_url) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) \
         RETURNING id, name, machine_id, ip, port, use_ssl, verify_tls, auth_token, web_app_url, created_at, updated_at",
    )
    .bind(&name)
    .bind(&machine_id)
    .bind(&input.ip)
    .bind(port)
    .bind(use_ssl)
    .bind(verify_tls)
    .bind(&input.auth_token)
    .bind(&input.web_app_url)
    .fetch_one(pool)
    .await
    {
        Ok(server) => {
            let mut value = serde_json::to_value(&server).unwrap_or_default();
            redact_sensitive_fields(&mut value);
            (StatusCode::CREATED, Json(value)).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to insert plex server");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal server error").into_response()
        }
    }
}

async fn update_server(
    State(state): State<Arc<AppState>>,
    Path(server_id): Path<i32>,
    Json(input): Json<UpdatePlexServerInput>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    let existing = sqlx::query_as::<_, PlexServer>(
        "SELECT id, name, machine_id, ip, port, use_ssl, verify_tls, auth_token, web_app_url, created_at, updated_at \
         FROM plex_servers WHERE id = $1",
    )
    .bind(server_id)
    .fetch_optional(pool)
    .await;

    let existing = match existing {
        Ok(Some(s)) => s,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            tracing::error!(error = %e, server_id, "failed to fetch plex server for update");
            return (StatusCode::INTERNAL_SERVER_ERROR, "internal server error").into_response();
        }
    };

    let name = input.name.unwrap_or(existing.name);
    let ip = input.ip.unwrap_or(existing.ip);
    let port = input.port.unwrap_or(existing.port);
    let use_ssl = input.use_ssl.unwrap_or(existing.use_ssl);
    let verify_tls = input.verify_tls.unwrap_or(existing.verify_tls);
    let auth_token = input.auth_token.or(existing.auth_token);
    let web_app_url = input.web_app_url.or(existing.web_app_url);

    match sqlx::query_as::<_, PlexServer>(
        "UPDATE plex_servers SET name = $1, ip = $2, port = $3, use_ssl = $4, verify_tls = $5, auth_token = $6, \
         web_app_url = $7, updated_at = NOW() WHERE id = $8 \
         RETURNING id, name, machine_id, ip, port, use_ssl, verify_tls, auth_token, web_app_url, created_at, updated_at",
    )
    .bind(&name)
    .bind(&ip)
    .bind(port)
    .bind(use_ssl)
    .bind(verify_tls)
    .bind(&auth_token)
    .bind(&web_app_url)
    .bind(server_id)
    .fetch_one(pool)
    .await
    {
        Ok(server) => {
            let mut value = serde_json::to_value(&server).unwrap_or_default();
            redact_sensitive_fields(&mut value);
            Json(value).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, server_id, "failed to update plex server");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal server error").into_response()
        }
    }
}

async fn delete_server(
    State(state): State<Arc<AppState>>,
    Path(server_id): Path<i32>,
) -> impl IntoResponse {
    let pool = state.db.pool();
    match sqlx::query("DELETE FROM plex_servers WHERE id = $1")
        .bind(server_id)
        .execute(pool)
        .await
    {
        Ok(r) if r.rows_affected() > 0 => StatusCode::NO_CONTENT.into_response(),
        Ok(_) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            tracing::error!(error = %e, server_id, "failed to delete plex server");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal server error").into_response()
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

    let server = match sqlx::query_as::<_, PlexServer>(
        "SELECT id, name, machine_id, ip, port, use_ssl, verify_tls, auth_token, web_app_url, created_at, updated_at \
         FROM plex_servers WHERE id = $1",
    )
    .bind(server_id)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(s)) => s,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            tracing::error!(error = %e, server_id, "failed to fetch plex server for library sync");
            return (StatusCode::INTERNAL_SERVER_ERROR, "internal server error").into_response();
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
             VALUES ($1, $2, $3, $4) \
             ON CONFLICT (plex_server_id, section_id) DO UPDATE SET name = $3",
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
         FROM plex_libraries WHERE plex_server_id = $1 ORDER BY id",
    )
    .bind(server_id)
    .fetch_all(pool)
    .await
    {
        Ok(libs) => Json(libs).into_response(),
        Err(e) => {
            tracing::error!(error = %e, server_id, "failed to list plex libraries");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal server error").into_response()
        }
    }
}

async fn update_library(
    State(state): State<Arc<AppState>>,
    Path(library_id): Path<i32>,
    Json(input): Json<UpdatePlexLibraryInput>,
) -> impl IntoResponse {
    let pool = state.db.pool();
    match sqlx::query_as::<_, PlexLibrary>(
        "UPDATE plex_libraries SET enabled = $1 WHERE id = $2 \
         RETURNING id, plex_server_id, section_id, name, enabled, library_type, last_scan",
    )
    .bind(input.enabled)
    .bind(library_id)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(lib)) => Json(lib).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            tracing::error!(error = %e, library_id, "failed to update plex library");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal server error").into_response()
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
            (StatusCode::INTERNAL_SERVER_ERROR, "internal server error").into_response()
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

// ── Router ─────────────────────────────────────────────────────────────────

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        // Server management
        .route("/api/v1/plex/servers", get(list_servers).post(create_server))
        .route(
            "/api/v1/plex/servers/{server_id}",
            put(update_server).delete(delete_server),
        )
        // Library management
        .route(
            "/api/v1/plex/servers/{server_id}/libraries",
            get(sync_libraries),
        )
        .route(
            "/api/v1/plex/libraries/{library_id}",
            put(update_library),
        )
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
}
