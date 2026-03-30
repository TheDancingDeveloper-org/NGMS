use std::path::PathBuf;
use std::sync::Arc;

use arc_swap::{ArcSwap, ArcSwapOption};
use ngms_cardigann::CardigannEngine;
use ngms_core::config::{AppConfig, EnabledModules};
use ngms_core::db::Database;
use ngms_download::DownloadClientManager;
use ngms_indexer::{IndexarrClient, IndexerManager};
use ngms_metadata::TmdbClient;
use tokio::sync::RwLock;

/// Shared application state available to all request handlers.
pub struct AppState {
    pub db: Database,
    pub config: Arc<ArcSwap<AppConfig>>,
    pub modules: EnabledModules,
    // Embedded engines (swappable — can be initialized after first-boot setup)
    pub torrent_session: ArcSwapOption<librtbit::Session>,
    pub torrent_api: ArcSwapOption<librtbit::Api>,
    pub usenet_queue: ArcSwapOption<nzb_web::QueueManager>,
    // Indexarr sidecar (initialized when config.indexarr.enabled + api_key)
    pub indexarr_client: Option<Arc<IndexarrClient>>,
    // Whether the Indexarr container is available (NGMS_INDEXARR_ENABLED env var)
    pub indexarr_available: bool,
    // Cardigann engine (always available — loads definitions from disk)
    pub cardigann_engine: Arc<CardigannEngine>,
    // Indexer + download client managers (loaded from DB at startup)
    pub indexer_manager: Arc<RwLock<IndexerManager>>,
    pub download_manager: Arc<RwLock<DownloadClientManager>>,
    // Rate limiter (optional — None disables rate limiting)
    pub rate_limiter: Option<Arc<crate::middleware::KeyedRateLimiter>>,
    // Shared TMDB client (rate-limited + cached, initialized when TMDB key available)
    pub tmdb_client: Option<Arc<TmdbClient>>,
    // Streaming server (initialized when config.streaming.enabled)
    pub stream_session_manager: Option<Arc<ngms_stream::SessionManager>>,
}

impl AppState {
    /// Initialize the embedded torrent engine if not already running.
    /// Called after first-boot setup or module toggle.
    pub async fn init_torrent_engine(&self) {
        if self.torrent_session.load().is_some() {
            return; // already running
        }
        let cfg = self.config.load();
        let download_dir = cfg
            .torrent
            .download_dir
            .clone()
            .unwrap_or_else(|| PathBuf::from("/downloads/torrent"));
        let opts = librtbit::SessionOptions {
            disable_dht: !cfg.torrent.dht_enabled,
            completed_folder: cfg.torrent.complete_dir.clone(),
            ..Default::default()
        };
        match librtbit::Session::new_with_opts(download_dir, opts).await {
            Ok(session) => {
                let api = librtbit::Api::new(Arc::clone(&session), None);
                self.torrent_session.store(Some(session));
                self.torrent_api.store(Some(Arc::new(api)));
                tracing::info!("torrent engine started (post-setup)");
            }
            Err(e) => {
                tracing::error!(error = %e, "failed to start torrent engine (post-setup)");
            }
        }
    }

    /// Initialize the embedded usenet engine if not already running.
    /// Called after first-boot setup or module toggle.
    pub async fn init_usenet_engine(&self) {
        if self.usenet_queue.load().is_some() {
            return; // already running
        }
        let cfg = self.config.load();

        // Load servers from DB
        let db_servers: Vec<nzb_core::config::ServerConfig> = match sqlx::query_as::<_, (i32, serde_json::Value, bool)>(
            "SELECT id, config, enabled FROM download_clients WHERE client_type = 'embedded_usenet' ORDER BY priority, id",
        )
        .fetch_all(self.db.pool())
        .await
        {
            Ok(rows) => {
                let mut servers = Vec::new();
                for (id, cfg_val, enabled) in rows {
                    match serde_json::from_value::<nzb_core::config::ServerConfig>(cfg_val) {
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
                servers
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to load usenet servers from database");
                Vec::new()
            }
        };

        // Convert TOML servers
        let toml_servers: Vec<nzb_core::config::ServerConfig> = cfg
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
                ramp_up_delay_ms: 250,
                proxy_url: s.proxy_url.clone(),
            })
            .collect();

        let mut nzb_servers = toml_servers;
        nzb_servers.extend(db_servers);

        if nzb_servers.is_empty() {
            tracing::info!("no usenet servers configured, skipping engine init (post-setup)");
            return;
        }

        let incomplete_dir = cfg
            .usenet
            .incomplete_dir
            .clone()
            .unwrap_or_else(|| PathBuf::from("/downloads/usenet/incomplete"));
        let complete_dir = cfg
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
                    cfg.usenet.max_active_downloads,
                    Vec::new(),
                    0,
                    0,
                );

                if let Err(e) = queue.restore_from_db() {
                    tracing::warn!(error = %e, "failed to restore usenet queue from database");
                }
                queue.spawn_speed_tracker();

                self.usenet_queue.store(Some(queue));
                tracing::info!("usenet engine started (post-setup)");
            }
            Err(e) => {
                tracing::error!(error = %e, "failed to open usenet queue database (post-setup)");
            }
        }
    }
}
