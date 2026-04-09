use anyhow::bail;
use tracing::{debug, info, warn};

use std::sync::Arc;

use crate::client::{DownloadClient, DownloadItem, DownloadProtocol, GrabRequest};

struct ManagedClient {
    id: i64,
    client: Arc<dyn DownloadClient>,
    enabled: bool,
    priority: i32,
}

/// Manages a collection of download clients and dispatches operations to them.
pub struct DownloadClientManager {
    clients: Vec<ManagedClient>,
}

impl DownloadClientManager {
    pub fn new() -> Self {
        Self {
            clients: Vec::new(),
        }
    }

    /// Register a download client with a database ID and priority (lower = higher priority).
    pub fn add_client(&mut self, id: i64, client: Box<dyn DownloadClient>, priority: i32) {
        let client: Arc<dyn DownloadClient> = Arc::from(client);
        debug!(id, name = client.name(), protocol = ?client.protocol(), priority, "registered download client");
        self.clients.push(ManagedClient {
            id,
            client,
            enabled: true,
            priority,
        });
    }

    /// Remove a client by database ID.
    pub fn remove_client(&mut self, id: i64) -> bool {
        let before = self.clients.len();
        self.clients.retain(|c| c.id != id);
        let removed = self.clients.len() < before;
        if removed {
            info!(id, "download client removed");
        }
        removed
    }

    /// Enable or disable a client without removing it.
    pub fn set_enabled(&mut self, id: i64, enabled: bool) {
        if let Some(c) = self.clients.iter_mut().find(|c| c.id == id) {
            debug!(
                id,
                name = c.client.name(),
                enabled,
                "download client enabled changed"
            );
            c.enabled = enabled;
        }
    }

    /// Get a reference to a specific client by database ID (regardless of enabled state).
    pub fn client_by_id(&self, id: i64) -> Option<Arc<dyn DownloadClient>> {
        self.clients
            .iter()
            .find(|c| c.id == id)
            .map(|c| Arc::clone(&c.client))
    }

    /// Return enabled clients matching the given protocol, sorted by priority
    /// (lowest first). Callers can use these outside the lock to perform
    /// network I/O without holding the read guard.
    pub fn grab_candidates(
        &self,
        protocol: DownloadProtocol,
    ) -> Vec<(i64, Arc<dyn DownloadClient>)> {
        let mut candidates: Vec<_> = self
            .clients
            .iter()
            .filter(|c| c.enabled && c.client.protocol() == protocol)
            .collect();
        candidates.sort_by_key(|c| c.priority);
        candidates
            .into_iter()
            .map(|c| (c.id, Arc::clone(&c.client)))
            .collect()
    }

    /// List all registered client IDs (for health check iteration).
    pub fn all_client_ids(&self) -> Vec<i64> {
        self.clients.iter().map(|c| c.id).collect()
    }

    /// Poll every enabled client and aggregate their download items.
    pub async fn get_items_all(&self) -> Vec<(i64, Vec<DownloadItem>)> {
        let mut results = Vec::new();
        for c in &self.clients {
            if !c.enabled {
                continue;
            }
            match c.client.get_items().await {
                Ok(items) => results.push((c.id, items)),
                Err(e) => {
                    warn!(client = c.client.name(), error = %e, "failed to poll download client");
                }
            }
        }
        results
    }

    /// Send a grab request to the highest-priority enabled client matching the
    /// requested protocol. Falls back to the next client on failure.
    pub async fn grab(&self, request: &GrabRequest) -> anyhow::Result<(i64, String)> {
        let mut candidates: Vec<&ManagedClient> = self
            .clients
            .iter()
            .filter(|c| c.enabled && c.client.protocol() == request.protocol)
            .collect();
        candidates.sort_by_key(|c| c.priority);

        for c in candidates {
            match c.client.add(request).await {
                Ok(download_id) => {
                    info!(
                        client = c.client.name(),
                        title = %request.title,
                        download_id = %download_id,
                        "download grabbed successfully"
                    );
                    return Ok((c.id, download_id));
                }
                Err(e) => {
                    warn!(client = c.client.name(), error = %e, "download client failed, trying next");
                }
            }
        }
        bail!("no {} download client available", request.protocol);
    }

    /// Return the number of enabled clients.
    pub fn len(&self) -> usize {
        self.clients.iter().filter(|c| c.enabled).count()
    }

    /// Whether there are no enabled clients.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// List the registered enabled (id, protocol) pairs.
    pub fn registered(&self) -> Vec<(i64, DownloadProtocol)> {
        self.clients
            .iter()
            .filter(|c| c.enabled)
            .map(|c| (c.id, c.client.protocol()))
            .collect()
    }
}

