use std::path::PathBuf;
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
    config: PathBuf,

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

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(clap::Subcommand, Debug)]
enum Commands {
    /// Import data from Sonarr, Radarr, and/or Prowlarr databases
    Migrate {
        /// Path to Sonarr SQLite database
        #[arg(long)]
        sonarr: Option<PathBuf>,
        /// Path to Radarr SQLite database
        #[arg(long)]
        radarr: Option<PathBuf>,
        /// Path to Prowlarr SQLite database
        #[arg(long)]
        prowlarr: Option<PathBuf>,
        /// Show what would be imported without writing
        #[arg(long)]
        dry_run: bool,
    },
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

    // 5. Connect to database
    let db = Database::connect(&config.database)
        .await
        .context("failed to connect to database")?;

    // 6. Run migrations
    tracing::info!("running database migrations");
    db.run_migrations().await.context("migration failed")?;

    // Handle subcommands
    match cli.command {
        Some(Commands::Migrate {
            sonarr,
            radarr,
            prowlarr,
            dry_run,
        }) => {
            tracing::info!(
                sonarr = ?sonarr,
                radarr = ?radarr,
                prowlarr = ?prowlarr,
                dry_run,
                "starting migration from *arr databases"
            );

            if sonarr.is_none() && radarr.is_none() && prowlarr.is_none() {
                anyhow::bail!(
                    "at least one of --sonarr, --radarr, or --prowlarr must be specified"
                );
            }

            let report = stackarr_migrate::run_migration(
                db.pool(),
                sonarr.as_deref(),
                radarr.as_deref(),
                prowlarr.as_deref(),
                dry_run,
            )
            .await
            .context("migration failed")?;

            println!("\n=== Migration Report ===");
            if report.dry_run {
                println!("(DRY RUN — no data was written)");
            }
            println!("Series imported:   {}", report.series_imported);
            println!("Movies imported:   {}", report.movies_imported);
            println!("Episodes imported: {}", report.episodes_imported);
            println!("Indexers imported:  {}", report.indexers_imported);
            if !report.warnings.is_empty() {
                println!("\nWarnings:");
                for w in &report.warnings {
                    println!("  - {w}");
                }
            }
            println!("========================\n");

            tracing::info!("migration complete");
            return Ok(());
        }
        None => {
            // Normal server startup
        }
    }

    tracing::info!(
        instance = %config.general.instance_name,
        "starting StackArr v{}",
        env!("CARGO_PKG_VERSION")
    );

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
    let _scheduler_handle = Scheduler::new(state.db.pool().clone())
        .start()
        .context("failed to start scheduler")?;

    // Start HTTP server
    tracing::info!(addr = %listen_addr, "starting HTTP server");
    stackarr_web::run(&listen_addr, state).await?;

    tracing::info!("StackArr shut down cleanly");
    Ok(())
}
