//! Token generation and hashing utilities.
//!
//! Provides cryptographically secure token generation and SHA-256 hashing
//! for OAuth tokens and authorization codes.

use rand::Rng;
use sha2::{Digest, Sha256};

/// Length of generated tokens in bytes (before base64 encoding).
/// 32 bytes = 256 bits of entropy.
const TOKEN_BYTES: usize = 32;

/// Length of generated authorization codes in bytes.
/// 16 bytes = 128 bits of entropy.
const CODE_BYTES: usize = 16;

/// Generates a cryptographically secure random token.
///
/// Returns a URL-safe base64-encoded string (43 characters).
pub fn generate_token() -> String {
    let mut rng = rand::thread_rng();
    let bytes: [u8; TOKEN_BYTES] = rng.gen();
    base64_url_encode(&bytes)
}

/// Generates a cryptographically secure random authorization code.
///
/// Returns a URL-safe base64-encoded string (22 characters).
pub fn generate_auth_code() -> String {
    let mut rng = rand::thread_rng();
    let bytes: [u8; CODE_BYTES] = rng.gen();
    base64_url_encode(&bytes)
}

/// Generates a cryptographically secure random client ID.
///
/// Returns a string in the format "kix_client_XXXX" where XXXX is random hex.
pub fn generate_client_id() -> String {
    let mut rng = rand::thread_rng();
    let bytes: [u8; 16] = rng.gen();
    format!("kix_client_{}", hex_encode(&bytes))
}

/// Computes SHA-256 hash of a token for secure storage.
///
/// Returns a hex-encoded hash string (64 characters).
pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let result = hasher.finalize();
    hex_encode(&result)
}

/// URL-safe base64 encoding (no padding).
fn base64_url_encode(bytes: &[u8]) -> String {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Hex encoding for hash output.
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_token_length() {
        let token = generate_token();
        // 32 bytes = 43 chars in URL-safe base64 without padding
        assert_eq!(token.len(), 43);
    }

    #[test]
    fn test_generate_token_uniqueness() {
        let token1 = generate_token();
        let token2 = generate_token();
        assert_ne!(token1, token2);
    }

    #[test]
    fn test_generate_auth_code_length() {
        let code = generate_auth_code();
        // 16 bytes = 22 chars in URL-safe base64 without padding
        assert_eq!(code.len(), 22);
    }

    #[test]
    fn test_generate_client_id_format() {
        let client_id = generate_client_id();
        assert!(client_id.starts_with("kix_client_"));
        // 16 bytes hex = 32 chars + prefix
        assert_eq!(client_id.len(), 11 + 32);
    }

    #[test]
    fn test_hash_token_consistency() {
        let token = "test_token_123";
        let hash1 = hash_token(token);
        let hash2 = hash_token(token);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_hash_token_length() {
        let hash = hash_token("any_token");
        // SHA-256 = 32 bytes = 64 hex chars
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn test_hash_token_different_inputs() {
        let hash1 = hash_token("token1");
        let hash2 = hash_token("token2");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_token_url_safe() {
        // Generate many tokens and ensure they're URL-safe
        for _ in 0..100 {
            let token = generate_token();
            assert!(
                token.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
                "Token contains invalid characters: {}",
                token
            );
        }
    }
}
