pub mod auth;
pub mod config;
pub mod dav_db;
pub mod db;
pub mod error;
pub mod log_buffer;
pub mod models;
pub mod notifications;

#[cfg(any(test, feature = "testing"))]
pub mod test_helpers;

pub use config::AppConfig;
pub use db::Database;
pub use error::{Error, Result};

/// Generate a 12-word BIP39 recovery phrase and its SHA-256 hex hash.
/// Returns `(phrase, hex_hash)`.
pub fn generate_recovery_phrase() -> Result<(String, String)> {
    use bip39::Mnemonic;
    use rand::Rng;
    use sha2::{Digest, Sha256};

    let mut entropy = [0u8; 16]; // 128-bit → 12 words
    rand::rng().fill_bytes(&mut entropy);
    let mnemonic = Mnemonic::from_entropy(&entropy)
        .map_err(|e| Error::Other(anyhow::anyhow!("failed to generate mnemonic: {e}")))?;
    let phrase = mnemonic.to_string();

    let mut hasher = Sha256::new();
    hasher.update(phrase.as_bytes());
    let hex_hash = hex::encode(hasher.finalize());

    Ok((phrase, hex_hash))
}
