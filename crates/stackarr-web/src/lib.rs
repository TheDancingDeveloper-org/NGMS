pub mod middleware;
pub mod routes;
pub mod state;

pub use state::AppState;

use std::sync::Arc;

use axum::http::{HeaderName, HeaderValue, Method};
use axum::middleware::from_fn_with_state;
use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;

/// Build the full API router with all routes mounted.
pub fn build_router(state: Arc<AppState>) -> Router {
    // ── Public routes (no auth required) ─────────────────────────────
    let public_routes = Router::new()
        .merge(routes::health::router())
        .merge(routes::system::public_router())
        .merge(routes::images::router())
        .merge(routes::auth::router())
        .merge(routes::plex::webhook_router())
        .merge(routes::stremio::router());

    // ── Protected routes (require API key) ───────────────────────────
    let protected_routes = Router::new()
        .merge(routes::system::protected_router())
        .merge(routes::series::router())
        .merge(routes::movies::router())
        .merge(routes::queue::router())
        .merge(routes::history::router())
        .merge(routes::releases::router())
        .merge(routes::quality::router())
        .merge(routes::medialibraryfolders::router())
        .merge(routes::tags::router())
        .merge(routes::naming::router())
        .merge(routes::downloadclients::router())
        .merge(routes::indexers::router())
        .merge(routes::calendar::router())
        .merge(routes::wanted::router())
        .merge(routes::episodes::router())
        .merge(routes::torrent::router())
        .merge(routes::usenet::router())
        .merge(routes::importlists::router())
        .merge(routes::indexarr::router())
        .merge(routes::discover::router())
        .merge(routes::plex::router())
        .merge(routes::blocklist::router())
        .merge(routes::backup::router())
        .merge(routes::logs::router())
        .merge(routes::stream::router())
        .merge(routes::remote::router())
        .merge(routes::search::router())
        .merge(routes::general::router())
        .merge(routes::mediamanagement::router())
        .merge(routes::admin::router())
        .merge(routes::user::router())
        .merge(routes::progress::router())
        .merge(routes::requests::router())
        .merge(routes::watchlist::router())
        .merge(routes::notifications::router())
        .merge(routes::activities::router())
        .merge(routes::bootstrap::router())
        .layer(from_fn_with_state(state.clone(), middleware::require_auth_middleware));

    // ── CORS configuration ───────────────────────────────────────────
    let cors = CorsLayer::new()
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::AUTHORIZATION,
            HeaderName::from_static("x-api-key"),
            HeaderName::from_static("x-csrf-token"),
        ])
        .allow_origin(tower_http::cors::AllowOrigin::mirror_request())
        .allow_credentials(true);

    // ── Security headers ─────────────────────────────────────────────
    let api_router = Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("x-frame-options"),
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("x-content-type-options"),
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("x-xss-protection"),
            HeaderValue::from_static("1; mode=block"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("referrer-policy"),
            HeaderValue::from_static("strict-origin-when-cross-origin"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("permissions-policy"),
            HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
        ))
        .with_state(state);

    // Serve client app at /app with SPA fallback
    let client_dir = std::env::var("STACKARR_CLIENT_DIR").unwrap_or_else(|_| "/client".to_string());
    let client_fallback = ServeFile::new(format!("{client_dir}/index.html"));
    let client_serve = ServeDir::new(&client_dir).fallback(client_fallback);

    // Serve UI static files with SPA fallback
    let ui_dir = std::env::var("STACKARR_UI_DIR").unwrap_or_else(|_| "/ui".to_string());
    let spa_fallback = ServeFile::new(format!("{ui_dir}/index.html"));
    let serve_dir = ServeDir::new(&ui_dir).fallback(spa_fallback);

    Router::new()
        .merge(api_router)
        .nest_service("/app", client_serve)
        .fallback_service(serve_dir)
}

