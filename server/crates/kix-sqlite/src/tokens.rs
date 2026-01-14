//! GitHub token storage operations for SQLite.
//!
//! Tokens are stored encrypted with AES-256-GCM.

use crate::error::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

/// Token record for SQLite storage.
///
/// The actual token is encrypted; only metadata is in plaintext.
#[derive(Clone, Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct TokenRecord {
    /// Scope: "global" or "project:{uuid}"
    pub scope: String,

    /// Encrypted token data as JSON (ciphertext + nonce)
    pub encrypted_token: String,

    /// Last 4 characters (for display only)
    pub token_suffix: Option<String>,

    /// Token format prefix (ghp_, gho_, github_pat_)
    pub token_format: Option<String>,

    /// Last verification timestamp (RFC3339)
    pub verified_at: Option<String>,

    /// GitHub username when verified
    pub verified_user: Option<String>,

    /// Creation timestamp (RFC3339)
    pub created_at: String,

    /// Last update timestamp (RFC3339)
    pub updated_at: String,
}

impl TokenRecord {
    /// Create a new token record.
    pub fn new(scope: impl Into<String>, encrypted_token: impl Into<String>) -> Self {
        let now = Utc::now().to_rfc3339();
        Self {
            scope: scope.into(),
            encrypted_token: encrypted_token.into(),
            token_suffix: None,
            token_format: None,
            verified_at: None,
            verified_user: None,
            created_at: now.clone(),
            updated_at: now,
        }
    }

    /// Create a global token record.
    pub fn global(encrypted_token: impl Into<String>) -> Self {
        Self::new("global", encrypted_token)
    }

    /// Create a project-scoped token record.
    pub fn for_project(project_id: impl Into<String>, encrypted_token: impl Into<String>) -> Self {
        Self::new(format!("project:{}", project_id.into()), encrypted_token)
    }

    /// Set token suffix (last 4 chars).
    pub fn with_suffix(mut self, suffix: impl Into<String>) -> Self {
        self.token_suffix = Some(suffix.into());
        self
    }

    /// Set token format.
    pub fn with_format(mut self, format: impl Into<String>) -> Self {
        self.token_format = Some(format.into());
        self
    }

    /// Set verification info.
    pub fn with_verification(mut self, user: impl Into<String>) -> Self {
        self.verified_at = Some(Utc::now().to_rfc3339());
        self.verified_user = Some(user.into());
        self
    }

    /// Check if this is a global token.
    pub fn is_global(&self) -> bool {
        self.scope == "global"
    }

    /// Get project ID if this is a project-scoped token.
    pub fn project_id(&self) -> Option<&str> {
        self.scope.strip_prefix("project:")
    }
}

/// Store a token (insert or update).
pub async fn store_token(pool: &SqlitePool, token: &TokenRecord) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO github_tokens (
            scope, encrypted_token, token_suffix, token_format,
            verified_at, verified_user, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(scope) DO UPDATE SET
            encrypted_token = excluded.encrypted_token,
            token_suffix = excluded.token_suffix,
            token_format = excluded.token_format,
            verified_at = excluded.verified_at,
            verified_user = excluded.verified_user,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(&token.scope)
    .bind(&token.encrypted_token)
    .bind(&token.token_suffix)
    .bind(&token.token_format)
    .bind(&token.verified_at)
    .bind(&token.verified_user)
    .bind(&token.created_at)
    .bind(Utc::now().to_rfc3339())
    .execute(pool)
    .await?;

    Ok(())
}

/// Get a token by scope.
pub async fn get_token(pool: &SqlitePool, scope: &str) -> Result<Option<TokenRecord>> {
    let token = sqlx::query_as::<_, TokenRecord>("SELECT * FROM github_tokens WHERE scope = ?")
        .bind(scope)
        .fetch_optional(pool)
        .await?;

    Ok(token)
}

/// Delete a token by scope.
pub async fn delete_token(pool: &SqlitePool, scope: &str) -> Result<bool> {
    let result = sqlx::query("DELETE FROM github_tokens WHERE scope = ?")
        .bind(scope)
        .execute(pool)
        .await?;

    Ok(result.rows_affected() > 0)
}

