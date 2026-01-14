//! Secure GitHub token storage.
//!
//! This module provides encrypted storage for GitHub Personal Access Tokens (PATs).
//! Tokens are encrypted using AES-256-GCM before being stored.

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::{Deserialize, Serialize};
use std::env;
use tracing::{debug, warn};

use crate::error::ProjectError;

/// Environment variable for the encryption key.
const ENCRYPTION_KEY_ENV: &str = "KIX_ENCRYPTION_KEY";

/// Encrypted token data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedToken {
    /// Base64-encoded encrypted token
    pub ciphertext: String,
    /// Base64-encoded nonce
    pub nonce: String,
}

/// Token scope (global or per-project).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenScope {
    /// Global token used as fallback
    Global,
    /// Project-specific token
    Project(String),
}

/// GitHub token manager.
///
/// Handles encryption, decryption, and storage of GitHub tokens.
/// Tokens are encrypted using AES-256-GCM.
pub struct GitHubTokenManager {
    cipher: Aes256Gcm,
}

impl GitHubTokenManager {
    /// Create a new token manager using the encryption key from environment.
    pub fn new() -> Result<Self, ProjectError> {
        let key = Self::get_encryption_key()?;
        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|e| ProjectError::TokenStorage(format!("Invalid encryption key: {}", e)))?;

        Ok(Self { cipher })
    }

    /// Create a token manager with a specific key (for testing).
    pub fn with_key(key: &[u8; 32]) -> Result<Self, ProjectError> {
        let cipher = Aes256Gcm::new_from_slice(key)
            .map_err(|e| ProjectError::TokenStorage(format!("Invalid encryption key: {}", e)))?;

        Ok(Self { cipher })
    }

    /// Get the encryption key from environment.
    fn get_encryption_key() -> Result<[u8; 32], ProjectError> {
        let key_str = env::var(ENCRYPTION_KEY_ENV).map_err(|_| {
            ProjectError::TokenStorage(format!(
                "Missing encryption key. Set {} environment variable with a 32-byte base64-encoded key.",
                ENCRYPTION_KEY_ENV
            ))
        })?;

        let key_bytes = BASE64.decode(&key_str).map_err(|e| {
            ProjectError::TokenStorage(format!("Invalid encryption key encoding: {}", e))
        })?;

        if key_bytes.len() != 32 {
            return Err(ProjectError::TokenStorage(format!(
                "Encryption key must be 32 bytes, got {}",
                key_bytes.len()
            )));
        }

        let mut key = [0u8; 32];
        key.copy_from_slice(&key_bytes);
        Ok(key)
    }

    /// Generate a new encryption key (for setup).
    pub fn generate_key() -> String {
        use rand::Rng;
        let mut key = [0u8; 32];
        rand::thread_rng().fill(&mut key);
        BASE64.encode(key)
    }

    /// Encrypt a token.
    pub fn encrypt(&self, token: &str) -> Result<EncryptedToken, ProjectError> {
        use rand::Rng;
        // Generate a random 12-byte nonce
        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt the token
        let ciphertext = self
            .cipher
            .encrypt(nonce, token.as_bytes())
            .map_err(|e| ProjectError::TokenStorage(format!("Encryption failed: {}", e)))?;

        Ok(EncryptedToken {
            ciphertext: BASE64.encode(&ciphertext),
            nonce: BASE64.encode(&nonce_bytes),
        })
    }

    /// Decrypt a token.
    pub fn decrypt(&self, encrypted: &EncryptedToken) -> Result<String, ProjectError> {
        // Decode nonce
        let nonce_bytes = BASE64
            .decode(&encrypted.nonce)
            .map_err(|e| ProjectError::TokenStorage(format!("Invalid nonce encoding: {}", e)))?;

        if nonce_bytes.len() != 12 {
            return Err(ProjectError::TokenStorage("Invalid nonce length".to_string()));
        }

        let nonce = Nonce::from_slice(&nonce_bytes);

        // Decode ciphertext
        let ciphertext = BASE64
            .decode(&encrypted.ciphertext)
            .map_err(|e| ProjectError::TokenStorage(format!("Invalid ciphertext encoding: {}", e)))?;

        // Decrypt
        let plaintext = self
            .cipher
            .decrypt(nonce, ciphertext.as_ref())
            .map_err(|e| ProjectError::TokenStorage(format!("Decryption failed: {}", e)))?;

        String::from_utf8(plaintext)
            .map_err(|e| ProjectError::TokenStorage(format!("Invalid token data: {}", e)))
    }

    /// Validate that a token has the expected format (starts with ghp_, gho_, etc.).
    pub fn validate_token_format(token: &str) -> Result<(), ProjectError> {
        if token.is_empty() {
            return Err(ProjectError::TokenStorage("Token cannot be empty".to_string()));
        }

        // GitHub token prefixes
        let valid_prefixes = ["ghp_", "gho_", "ghs_", "ghu_", "github_pat_"];

        if !valid_prefixes.iter().any(|p| token.starts_with(p)) {
            warn!("Token doesn't match known GitHub token formats");
            // Don't fail - some tokens might be fine-grained PATs with different format
        }

        Ok(())
    }
}

