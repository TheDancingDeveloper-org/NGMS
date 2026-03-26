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
        .merge(routes::rootfolders::router())
        .merge(routes::tags::router())
        .merge(routes::naming::router())
        .merge(routes::downloadclients::router())
        .merge(routes::indexers::router())
        .merge(routes::calendar::router())
        .merge(routes::wanted::router())
        .merge(routes::episodes::router())
        .merge(routes::torrent::router())
        .merge(routes::usenet::router())
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
