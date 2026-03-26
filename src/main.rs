use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use arc_swap::ArcSwap;
use clap::Parser;
use tokio::sync::RwLock;
use tracing_subscriber::EnvFilter;

use stackarr_core::config::{AppConfig, EnabledModules};
use stackarr_core::db::Database;
use stackarr_download::DownloadClientManager;
use stackarr_indexer::IndexerManager;
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

    // 8. Initialize embedded torrent engine
    let torrent_session = if config.torrent.enabled {
        tracing::info!("initializing embedded torrent engine");
        let download_dir = config
            .torrent
            .download_dir
            .clone()
            .unwrap_or_else(|| PathBuf::from("/downloads/torrent"));
        let opts = librtbit::SessionOptions {
            disable_dht: !config.torrent.dht_enabled,
            completed_folder: config.torrent.complete_dir.clone(),
            ..Default::default()
        };
        match librtbit::Session::new_with_opts(download_dir, opts).await {
            Ok(session) => {
                tracing::info!("torrent engine started");
                Some(session)
            }
            Err(e) => {
                tracing::error!(error = %e, "failed to start torrent engine");
                None
            }
        }
    } else {
        None
    };

    let torrent_api = torrent_session.as_ref().map(|s| {
        librtbit::Api::new(Arc::clone(s), None)
    });

    // 9. Initialize embedded usenet engine
    let usenet_queue = if config.usenet.enabled && !config.usenet.servers.is_empty() {
        tracing::info!("initializing embedded usenet engine");

        let incomplete_dir = config
            .usenet
            .incomplete_dir
            .clone()
            .unwrap_or_else(|| PathBuf::from("/downloads/usenet/incomplete"));
        let complete_dir = config
            .usenet
            .complete_dir
            .clone()
            .unwrap_or_else(|| PathBuf::from("/downloads/usenet/complete"));

        // Ensure directories exist
        if let Err(e) = std::fs::create_dir_all(&incomplete_dir) {
            tracing::warn!(error = %e, "failed to create usenet incomplete dir");
        }
        if let Err(e) = std::fs::create_dir_all(&complete_dir) {
            tracing::warn!(error = %e, "failed to create usenet complete dir");
        }

        // Open a dedicated SQLite database for the usenet engine
        let db_path = incomplete_dir.join("usenet_queue.db");
        match nzb_core::db::Database::open(&db_path) {
            Ok(nzb_db) => {
                // Convert StackArr server configs to nzb-core ServerConfig
                let nzb_servers: Vec<nzb_core::config::ServerConfig> = config
                    .usenet
                    .servers
                    .iter()
                    .enumerate()
                    .map(|(i, s)| nzb_core::config::ServerConfig {
                        id: format!("server-{i}"),
                        name: s.name.clone(),
                        host: s.host.clone(),
                        port: s.port,
                        ssl: s.ssl,
                        ssl_verify: s.ssl, // verify certs when SSL is enabled
                        username: s.username.clone(),
                        password: s.password.clone(),
                        connections: s.connections,
                        priority: s.priority,
                        enabled: true,
                        retention: 0,
                        pipelining: 10,
                        optional: false,
                        compress: false,
                    })
                    .collect();

                let log_buffer = nzb_web::LogBuffer::default();
                let queue = nzb_web::QueueManager::new(
                    nzb_servers,
                    nzb_db,
                    incomplete_dir,
                    complete_dir,
                    log_buffer,
                    config.usenet.max_active_downloads,
                    Vec::new(), // no category configs
                    0,          // no min free space limit
                    0,          // no speed limit
                );

                // Restore any in-progress jobs from the database
                if let Err(e) = queue.restore_from_db() {
                    tracing::warn!(error = %e, "failed to restore usenet queue from database");
                }

                // Spawn the speed tracker background task
                queue.spawn_speed_tracker();

                tracing::info!("usenet engine started");
                Some(queue)
            }
            Err(e) => {
                tracing::error!(error = %e, "failed to open usenet database");
                None
            }
        }
    } else {
        None
    };

    // 10. Initialize Indexarr sidecar client
    let indexarr_client = if config.indexarr.enabled {
        match &config.indexarr.api_key {
            Some(api_key) if !api_key.is_empty() => {
                tracing::info!(url = %config.indexarr.url, "initializing Indexarr sidecar client");
                let client = stackarr_indexer::IndexarrClient::new(
                    &config.indexarr.url,
                    api_key,
                );
                Some(Arc::new(client))
            }
            _ => {
                tracing::warn!("Indexarr enabled but no api_key configured — skipping");
                None
            }
        }
    } else {
        None
    };

    // 11. Initialize IndexerManager from database
    let indexer_manager = {
        let mut mgr = IndexerManager::new();
        if let Some(ref client) = indexarr_client {
            mgr.set_indexarr(Arc::clone(client));
        }
        match sqlx::query_as::<_, (i32, String, String, Option<String>, String, bool)>(
            "SELECT id, name, base_url, api_key, protocol, enabled FROM indexers ORDER BY priority, id",
        )
        .fetch_all(db.pool())
        .await
        {
            Ok(rows) => {
                for (id, name, base_url, api_key, protocol, enabled) in rows {
                    let proto = match protocol.as_str() {
                        "torrent" => stackarr_indexer::newznab::Protocol::Torrent,
                        _ => stackarr_indexer::newznab::Protocol::Usenet,
                    };
                    mgr.add_indexer(id as i64, &name, &base_url, api_key.as_deref().unwrap_or(""), proto);
                    if !enabled {
                        mgr.set_enabled(id as i64, false);
                    }
                }
                tracing::info!(count = mgr.len(), "loaded indexers from database");
            }
            Err(e) => tracing::warn!(error = %e, "failed to load indexers from database"),
        }
        Arc::new(RwLock::new(mgr))
    };

    // 12. Initialize DownloadClientManager from database
    let download_manager = {
        let mut mgr = DownloadClientManager::new();
        match sqlx::query_as::<_, (i32, String, String, String, serde_json::Value, bool)>(
            "SELECT id, name, client_type, protocol, config, enabled FROM download_clients WHERE enabled = true ORDER BY priority, id",
        )
        .fetch_all(db.pool())
        .await
        {
            Ok(rows) => {
                for (id, name, client_type, _protocol, cfg, _enabled) in rows {
                    match build_download_client(&client_type, &cfg) {
                        Ok(client) => {
                            mgr.add_client(id as i64, client);
                            tracing::debug!(id, name = %name, client_type = %client_type, "registered download client");
                        }
                        Err(e) => tracing::warn!(id, name = %name, error = %e, "failed to create download client"),
                    }
                }
                tracing::info!(count = mgr.len(), "loaded download clients from database");
            }
            Err(e) => tracing::warn!(error = %e, "failed to load download clients from database"),
        }
        Arc::new(RwLock::new(mgr))
    };

    // 13. Initialize rate limiter (50 requests/second per IP)
    let rate_limiter = Some(stackarr_web::middleware::create_rate_limiter(50));

    // 14. Initialize Cardigann engine (load bundled definitions)
    let cardigann_engine = {
        let definitions_dir = std::path::Path::new("crates/stackarr-cardigann/definitions");
        let mut engine = stackarr_cardigann::CardigannEngine::new(definitions_dir);
        match engine.load_definitions() {
            Ok(count) => tracing::info!(count, "loaded Cardigann indexer definitions"),
            Err(e) => tracing::warn!(error = %e, "failed to load Cardigann definitions"),
        }
        let engine = Arc::new(engine);
        // Share the engine with IndexerManager
        {
            let mut mgr = indexer_manager.write().await;
            mgr.set_cardigann_engine(Arc::clone(&engine));
        }
        engine
    };

    // 15. Initialize streaming server
    let stream_session_manager = if config.streaming.enabled {
        tracing::info!("initializing streaming server");
        let transcode_dir = config
            .streaming
            .transcode_dir
            .clone()
            .unwrap_or_else(|| config.general.data_dir.join("transcode"));
        if let Err(e) = std::fs::create_dir_all(&transcode_dir) {
            tracing::warn!(error = %e, "failed to create transcode directory");
        }
        let mut streaming_config = config.streaming.clone();
        streaming_config.transcode_dir = Some(transcode_dir);
        let mgr = Arc::new(stackarr_stream::SessionManager::new(
            streaming_config,
            db.pool().clone(),
        ));
        mgr.spawn_cleanup_task();
        tracing::info!("streaming server ready");
        Some(mgr)
    } else {
        None
    };

    // 16. Determine listen address
    let listen_addr = format!("{}:{}", config.general.bind_addr, config.general.port);

    // Build shared state
    let state = Arc::new(AppState {
        db,
        config: Arc::new(ArcSwap::new(Arc::new(config))),
        modules,
        torrent_session,
        torrent_api,
        usenet_queue,
        indexarr_client,
        cardigann_engine,
        indexer_manager,
        download_manager,
        rate_limiter,
        stream_session_manager,
    });

    // Start background scheduler
    tracing::info!("starting background scheduler");
    let _scheduler_handle = Scheduler::new(state.db.pool().clone())
        .start()
        .await
        .context("failed to start scheduler")?;

    // Start HTTP server
    tracing::info!(addr = %listen_addr, "starting HTTP server");
    stackarr_web::run(&listen_addr, state).await?;

    tracing::info!("StackArr shut down cleanly");
    Ok(())
}