/// Token storage trait for abstracting storage backend.
pub trait TokenStorage: Send + Sync {
    /// Store a token.
    fn store_token(
        &self,
        scope: &TokenScope,
        encrypted: &EncryptedToken,
    ) -> Result<(), ProjectError>;

    /// Get a token.
    fn get_token(&self, scope: &TokenScope) -> Result<Option<EncryptedToken>, ProjectError>;

    /// Delete a token.
    fn delete_token(&self, scope: &TokenScope) -> Result<bool, ProjectError>;

    /// List all project-specific token scopes.
    fn list_project_tokens(&self) -> Result<Vec<String>, ProjectError>;
}

/// In-memory token storage (for testing).
#[derive(Default)]
pub struct InMemoryTokenStorage {
    tokens: std::sync::RwLock<std::collections::HashMap<String, EncryptedToken>>,
}

impl TokenStorage for InMemoryTokenStorage {
    fn store_token(
        &self,
        scope: &TokenScope,
        encrypted: &EncryptedToken,
    ) -> Result<(), ProjectError> {
        let key = scope_to_key(scope);
        let mut tokens = self.tokens.write().map_err(|e| {
            ProjectError::TokenStorage(format!("Failed to acquire lock: {}", e))
        })?;
        tokens.insert(key, encrypted.clone());
        Ok(())
    }

    fn get_token(&self, scope: &TokenScope) -> Result<Option<EncryptedToken>, ProjectError> {
        let key = scope_to_key(scope);
        let tokens = self.tokens.read().map_err(|e| {
            ProjectError::TokenStorage(format!("Failed to acquire lock: {}", e))
        })?;
        Ok(tokens.get(&key).cloned())
    }

    fn delete_token(&self, scope: &TokenScope) -> Result<bool, ProjectError> {
        let key = scope_to_key(scope);
        let mut tokens = self.tokens.write().map_err(|e| {
            ProjectError::TokenStorage(format!("Failed to acquire lock: {}", e))
        })?;
        Ok(tokens.remove(&key).is_some())
    }

    fn list_project_tokens(&self) -> Result<Vec<String>, ProjectError> {
        let tokens = self.tokens.read().map_err(|e| {
            ProjectError::TokenStorage(format!("Failed to acquire lock: {}", e))
        })?;
        let project_ids: Vec<String> = tokens
            .keys()
            .filter_map(|k| k.strip_prefix("project:").map(|s| s.to_string()))
            .collect();
        Ok(project_ids)
    }
}

fn scope_to_key(scope: &TokenScope) -> String {
    match scope {
        TokenScope::Global => "global".to_string(),
        TokenScope::Project(id) => format!("project:{}", id),
    }
}

/// Implement TokenStorage for Arc<dyn TokenStorage> to allow dynamic dispatch.
impl TokenStorage for std::sync::Arc<dyn TokenStorage> {
    fn store_token(
        &self,
        scope: &TokenScope,
        encrypted: &EncryptedToken,
    ) -> Result<(), ProjectError> {
        (**self).store_token(scope, encrypted)
    }

    fn get_token(&self, scope: &TokenScope) -> Result<Option<EncryptedToken>, ProjectError> {
        (**self).get_token(scope)
    }

    fn delete_token(&self, scope: &TokenScope) -> Result<bool, ProjectError> {
        (**self).delete_token(scope)
    }

