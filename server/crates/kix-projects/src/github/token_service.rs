//! Unified GitHub Token Service.
//!
//! This module provides a high-level service for managing GitHub tokens,
//! combining persistence (TokenStore), encryption (GitHubTokenManager),
//! and verification (GitHubRestClient).

use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use kix_store::tokens::{EncryptedTokenData, TokenRecord, TokenStore};

use super::models::AuthenticatedUser;
use super::rest_client::GitHubRestClient;
use super::tokens::{EncryptedToken, GitHubTokenManager};
use crate::error::ProjectError;

// ============================================================================
// Token Service Types
// ============================================================================

/// Result of storing a token.
#[derive(Debug, Clone)]
pub struct TokenStoreResult {
    /// The scope where the token was stored
    pub scope: String,
    /// Whether the token was verified (and with what user)
    pub verified_user: Option<String>,
}

/// Type of token configuration for a project.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProjectTokenType {
    /// Project has its own token
    ProjectSpecific {
        /// Last 4 chars of the token
        suffix: Option<String>,
        /// GitHub username when verified
        verified_user: Option<String>,
    },
    /// Project uses the global token
    UsingGlobal {
        /// Last 4 chars of the global token
        suffix: Option<String>,
    },
    /// No token configured
    NotConfigured,
}

// ============================================================================
// Token Service
// ============================================================================

/// Unified token service combining persistence, encryption, and verification.
///
/// This service provides a high-level API for managing GitHub tokens:
/// - Setting/getting global and project-specific tokens
/// - Token verification against GitHub API
/// - Token type detection for UI display
pub struct TokenService {
    /// Persistent token storage
    store: Arc<RwLock<TokenStore>>,
    /// Encryption manager (None if encryption key not set)
    manager: Option<Arc<GitHubTokenManager>>,
}

impl TokenService {
    /// Create a new token service.
    ///
    /// If `manager` is None, tokens will be stored in plaintext (development mode).
    pub fn new(store: Arc<RwLock<TokenStore>>, manager: Option<Arc<GitHubTokenManager>>) -> Self {
        if manager.is_none() {
            warn!("TokenService created without encryption - tokens will be stored in plaintext");
        }
        Self { store, manager }
    }

    /// Create a token service with encryption enabled.
    pub fn with_encryption(store: Arc<RwLock<TokenStore>>) -> Result<Self, ProjectError> {
        let manager = GitHubTokenManager::new()?;
        Ok(Self {
            store,
            manager: Some(Arc::new(manager)),
        })
    }

    // ========================================================================
    // Token Storage Operations
    // ========================================================================

    /// Set the global GitHub token.
    ///
    /// Optionally verifies the token against GitHub API before storing.
    pub async fn set_global_token(
        &self,
        token: &str,
        verify: bool,
    ) -> Result<TokenStoreResult, ProjectError> {
        self.store_token_internal("global", token, verify).await
    }

    /// Set a project-specific GitHub token.
    ///
    /// If `verify_repo` is provided, verifies the token has access to that repo.
    pub async fn set_project_token(
        &self,
        project_id: &str,
        token: &str,
        verify_repo: Option<(&str, &str)>, // (owner, repo)
    ) -> Result<TokenStoreResult, ProjectError> {
        let scope = format!("project:{}", project_id);

        // Validate format
        GitHubTokenManager::validate_token_format(token)?;

        // Verify token if requested
        let verified_user = if let Some((owner, repo)) = verify_repo {
            let client = GitHubRestClient::new(token)?;

            // Verify access to repo
            if !client.verify_access(owner, repo).await? {
                return Err(ProjectError::TokenStorage(format!(
                    "Token does not have access to {}/{}",
                    owner, repo
                )));
            }

            // Get authenticated user for verification info
            match client.get_authenticated_user().await {
                Ok(user) => Some(user.login),
                Err(_) => None,
            }
        } else {
            None
        };

        // Encrypt and store
        let encrypted = self.encrypt_token(token)?;
        let encrypted_json = encrypted.to_json()
            .map_err(|e| ProjectError::TokenStorage(format!("Failed to serialize token: {}", e)))?;

        // Extract token metadata
        let suffix = token.chars().rev().take(4).collect::<String>().chars().rev().collect::<String>();
        let format = if token.starts_with("ghp_") {
            "ghp_".to_string()
        } else if token.starts_with("gho_") {
            "gho_".to_string()
        } else if token.starts_with("github_pat_") {
            "github_pat_".to_string()
        } else {
            "unknown".to_string()
        };

        let record = TokenRecord::new(&scope, encrypted_json)
            .with_suffix(suffix)
            .with_format(format);

        let record = if let Some(ref user) = verified_user {
            record.with_verification(user)
        } else {
            record
        };

        self.store.write().await.store_token(&record).await
            .map_err(|e| ProjectError::TokenStorage(e.to_string()))?;

        info!("Stored project token for: {}", project_id);

        Ok(TokenStoreResult {
            scope,
            verified_user,
        })
    }

