use std::path::PathBuf;
use std::sync::Arc;

use arc_swap::{ArcSwap, ArcSwapOption};
use stackarr_cardigann::CardigannEngine;
use stackarr_core::config::{AppConfig, EnabledModules};
use stackarr_core::db::Database;
use stackarr_core::log_buffer::LogBuffer;
use stackarr_download::DownloadClientManager;
use stackarr_indexer::{IndexarrClient, IndexerManager};
use stackarr_metadata::TmdbClient;
use tokio::sync::RwLock;

/// Shared application state available to all request handlers.
pub struct AppState {
    pub db: Database,
    pub config: Arc<ArcSwap<AppConfig>>,
    pub modules: EnabledModules,
    /// Timestamp when the application started (for uptime calculation).
    pub start_time: std::time::Instant,
    // Embedded engines (swappable — can be initialized after first-boot setup)
    pub torrent_session: ArcSwapOption<librtbit::Session>,
    pub torrent_api: ArcSwapOption<librtbit::Api>,
    pub usenet_queue: ArcSwapOption<nzb_web::QueueManager>,
    // Indexarr sidecar (initialized when config.indexarr.enabled + api_key)
    pub indexarr_client: Option<Arc<IndexarrClient>>,
    // Whether the Indexarr container is available (STACKARR_INDEXARR_ENABLED env var)
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
    pub stream_session_manager: Option<Arc<stackarr_stream::SessionManager>>,
    // Application-wide in-memory log buffer (captured via tracing layer)
    pub log_buffer: LogBuffer,
    // Cached auth config — avoids DB queries on every request
    pub cached_api_key: ArcSwap<Option<String>>,
    pub cached_auth_method: ArcSwap<String>,
    // Scheduler task registry (populated after scheduler starts)
    pub scheduler_registry: ArcSwapOption<stackarr_scheduler::TaskRegistry>,
    // Cancellation tokens for long-running search tasks (keyed by activity ID)
    pub search_cancel_tokens: dashmap::DashMap<i64, tokio_util::sync::CancellationToken>,
    // DAV streaming engine (initialized when dav_streaming module enabled)
    pub dav_manager: ArcSwapOption<crate::dav_manager::DavManager>,
}

impl AppState {
    /// Load api_key and auth_method from DB into the in-memory cache.
    /// Call once at startup, before serving requests.
    pub async fn load_auth_cache(&self) {
        let pool = self.db.pool();

        let api_key: Option<String> = sqlx::query_scalar::<_, serde_json::Value>(
            "SELECT value FROM app_config WHERE key = 'api_key'",
        )
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .and_then(|v| v.as_str().map(String::from));
        self.cached_api_key.store(Arc::new(api_key));

        let auth_method: String = sqlx::query_scalar::<_, serde_json::Value>(
            "SELECT value FROM app_config WHERE key = 'auth_method'",
        )
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_else(|| "none".to_string());
        self.cached_auth_method.store(Arc::new(auth_method));
    }

    /// Update the cached API key after a DB write.
    pub fn set_cached_api_key(&self, key: Option<String>) {
        self.cached_api_key.store(Arc::new(key));
    }

    /// Update the cached auth method after a DB write.
    pub fn set_cached_auth_method(&self, method: String) {
        self.cached_auth_method.store(Arc::new(method));
    }

    /// Load a directory path from the `app_config` DB table.
    async fn load_dir_setting(&self, key: &str) -> Option<PathBuf> {
        sqlx::query_scalar::<_, serde_json::Value>("SELECT value FROM app_config WHERE key = $1")
            .bind(key)
            .fetch_optional(self.db.pool())
            .await
            .ok()
            .flatten()
            .and_then(|v| v.as_str().map(PathBuf::from))
    }

