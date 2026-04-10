mod config;
mod db;
mod relay;
mod routes;
mod state;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::routing::{any, delete, get, post};
use clap::Parser;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::EnvFilter;

use config::Config;
use db::BootstrapDb;
use state::BootstrapState;

#[derive(Parser, Debug)]
#[command(
    name = "stackarr-bootstrap",
    version,
    about = "StackArr bootstrap/discovery node"
)]
struct Cli {
    /// Path to the configuration file
    #[arg(
        short,
        long,
        default_value = "bootstrap.toml",
        env = "BOOTSTRAP_CONFIG"
    )]
    config: PathBuf,

    /// Log level
    #[arg(long, env = "BOOTSTRAP_LOG_LEVEL", default_value = "info")]
    log_level: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Init tracing
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&cli.log_level));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .init();

    // Load config
    let content = std::fs::read_to_string(&cli.config)
        .map_err(|e| anyhow::anyhow!("failed to read config {}: {e}", cli.config.display()))?;
    let config: Config =
        toml::from_str(&content).map_err(|e| anyhow::anyhow!("failed to parse config: {e}"))?;

    let listen_addr = format!("{}:{}", config.bootstrap.bind_addr, config.bootstrap.port);

    // Initialize SQLite persistence
    let db = BootstrapDb::new(&config.bootstrap.database_path)
        .map_err(|e| anyhow::anyhow!("failed to open bootstrap database: {e}"))?;
    tracing::info!(path = %config.bootstrap.database_path, "bootstrap database initialized");

    let state = Arc::new(BootstrapState::new(&config.bootstrap, db));

    // Spawn cleanup task
    let cleanup_state = Arc::clone(&state);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            cleanup_state.sweep_expired().await;
        }
    });

    // Build router
    let app = Router::new()
        .route("/api/v1/servers/register", post(routes::register_server))
        .route(
            "/api/v1/servers/{server_id}",
            delete(routes::deregister_server),
        )
        .route("/api/v1/claims", post(routes::create_claim))
        .route("/api/v1/claims/{code}/redeem", post(routes::redeem_claim))
        .route(
            "/api/v1/servers/by-name/{name}",
            get(routes::lookup_by_name),
        )
        .route("/api/v1/servers/register-name", post(routes::register_name))
        .route("/api/v1/servers/recover-name", post(routes::recover_name))
        .route("/api/v1/servers/check-name/{name}", get(routes::check_name))
        .route("/api/v1/servers/check-port", post(routes::check_port))
        .route("/api/v1/health", get(routes::health))
        .route("/health", get(routes::health))
        .route("/relay/{server_id}/{*path}", any(relay::relay_handler))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .with_state(state);

    tracing::info!(addr = %listen_addr, "starting bootstrap node");
    let listener = tokio::net::TcpListener::bind(&listen_addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;

    Ok(())
}