    /// Internal helper for storing tokens with optional verification.
    async fn store_token_internal(
        &self,
        scope: &str,
        token: &str,
        verify: bool,
    ) -> Result<TokenStoreResult, ProjectError> {
        // Validate format
        GitHubTokenManager::validate_token_format(token)?;

        // Verify token if requested
        let verified_user = if verify {
            let client = GitHubRestClient::new(token)?;
            match client.get_authenticated_user().await {
                Ok(user) => Some(user.login),
                Err(e) => {
                    return Err(ProjectError::TokenStorage(format!(
                        "Token verification failed: {}",
                        e
                    )));
                }
            }
        } else {
            None
        };

        // Encrypt and store
        let encrypted = self.encrypt_token(token)?;
        let encrypted_json = encrypted.to_json()
            .map_err(|e| ProjectError::TokenStorage(format!("Failed to serialize token: {}", e)))?;

        // Extract token metadata (suffix = last 4 chars, format = prefix type)
        let suffix = token.chars().rev().take(4).collect::<String>().chars().rev().collect::<String>();
        let format = if token.starts_with("ghp_") {
            "ghp_".to_string()
        } else if token.starts_with("gho_") {
            "gho_".to_string()
        } else if token.starts_with("github_pat_") {
            "github_pat_".to_string()
        } else {
            "unknown".to_string()
        };

        let record = TokenRecord::new(scope, encrypted_json)
            .with_suffix(suffix)
            .with_format(format);

        let record = if let Some(ref user) = verified_user {
            record.with_verification(user)
        } else {
            record
        };

        self.store.write().await.store_token(&record).await
            .map_err(|e| ProjectError::TokenStorage(e.to_string()))?;

        info!("Stored token for scope: {}", scope);

        Ok(TokenStoreResult {
            scope: scope.to_string(),
            verified_user,
        })
    }

    /// Delete a token by scope.
    pub async fn delete_token(&self, scope: &str) -> Result<bool, ProjectError> {
        self.store.write().await.delete_token(scope).await
            .map_err(|e| ProjectError::TokenStorage(e.to_string()))
    }

    /// Delete a project-specific token.
    pub async fn delete_project_token(&self, project_id: &str) -> Result<bool, ProjectError> {
        let scope = format!("project:{}", project_id);
        self.delete_token(&scope).await
    }

    // ========================================================================
    // Token Retrieval Operations
    // ========================================================================

    /// Get the decrypted token for a project (falls back to global).
    pub async fn get_token_for_project(&self, project_id: &str) -> Result<String, ProjectError> {
        let project_scope = format!("project:{}", project_id);

        // Try project-specific first
        if let Some(record) = self.store.read().await.get_token(&project_scope).await
            .map_err(|e| ProjectError::TokenStorage(e.to_string()))?
        {
            debug!("Using project-specific token for {}", project_id);
            let encrypted = EncryptedTokenData::from_json(&record.encrypted_token)
                .map_err(|e| ProjectError::TokenStorage(format!("Failed to parse token: {}", e)))?;
            return self.decrypt_token(&encrypted);
        }

        // Fall back to global
        self.get_global_token_decrypted().await
    }

    /// Get the decrypted global token.
    pub async fn get_global_token_decrypted(&self) -> Result<String, ProjectError> {
        let record = self.store.read().await.get_global_token().await
            .map_err(|e| ProjectError::TokenStorage(e.to_string()))?
            .ok_or_else(|| ProjectError::TokenStorage(
                "No global GitHub token configured. Set a token in Administration settings.".to_string()
            ))?;

        let encrypted = EncryptedTokenData::from_json(&record.encrypted_token)
            .map_err(|e| ProjectError::TokenStorage(format!("Failed to parse token: {}", e)))?;
        self.decrypt_token(&encrypted)
    }

    /// Check if a global token is configured.
    pub async fn has_global_token(&self) -> Result<bool, ProjectError> {
        self.store.read().await.has_global_token().await
            .map_err(|e| ProjectError::TokenStorage(e.to_string()))
    }

    /// Get the global token record (for metadata, not the actual token).
    pub async fn get_global_token_info(&self) -> Result<Option<TokenRecord>, ProjectError> {
        self.store.read().await.get_global_token().await
            .map_err(|e| ProjectError::TokenStorage(e.to_string()))
    }

