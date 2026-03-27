use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use arc_swap::ArcSwap;
use clap::Parser;
use tokio::sync::RwLock;
use tracing_subscriber::fmt::time::UtcTime;
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
        /// Remap a path prefix: --path-map /old/path=/new/path (repeatable)
        #[arg(long = "path-map", value_name = "FROM=TO")]
        path_maps: Vec<String>,
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
        .with_timer(UtcTime::rfc_3339())
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

    // 6b. Ensure stable server identity exists
    let server_id = db
        .ensure_server_id()
        .await
        .context("failed to ensure server identity")?;
    tracing::info!(%server_id, "server identity loaded");

    // First-boot admin setup is handled via POST /api/v1/auth/setup

    // Handle subcommands
    match cli.command {
        Some(Commands::Migrate {
            sonarr,
            radarr,
            prowlarr,
            path_maps,
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

            let path_mappings: Vec<stackarr_migrate::PathMapping> = path_maps
                .iter()
                .filter_map(|s| {
                    let (from, to) = s.split_once('=')?;
                    Some(stackarr_migrate::PathMapping {
                        from: from.to_string(),
                        to: to.to_string(),
                    })
                })
                .collect();

            let report = stackarr_migrate::run_migration(
                db.pool(),
                sonarr.as_deref(),
                radarr.as_deref(),
                prowlarr.as_deref(),
                &path_mappings,
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
    //    Merge servers from TOML config AND database (embedded_usenet download_clients)
    let usenet_queue = if config.usenet.enabled {
        tracing::info!("initializing embedded usenet engine");

        // Load embedded_usenet servers from the database
        let db_usenet_servers: Vec<nzb_core::config::ServerConfig> = match sqlx::query_as::<_, (i32, serde_json::Value, bool)>(
            "SELECT id, config, enabled FROM download_clients WHERE client_type = 'embedded_usenet' ORDER BY priority, id",
        )
        .fetch_all(db.pool())
        .await
        {
            Ok(rows) => {
                let mut servers = Vec::new();
                for (id, cfg, enabled) in rows {
                    match serde_json::from_value::<nzb_core::config::ServerConfig>(cfg) {
                        Ok(mut s) => {
                            s.enabled = enabled;
                            if s.id.is_empty() {
                                s.id = format!("db-server-{id}");
                            }
                            servers.push(s);
                        }
                        Err(e) => tracing::warn!(id, error = %e, "failed to parse embedded usenet server config"),
                    }
                }
                tracing::info!(count = servers.len(), "loaded usenet servers from database");
                servers
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to load usenet servers from database");
                Vec::new()
            }
        };

        // Convert TOML config servers
        let toml_servers: Vec<nzb_core::config::ServerConfig> = config
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
                ssl_verify: s.ssl,
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

        // Merge: TOML servers first, then DB servers
        let mut nzb_servers = toml_servers;
        nzb_servers.extend(db_usenet_servers);

        if nzb_servers.is_empty() {
            tracing::info!("no usenet servers configured, skipping engine init");
            None
        } else {
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

            if let Err(e) = std::fs::create_dir_all(&incomplete_dir) {
                tracing::warn!(error = %e, "failed to create usenet incomplete dir");
            }
            if let Err(e) = std::fs::create_dir_all(&complete_dir) {
                tracing::warn!(error = %e, "failed to create usenet complete dir");
            }

            let db_path = incomplete_dir.join("usenet_queue.db");
            match nzb_core::db::Database::open(&db_path) {
                Ok(nzb_db) => {
                    let log_buffer = nzb_web::LogBuffer::default();
                    let queue = nzb_web::QueueManager::new(
                        nzb_servers,
                        nzb_db,
                        incomplete_dir,
                        complete_dir,
                        log_buffer,
                        config.usenet.max_active_downloads,
                        Vec::new(),
                        0,
                        0,
                    );

                    if let Err(e) = queue.restore_from_db() {
                        tracing::warn!(error = %e, "failed to restore usenet queue from database");
                    }
                    queue.spawn_speed_tracker();

                    tracing::info!("usenet engine started");
                    Some(queue)
                }
                Err(e) => {
                    tracing::error!(error = %e, "failed to open usenet database");
                    None
                }
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
        match sqlx::query_as::<_, (i32, String, String, Option<String>, String, bool, i32)>(
            "SELECT id, name, base_url, api_key, protocol, enabled, priority FROM indexers ORDER BY priority, id",
        )
        .fetch_all(db.pool())
        .await
        {
            Ok(rows) => {
                for (id, name, base_url, api_key, protocol, enabled, priority) in rows {
                    let proto = match protocol.as_str() {
                        "torrent" => stackarr_indexer::newznab::Protocol::Torrent,
                        _ => stackarr_indexer::newznab::Protocol::Usenet,
                    };
                    mgr.add_indexer(id as i64, &name, &base_url, api_key.as_deref().unwrap_or(""), proto, priority);
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
    //     Skip embedded_usenet — those are handled by the usenet engine above.
    let download_manager = {
        let mut mgr = DownloadClientManager::new();
        match sqlx::query_as::<_, (i32, String, String, String, serde_json::Value, bool, i32)>(
            "SELECT id, name, client_type, protocol, config, enabled, priority \
             FROM download_clients \
             WHERE enabled = true AND client_type != 'embedded_usenet' \
             ORDER BY priority, id",
        )
        .fetch_all(db.pool())
        .await
        {
            Ok(rows) => {
                for (id, name, client_type, _protocol, cfg, _enabled, priority) in rows {
                    match stackarr_download::build_from_config(&client_type, &cfg) {
                        Ok(client) => {
                            mgr.add_client(id as i64, client, priority);
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

        // Ensure ffmpeg/ffprobe are available (downloads static builds if needed)
        let ffmpeg_paths = stackarr_stream::ensure_ffmpeg(
            &config.streaming.ffmpeg_path,
            &config.streaming.ffprobe_path,
            &config.general.data_dir,
        )
        .await
        .context("failed to provision ffmpeg/ffprobe")?;

        let transcode_dir = config
            .streaming
            .transcode_dir
            .clone()
            .unwrap_or_else(|| config.general.data_dir.join("transcode"));
        if let Err(e) = std::fs::create_dir_all(&transcode_dir) {
            tracing::warn!(error = %e, "failed to create transcode directory");
        }
        let mut streaming_config = config.streaming.clone();
        streaming_config.ffmpeg_path = ffmpeg_paths.ffmpeg;
        streaming_config.ffprobe_path = ffmpeg_paths.ffprobe;
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

    // 16. Start bootstrap heartbeat (if configured)
    if config.bootstrap.enabled {
        let port = config.bootstrap.advertise_port.unwrap_or(config.general.port);
        tracing::info!(
            %port,
            url = config.bootstrap.url.as_deref().unwrap_or("(none)"),
            upnp = config.bootstrap.upnp_enabled,
            "bootstrap enabled — advertise_port={port}"
        );

        // UPnP port forwarding (if enabled)
        if config.bootstrap.upnp_enabled {
            tracing::info!(%port, "attempting UPnP port forward");
            match librtbit_upnp::UpnpPortForwarder::new(vec![port], None, None) {
                Ok(forwarder) => {
                    tokio::spawn(async move {
                        forwarder.run_forever().await;
                    });
                    tracing::info!(%port, "UPnP port forwarder running");
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e, %port,
                        "UPnP port forward failed — configure port forwarding on your router manually"
                    );
                }
            }
        }

        if let (Some(url), Some(token)) = (&config.bootstrap.url, &config.bootstrap.token) {
            tracing::info!(
                %url, %server_id, %port,
                name = %config.general.instance_name,
                "starting bootstrap heartbeat"
            );
            let bootstrap_url = url.clone();
            let bootstrap_token = token.clone();
            let instance_name = config.general.instance_name.clone();

            tokio::spawn(bootstrap_heartbeat(
                bootstrap_url,
                bootstrap_token,
                server_id,
                instance_name,
                port,
            ));
        } else {
            tracing::warn!("bootstrap enabled but url and/or token not configured — heartbeat will not start");
        }
    } else {
        tracing::info!("bootstrap disabled");
    }

    // 17. Ensure image cache directory exists
    let image_cache_dir = config.general.data_dir.join("image_cache");
    if let Err(e) = std::fs::create_dir_all(&image_cache_dir) {
        tracing::warn!(error = %e, "failed to create image cache directory");
    }

    // 18. Determine listen address
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
        .with_managers(
            Arc::clone(&state.download_manager),
            Arc::clone(&state.indexer_manager),
        )
        .start()
        .await
        .context("failed to start scheduler")?;

    // Start HTTP server
    tracing::info!(addr = %listen_addr, "starting HTTP server");
    stackarr_web::run(&listen_addr, state).await?;

    tracing::info!("StackArr shut down cleanly");
    Ok(())
}

async fn bootstrap_heartbeat(
    url: String,
    token: String,
    server_id: uuid::Uuid,
    server_name: String,
    port: u16,
) {
    use network_interface::{NetworkInterface, NetworkInterfaceConfig};
    use std::net::IpAddr;

    let client = reqwest::Client::new();
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));

    // Detect local IPs once at startup
    let local_ips: Vec<IpAddr> = NetworkInterface::show()
        .unwrap_or_default()
        .iter()
        .flat_map(|iface| iface.addr.iter())
        .map(|addr| addr.ip())
        .filter(|ip| !ip.is_loopback())
        .collect();

    tracing::info!(
        local_ips = ?local_ips,
        %server_id,
        %server_name,
        %port,
        "bootstrap heartbeat starting — target: {url}"
    );

    let mut first = true;
    loop {
        interval.tick().await;
        let res = client
            .post(format!("{url}/api/v1/servers/register"))
            .bearer_auth(&token)
            .json(&serde_json::json!({
                "serverId": server_id,
                "serverName": server_name,
                "localIps": local_ips,
                "port": port,
                "version": env!("CARGO_PKG_VERSION"),
            }))
            .send()
            .await;

        match res {
            Ok(r) if r.status().is_success() => {
                if first {
                    // Log the response on first success so we can see the public IP
                    if let Ok(body) = r.json::<serde_json::Value>().await {
                        tracing::info!(
                            public_ip = body.get("publicIp").and_then(|v| v.as_str()).unwrap_or("unknown"),
                            ttl_secs = body.get("ttlSecs").and_then(|v| v.as_u64()).unwrap_or(0),
                            "bootstrap heartbeat: registered successfully"
                        );
                    } else {
                        tracing::info!("bootstrap heartbeat: registered successfully");
                    }
                    first = false;
                } else {
                    tracing::debug!("bootstrap heartbeat ok");
                }
            }
            Ok(r) => {
                let status = r.status();
                let body = r.text().await.unwrap_or_default();
                tracing::warn!(%status, %body, "bootstrap heartbeat rejected");
            }
            Err(e) => tracing::warn!(error = %e, "bootstrap heartbeat failed"),
        }
    }
}

