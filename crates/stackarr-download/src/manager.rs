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