/// Build a download client instance from the database `client_type` and JSON config.
fn build_download_client(
    client_type: &str,
    config: &serde_json::Value,
) -> anyhow::Result<Box<dyn stackarr_download::DownloadClient>> {
    match client_type {
        "qbittorrent" => {
            let host = config["host"].as_str().unwrap_or("http://localhost:8080");
            let username = config["username"].as_str().unwrap_or("");
            let password = config["password"].as_str().unwrap_or("");
            Ok(Box::new(stackarr_download::qbittorrent::QBittorrentClient::new(
                host, username, password,
            )))
        }
        "transmission" => {
            let host = config["host"].as_str().unwrap_or("http://localhost:9091");
            let username = config["username"].as_str().map(String::from);
            let password = config["password"].as_str().map(String::from);
            Ok(Box::new(stackarr_download::transmission::TransmissionClient::new(
                host, username, password,
            )))
        }
        "sabnzbd" => {
            let host = config["host"].as_str().unwrap_or("http://localhost:8080");
            let api_key = config["apiKey"].as_str().unwrap_or("");
            Ok(Box::new(stackarr_download::sabnzbd::SabnzbdClient::new(
                host, api_key,
            )))
        }
        "nzbget" => {
            let host = config["host"].as_str().unwrap_or("http://localhost:6789");
            let username = config["username"].as_str().unwrap_or("");
            let password = config["password"].as_str().unwrap_or("");
            Ok(Box::new(stackarr_download::nzbget::NzbgetClient::new(
                host, username, password,
            )))
        }
        other => anyhow::bail!("unknown download client type: {other}"),
    }
}
