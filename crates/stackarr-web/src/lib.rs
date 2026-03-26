pub mod routes;
pub mod state;

pub use state::AppState;

use std::sync::Arc;

use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

/// Build the full API router with all routes mounted.
pub fn build_router(state: Arc<AppState>) -> Router {
    let api_router = Router::new()
        .merge(routes::health::router())
        .merge(routes::system::router())
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
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state);

    // Serve UI static files with SPA fallback
    let ui_dir = std::env::var("STACKARR_UI_DIR").unwrap_or_else(|_| "/ui".to_string());
    let spa_fallback = ServeFile::new(format!("{ui_dir}/index.html"));
    let serve_dir = ServeDir::new(&ui_dir).fallback(spa_fallback);

    Router::new()
        .merge(api_router)
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
    use stackarr_core::test_helpers::{TestDb, seed_quality_profile, seed_media_library_folder};

    async fn test_state() -> (Arc<AppState>, TestDb) {
        let db = TestDb::new().await;
        let database = Database::from_pool(db.pool.clone());
        let config = Arc::new(ArcSwap::from_pointee(AppConfig::default()));
        let state = Arc::new(AppState {
            db: database,
            config,
            modules: EnabledModules::default(),
            torrent_session: None,
            torrent_api: None,
            usenet_queue: None,
            indexarr_client: None,
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
