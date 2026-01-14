//! Token store wrapper for backward compatibility.
//!
//! This module provides a `TokenStore` that wraps `SqliteStore`
//! for backward compatibility with existing code.

use crate::error::StoreError;
use serde::{Deserialize, Serialize};
use std::path::Path;

// Re-export TokenRecord for convenience
pub use kix_sqlite::TokenRecord;
use kix_sqlite::SqliteStore;

/// Encrypted token data (ciphertext and nonce).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EncryptedTokenData {
    /// Base64-encoded ciphertext
    pub ciphertext: String,
    /// Base64-encoded nonce (or "plaintext" marker for dev mode)
    pub nonce: String,
}

impl EncryptedTokenData {
    /// Create new encrypted token data.
    pub fn new(ciphertext: impl Into<String>, nonce: impl Into<String>) -> Self {
        Self {
            ciphertext: ciphertext.into(),
            nonce: nonce.into(),
        }
    }

    /// Serialize to JSON string for storage.
    pub fn to_json(&self) -> Result<String, StoreError> {
        serde_json::to_string(self)
            .map_err(|e| StoreError::Database(format!("Failed to serialize token data: {}", e)))
    }

    /// Deserialize from JSON string.
    pub fn from_json(json: &str) -> Result<Self, StoreError> {
        serde_json::from_str(json)
            .map_err(|e| StoreError::Database(format!("Failed to deserialize token data: {}", e)))
    }

    /// Convenience method that returns error string instead of StoreError (for kix-projects).
    pub fn parse(json: &str) -> Result<Self, String> {
        serde_json::from_str(json)
            .map_err(|e| format!("Failed to parse encrypted token data: {}", e))
    }
}

/// Token store wrapper.
///
/// Wraps `SqliteStore` to provide token-specific operations.
pub struct TokenStore {
    sqlite: SqliteStore,
}

impl TokenStore {
    /// Create a new token store at the given SQLite database path.
    pub async fn new(db_path: &str) -> Result<Self, StoreError> {
        let path = Path::new(db_path);
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                StoreError::Database(format!("Failed to create parent directory: {}", e))
            })?;
        }

        let sqlite = SqliteStore::new(path).await.map_err(|e| {
            StoreError::Database(format!("Failed to create SQLite store: {}", e))
        })?;

        Ok(Self { sqlite })
    }

    /// Create from existing SqliteStore.
    pub fn from_sqlite(sqlite: SqliteStore) -> Self {
        Self { sqlite }
    }

    /// Initialize tables (no-op as SqliteStore handles migrations).
    pub async fn init_tables(&mut self) -> Result<(), StoreError> {
        // Tables are created during SqliteStore::new() via migrations
        Ok(())
    }

    /// Store a token (insert or update).
    pub async fn store_token(&self, token: &TokenRecord) -> Result<(), StoreError> {
        self.sqlite.store_token(token).await.map_err(|e| {
            StoreError::Database(format!("Failed to store token: {}", e))
        })
    }

    /// Get a token by scope.
    pub async fn get_token(&self, scope: &str) -> Result<Option<TokenRecord>, StoreError> {
        self.sqlite.get_token(scope).await.map_err(|e| {
            StoreError::Database(format!("Failed to get token: {}", e))
        })
    }

    /// Get the global token.
    pub async fn get_global_token(&self) -> Result<Option<TokenRecord>, StoreError> {
        self.get_token("global").await
    }

    /// Check if a global token exists.
    pub async fn has_global_token(&self) -> Result<bool, StoreError> {
        Ok(self.get_global_token().await?.is_some())
    }

    /// Delete a token by scope.
    pub async fn delete_token(&self, scope: &str) -> Result<bool, StoreError> {
        self.sqlite.delete_token(scope).await.map_err(|e| {
            StoreError::Database(format!("Failed to delete token: {}", e))
        })
    }

    /// List all tokens.
    pub async fn list_tokens(&self) -> Result<Vec<TokenRecord>, StoreError> {
        self.sqlite.list_tokens().await.map_err(|e| {
            StoreError::Database(format!("Failed to list tokens: {}", e))
        })
    }

    /// Close the store.
    pub async fn close(&self) {
        self.sqlite.close().await;
    }
}