    fn list_project_tokens(&self) -> Result<Vec<String>, ProjectError> {
        (**self).list_project_tokens()
    }
}

/// Get a token, falling back to global if project-specific not found.
pub fn get_token_with_fallback<S: TokenStorage>(
    storage: &S,
    manager: &GitHubTokenManager,
    project_id: Option<&str>,
) -> Result<String, ProjectError> {
    // Try project-specific first
    if let Some(pid) = project_id {
        let scope = TokenScope::Project(pid.to_string());
        if let Some(encrypted) = storage.get_token(&scope)? {
            debug!("Using project-specific token for {}", pid);
            return manager.decrypt(&encrypted);
        }
    }

    // Fall back to global
    let global = storage.get_token(&TokenScope::Global)?;
    match global {
        Some(encrypted) => {
            debug!("Using global token");
            manager.decrypt(&encrypted)
        }
        None => Err(ProjectError::TokenStorage(
            "No GitHub token configured. Use set_github_token to configure a token.".to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> [u8; 32] {
        [0u8; 32] // Simple test key
    }

    #[test]
    fn test_encrypt_decrypt() {
        let manager = GitHubTokenManager::with_key(&test_key()).unwrap();
        let token = "ghp_test_token_12345";

        let encrypted = manager.encrypt(token).unwrap();
        assert!(!encrypted.ciphertext.is_empty());
        assert!(!encrypted.nonce.is_empty());

        let decrypted = manager.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, token);
    }

    #[test]
    fn test_different_nonces() {
        let manager = GitHubTokenManager::with_key(&test_key()).unwrap();
        let token = "ghp_test_token";

        let encrypted1 = manager.encrypt(token).unwrap();
        let encrypted2 = manager.encrypt(token).unwrap();

        // Same token should produce different ciphertexts due to random nonce
        assert_ne!(encrypted1.nonce, encrypted2.nonce);
        assert_ne!(encrypted1.ciphertext, encrypted2.ciphertext);

        // Both should decrypt to same token
        assert_eq!(manager.decrypt(&encrypted1).unwrap(), token);
        assert_eq!(manager.decrypt(&encrypted2).unwrap(), token);
    }

    #[test]
    fn test_in_memory_storage() {
        let storage = InMemoryTokenStorage::default();

        let encrypted = EncryptedToken {
            ciphertext: "test".to_string(),
            nonce: "nonce".to_string(),
        };

        // Store global
        storage
            .store_token(&TokenScope::Global, &encrypted)
            .unwrap();
        let retrieved = storage.get_token(&TokenScope::Global).unwrap();
        assert!(retrieved.is_some());

        // Store project
        let project_scope = TokenScope::Project("proj-123".to_string());
        storage.store_token(&project_scope, &encrypted).unwrap();
        let retrieved = storage.get_token(&project_scope).unwrap();
        assert!(retrieved.is_some());

        // List projects
        let projects = storage.list_project_tokens().unwrap();
        assert_eq!(projects.len(), 1);
        assert!(projects.contains(&"proj-123".to_string()));

        // Delete
        let deleted = storage.delete_token(&project_scope).unwrap();
        assert!(deleted);
        let retrieved = storage.get_token(&project_scope).unwrap();
        assert!(retrieved.is_none());
    }

    #[test]
    fn test_generate_key() {
        let key1 = GitHubTokenManager::generate_key();
        let key2 = GitHubTokenManager::generate_key();

        // Should be valid base64
        assert!(BASE64.decode(&key1).is_ok());
        assert!(BASE64.decode(&key2).is_ok());

        // Should be different
        assert_ne!(key1, key2);

        // Should decode to 32 bytes
        assert_eq!(BASE64.decode(&key1).unwrap().len(), 32);
    }

    #[test]
    fn test_token_format_validation() {
        // Valid formats
        assert!(GitHubTokenManager::validate_token_format("ghp_test").is_ok());
        assert!(GitHubTokenManager::validate_token_format("gho_test").is_ok());
        assert!(GitHubTokenManager::validate_token_format("github_pat_test").is_ok());

        // Empty token should fail
        assert!(GitHubTokenManager::validate_token_format("").is_err());

        // Unknown format - should warn but not fail (for compatibility)
        assert!(GitHubTokenManager::validate_token_format("unknown_format").is_ok());
    }
}