    /// Initialize the embedded torrent engine if not already running.
    /// Called after first-boot setup or module toggle.
    pub async fn init_torrent_engine(&self) {
        if self.torrent_session.load().is_some() {
            return; // already running
        }
        let cfg = self.config.load();
        let download_dir = self
            .load_dir_setting("torrent_download_dir")
            .await
            .or_else(|| cfg.torrent.download_dir.clone())
            .unwrap_or_else(|| PathBuf::from("/downloads/torrent"));
        let completed_folder = self
            .load_dir_setting("torrent_complete_dir")
            .await
            .or_else(|| cfg.torrent.complete_dir.clone());
        let persistence_dir = download_dir.join(".session");
        let opts = librtbit::SessionOptions {
            disable_dht: !cfg.torrent.dht_enabled,
            completed_folder,
            persistence: Some(librtbit::SessionPersistenceConfig::Json {
                folder: Some(persistence_dir),
            }),
            fastresume: true,
            ..Default::default()
        };
        match librtbit::Session::new_with_opts(download_dir, opts).await {
            Ok(session) => {
                let api = Arc::new(librtbit::Api::new(Arc::clone(&session), None));
                self.torrent_session.store(Some(session));
                self.torrent_api.store(Some(Arc::clone(&api)));

                // Register in download manager for grab dispatch
                let priority = sqlx::query_scalar::<_, serde_json::Value>(
                    "SELECT value FROM app_config WHERE key = 'embedded_torrent_priority'",
                )
                .fetch_optional(self.db.pool())
                .await
                .ok()
                .flatten()
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as i32;
                let client = stackarr_download::embedded_torrent::EmbeddedTorrentClient::new(api);
                let mut mgr = self.download_manager.write().await;
                mgr.remove_client(-1);
                mgr.add_client(-1, Box::new(client), priority);

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
        let db_servers: Vec<nzb_web::nzb_core::config::ServerConfig> = match sqlx::query_as::<_, (i32, serde_json::Value, bool)>(
            "SELECT id, config, enabled FROM download_clients WHERE client_type = 'embedded_usenet' ORDER BY priority, id",
        )
        .fetch_all(self.db.pool())
        .await
        {
            Ok(rows) => {
                let mut servers = Vec::new();
                for (id, cfg_val, enabled) in rows {
                    match serde_json::from_value::<nzb_web::nzb_core::config::ServerConfig>(cfg_val) {
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
        let toml_servers: Vec<nzb_web::nzb_core::config::ServerConfig> = cfg
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

        let mut nzb_servers = toml_servers;
        nzb_servers.extend(db_servers);

        if nzb_servers.is_empty() {
            tracing::info!("no usenet servers configured, skipping engine init (post-setup)");
            return;
        }

        // Load from DB first, then TOML, then hardcoded default
        let incomplete_dir = self
            .load_dir_setting("usenet_incomplete_dir")
            .await
            .or_else(|| cfg.usenet.incomplete_dir.clone())
            .unwrap_or_else(|| PathBuf::from("/downloads/usenet/incomplete"));
        let complete_dir = self
            .load_dir_setting("usenet_complete_dir")
            .await
            .or_else(|| cfg.usenet.complete_dir.clone())
            .unwrap_or_else(|| PathBuf::from("/downloads/usenet/complete"));

        if let Err(e) = tokio::fs::create_dir_all(&incomplete_dir).await {
            tracing::warn!(error = %e, "failed to create usenet incomplete dir");
        }
        if let Err(e) = tokio::fs::create_dir_all(&complete_dir).await {
            tracing::warn!(error = %e, "failed to create usenet complete dir");
        }

        let db_path = incomplete_dir.join("usenet_queue.db");
        match nzb_web::nzb_core::db::Database::open(&db_path) {
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
                    cfg.usenet.direct_unpack,
                    true,  // abort_hopeless
                    true,  // early_failure_check
                    100.2, // required_completion_pct
                );

                if let Err(e) = queue.restore_from_db() {
                    tracing::warn!(error = %e, "failed to restore usenet queue from database");
                }
                queue.spawn_speed_tracker();

                // Register in download manager for grab dispatch
                let priority = sqlx::query_scalar::<_, serde_json::Value>(
                    "SELECT value FROM app_config WHERE key = 'embedded_usenet_priority'",
                )
                .fetch_optional(self.db.pool())
                .await
                .ok()
                .flatten()
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as i32;
                let client = stackarr_download::embedded_usenet::EmbeddedUsenetClient::new(
                    Arc::clone(&queue),
                );
                let mut mgr = self.download_manager.write().await;
                mgr.remove_client(-2);
                mgr.add_client(-2, Box::new(client), priority);
                drop(mgr);

                self.usenet_queue.store(Some(queue));
                tracing::info!("usenet engine started (post-setup)");
            }
            Err(e) => {
                tracing::error!(error = %e, "failed to open usenet queue database (post-setup)");
            }
        }
    }

    /// Initialize the DAV streaming engine (idempotent).
    ///
    /// Builds dedicated nzb-nntp connection pools from configured usenet
    /// servers, creates the provider, store, pipeline processor.
    pub async fn init_dav_engine(&self) {
        if self.dav_manager.load().is_some() {
            return; // already running
        }

        // Build dedicated pools from configured usenet servers
        let dav_pools = crate::dav_manager::build_dav_pools(self.db.pool()).await;
        if dav_pools.is_empty() {
            tracing::info!("no usenet servers configured — DAV engine starting with empty pools");
        }

        let provider = Arc::new(nzbdav_stream::UsenetArticleProvider::new(dav_pools));
        let dav_db: Arc<dyn nzbdav_core::database::DavDatabase> = Arc::new(
            stackarr_core::dav_db::PostgresDavDatabase::new(self.db.pool().clone()),
        );

        // Seed root DAV filesystem items
        if let Err(e) = nzbdav_core::seed::seed_root_items(&*dav_db).await {
            tracing::error!(error = %e, "failed to seed DAV root items");
            return;
        }

        let lookahead: usize =
            sqlx::query_scalar::<_, String>("SELECT value FROM dav_config WHERE key = 'lookahead'")
                .fetch_optional(self.db.pool())
                .await
                .ok()
                .flatten()
                .and_then(|v| v.parse().ok())
                .unwrap_or(8);
        let store = Arc::new(nzbdav_dav::DatabaseStore::new(
            dav_db.clone(),
            provider.clone(),
            lookahead,
        ));

        let pipeline_config = nzbdav_pipeline::queue_item_processor::PipelineConfig::default();
        let processor = Arc::new(
            nzbdav_pipeline::queue_item_processor::QueueItemProcessor::new(
                provider.clone(),
                pipeline_config,
            ),
        );

        let manager = crate::dav_manager::DavManager {
            provider,
            store,
            processor,
            db: dav_db,
        };

        self.dav_manager.store(Some(Arc::new(manager)));
        tracing::info!("DAV streaming engine started");
    }
}
