//! DAV streaming engine manager.
//!
//! Wraps the nzbdav components (provider, store, pipeline processor) into a
//! single `DavManager` struct that lives in `AppState` behind an `ArcSwapOption`.

use std::sync::Arc;

use nzbdav_core::database::DavDatabase;
use nzbdav_dav::DatabaseStore;
use nzbdav_pipeline::queue_item_processor::QueueItemProcessor;
use nzbdav_stream::UsenetArticleProvider;
use nzbdav_stream::nzb_nntp::ConnectionPool;
use sqlx::PgPool;

/// Holds the initialized DAV streaming components.
pub struct DavManager {
    /// Usenet article provider with dedicated connection pools.
    pub provider: Arc<UsenetArticleProvider>,
    /// WebDAV store (resolves paths, builds streaming bodies).
    pub store: Arc<DatabaseStore>,
    /// Pipeline processor for inline NZB processing.
    pub processor: Arc<QueueItemProcessor>,
    /// Database implementation (PostgresDavDatabase).
    pub db: Arc<dyn DavDatabase>,
}

/// Default number of dedicated DAV connections per usenet server.
const DEFAULT_DAV_CONNECTIONS: u16 = 10;

/// Build dedicated nzb-nntp connection pools for the DAV streaming module.
///
/// These are separate from the embedded usenet engine's pools to prevent
/// streaming from starving downloads and vice versa.
pub async fn build_dav_pools(pool: &PgPool) -> Vec<Arc<ConnectionPool>> {
    // Load usenet server configs from download_clients table
    let rows: Vec<(i32, serde_json::Value, bool)> = sqlx::query_as(
        "SELECT id, config, enabled FROM download_clients \
         WHERE client_type = 'embedded_usenet' AND enabled = true \
         ORDER BY priority, id",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_else(|e| {
        tracing::warn!(error = %e, "failed to load usenet servers for DAV pools");
        Vec::new()
    });

    let mut pools = Vec::new();
    for (id, config_json, _enabled) in &rows {
        let host = config_json["host"].as_str().unwrap_or_default();
        if host.is_empty() {
            continue;
        }

        let server_config = nzbdav_stream::nzb_nntp::ServerConfig {
            id: format!("dav-{id}"),
            name: config_json["name"]
                .as_str()
                .unwrap_or("DAV Server")
                .to_string(),
            host: host.to_string(),
            port: config_json["port"].as_u64().unwrap_or(563) as u16,
            ssl: config_json["ssl"].as_bool().unwrap_or(true),
            ssl_verify: config_json["sslVerify"]
                .as_bool()
                .or_else(|| config_json["ssl_verify"].as_bool())
                .unwrap_or(true),
            username: config_json["username"].as_str().map(String::from),
            password: config_json["password"].as_str().map(String::from),
            connections: config_json["davConnections"]
                .as_u64()
                .or_else(|| config_json["dav_connections"].as_u64())
                .unwrap_or(DEFAULT_DAV_CONNECTIONS as u64) as u16,
            priority: config_json["priority"].as_u64().unwrap_or(0) as u8,
            pipelining: 15,
            optional: config_json["optional"].as_bool().unwrap_or(false),
            recv_buffer_size: 0, // OS default
            proxy_url: config_json["proxyUrl"]
                .as_str()
                .or_else(|| config_json["proxy_url"].as_str())
                .map(String::from),
            ..Default::default()
        };

        pools.push(Arc::new(ConnectionPool::new(Arc::new(server_config))));
    }

    pools
}
