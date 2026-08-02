use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use arc_swap::ArcSwap;
use clap::Parser;
use tokio::sync::RwLock;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::time::UtcTime;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

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
    #[arg(
        short,
        long,
        default_value = "/config/stackarr.toml",
        env = "STACKARR_CONFIG"
    )]
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

/// Load a directory path from the `app_config` DB table.
async fn load_dir_setting(pool: &sqlx::MySqlPool, key: &str) -> Option<PathBuf> {
    sqlx::query_scalar::<_, serde_json::Value>("SELECT value FROM app_config WHERE key = ?")
        .bind(key)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .and_then(|v| v.as_str().map(PathBuf::from))
}

#[tokio::main]
async fn main() -> Result<()> {
    // 0. Install rustls crypto provider before any TLS usage
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls CryptoProvider");

    // 1. Parse CLI args
    let cli = Cli::parse();

    // 2. Init tracing EARLY so config-load errors are visible
    let log_buffer = stackarr_core::log_buffer::LogBuffer::new();
    let buffer_layer = stackarr_core::log_buffer::LogBufferLayer::new(log_buffer.clone());

    // Create nzb-web log buffer early so its tracing layer captures job_id fields
    let nzb_log_buffer = nzb_web::LogBuffer::new();
    let nzb_buffer_layer = nzb_web::LogBufferLayer::new(nzb_log_buffer.clone());

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&cli.log_level));
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_timer(UtcTime::rfc_3339());

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .with(buffer_layer)
        .with(nzb_buffer_layer)
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

    // 4b. Validate config
    config
        .validate()
        .context("configuration validation failed")?;

    // 5. Connect to database
    let db = Database::connect(&config.database)
        .await
        .context("failed to connect to database")?;

    // 6. Run migrations
    tracing::info!("running database migrations");
    db.run_migrations().await.context("migration failed")?;

    // 6b. Clean up stale activities from prior runs
    match db.cleanup_stale_activities().await {
        Ok(n) if n > 0 => tracing::info!(
            count = n,
            "cleaned up stale running activities from prior shutdown"
        ),
        Ok(_) => {}
        Err(e) => tracing::warn!(error = %e, "failed to clean up stale activities"),
    }

    // 6c. Ensure stable server identity exists
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

    // 7. Load enabled modules from DB and reconcile with TOML config
    let modules = db.load_enabled_modules().await.unwrap_or_else(|e| {
        tracing::warn!(error = %e, "failed to load enabled modules, using defaults");
        EnabledModules::default()
    });

    if db.is_first_boot().await.unwrap_or(true) {
        tracing::info!("first boot detected — no modules enabled yet");
    } else {
        // DB module states (set via first-boot setup or UI) override TOML defaults
        if modules.torrent_embedded && !config.torrent.enabled {
            tracing::info!("enabling torrent engine (enabled in DB)");
            config.torrent.enabled = true;
        }
        if modules.usenet_embedded && !config.usenet.enabled {
            tracing::info!("enabling usenet engine (enabled in DB)");
            config.usenet.enabled = true;
        }
        if modules.streaming && !config.streaming.enabled {
            tracing::info!("enabling streaming server (enabled in DB)");
            config.streaming.enabled = true;
        }
        if modules.indexarr_sidecar && !config.indexarr.enabled {
            tracing::info!("enabling Indexarr sidecar (enabled in DB)");
            config.indexarr.enabled = true;
        }
        // Load Indexarr URL + API key from app_config if not in TOML
        if config.indexarr.enabled {
            if config
                .indexarr
                .api_key
                .as_ref()
                .is_none_or(|k| k.is_empty())
                && let Ok(Some(val)) = sqlx::query_scalar::<_, serde_json::Value>(
                    "SELECT value FROM app_config WHERE key = 'indexarr_api_key'",
                )
                .fetch_optional(db.pool())
                .await
                && let Some(key) = val.as_str().filter(|s| !s.is_empty())
            {
                tracing::info!("loaded Indexarr api_key from app_config");
                config.indexarr.api_key = Some(key.to_string());
            }
            if config.indexarr.url.is_empty()
                && let Ok(Some(val)) = sqlx::query_scalar::<_, serde_json::Value>(
                    "SELECT value FROM app_config WHERE key = 'indexarr_url'",
                )
                .fetch_optional(db.pool())
                .await
                && let Some(url) = val.as_str().filter(|s| !s.is_empty())
            {
                tracing::info!(url = %url, "loaded Indexarr URL from app_config");
                config.indexarr.url = url.to_string();
            }
        }
        if modules.remote_access && !config.bootstrap.enabled {
            tracing::info!("enabling bootstrap/remote access (enabled in DB)");
            config.bootstrap.enabled = true;
        }
    }

    // 8. Initialize embedded torrent engine
    let torrent_session = if config.torrent.enabled {
        tracing::info!("initializing embedded torrent engine");
        let download_dir = load_dir_setting(db.pool(), "torrent_download_dir")
            .await
            .or_else(|| config.torrent.download_dir.clone())
            .unwrap_or_else(|| PathBuf::from("/downloads/torrent"));
        let completed_folder = load_dir_setting(db.pool(), "torrent_complete_dir")
            .await
            .or_else(|| config.torrent.complete_dir.clone());
        let persistence_dir = download_dir.join(".session");
        let opts = librtbit::SessionOptions {
            disable_dht: !config.torrent.dht_enabled,
            completed_folder,
            persistence: Some(librtbit::SessionPersistenceConfig::Json {
                folder: Some(persistence_dir),
            }),
            fastresume: true,
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

    let torrent_api = torrent_session
        .as_ref()
        .map(|s| Arc::new(librtbit::Api::new(Arc::clone(s), None)));

    // 9. Initialize embedded usenet engine
    //    Merge servers from TOML config AND database (embedded_usenet download_clients)
    let usenet_queue = if config.usenet.enabled {
        tracing::info!("initializing embedded usenet engine");

        // Load embedded_usenet servers from the database
        let db_usenet_servers: Vec<nzb_web::nzb_core::config::ServerConfig> = match sqlx::query_as::<_, (i32, serde_json::Value, bool)>(
            "SELECT id, config, enabled FROM download_clients WHERE client_type = 'embedded_usenet' ORDER BY priority, id",
        )
        .fetch_all(db.pool())
        .await
        {
            Ok(rows) => {
                let mut servers = Vec::new();
                for (id, cfg, enabled) in rows {
                    match serde_json::from_value::<nzb_web::nzb_core::config::ServerConfig>(cfg) {
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
        let toml_servers: Vec<nzb_web::nzb_core::config::ServerConfig> = config
            .usenet
            .servers
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let mut c =
                    nzb_web::nzb_core::config::ServerConfig::new(format!("server-{i}"), &s.host);
                c.name = s.name.clone();
                c.port = s.port;
                c.ssl = s.ssl;
                c.ssl_verify = s.ssl;
                c.username = s.username.clone();
                c.password = s.password.clone();
                c.connections = s.connections;
                c.priority = s.priority;
                c.pipelining = 15;
                c.recv_buffer_size = 0;
                c.proxy_url = s.proxy_url.clone();
                c
            })
            .collect();

        // Merge: TOML servers first, then DB servers
        let mut nzb_servers = toml_servers;
        nzb_servers.extend(db_usenet_servers);

        if nzb_servers.is_empty() {
            tracing::info!("no usenet servers configured, skipping engine init");
            None
        } else {
            let incomplete_dir = load_dir_setting(db.pool(), "usenet_incomplete_dir")
                .await
                .or_else(|| config.usenet.incomplete_dir.clone())
                .unwrap_or_else(|| PathBuf::from("/downloads/usenet/incomplete"));
            let complete_dir = load_dir_setting(db.pool(), "usenet_complete_dir")
                .await
                .or_else(|| config.usenet.complete_dir.clone())
                .unwrap_or_else(|| PathBuf::from("/downloads/usenet/complete"));

            if let Err(e) = std::fs::create_dir_all(&incomplete_dir) {
                tracing::warn!(error = %e, "failed to create usenet incomplete dir");
            }
            if let Err(e) = std::fs::create_dir_all(&complete_dir) {
                tracing::warn!(error = %e, "failed to create usenet complete dir");
            }

            let db_path = incomplete_dir.join("usenet_queue.db");
            match nzb_web::nzb_core::db::Database::open(&db_path) {
                Ok(nzb_db) => {
                    let log_buffer = nzb_log_buffer.clone();
                    // Load max_active_downloads: DB override > TOML config > default
                    let max_active = match sqlx::query_scalar::<_, serde_json::Value>(
                        "SELECT value FROM app_config WHERE key = 'usenet_max_active_downloads'",
                    )
                    .fetch_optional(db.pool())
                    .await
                    {
                        Ok(Some(v)) => v
                            .as_u64()
                            .unwrap_or(config.usenet.max_active_downloads as u64)
                            as usize,
                        _ => config.usenet.max_active_downloads,
                    };

                    let queue = nzb_web::QueueManager::new(
                        nzb_servers,
                        nzb_db,
                        incomplete_dir,
                        complete_dir,
                        log_buffer,
                        max_active,
                        Vec::new(),
                        0,
                        0,
                        config.usenet.direct_unpack,
                        config.usenet.max_nested_archive_depth,
                        true,  // abort_hopeless
                        true,  // early_failure_check
                        100.2, // required_completion_pct
                        config.usenet.article_timeout_secs,
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
                let client = stackarr_indexer::IndexarrClient::new(&config.indexarr.url, api_key);
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

    // 11. Initialize Cardigann engine (load bundled definitions) — must happen before IndexerManager
    let cardigann_engine = {
        let definitions_dir = &config.general.definitions_dir;
        let mut engine = stackarr_cardigann::CardigannEngine::new(definitions_dir);
        match engine.load_definitions() {
            Ok(count) => tracing::info!(count, "loaded Cardigann indexer definitions"),
            Err(e) => tracing::warn!(error = %e, "failed to load Cardigann definitions"),
        }
        Arc::new(engine)
    };

    // 12. Initialize IndexerManager from database (uses Cardigann engine for Cardigann indexers)
    let indexer_manager = {
        let mut mgr = IndexerManager::new();
        mgr.set_cardigann_engine(Arc::clone(&cardigann_engine));
        if let Some(ref client) = indexarr_client {
            mgr.set_indexarr(Arc::clone(client));
        }
        match sqlx::query_as::<_, (i32, String, String, Option<String>, String, bool, i32, String, Option<serde_json::Value>)>(
            "SELECT id, name, base_url, api_key, protocol, enabled, priority, indexer_type, config FROM indexers ORDER BY priority, id",
        )
        .fetch_all(db.pool())
        .await
        {
            Ok(rows) => {
                for (id, name, base_url, api_key, protocol, enabled, priority, indexer_type, config) in rows {
                    if indexer_type.eq_ignore_ascii_case("cardigann") {
                        // Load Cardigann indexer from definition + config
                        let def_file = config
                            .as_ref()
                            .and_then(|c| c.get("definitionFile"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        if let Some(def) = cardigann_engine.get_definition(def_file) {
                            let mut idx_config = std::collections::HashMap::new();
                            idx_config.insert("baseUrl".into(), base_url.clone());
                            if let Some(ref key) = api_key {
                                idx_config.insert("apiKey".into(), key.clone());
                            }
                            if let Some(serde_json::Value::Object(map)) = config.as_ref() {
                                for (k, v) in map {
                                    if k != "definitionFile" {
                                        let val = match v {
                                            serde_json::Value::String(s) => s.clone(),
                                            serde_json::Value::Number(n) => n.to_string(),
                                            serde_json::Value::Bool(b) => b.to_string(),
                                            _ => continue,
                                        };
                                        idx_config.insert(k.clone(), val);
                                    }
                                }
                            }
                            match stackarr_cardigann::search::CardigannIndexer::new(
                                def.clone(), idx_config, id as i64,
                            ) {
                                Ok(indexer) => {
                                    mgr.add_cardigann_indexer(id as i64, &name, indexer, priority);
                                }
                                Err(e) => tracing::warn!(name, error = %e, "failed to create Cardigann indexer"),
                            }
                        } else {
                            tracing::warn!(name, definition = def_file, "Cardigann definition not found");
                        }
                    } else {
                        let proto = match protocol.as_str() {
                            "torrent" => stackarr_indexer::newznab::Protocol::Torrent,
                            _ => stackarr_indexer::newznab::Protocol::Usenet,
                        };
                        mgr.add_indexer(id as i64, &name, &base_url, api_key.as_deref().unwrap_or(""), proto, priority);
                    }
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

    // 13. Initialize DownloadClientManager from database
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
                        Err(e) => {
                            tracing::warn!(id, name = %name, error = %e, "failed to create download client")
                        }
                    }
                }
                tracing::info!(count = mgr.len(), "loaded download clients from database");
            }
            Err(e) => tracing::warn!(error = %e, "failed to load download clients from database"),
        }

        // Resolve archive dirs once up front — both clients share the config.
        let archive_torrent_dir = if config.storage.archive.enabled {
            Some(
                config
                    .storage
                    .archive
                    .resolved_torrent_dir(&config.general.data_dir),
            )
        } else {
            None
        };
        let archive_nzb_dir = if config.storage.archive.enabled {
            Some(
                config
                    .storage
                    .archive
                    .resolved_nzb_dir(&config.general.data_dir),
            )
        } else {
            None
        };

        // Register embedded engines in the download manager so they participate
        // in priority-based grab dispatch alongside external clients.
        if let Some(ref api) = torrent_api {
            let priority = sqlx::query_scalar::<_, serde_json::Value>(
                "SELECT value FROM app_config WHERE key = 'embedded_torrent_priority'",
            )
            .fetch_optional(db.pool())
            .await
            .ok()
            .flatten()
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;
            let client =
                stackarr_download::embedded_torrent::EmbeddedTorrentClient::new(Arc::clone(api))
                    .with_archive_dir(archive_torrent_dir.clone());
            mgr.add_client(-1, Box::new(client), priority);
            tracing::info!(priority, "registered embedded torrent client");
        }
        if let Some(ref queue) = usenet_queue {
            let priority = sqlx::query_scalar::<_, serde_json::Value>(
                "SELECT value FROM app_config WHERE key = 'embedded_usenet_priority'",
            )
            .fetch_optional(db.pool())
            .await
            .ok()
            .flatten()
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;
            let client =
                stackarr_download::embedded_usenet::EmbeddedUsenetClient::new(Arc::clone(queue))
                    .with_archive_dir(archive_nzb_dir.clone());
            mgr.add_client(-2, Box::new(client), priority);
            tracing::info!(priority, "registered embedded usenet client");
        }

        Arc::new(RwLock::new(mgr))
    };

    // 14. Initialize rate limiter (50 requests/second per IP)
    let rate_limiter = Some(stackarr_web::middleware::create_rate_limiter(50));

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
        streaming_config.ffmpeg_path = ffmpeg_paths.ffmpeg.clone();
        streaming_config.ffprobe_path = ffmpeg_paths.ffprobe;
        streaming_config.transcode_dir = Some(transcode_dir);

        // Probe hardware acceleration capabilities
        let detected_accel =
            stackarr_stream::probe_hwaccel(&ffmpeg_paths.ffmpeg, &streaming_config.hwaccel).await;
        tracing::info!(accel = %detected_accel, "streaming encoder: {detected_accel}");

        let mgr = Arc::new(stackarr_stream::SessionManager::new(
            streaming_config,
            detected_accel,
            db.pool().clone(),
        ));
        mgr.spawn_cleanup_task();
        tracing::info!("streaming server ready");
        Some(mgr)
    } else {
        None
    };

    // 16. Start bootstrap heartbeat (if configured)
    // Shared watch channel: heartbeat publishes cert updates, TLS listener reloads
    let (tls_cert_tx, tls_cert_rx) =
        tokio::sync::watch::channel::<Option<stackarr_web::TlsCertData>>(None);
    if config.bootstrap.enabled {
        let port = config
            .bootstrap
            .advertise_port
            .unwrap_or(config.general.port);
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
            // Read discovery_name or instance_name from DB, fall back to TOML
            let discovery_name: String = {
                let mut name = None;
                for key in &["discovery_name", "instance_name"] {
                    if let Ok(Some(val)) = sqlx::query_scalar::<_, serde_json::Value>(
                        "SELECT value FROM app_config WHERE key = ?",
                    )
                    .bind(key)
                    .fetch_optional(db.pool())
                    .await
                        && let Some(s) = val.as_str()
                        && !s.is_empty()
                    {
                        name = Some(s.to_string());
                        break;
                    }
                }
                name.unwrap_or_else(|| config.general.instance_name.clone())
            };

            tracing::info!(
                %url, %server_id, %port,
                name = %discovery_name,
                "starting bootstrap heartbeat"
            );
            let bootstrap_url = url.clone();
            let bootstrap_token = token.clone();

            tokio::spawn(bootstrap_heartbeat(
                bootstrap_url,
                bootstrap_token,
                server_id,
                discovery_name,
                port,
                config.general.data_dir.clone(),
                tls_cert_tx.clone(),
            ));
        } else {
            tracing::warn!(
                "bootstrap enabled but url and/or token not configured — heartbeat will not start"
            );
        }
    } else {
        tracing::info!("bootstrap disabled");
    }

    // 17. Ensure image cache directory exists
    let image_cache_dir = config.general.data_dir.join("image_cache");
    if let Err(e) = std::fs::create_dir_all(&image_cache_dir) {
        tracing::warn!(error = %e, "failed to create image cache directory");
    }

    // 18. Initialize shared TMDB client (rate-limited + cached)
    let tmdb_client = {
        let key = std::env::var("STACKARR_TMDB_API_KEY")
            .ok()
            .filter(|k| !k.is_empty());
        let key = match key {
            Some(k) => Some(k),
            None => sqlx::query_scalar::<_, serde_json::Value>(
                "SELECT value FROM app_config WHERE key = 'tmdb_api_key'",
            )
            .fetch_optional(db.pool())
            .await
            .ok()
            .flatten()
            .and_then(|v| v.as_str().map(String::from))
            .filter(|s| !s.is_empty()),
        };
        key.map(|k| {
            tracing::info!("initialized shared TMDB client (rate-limited + cached)");
            Arc::new(stackarr_metadata::TmdbClient::new(k))
        })
    };

    // 19. Determine listen address
    let listen_addr = format!("{}:{}", config.general.bind_addr, config.general.port);

    // Check if Indexarr container is available (set by docker-compose env var)
    let indexarr_available = std::env::var("STACKARR_INDEXARR_ENABLED")
        .map(|v| matches!(v.to_lowercase().as_str(), "true" | "1" | "yes"))
        .unwrap_or(false);

    // Build shared state
    let state = Arc::new(AppState {
        db,
        config: Arc::new(ArcSwap::new(Arc::new(config))),
        modules,
        start_time: std::time::Instant::now(),
        torrent_session: arc_swap::ArcSwapOption::new(torrent_session),
        torrent_api: arc_swap::ArcSwapOption::new(torrent_api),
        usenet_queue: arc_swap::ArcSwapOption::new(usenet_queue),
        indexarr_client,
        indexarr_available,
        cardigann_engine,
        indexer_manager,
        download_manager,
        rate_limiter,
        tmdb_client: tmdb_client.clone(),
        stream_session_manager,
        log_buffer,
        cached_api_key: arc_swap::ArcSwap::from_pointee(None),
        cached_auth_method: arc_swap::ArcSwap::from_pointee("none".to_string()),
        scheduler_registry: arc_swap::ArcSwapOption::empty(),
        search_cancel_tokens: Arc::new(dashmap::DashMap::new()),
        dav_manager: arc_swap::ArcSwapOption::empty(),
    });

    // Populate auth cache from DB before serving requests
    state.load_auth_cache().await;

    // Initialize DAV streaming engine if module is enabled
    if state.modules.dav_streaming {
        state.init_dav_engine().await;
    }

    // Start background scheduler
    tracing::info!("starting background scheduler");
    let mut scheduler = Scheduler::new(state.db.pool().clone())
        .with_managers(
            Arc::clone(&state.download_manager),
            Arc::clone(&state.indexer_manager),
        )
        .with_tmdb_client(state.tmdb_client.clone())
        .with_ffprobe_path(state.config.load().streaming.ffprobe_path.clone())
        .with_search_cancel_tokens(Arc::clone(&state.search_cancel_tokens));

    // Wire archive cleanup from storage.archive config (if enabled).
    {
        let cfg = state.config.load();
        if cfg.storage.archive.enabled {
            let arc_cfg = stackarr_scheduler::ArchiveCleanupConfig {
                interval: std::time::Duration::from_secs(
                    cfg.storage.archive.cleanup_interval_hours.max(1) * 3600,
                ),
                torrent_dir: cfg
                    .storage
                    .archive
                    .resolved_torrent_dir(&cfg.general.data_dir),
                nzb_dir: cfg.storage.archive.resolved_nzb_dir(&cfg.general.data_dir),
                nzb_failed_dir: cfg
                    .storage
                    .archive
                    .resolved_nzb_failed_dir(&cfg.general.data_dir),
                max_torrent_files: cfg.storage.archive.max_torrent_files,
                max_nzb_files: cfg.storage.archive.max_nzb_files,
                max_failed_nzb_files: cfg.storage.archive.max_failed_nzb_files,
            };
            // Ensure dirs exist so the download clients have a write target.
            for d in [
                &arc_cfg.torrent_dir,
                &arc_cfg.nzb_dir,
                &arc_cfg.nzb_failed_dir,
            ] {
                if let Err(e) = std::fs::create_dir_all(d) {
                    tracing::warn!(error = %e, dir = %d.display(), "failed to create archive dir");
                }
            }
            scheduler = scheduler.with_archive_cleanup(arc_cfg);
        }
    }

    let scheduler_handle = scheduler
        .start()
        .await
        .context("failed to start scheduler")?;
    state
        .scheduler_registry
        .store(Some(Arc::clone(scheduler_handle.registry())));
    // Keep the handle alive so tasks aren't cancelled
    let _scheduler_handle = scheduler_handle;

    // Start HTTP server (with optional direct TLS listener for mobile streaming)
    tracing::info!(addr = %listen_addr, "starting HTTP server");
    let (tls_enabled, tls_addr) = {
        let cfg = state.config.load();
        if cfg.bootstrap.enabled && cfg.bootstrap.tls_port > 0 {
            (
                true,
                format!("{}:{}", cfg.general.bind_addr, cfg.bootstrap.tls_port),
            )
        } else {
            (false, String::new())
        }
    };
    let tls_cfg = if tls_enabled {
        tracing::info!(addr = %tls_addr, "starting HTTPS listener for direct streaming");
        Some(stackarr_web::TlsListenerConfig {
            addr: tls_addr,
            cert_rx: tls_cert_rx,
        })
    } else {
        None
    };
    stackarr_web::run_with_tls(&listen_addr, state, tls_cfg).await?;

    tracing::info!("StackArr shut down cleanly");
    Ok(())
}

async fn bootstrap_heartbeat(
    url: String,
    token: String,
    server_id: uuid::Uuid,
    server_name: String,
    port: u16,
    data_dir: std::path::PathBuf,
    tls_cert_tx: tokio::sync::watch::Sender<Option<stackarr_web::TlsCertData>>,
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

    // Cert storage paths
    let tls_dir = data_dir.join("tls");
    let cert_path = tls_dir.join("cert.pem");
    let key_path = tls_dir.join("key.pem");
    let fingerprint_path = tls_dir.join("fingerprint");
    let _ = tokio::fs::create_dir_all(&tls_dir).await;

    // Load cached fingerprint (if any) from previous runs
    let mut current_fingerprint: Option<String> =
        tokio::fs::read_to_string(&fingerprint_path).await.ok();

    // If we have a cached cert on disk, prime the watch channel so the
    // TLS listener can start serving immediately
    if let (Ok(cert_pem), Ok(key_pem)) = (
        tokio::fs::read(&cert_path).await,
        tokio::fs::read(&key_path).await,
    ) {
        let _ = tls_cert_tx.send(Some(stackarr_web::TlsCertData { cert_pem, key_pem }));
        tracing::info!("TLS cert loaded from disk cache");
    }

    let mut first = true;
    loop {
        interval.tick().await;

        let mut req_body = serde_json::json!({
            "serverId": server_id,
            "serverName": server_name,
            "localIps": local_ips,
            "port": port,
            "version": env!("CARGO_PKG_VERSION"),
        });
        if let Some(fp) = current_fingerprint.as_ref() {
            req_body["tlsCertFingerprint"] = serde_json::Value::String(fp.clone());
        }

        let res = client
            .post(format!("{url}/api/v1/servers/register"))
            .bearer_auth(&token)
            .json(&req_body)
            .send()
            .await;

        match res {
            Ok(r) if r.status().is_success() => {
                let body: serde_json::Value = match r.json().await {
                    Ok(b) => b,
                    Err(e) => {
                        tracing::warn!(error = %e, "failed to parse heartbeat response");
                        continue;
                    }
                };

                if first {
                    tracing::info!(
                        public_ip = body
                            .get("publicIp")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown"),
                        ttl_secs = body.get("ttlSecs").and_then(|v| v.as_u64()).unwrap_or(0),
                        tls_domain = body.get("tlsDomain").and_then(|v| v.as_str()).unwrap_or(""),
                        "bootstrap heartbeat: registered successfully"
                    );
                    first = false;
                }

                // If the server returned a new cert, write it to disk and
                // notify the TLS listener to reload
                if let (Some(cert_pem), Some(key_pem), Some(fingerprint)) = (
                    body.get("tlsCertPem").and_then(|v| v.as_str()),
                    body.get("tlsKeyPem").and_then(|v| v.as_str()),
                    body.get("tlsCertFingerprint").and_then(|v| v.as_str()),
                ) {
                    tracing::info!(
                        fingerprint = &fingerprint[..fingerprint.len().min(12)],
                        "bootstrap delivered new TLS cert"
                    );
                    if let Err(e) = tokio::fs::write(&cert_path, cert_pem).await {
                        tracing::warn!(error = %e, "failed to write cert");
                        continue;
                    }
                    if let Err(e) = tokio::fs::write(&key_path, key_pem).await {
                        tracing::warn!(error = %e, "failed to write key");
                        continue;
                    }
                    let _ = tokio::fs::write(&fingerprint_path, fingerprint).await;
                    current_fingerprint = Some(fingerprint.to_string());
                    let _ = tls_cert_tx.send(Some(stackarr_web::TlsCertData {
                        cert_pem: cert_pem.as_bytes().to_vec(),
                        key_pem: key_pem.as_bytes().to_vec(),
                    }));
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
