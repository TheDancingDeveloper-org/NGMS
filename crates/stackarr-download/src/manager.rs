use anyhow::bail;
use tracing::warn;

use crate::client::{DownloadClient, DownloadItem, DownloadProtocol, GrabRequest};

/// Manages a collection of download clients and dispatches operations to them.
pub struct DownloadClientManager {
    clients: Vec<(i64, Box<dyn DownloadClient>)>,
}

impl DownloadClientManager {
    pub fn new() -> Self {
        Self {
            clients: Vec::new(),
        }
    }

    /// Register a download client with a database ID.
    pub fn add_client(&mut self, id: i64, client: Box<dyn DownloadClient>) {
        self.clients.push((id, client));
    }

    /// Remove a client by database ID.
    pub fn remove_client(&mut self, id: i64) -> bool {
        let before = self.clients.len();
        self.clients.retain(|(cid, _)| *cid != id);
        self.clients.len() < before
    }

    /// Get a reference to a specific client by database ID.
    pub fn client_by_id(&self, id: i64) -> Option<&dyn DownloadClient> {
        self.clients
            .iter()
            .find(|(cid, _)| *cid == id)
            .map(|(_, c)| c.as_ref())
    }

    /// Poll every registered client and aggregate their download items.
    pub async fn get_items_all(&self) -> Vec<(i64, Vec<DownloadItem>)> {
        let mut results = Vec::new();
        for (id, client) in &self.clients {
            match client.get_items().await {
                Ok(items) => results.push((*id, items)),
                Err(e) => {
                    warn!(client = client.name(), error = %e, "failed to poll download client");
                }
            }
        }
        results
    }

    /// Send a grab request to the first available client that matches the
    /// requested protocol.
    pub async fn grab(&self, request: &GrabRequest) -> anyhow::Result<(i64, String)> {
        for (id, client) in &self.clients {
            if client.protocol() == request.protocol {
                let download_id = client.add(request).await?;
                return Ok((*id, download_id));
            }
        }
        bail!("no {} download client configured", request.protocol);
    }

    /// Return the number of registered clients.
    pub fn len(&self) -> usize {
        self.clients.len()
    }

    /// Whether there are no registered clients.
    pub fn is_empty(&self) -> bool {
        self.clients.is_empty()
    }

    /// List the registered (id, protocol) pairs.
    pub fn registered(&self) -> Vec<(i64, DownloadProtocol)> {
        self.clients
            .iter()
            .map(|(id, c)| (*id, c.protocol()))
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
            Self { id_name: name.to_string(), proto: DownloadProtocol::Torrent }
        }
        fn usenet(name: &str) -> Self {
            Self { id_name: name.to_string(), proto: DownloadProtocol::Usenet }
        }
    }

    #[async_trait::async_trait]
    impl DownloadClient for MockClient {
        fn name(&self) -> &str { &self.id_name }
        fn protocol(&self) -> DownloadProtocol { self.proto }
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
            }])
        }
        async fn remove(&self, _id: &str, _delete_data: bool) -> anyhow::Result<()> { Ok(()) }
        async fn pause(&self, _id: &str) -> anyhow::Result<()> { Ok(()) }
        async fn resume(&self, _id: &str) -> anyhow::Result<()> { Ok(()) }
        async fn test(&self) -> anyhow::Result<()> { Ok(()) }
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
        mgr.add_client(1, Box::new(MockClient::torrent("qbit")));
        assert_eq!(mgr.len(), 1);
        assert!(!mgr.is_empty());
    }

    #[test]
    fn test_manager_remove_client() {
        let mut mgr = DownloadClientManager::new();
        mgr.add_client(1, Box::new(MockClient::torrent("qbit")));
        mgr.add_client(2, Box::new(MockClient::usenet("sab")));
        assert!(mgr.remove_client(1));
        assert_eq!(mgr.len(), 1);
    }

    #[test]
    fn test_manager_remove_nonexistent() {
        let mut mgr = DownloadClientManager::new();
        mgr.add_client(1, Box::new(MockClient::torrent("qbit")));
        assert!(!mgr.remove_client(999));
        assert_eq!(mgr.len(), 1);
    }

    #[test]
    fn test_manager_client_by_id() {
        let mut mgr = DownloadClientManager::new();
        mgr.add_client(5, Box::new(MockClient::torrent("qbit")));
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
        mgr.add_client(1, Box::new(MockClient::torrent("qbit")));
        mgr.add_client(2, Box::new(MockClient::usenet("sab")));
        let pairs = mgr.registered();
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0], (1, DownloadProtocol::Torrent));
        assert_eq!(pairs[1], (2, DownloadProtocol::Usenet));
    }

    #[tokio::test]
    async fn test_manager_grab_selects_protocol() {
        let mut mgr = DownloadClientManager::new();
        mgr.add_client(1, Box::new(MockClient::torrent("qbit")));
        mgr.add_client(2, Box::new(MockClient::usenet("sab")));

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
        mgr.add_client(1, Box::new(MockClient::torrent("qbit")));

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
        mgr.add_client(1, Box::new(MockClient::torrent("qbit")));
        mgr.add_client(2, Box::new(MockClient::usenet("sab")));

        let items = mgr.get_items_all().await;
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].0, 1);
        assert_eq!(items[0].1.len(), 1);
        assert_eq!(items[1].0, 2);
    }
}
