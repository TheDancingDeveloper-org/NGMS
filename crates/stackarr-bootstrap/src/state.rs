use std::net::IpAddr;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use rand::RngExt;
use uuid::Uuid;

use crate::config::BootstrapSection;
use crate::db::BootstrapDb;

pub struct BootstrapState {
    pub servers: DashMap<Uuid, ServerRegistration>,
    pub claims: DashMap<String, PendingClaim>,
    pub bootstrap_token: String,
    pub registration_ttl: Duration,
    pub claim_ttl: Duration,
    pub db: BootstrapDb,
    /// Shared HTTP client for relay proxying.
    pub relay_client: reqwest::Client,
    /// Whether relay is enabled.
    pub relay_enabled: bool,
    /// Relay timeout for upstream requests.
    pub relay_timeout: Duration,
    /// Public relay hostname (e.g. "streamrelay.indexarr.net").
    pub relay_host: Option<String>,
}

pub struct ServerRegistration {
    pub server_id: Uuid,
    pub server_name: String,
    pub public_ip: IpAddr,
    pub local_ips: Vec<IpAddr>,
    pub port: u16,
    pub version: String,
    pub last_heartbeat: Instant,
}

pub struct PendingClaim {
    pub server_id: Uuid,
    pub client_token: Option<Uuid>,
    pub expires_at: Instant,
    pub claim_type: String,          // "invite" or "device"
    pub invite_code: Option<String>, // present if claim_type == "invite"
}

impl BootstrapState {
    pub fn new(config: &BootstrapSection, db: BootstrapDb) -> Self {
        let relay_client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .build()
            .expect("failed to create relay HTTP client");

        Self {
            servers: DashMap::new(),
            claims: DashMap::new(),
            bootstrap_token: config.bootstrap_token.clone(),
            registration_ttl: Duration::from_secs(config.registration_ttl_secs),
            claim_ttl: Duration::from_secs(config.claim_ttl_secs),
            db,
            relay_client,
            relay_enabled: config.relay_enabled,
            relay_timeout: Duration::from_secs(config.relay_timeout_secs),
            relay_host: config.relay_host.clone(),
        }
    }

    /// Build the relay URL for a given server ID, if relay is enabled.
    pub fn relay_url_for(&self, server_id: &Uuid) -> Option<String> {
        if !self.relay_enabled {
            return None;
        }
        let host = self
            .relay_host
            .as_deref()
            .unwrap_or("streamrelay.indexarr.net");
        Some(format!("https://{host}/relay/{server_id}"))
    }

    /// Generate a unique 4-character alphanumeric claim code (uppercase).
    pub fn generate_claim_code(&self) -> String {
        const CHARS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789"; // no I/O/0/1 to avoid confusion
        let mut rng = rand::rng();
        loop {
            let code: String = (0..4)
                .map(|_| {
                    let idx = rng.random_range(0..CHARS.len());
                    CHARS[idx] as char
                })
                .collect();
            // Ensure no collision with active claims
            if !self.claims.contains_key(&code) {
                return code;
            }
        }
    }

    /// Remove expired server registrations and claim codes.
    pub async fn sweep_expired(&self) {
        let now = Instant::now();

        self.servers
            .retain(|_, reg| now.duration_since(reg.last_heartbeat) < self.registration_ttl);

        self.claims.retain(|_, claim| now < claim.expires_at);

        if let Err(e) = self.db.sweep_expired_claims().await {
            tracing::warn!(error = %e, "failed to sweep expired claims from SQLite");
        }
    }
}
