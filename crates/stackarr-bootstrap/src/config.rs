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
