pub mod routes;
pub mod state;

pub use state::AppState;

use std::sync::Arc;

use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

/// Build the full API router with all routes mounted.
pub fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
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
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state)
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
