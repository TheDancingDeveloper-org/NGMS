/// Wrapper around the embedded nzb usenet download engine.
pub struct EmbeddedUsenetClient {
    // TODO: hold nzb-web engine handle once integrated
}

impl EmbeddedUsenetClient {
    /// Create a new embedded usenet client.
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for EmbeddedUsenetClient {
    fn default() -> Self {
        Self::new()
    }
}