/// Start the Axum server on the given address.
pub async fn run(addr: &str, state: Arc<AppState>) -> anyhow::Result<()> {
    let router = build_router(state);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("listening on {addr}");
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install CTRL+C handler");
    tracing::info!("shutdown signal received");
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use arc_swap::ArcSwap;
    use stackarr_core::config::{AppConfig, EnabledModules};
    use stackarr_core::db::Database;
    use stackarr_core::test_helpers::{TestDb, seed_quality_profile};
    use stackarr_download::DownloadClientManager;
    use stackarr_indexer::IndexerManager;
    use tokio::sync::RwLock;

    async fn test_state() -> (Arc<AppState>, TestDb) {
        let db = TestDb::new().await;
        let database = Database::from_pool(db.pool.clone());
        let config = Arc::new(ArcSwap::from_pointee(AppConfig::default()));
        let state = Arc::new(AppState {
            db: database,
            config,
            modules: EnabledModules::default(),
            torrent_session: arc_swap::ArcSwapOption::empty(),
            torrent_api: arc_swap::ArcSwapOption::empty(),
            usenet_queue: arc_swap::ArcSwapOption::empty(),
            indexarr_client: None,
            indexarr_available: false,
            cardigann_engine: Arc::new(stackarr_cardigann::CardigannEngine::new(std::path::Path::new(""))),
            indexer_manager: Arc::new(RwLock::new(IndexerManager::new())),
            download_manager: Arc::new(RwLock::new(DownloadClientManager::new())),
            rate_limiter: None,
            tmdb_client: None,
            stream_session_manager: None,
        });
        (state, db)
    }

    async fn body_to_json(body: Body) -> serde_json::Value {
        let bytes = body.collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
    }

    #[tokio::test]
    #[ignore = "requires running postgres"]
    async fn test_health_check() {
        let (state, db) = test_state().await;
        let app = build_router(state);

        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
        db.cleanup().await;
    }

    #[tokio::test]
    #[ignore = "requires running postgres"]
    async fn test_system_health() {
        let (state, db) = test_state().await;
        let app = build_router(state);

        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/v1/system/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
        let json = body_to_json(resp.into_body()).await;
        assert_eq!(json["status"], "ok");
        db.cleanup().await;
    }

    #[tokio::test]
    #[ignore = "requires running postgres"]
    async fn test_list_series_empty() {
        let (state, db) = test_state().await;
        let app = build_router(state);

        // No API key stored = first boot = no auth required
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/v1/series")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
        let json = body_to_json(resp.into_body()).await;
        assert_eq!(json, serde_json::json!([]));
        db.cleanup().await;
    }

    #[tokio::test]
    #[ignore = "requires running postgres"]
    async fn test_create_series() {
        let (state, db) = test_state().await;
        let profile_id = seed_quality_profile(&db.pool).await;
        let app = build_router(state);

        let body = serde_json::json!({
            "title": "Breaking Bad",
            "path": "/tv/Breaking Bad",
            "qualityProfileId": profile_id,
            "monitored": true,
            "tvdbId": 81189
        });

        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/v1/series")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert!(resp.status().is_success());
        let json = body_to_json(resp.into_body()).await;
        assert_eq!(json["title"], "Breaking Bad");
        db.cleanup().await;
    }

    #[tokio::test]
    #[ignore = "requires running postgres"]
    async fn test_list_tags_empty() {
        let (state, db) = test_state().await;
        let app = build_router(state);

        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/v1/tag")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
        let json = body_to_json(resp.into_body()).await;
        assert_eq!(json, serde_json::json!([]));
        db.cleanup().await;
    }

    #[tokio::test]
    #[ignore = "requires running postgres"]
    async fn test_list_queue_empty() {
        let (state, db) = test_state().await;
        let app = build_router(state);

        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/v1/queue")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
        let json = body_to_json(resp.into_body()).await;
        assert_eq!(json, serde_json::json!([]));
        db.cleanup().await;
    }

    #[tokio::test]
    #[ignore = "requires running postgres"]
    async fn test_list_quality_profiles_empty() {
        let (state, db) = test_state().await;
        let app = build_router(state);

        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/v1/qualityprofile")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
        db.cleanup().await;
    }

    #[tokio::test]
    #[ignore = "requires running postgres"]
    async fn test_list_media_library_folders_empty() {
        let (state, db) = test_state().await;
        let app = build_router(state);

        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/v1/medialibraryfolder")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
        db.cleanup().await;
    }
}
