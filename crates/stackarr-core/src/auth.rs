// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

use argon2::password_hash::SaltString;
use argon2::password_hash::rand_core::OsRng;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::RngExt;
use sha2::{Digest, Sha256};

/// Hash a password using Argon2id.
pub fn hash_password(password: &str) -> crate::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| crate::Error::Other(anyhow::anyhow!("password hash failed: {e}")))?;
    Ok(hash.to_string())
}

/// Verify a password against a stored Argon2id hash.
pub fn verify_password(password: &str, hash: &str) -> crate::Result<bool> {
    let parsed = PasswordHash::new(hash)
        .map_err(|e| crate::Error::Other(anyhow::anyhow!("invalid password hash: {e}")))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

/// Generate a cryptographically secure session token (32 random bytes, base64url-encoded).
pub fn generate_session_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Hash a session token with SHA-256 for storage (hex-encoded).
pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

/// Generate an 8-character alphanumeric invite code.
pub fn generate_invite_code() -> String {
    const CHARSET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut rng = rand::rng();
    (0..8)
        .map(|_| {
            let idx = rng.random_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_and_verify_password() {
        let hash = hash_password("hunter2").unwrap();
        assert!(verify_password("hunter2", &hash).unwrap());
        assert!(!verify_password("wrong", &hash).unwrap());
    }

    #[test]
    fn test_generate_session_token() {
        let token = generate_session_token();
        assert!(!token.is_empty());
        assert!(token.len() > 20);
    }

    #[test]
    fn test_hash_token_deterministic() {
        let h1 = hash_token("test-token");
        let h2 = hash_token("test-token");
        assert_eq!(h1, h2);
        assert_ne!(h1, hash_token("other"));
    }

    #[test]
    fn test_generate_invite_code() {
        let code = generate_invite_code();
        assert_eq!(code.len(), 8);
        assert!(code.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn test_hash_password_produces_argon2_format() {
        let hash = hash_password("test").unwrap();
        assert!(hash.starts_with("$argon2"));
    }

    #[test]
    fn test_hash_password_different_each_call() {
        let h1 = hash_password("same").unwrap();
        let h2 = hash_password("same").unwrap();
        assert_ne!(h1, h2); // Different salts
    }

    #[test]
    fn test_verify_password_wrong_password() {
        let hash = hash_password("correct").unwrap();
        assert!(!verify_password("incorrect", &hash).unwrap());
    }

    #[test]
    fn test_verify_password_empty_password() {
        let hash = hash_password("").unwrap();
        assert!(verify_password("", &hash).unwrap());
        assert!(!verify_password("notempty", &hash).unwrap());
    }

    #[test]
    fn test_verify_password_invalid_hash() {
        let result = verify_password("test", "not-a-hash");
        assert!(result.is_err());
    }

    #[test]
    fn test_session_token_uniqueness() {
        let t1 = generate_session_token();
        let t2 = generate_session_token();
        assert_ne!(t1, t2);
    }

    #[test]
    fn test_session_token_length() {
        let token = generate_session_token();
        // 32 bytes base64url → 43 chars
        assert_eq!(token.len(), 43);
    }

    #[test]
    fn test_session_token_is_base64url() {
        let token = generate_session_token();
        assert!(
            token
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        );
    }

    #[test]
    fn test_hash_token_is_hex() {
        let hash = hash_token("test");
        assert_eq!(hash.len(), 64); // SHA-256 = 32 bytes = 64 hex chars
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_hash_token_different_inputs() {
        assert_ne!(hash_token("a"), hash_token("b"));
    }

    #[test]
    fn test_invite_code_charset() {
        // Should not contain 0, 1, I, O
        for _ in 0..20 {
            let code = generate_invite_code();
            assert!(!code.contains('0'));
            assert!(!code.contains('1'));
            assert!(!code.contains('I'));
            assert!(!code.contains('O'));
        }
    }

    #[test]
    fn test_invite_code_uniqueness() {
        let c1 = generate_invite_code();
        let c2 = generate_invite_code();
        // Extremely unlikely to be equal
        assert_ne!(c1, c2);
    }
}
