use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde_json::json;

use stackarr_plex::types::*;
use stackarr_plex::{PlexApi, PlexScanner, PlexTvApi};

use crate::AppState;

// ── Plex Server CRUD ───────────────────────────────────────────────────────

async fn list_servers(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let pool = state.db.pool();
    match sqlx::query_as::<_, PlexServer>(
        "SELECT id, name, machine_id, ip, port, use_ssl, auth_token, web_app_url, created_at, updated_at \
         FROM plex_servers ORDER BY id",
    )
    .fetch_all(pool)
    .await
    {
        Ok(servers) => Json(servers).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
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

    // Validate connection by fetching server info
    let api = PlexApi::new(&input.ip, port, use_ssl, &input.auth_token);
    let machine_id = match api.get_status().await {
        Ok(info) => info.machine_identifier,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": format!("Failed to connect to Plex server: {e}")})),
            )
                .into_response();
        }
    };

    match sqlx::query_as::<_, PlexServer>(
        "INSERT INTO plex_servers (name, machine_id, ip, port, use_ssl, auth_token, web_app_url) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) \
         RETURNING id, name, machine_id, ip, port, use_ssl, auth_token, web_app_url, created_at, updated_at",
    )
    .bind(&name)
    .bind(&machine_id)
    .bind(&input.ip)
    .bind(port)
    .bind(use_ssl)
    .bind(&input.auth_token)
    .bind(&input.web_app_url)
    .fetch_one(pool)
    .await
    {
        Ok(server) => (StatusCode::CREATED, Json(server)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

async fn update_server(
    State(state): State<Arc<AppState>>,
    Path(server_id): Path<i64>,
    Json(input): Json<UpdatePlexServerInput>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    let existing = sqlx::query_as::<_, PlexServer>(
        "SELECT id, name, machine_id, ip, port, use_ssl, auth_token, web_app_url, created_at, updated_at \
         FROM plex_servers WHERE id = $1",
    )
    .bind(server_id)
    .fetch_optional(pool)
    .await;

    let existing = match existing {
        Ok(Some(s)) => s,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    let name = input.name.unwrap_or(existing.name);
    let ip = input.ip.unwrap_or(existing.ip);
    let port = input.port.unwrap_or(existing.port);
    let use_ssl = input.use_ssl.unwrap_or(existing.use_ssl);
    let auth_token = input.auth_token.or(existing.auth_token);
    let web_app_url = input.web_app_url.or(existing.web_app_url);

    match sqlx::query_as::<_, PlexServer>(
        "UPDATE plex_servers SET name = $1, ip = $2, port = $3, use_ssl = $4, auth_token = $5, \
         web_app_url = $6, updated_at = NOW() WHERE id = $7 \
         RETURNING id, name, machine_id, ip, port, use_ssl, auth_token, web_app_url, created_at, updated_at",
    )
    .bind(&name)
    .bind(&ip)
    .bind(port)
    .bind(use_ssl)
    .bind(&auth_token)
    .bind(&web_app_url)
    .bind(server_id)
    .fetch_one(pool)
    .await
    {
        Ok(server) => Json(server).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn delete_server(
    State(state): State<Arc<AppState>>,
    Path(server_id): Path<i64>,
) -> impl IntoResponse {
    let pool = state.db.pool();
    match sqlx::query("DELETE FROM plex_servers WHERE id = $1")
        .bind(server_id)
        .execute(pool)
        .await
    {
        Ok(r) if r.rows_affected() > 0 => StatusCode::NO_CONTENT.into_response(),
        Ok(_) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

// ── Libraries ──────────────────────────────────────────────────────────────

/// Sync libraries from the Plex server and return the list.
async fn sync_libraries(
    State(state): State<Arc<AppState>>,
    Path(server_id): Path<i64>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    let server = match sqlx::query_as::<_, PlexServer>(
        "SELECT id, name, machine_id, ip, port, use_ssl, auth_token, web_app_url, created_at, updated_at \
         FROM plex_servers WHERE id = $1",
    )
    .bind(server_id)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(s)) => s,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
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
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": format!("Failed to fetch libraries: {e}")})),
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
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn update_library(
    State(state): State<Arc<AppState>>,
    Path(library_id): Path<i64>,
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
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
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
        Err(e) => (
            StatusCode::UNAUTHORIZED,
            Json(json!({"valid": false, "error": format!("{e}")})),
        )
            .into_response(),
    }
}

/// Discover Plex servers for a given token.
async fn discover_servers(Json(input): Json<PlexAuthInput>) -> impl IntoResponse {
    let tv_api = PlexTvApi::new(&input.auth_token);
    match tv_api.get_servers().await {
        Ok(servers) => Json(servers).into_response(),
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({"error": format!("{e}")})),
        )
            .into_response(),
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
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
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
        // Watchlist
        .route("/api/v1/plex/watchlist", get(list_watchlist))
        .route("/api/v1/plex/watchlist/sync", post(trigger_watchlist_sync))
}
