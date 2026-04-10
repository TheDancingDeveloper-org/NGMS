use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub bootstrap: BootstrapSection,
}

#[derive(Debug, Deserialize)]
pub struct BootstrapSection {
    #[serde(default = "default_bind_addr")]
    pub bind_addr: String,
    #[serde(default = "default_port")]
    pub port: u16,
    pub bootstrap_token: String,
    #[serde(default = "default_registration_ttl")]
    pub registration_ttl_secs: u64,
    #[serde(default = "default_claim_ttl")]
    pub claim_ttl_secs: u64,
    #[serde(default = "default_database_path")]
    pub database_path: String,
    /// Enable the HTTPS relay for proxying API calls to servers.
    #[serde(default = "default_true")]
    pub relay_enabled: bool,
    /// Max request body size for relay (bytes). Default: 10 MB.
    #[serde(default = "default_relay_max_body")]
    pub relay_max_body_bytes: usize,
    /// Timeout for relay upstream requests (seconds). Default: 30.
    #[serde(default = "default_relay_timeout")]
    pub relay_timeout_secs: u64,
    /// Public hostname for the relay (used in relayUrl responses).
    #[serde(default)]
    pub relay_host: Option<String>,
}

fn default_bind_addr() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    7890
}

fn default_registration_ttl() -> u64 {
    120
}

fn default_claim_ttl() -> u64 {
    240
}

fn default_database_path() -> String {
    "/config/bootstrap.db".to_string()
}

fn default_true() -> bool {
    true
}

fn default_relay_max_body() -> usize {
    10 * 1024 * 1024 // 10 MB
}

fn default_relay_timeout() -> u64 {
    30
}