    /// Get the token type for a project (for UI display).
    pub async fn get_project_token_type(&self, project_id: &str) -> Result<ProjectTokenType, ProjectError> {
        let project_scope = format!("project:{}", project_id);

        // Check project-specific first
        if let Some(record) = self.store.read().await.get_token(&project_scope).await
            .map_err(|e| ProjectError::TokenStorage(e.to_string()))?
        {
            return Ok(ProjectTokenType::ProjectSpecific {
                suffix: record.token_suffix,
                verified_user: record.verified_user,
            });
        }

        // Check global
        if let Some(record) = self.store.read().await.get_global_token().await
            .map_err(|e| ProjectError::TokenStorage(e.to_string()))?
        {
            return Ok(ProjectTokenType::UsingGlobal {
                suffix: record.token_suffix,
            });
        }

        Ok(ProjectTokenType::NotConfigured)
    }

    // ========================================================================
    // Verification Operations
    // ========================================================================

    /// Verify a token is valid and get the authenticated user.
    pub async fn verify_token(&self, token: &str) -> Result<AuthenticatedUser, ProjectError> {
        let client = GitHubRestClient::new(token)?;
        client.get_authenticated_user().await
    }

    /// Verify a token has access to a specific repository.
    pub async fn verify_repo_access(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
    ) -> Result<bool, ProjectError> {
        let client = GitHubRestClient::new(token)?;
        client.verify_access(owner, repo).await
    }

    /// Create a REST client using the token for a project.
    pub async fn create_client_for_project(&self, project_id: &str) -> Result<GitHubRestClient, ProjectError> {
        let token = self.get_token_for_project(project_id).await?;
        GitHubRestClient::new(&token)
    }

    /// Create a REST client using the global token.
    pub async fn create_global_client(&self) -> Result<GitHubRestClient, ProjectError> {
        let token = self.get_global_token_decrypted().await?;
        GitHubRestClient::new(&token)
    }

    // ========================================================================
    // Encryption/Decryption Helpers
    // ========================================================================

    /// Encrypt a token.
    fn encrypt_token(&self, token: &str) -> Result<EncryptedTokenData, ProjectError> {
        if let Some(ref manager) = self.manager {
            let encrypted = manager.encrypt(token)?;
            Ok(EncryptedTokenData {
                ciphertext: encrypted.ciphertext,
                nonce: encrypted.nonce,
            })
        } else {
            // Development mode: store as "plaintext" (base64 encoded for consistency)
            warn!("Storing token without encryption (development mode)");
            Ok(EncryptedTokenData {
                ciphertext: base64::Engine::encode(&base64::engine::general_purpose::STANDARD, token),
                nonce: "plaintext".to_string(), // Marker for plaintext
            })
        }
    }

    /// Decrypt a token.
    fn decrypt_token(&self, encrypted: &EncryptedTokenData) -> Result<String, ProjectError> {
        if encrypted.nonce == "plaintext" {
            // Development mode: decode base64
            let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &encrypted.ciphertext)
                .map_err(|e| ProjectError::TokenStorage(format!("Failed to decode token: {}", e)))?;
            String::from_utf8(bytes)
                .map_err(|e| ProjectError::TokenStorage(format!("Invalid token data: {}", e)))
        } else if let Some(ref manager) = self.manager {
            let encrypted_token = EncryptedToken {
                ciphertext: encrypted.ciphertext.clone(),
                nonce: encrypted.nonce.clone(),
            };
            manager.decrypt(&encrypted_token)
        } else {
            Err(ProjectError::TokenStorage(
                "Token is encrypted but encryption key is not configured".to_string()
            ))
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_token_type_serialization() {
        let token_type = ProjectTokenType::ProjectSpecific {
            suffix: Some("1234".to_string()),
            verified_user: Some("octocat".to_string()),
        };

        let json = serde_json::to_string(&token_type).unwrap();
        assert!(json.contains("project_specific"));
        assert!(json.contains("1234"));
        assert!(json.contains("octocat"));
    }

    #[test]
    fn test_project_token_type_using_global() {
        let token_type = ProjectTokenType::UsingGlobal {
            suffix: Some("5678".to_string()),
        };

        let json = serde_json::to_string(&token_type).unwrap();
        assert!(json.contains("using_global"));
        assert!(json.contains("5678"));
    }

    #[test]
    fn test_project_token_type_not_configured() {
        let token_type = ProjectTokenType::NotConfigured;

        let json = serde_json::to_string(&token_type).unwrap();
        assert!(json.contains("not_configured"));
    }

    #[test]
    fn test_token_store_result() {
        let result = TokenStoreResult {
            scope: "global".to_string(),
            verified_user: Some("octocat".to_string()),
        };

        assert_eq!(result.scope, "global");
        assert_eq!(result.verified_user, Some("octocat".to_string()));
    }
}
