use std::sync::Arc;

use anyhow::{Context, Result};
use arc_swap::ArcSwap;
use clap::Parser;
use tracing_subscriber::EnvFilter;

use stackarr_core::config::{AppConfig, EnabledModules};
use stackarr_core::db::Database;
use stackarr_scheduler::Scheduler;
use stackarr_web::AppState;

#[derive(Parser, Debug)]
#[command(name = "stackarr", version, about = "StackArr media management server")]
struct Cli {
    /// Path to the configuration file
    #[arg(short, long, default_value = "/config/stackarr.toml", env = "STACKARR_CONFIG")]
    config: std::path::PathBuf,

    /// Override bind address
    #[arg(long, env = "STACKARR_BIND")]
    bind: Option<String>,

    /// Override port
    #[arg(long, env = "STACKARR_PORT")]
    port: Option<u16>,

    /// Override database URL
    #[arg(long, env = "STACKARR_DATABASE_URL")]
    database_url: Option<String>,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, env = "STACKARR_LOG_LEVEL", default_value = "info")]
    log_level: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Parse CLI args
    let cli = Cli::parse();

    // 2. Init tracing EARLY so config-load errors are visible
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&cli.log_level));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .init();

    // 3. Load config — generate default if file is missing
    let mut config = match AppConfig::load(&cli.config) {
        Ok(cfg) => cfg,
        Err(_) if !cli.config.exists() => {
            tracing::warn!(
                path = %cli.config.display(),
                "config file not found — generating default"
            );
            AppConfig::generate_default(&cli.config)
                .context("failed to generate default configuration")?
        }
        Err(e) => return Err(e).context("failed to load configuration"),
    };

    // 4. Override config values with CLI/env if provided
    if let Some(ref db_url) = cli.database_url {
        config.database.url = db_url.clone();
    }
    if let Some(ref bind) = cli.bind {
        config.general.bind_addr = bind.clone();
    }
    if let Some(port) = cli.port {
        config.general.port = port;
    }

    tracing::info!(
        instance = %config.general.instance_name,
        "starting StackArr v{}",
        env!("CARGO_PKG_VERSION")
    );

    // 5. Connect to database
    let db = Database::connect(&config.database)
        .await
        .context("failed to connect to database")?;

    // 6. Run migrations
    tracing::info!("running database migrations");
    db.run_migrations().await.context("migration failed")?;

    // 7. Load enabled modules from DB
    let modules = db.load_enabled_modules().await.unwrap_or_else(|e| {
        tracing::warn!(error = %e, "failed to load enabled modules, using defaults");
        EnabledModules::default()
    });

    if db.is_first_boot().await.unwrap_or(true) {
        tracing::info!("first boot detected — no modules enabled yet");
    }

    // 8. Determine listen address
    let listen_addr = format!("{}:{}", config.general.bind_addr, config.general.port);

    // Build shared state
    let state = Arc::new(AppState {
        db,
        config: Arc::new(ArcSwap::new(Arc::new(config))),
        modules,
    });

    // Start background scheduler
    tracing::info!("starting background scheduler");
    let _scheduler_handle = Scheduler::new()
        .start()
        .context("failed to start scheduler")?;

    // Start HTTP server
    tracing::info!(addr = %listen_addr, "starting HTTP server");
    stackarr_web::run(&listen_addr, state).await?;

    tracing::info!("StackArr shut down cleanly");
    Ok(())
}