/// List all tokens.
pub async fn list_tokens(pool: &SqlitePool) -> Result<Vec<TokenRecord>> {
    let tokens = sqlx::query_as::<_, TokenRecord>(
        "SELECT * FROM github_tokens ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await?;

    Ok(tokens)
}

/// List tokens by scope prefix (e.g., "project:" for all project tokens).
pub async fn list_tokens_by_prefix(
    pool: &SqlitePool,
    prefix: &str,
) -> Result<Vec<TokenRecord>> {
    let tokens = sqlx::query_as::<_, TokenRecord>(
        "SELECT * FROM github_tokens WHERE scope LIKE ? ORDER BY created_at DESC",
    )
    .bind(format!("{}%", prefix))
    .fetch_all(pool)
    .await?;

    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::{create_pool, run_migrations};
    use tempfile::TempDir;

    async fn setup_test_db() -> (SqlitePool, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let pool = create_pool(&db_path).await.unwrap();
        run_migrations(&pool).await.unwrap();
        (pool, temp_dir)
    }

    #[tokio::test]
    async fn test_store_and_get_global_token() {
        let (pool, _temp_dir) = setup_test_db().await;

        let token = TokenRecord::global(r#"{"ciphertext":"abc","nonce":"123"}"#)
            .with_suffix("1234")
            .with_format("ghp_")
            .with_verification("testuser");

        store_token(&pool, &token).await.unwrap();

        let retrieved = get_token(&pool, "global").await.unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert!(retrieved.is_global());
        assert_eq!(retrieved.token_suffix, Some("1234".to_string()));
        assert_eq!(retrieved.verified_user, Some("testuser".to_string()));
    }

    #[tokio::test]
    async fn test_project_scoped_token() {
        let (pool, _temp_dir) = setup_test_db().await;

        let token = TokenRecord::for_project("proj-123", r#"{"encrypted":"data"}"#);
        store_token(&pool, &token).await.unwrap();

        let retrieved = get_token(&pool, "project:proj-123").await.unwrap().unwrap();
        assert!(!retrieved.is_global());
        assert_eq!(retrieved.project_id(), Some("proj-123"));
    }

    #[tokio::test]
    async fn test_token_upsert() {
        let (pool, _temp_dir) = setup_test_db().await;

        // Store initial token
        let token1 = TokenRecord::global(r#"{"old":"data"}"#);
        store_token(&pool, &token1).await.unwrap();

        // Update with new token
        let token2 = TokenRecord::global(r#"{"new":"data"}"#).with_suffix("5678");
        store_token(&pool, &token2).await.unwrap();

        // Should have updated, not duplicated
        let all = list_tokens(&pool).await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].token_suffix, Some("5678".to_string()));
    }

    #[tokio::test]
    async fn test_delete_token() {
        let (pool, _temp_dir) = setup_test_db().await;

        let token = TokenRecord::global(r#"{"data":"test"}"#);
        store_token(&pool, &token).await.unwrap();

        assert!(get_token(&pool, "global").await.unwrap().is_some());

        let deleted = delete_token(&pool, "global").await.unwrap();
        assert!(deleted);

        assert!(get_token(&pool, "global").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_list_tokens_by_prefix() {
        let (pool, _temp_dir) = setup_test_db().await;

        // Create global and project tokens
        store_token(&pool, &TokenRecord::global(r#"{"g":"1"}"#)).await.unwrap();
        store_token(&pool, &TokenRecord::for_project("p1", r#"{"p":"1"}"#)).await.unwrap();
        store_token(&pool, &TokenRecord::for_project("p2", r#"{"p":"2"}"#)).await.unwrap();

        // List only project tokens
        let project_tokens = list_tokens_by_prefix(&pool, "project:").await.unwrap();
        assert_eq!(project_tokens.len(), 2);

        // List all tokens
        let all = list_tokens(&pool).await.unwrap();
        assert_eq!(all.len(), 3);
    }
}
