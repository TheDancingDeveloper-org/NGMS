use std::sync::Arc;

use librtbit::Session;

/// Wrapper around the embedded librtbit torrent session.
pub struct EmbeddedTorrentClient {
    session: Arc<Session>,
}

impl EmbeddedTorrentClient {
    /// Create a new embedded torrent client with the given session.
    pub fn new(session: Arc<Session>) -> Self {
        Self { session }
    }

    /// Get a reference to the underlying librtbit session.
    pub fn session(&self) -> &Arc<Session> {
        &self.session
    }
}
