use std::net::IpAddr;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use rand::Rng;
use uuid::Uuid;

use crate::config::BootstrapSection;

pub struct BootstrapState {
    pub servers: DashMap<Uuid, ServerRegistration>,
    pub claims: DashMap<String, PendingClaim>,
    pub bootstrap_token: String,
    pub registration_ttl: Duration,
    pub claim_ttl: Duration,
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
    pub client_token: Uuid,
    pub expires_at: Instant,
}

impl BootstrapState {
    pub fn new(config: &BootstrapSection) -> Self {
        Self {
            servers: DashMap::new(),
            claims: DashMap::new(),
            bootstrap_token: config.bootstrap_token.clone(),
            registration_ttl: Duration::from_secs(config.registration_ttl_secs),
            claim_ttl: Duration::from_secs(config.claim_ttl_secs),
        }
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
    pub fn sweep_expired(&self) {
        let now = Instant::now();

        self.servers.retain(|_, reg| {
            now.duration_since(reg.last_heartbeat) < self.registration_ttl
        });

        self.claims.retain(|_, claim| now < claim.expires_at);
    }
}