impl Default for DownloadClientManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{ClientStatus, DownloadItemStatus};

    struct MockClient {
        id_name: String,
        proto: DownloadProtocol,
    }

    impl MockClient {
        fn torrent(name: &str) -> Self {
            Self {
                id_name: name.to_string(),
                proto: DownloadProtocol::Torrent,
            }
        }
        fn usenet(name: &str) -> Self {
            Self {
                id_name: name.to_string(),
                proto: DownloadProtocol::Usenet,
            }
        }
    }

    #[async_trait::async_trait]
    impl DownloadClient for MockClient {
        fn name(&self) -> &str {
            &self.id_name
        }
        fn protocol(&self) -> DownloadProtocol {
            self.proto
        }
        async fn add(&self, _request: &GrabRequest) -> anyhow::Result<String> {
            Ok(format!("mock-dl-{}", self.id_name))
        }
        async fn get_items(&self) -> anyhow::Result<Vec<DownloadItem>> {
            Ok(vec![DownloadItem {
                download_id: "item-1".into(),
                title: "Test Download".into(),
                status: DownloadItemStatus::Downloading,
                total_size: 1_000_000,
                remaining_size: 500_000,
                output_path: None,
                category: None,
                can_move_files: true,
                can_be_removed: true,
                protocol: self.proto,
                error_message: None,
            }])
        }
        async fn remove(&self, _id: &str, _delete_data: bool) -> anyhow::Result<()> {
            Ok(())
        }
        async fn pause(&self, _id: &str) -> anyhow::Result<()> {
            Ok(())
        }
        async fn resume(&self, _id: &str) -> anyhow::Result<()> {
            Ok(())
        }
        async fn test(&self) -> anyhow::Result<()> {
            Ok(())
        }
        async fn status(&self) -> anyhow::Result<ClientStatus> {
            Ok(ClientStatus {
                name: self.id_name.clone(),
                protocol: self.proto,
                version: "1.0".into(),
                is_connected: true,
            })
        }
    }

    #[test]
    fn test_manager_new_is_empty() {
        let mgr = DownloadClientManager::new();
        assert!(mgr.is_empty());
        assert_eq!(mgr.len(), 0);
    }

    #[test]
    fn test_manager_add_client() {
        let mut mgr = DownloadClientManager::new();
        mgr.add_client(1, Box::new(MockClient::torrent("qbit")), 5);
        assert_eq!(mgr.len(), 1);
        assert!(!mgr.is_empty());
    }

    #[test]
    fn test_manager_remove_client() {
        let mut mgr = DownloadClientManager::new();
        mgr.add_client(1, Box::new(MockClient::torrent("qbit")), 5);
        mgr.add_client(2, Box::new(MockClient::usenet("sab")), 5);
        assert!(mgr.remove_client(1));
        assert_eq!(mgr.len(), 1);
    }

    #[test]
    fn test_manager_remove_nonexistent() {
        let mut mgr = DownloadClientManager::new();
        mgr.add_client(1, Box::new(MockClient::torrent("qbit")), 5);
        assert!(!mgr.remove_client(999));
        assert_eq!(mgr.len(), 1);
    }

    #[test]
    fn test_manager_client_by_id() {
        let mut mgr = DownloadClientManager::new();
        mgr.add_client(5, Box::new(MockClient::torrent("qbit")), 5);
        let client = mgr.client_by_id(5);
        assert!(client.is_some());
        assert_eq!(client.unwrap().name(), "qbit");
    }

    #[test]
    fn test_manager_client_by_id_missing() {
        let mgr = DownloadClientManager::new();
        assert!(mgr.client_by_id(1).is_none());
    }

    #[test]
    fn test_manager_registered() {
        let mut mgr = DownloadClientManager::new();
        mgr.add_client(1, Box::new(MockClient::torrent("qbit")), 5);
        mgr.add_client(2, Box::new(MockClient::usenet("sab")), 5);
        let pairs = mgr.registered();
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0], (1, DownloadProtocol::Torrent));
        assert_eq!(pairs[1], (2, DownloadProtocol::Usenet));
    }

    #[tokio::test]
    async fn test_manager_grab_selects_protocol() {
        let mut mgr = DownloadClientManager::new();
        mgr.add_client(1, Box::new(MockClient::torrent("qbit")), 5);
        mgr.add_client(2, Box::new(MockClient::usenet("sab")), 5);

        let req = GrabRequest {
            title: "Test Release".into(),
            download_url: "http://example.com/dl".into(),
            category: None,
            protocol: DownloadProtocol::Usenet,
        };
        let (id, dl_id) = mgr.grab(&req).await.expect("grab should succeed");
        assert_eq!(id, 2);
        assert_eq!(dl_id, "mock-dl-sab");
    }

    #[tokio::test]
    async fn test_manager_grab_no_matching_protocol() {
        let mut mgr = DownloadClientManager::new();
        mgr.add_client(1, Box::new(MockClient::torrent("qbit")), 5);

        let req = GrabRequest {
            title: "Test".into(),
            download_url: "http://example.com/dl".into(),
            category: None,
            protocol: DownloadProtocol::Usenet,
        };
        let result = mgr.grab(&req).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("usenet"));
    }

    #[tokio::test]
    async fn test_manager_get_items_all() {
        let mut mgr = DownloadClientManager::new();
        mgr.add_client(1, Box::new(MockClient::torrent("qbit")), 5);
        mgr.add_client(2, Box::new(MockClient::usenet("sab")), 5);

        let items = mgr.get_items_all().await;
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].0, 1);
        assert_eq!(items[0].1.len(), 1);
        assert_eq!(items[1].0, 2);
    }

    #[test]
    fn test_manager_set_enabled_disables() {
        let mut mgr = DownloadClientManager::new();
        mgr.add_client(1, Box::new(MockClient::torrent("qbit")), 5);
        assert_eq!(mgr.len(), 1);
        mgr.set_enabled(1, false);
        assert_eq!(mgr.len(), 0); // len counts only enabled
        assert!(mgr.is_empty());
    }

    #[test]
    fn test_manager_set_enabled_reenables() {
        let mut mgr = DownloadClientManager::new();
        mgr.add_client(1, Box::new(MockClient::torrent("qbit")), 5);
        mgr.set_enabled(1, false);
        assert_eq!(mgr.len(), 0);
        mgr.set_enabled(1, true);
        assert_eq!(mgr.len(), 1);
    }

    #[test]
    fn test_manager_set_enabled_nonexistent_id() {
        let mut mgr = DownloadClientManager::new();
        mgr.add_client(1, Box::new(MockClient::torrent("qbit")), 5);
        mgr.set_enabled(999, false); // Should not panic
        assert_eq!(mgr.len(), 1);
    }

    #[test]
    fn test_manager_disabled_client_not_in_registered() {
        let mut mgr = DownloadClientManager::new();
        mgr.add_client(1, Box::new(MockClient::torrent("qbit")), 5);
        mgr.add_client(2, Box::new(MockClient::usenet("sab")), 5);
        mgr.set_enabled(1, false);
        let pairs = mgr.registered();
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].0, 2);
    }

    #[test]
    fn test_manager_client_by_id_returns_disabled() {
        let mut mgr = DownloadClientManager::new();
        mgr.add_client(1, Box::new(MockClient::torrent("qbit")), 5);
        mgr.set_enabled(1, false);
        // client_by_id returns regardless of enabled state
        assert!(mgr.client_by_id(1).is_some());
    }

    #[test]
    fn test_manager_all_client_ids_includes_disabled() {
        let mut mgr = DownloadClientManager::new();
        mgr.add_client(1, Box::new(MockClient::torrent("qbit")), 5);
        mgr.add_client(2, Box::new(MockClient::usenet("sab")), 5);
        mgr.set_enabled(1, false);
        let ids = mgr.all_client_ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&1));
        assert!(ids.contains(&2));
    }

    #[tokio::test]
    async fn test_manager_grab_respects_priority_order() {
        let mut mgr = DownloadClientManager::new();
        // Add high priority (lower number = higher priority)
        mgr.add_client(1, Box::new(MockClient::torrent("qbit-low")), 10);
        mgr.add_client(2, Box::new(MockClient::torrent("qbit-high")), 1);

        let req = GrabRequest {
            title: "Test".into(),
            download_url: "http://example.com/dl".into(),
            category: None,
            protocol: DownloadProtocol::Torrent,
        };
        let (id, dl_id) = mgr.grab(&req).await.expect("grab should succeed");
        // Should pick priority 1 (id=2) first
        assert_eq!(id, 2);
        assert_eq!(dl_id, "mock-dl-qbit-high");
    }

    #[tokio::test]
    async fn test_manager_grab_skips_disabled_clients() {
        let mut mgr = DownloadClientManager::new();
        mgr.add_client(1, Box::new(MockClient::torrent("qbit1")), 1);
        mgr.add_client(2, Box::new(MockClient::torrent("qbit2")), 5);
        mgr.set_enabled(1, false);

        let req = GrabRequest {
            title: "Test".into(),
            download_url: "http://example.com/dl".into(),
            category: None,
            protocol: DownloadProtocol::Torrent,
        };
        let (id, _) = mgr.grab(&req).await.expect("grab should succeed");
        assert_eq!(id, 2); // Skipped disabled id=1
    }

    #[tokio::test]
    async fn test_manager_get_items_skips_disabled() {
        let mut mgr = DownloadClientManager::new();
        mgr.add_client(1, Box::new(MockClient::torrent("qbit")), 5);
        mgr.add_client(2, Box::new(MockClient::usenet("sab")), 5);
        mgr.set_enabled(1, false);

        let items = mgr.get_items_all().await;
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].0, 2);
    }

    #[tokio::test]
    async fn test_manager_grab_empty_manager() {
        let mgr = DownloadClientManager::new();
        let req = GrabRequest {
            title: "Test".into(),
            download_url: "http://example.com/dl".into(),
            category: None,
            protocol: DownloadProtocol::Torrent,
        };
        let result = mgr.grab(&req).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_manager_default_is_new() {
        let mgr = DownloadClientManager::default();
        assert!(mgr.is_empty());
        assert_eq!(mgr.len(), 0);
    }
}
