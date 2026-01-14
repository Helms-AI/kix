//! SQLite persistence layer for OAuth authentication data.
//!
//! AuthStore manages clients, tokens, sessions, and authorization codes
//! with durable storage in SQLite. It is designed to be wrapped in
//! `Arc` for sharing across multiple async tasks.

use chrono::{DateTime, Duration, Utc};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::path::Path;
use std::str::FromStr;
use tracing::{debug, info, warn};

use crate::error::{AuthError, AuthResult};
use crate::models::{AuthorizationCode, OAuthClient, OAuthSession, OAuthToken, ValidatedToken};
use crate::token::hash_token;

/// Statistics from cleanup operations.
#[derive(Debug, Clone, Default)]
pub struct CleanupStats {
    /// Number of expired tokens removed
    pub tokens_removed: u64,
    /// Number of expired sessions removed
    pub sessions_removed: u64,
    /// Number of expired/used codes removed
    pub codes_removed: u64,
}

/// SQLite-backed authentication store.
///
/// Manages OAuth clients, tokens, sessions, and authorization codes.
/// Thread-safe via internal connection pool; wrap in `Arc` for sharing.
///
/// # Example
///
/// ```rust,ignore
/// use kix_auth::AuthStore;
/// use std::sync::Arc;
///
/// let store = AuthStore::new("./data/auth.db").await?;
/// store.init_schema().await?;
///
/// // Share across tasks
/// let shared_store = Arc::new(store);
/// ```
pub struct AuthStore {
    pool: SqlitePool,
}

impl AuthStore {
    /// Creates a new auth store with SQLite at the given path.
    ///
    /// Creates the database file if it doesn't exist.
    pub async fn new<P: AsRef<Path>>(db_path: P) -> AuthResult<Self> {
        let path = db_path.as_ref();
        info!("Connecting to auth SQLite database at: {}", path.display());

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                AuthError::Internal(format!("Failed to create database directory: {}", e))
            })?;
        }

        let options = SqliteConnectOptions::from_str(&format!("sqlite:{}?mode=rwc", path.display()))
            .map_err(|e| AuthError::Internal(format!("Invalid database path: {}", e)))?
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .busy_timeout(std::time::Duration::from_secs(30))
            .create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;

        // Set additional performance PRAGMAs
        sqlx::query("PRAGMA synchronous = NORMAL").execute(&pool).await?;
        sqlx::query("PRAGMA cache_size = -64000").execute(&pool).await?; // 64MB
        sqlx::query("PRAGMA mmap_size = 268435456").execute(&pool).await?; // 256MB
        sqlx::query("PRAGMA temp_store = MEMORY").execute(&pool).await?;
        sqlx::query("PRAGMA foreign_keys = ON").execute(&pool).await?;

        Ok(Self { pool })
    }

    /// Initializes the database schema (creates tables if they don't exist).
    pub async fn init_schema(&self) -> AuthResult<()> {
        info!("Initializing auth database schema");

        // Execute schema SQL (split by semicolons for individual statements)
        let schema = include_str!("schema.sql");

        // Remove all SQL comments (both line and inline) before processing
        let schema_no_comments: String = schema
            .lines()
            .map(|line| {
                // Remove inline comments (everything after --)
                if let Some(idx) = line.find("--") {
                    &line[..idx]
                } else {
                    line
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        for statement in schema_no_comments.split(';') {
            let stmt = statement.trim();
            if !stmt.is_empty() {
                sqlx::query(stmt).execute(&self.pool).await?;
            }
        }

        info!("Auth database schema initialized successfully");
        Ok(())
    }

    // ==================== Client Operations ====================

    /// Registers a new OAuth client.
    pub async fn register_client(&self, client: &OAuthClient) -> AuthResult<()> {
        debug!("Registering client: {}", client.client_id);

        sqlx::query(
            r#"
            INSERT INTO oauth_clients (
                client_id, client_name, redirect_uris, grant_types,
                response_types, token_endpoint_auth_method, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&client.client_id)
        .bind(&client.client_name)
        .bind(serde_json::to_string(&client.redirect_uris).unwrap())
        .bind(serde_json::to_string(&client.grant_types).unwrap())
        .bind(serde_json::to_string(&client.response_types).unwrap())
        .bind(&client.token_endpoint_auth_method)
        .bind(client.created_at.to_rfc3339())
        .bind(client.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        info!("Registered client: {} ({})", client.client_name, client.client_id);
        Ok(())
    }

    /// Retrieves a client by ID.
    pub async fn get_client(&self, client_id: &str) -> AuthResult<Option<OAuthClient>> {
        let row = sqlx::query(
            r#"
            SELECT client_id, client_name, redirect_uris, grant_types,
                   response_types, token_endpoint_auth_method, created_at, updated_at
            FROM oauth_clients WHERE client_id = ?
            "#,
        )
        .bind(client_id)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let client = OAuthClient {
                    client_id: row.get("client_id"),
                    client_name: row.get("client_name"),
                    redirect_uris: serde_json::from_str(row.get("redirect_uris")).unwrap_or_default(),
                    grant_types: serde_json::from_str(row.get("grant_types")).unwrap_or_default(),
                    response_types: serde_json::from_str(row.get("response_types")).unwrap_or_default(),
                    token_endpoint_auth_method: row.get("token_endpoint_auth_method"),
                    created_at: DateTime::parse_from_rfc3339(row.get("created_at"))
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    updated_at: DateTime::parse_from_rfc3339(row.get("updated_at"))
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                };
                Ok(Some(client))
            }
            None => Ok(None),
        }
    }

    /// Lists all registered clients.
    pub async fn list_clients(&self) -> AuthResult<Vec<OAuthClient>> {
        let rows = sqlx::query(
            r#"
            SELECT client_id, client_name, redirect_uris, grant_types,
                   response_types, token_endpoint_auth_method, created_at, updated_at
            FROM oauth_clients ORDER BY created_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let clients = rows
            .into_iter()
            .map(|row| OAuthClient {
                client_id: row.get("client_id"),
                client_name: row.get("client_name"),
                redirect_uris: serde_json::from_str(row.get("redirect_uris")).unwrap_or_default(),
                grant_types: serde_json::from_str(row.get("grant_types")).unwrap_or_default(),
                response_types: serde_json::from_str(row.get("response_types")).unwrap_or_default(),
                token_endpoint_auth_method: row.get("token_endpoint_auth_method"),
                created_at: DateTime::parse_from_rfc3339(row.get("created_at"))
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                updated_at: DateTime::parse_from_rfc3339(row.get("updated_at"))
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
            .collect();

        Ok(clients)
    }

    /// Deletes a client and all associated tokens/sessions.
    pub async fn delete_client(&self, client_id: &str) -> AuthResult<bool> {
        let result = sqlx::query("DELETE FROM oauth_clients WHERE client_id = ?")
            .bind(client_id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    // ==================== Token Operations ====================

    /// Stores a new token (access or refresh).
    ///
    /// The raw token value is hashed before storage.
    pub async fn store_token(&self, token: &OAuthToken, token_value: &str) -> AuthResult<()> {
        let token_hash = hash_token(token_value);

        sqlx::query(
            r#"
            INSERT INTO oauth_tokens (
                token_id, token_hash, token_type, client_id, session_id,
                scope, issued_at, expires_at, revoked_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&token.token_id)
        .bind(&token_hash)
        .bind(token.token_type.as_str())
        .bind(&token.client_id)
        .bind(&token.session_id)
        .bind(&token.scope)
        .bind(token.issued_at.to_rfc3339())
        .bind(token.expires_at.to_rfc3339())
        .bind(token.revoked_at.map(|dt| dt.to_rfc3339()))
        .execute(&self.pool)
        .await?;

        debug!(
            "Stored {} token for client {} (expires: {})",
            token.token_type.as_str(),
            token.client_id,
            token.expires_at
        );
        Ok(())
    }

    /// Validates a token and returns its claims if valid.
    ///
    /// Returns `None` if the token is invalid, expired, or revoked.
    pub async fn validate_token(&self, token_value: &str) -> AuthResult<Option<ValidatedToken>> {
        let token_hash = hash_token(token_value);

        let row = sqlx::query(
            r#"
            SELECT token_id, token_type, client_id, session_id, scope,
                   issued_at, expires_at, revoked_at
            FROM oauth_tokens WHERE token_hash = ?
            "#,
        )
        .bind(&token_hash)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let expires_at: String = row.get("expires_at");
                let expires_at = DateTime::parse_from_rfc3339(&expires_at)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());

                let revoked_at: Option<String> = row.get("revoked_at");

                // Check if revoked
                if revoked_at.is_some() {
                    debug!("Token validation failed: revoked");
                    return Ok(None);
                }

                // Check if expired
                if Utc::now() > expires_at {
                    debug!("Token validation failed: expired");
                    return Ok(None);
                }

                Ok(Some(ValidatedToken {
                    client_id: row.get("client_id"),
                    session_id: row.get("session_id"),
                    scope: row.get("scope"),
                    expires_at,
                }))
            }
            None => {
                debug!("Token validation failed: not found");
                Ok(None)
            }
        }
    }

    /// Revokes a specific token.
    pub async fn revoke_token(&self, token_value: &str) -> AuthResult<bool> {
        let token_hash = hash_token(token_value);
        let now = Utc::now().to_rfc3339();

        let result = sqlx::query(
            "UPDATE oauth_tokens SET revoked_at = ? WHERE token_hash = ? AND revoked_at IS NULL",
        )
        .bind(&now)
        .bind(&token_hash)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() > 0 {
            debug!("Revoked token");
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Revokes all tokens for a session.
    pub async fn revoke_tokens_for_session(&self, session_id: &str) -> AuthResult<u64> {
        let now = Utc::now().to_rfc3339();

        let result = sqlx::query(
            "UPDATE oauth_tokens SET revoked_at = ? WHERE session_id = ? AND revoked_at IS NULL",
        )
        .bind(&now)
        .bind(session_id)
        .execute(&self.pool)
        .await?;

        let count = result.rows_affected();
        if count > 0 {
            debug!("Revoked {} tokens for session {}", count, session_id);
        }
        Ok(count)
    }

    /// Revokes all tokens for a client.
    pub async fn revoke_tokens_for_client(&self, client_id: &str) -> AuthResult<u64> {
        let now = Utc::now().to_rfc3339();

        let result = sqlx::query(
            "UPDATE oauth_tokens SET revoked_at = ? WHERE client_id = ? AND revoked_at IS NULL",
        )
        .bind(&now)
        .bind(client_id)
        .execute(&self.pool)
        .await?;

        let count = result.rows_affected();
        if count > 0 {
            debug!("Revoked {} tokens for client {}", count, client_id);
        }
        Ok(count)
    }

    // ==================== Session Operations ====================

    /// Creates a new session.
    pub async fn create_session(&self, session: &OAuthSession) -> AuthResult<()> {
        sqlx::query(
            r#"
            INSERT INTO oauth_sessions (
                session_id, client_id, created_at, last_activity, expires_at, metadata
            ) VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&session.session_id)
        .bind(&session.client_id)
        .bind(session.created_at.to_rfc3339())
        .bind(session.last_activity.to_rfc3339())
        .bind(session.expires_at.to_rfc3339())
        .bind(session.metadata.as_ref().map(|m| m.to_string()))
        .execute(&self.pool)
        .await?;

        debug!("Created session {} for client {}", session.session_id, session.client_id);
        Ok(())
    }

    /// Retrieves a session by ID.
    pub async fn get_session(&self, session_id: &str) -> AuthResult<Option<OAuthSession>> {
        let row = sqlx::query(
            r#"
            SELECT session_id, client_id, created_at, last_activity, expires_at, metadata
            FROM oauth_sessions WHERE session_id = ?
            "#,
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let metadata: Option<String> = row.get("metadata");
                Ok(Some(OAuthSession {
                    session_id: row.get("session_id"),
                    client_id: row.get("client_id"),
                    created_at: DateTime::parse_from_rfc3339(row.get("created_at"))
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    last_activity: DateTime::parse_from_rfc3339(row.get("last_activity"))
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    expires_at: DateTime::parse_from_rfc3339(row.get("expires_at"))
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    metadata: metadata.and_then(|m| serde_json::from_str(&m).ok()),
                }))
            }
            None => Ok(None),
        }
    }

    /// Updates session last activity timestamp.
    pub async fn update_session_activity(&self, session_id: &str) -> AuthResult<bool> {
        let now = Utc::now().to_rfc3339();

        let result = sqlx::query("UPDATE oauth_sessions SET last_activity = ? WHERE session_id = ?")
            .bind(&now)
            .bind(session_id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Deletes a session and revokes all associated tokens.
    pub async fn delete_session(&self, session_id: &str) -> AuthResult<bool> {
        // First revoke all tokens for this session
        self.revoke_tokens_for_session(session_id).await?;

        // Then delete the session
        let result = sqlx::query("DELETE FROM oauth_sessions WHERE session_id = ?")
            .bind(session_id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    // ==================== Authorization Code Operations ====================

    /// Stores an authorization code.
    ///
    /// The raw code value is hashed before storage.
    pub async fn store_code(&self, code: &AuthorizationCode, code_value: &str) -> AuthResult<()> {
        let code_hash = hash_token(code_value);

        sqlx::query(
            r#"
            INSERT INTO oauth_codes (
                code_hash, client_id, redirect_uri, scope,
                code_challenge, code_challenge_method, created_at, expires_at, used_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&code_hash)
        .bind(&code.client_id)
        .bind(&code.redirect_uri)
        .bind(&code.scope)
        .bind(&code.code_challenge)
        .bind(&code.code_challenge_method)
        .bind(code.created_at.to_rfc3339())
        .bind(code.expires_at.to_rfc3339())
        .bind(code.used_at.map(|dt| dt.to_rfc3339()))
        .execute(&self.pool)
        .await?;

        debug!("Stored authorization code for client {}", code.client_id);
        Ok(())
    }

    /// Consumes an authorization code (marks as used and returns it).
    ///
    /// Returns `None` if the code is invalid, expired, or already used.
    pub async fn consume_code(&self, code_value: &str) -> AuthResult<Option<AuthorizationCode>> {
        let code_hash = hash_token(code_value);

        // Fetch the code
        let row = sqlx::query(
            r#"
            SELECT client_id, redirect_uri, scope, code_challenge, code_challenge_method,
                   created_at, expires_at, used_at
            FROM oauth_codes WHERE code_hash = ?
            "#,
        )
        .bind(&code_hash)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let expires_at: String = row.get("expires_at");
                let expires_at = DateTime::parse_from_rfc3339(&expires_at)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());

                let used_at: Option<String> = row.get("used_at");

                // Check if already used
                if used_at.is_some() {
                    warn!("Authorization code already used");
                    return Ok(None);
                }

                // Check if expired
                if Utc::now() > expires_at {
                    warn!("Authorization code expired");
                    return Ok(None);
                }

                // Mark as used
                let now = Utc::now().to_rfc3339();
                sqlx::query("UPDATE oauth_codes SET used_at = ? WHERE code_hash = ?")
                    .bind(&now)
                    .bind(&code_hash)
                    .execute(&self.pool)
                    .await?;

                let created_at: String = row.get("created_at");
                Ok(Some(AuthorizationCode {
                    client_id: row.get("client_id"),
                    redirect_uri: row.get("redirect_uri"),
                    scope: row.get("scope"),
                    code_challenge: row.get("code_challenge"),
                    code_challenge_method: row.get("code_challenge_method"),
                    created_at: DateTime::parse_from_rfc3339(&created_at)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    expires_at,
                    used_at: Some(Utc::now()),
                }))
            }
            None => {
                warn!("Authorization code not found");
                Ok(None)
            }
        }
    }

    // ==================== Cleanup Operations ====================

    /// Removes expired tokens, sessions, and codes.
    ///
    /// Should be called periodically (e.g., every hour) to clean up stale data.
    pub async fn cleanup_expired(&self) -> AuthResult<CleanupStats> {
        let now = Utc::now().to_rfc3339();
        let mut stats = CleanupStats::default();

        // Clean up expired tokens
        let result = sqlx::query("DELETE FROM oauth_tokens WHERE expires_at < ?")
            .bind(&now)
            .execute(&self.pool)
            .await?;
        stats.tokens_removed = result.rows_affected();

        // Clean up expired sessions
        let result = sqlx::query("DELETE FROM oauth_sessions WHERE expires_at < ?")
            .bind(&now)
            .execute(&self.pool)
            .await?;
        stats.sessions_removed = result.rows_affected();

        // Clean up expired or used codes (keep used codes for 1 hour for debugging)
        let one_hour_ago = (Utc::now() - Duration::hours(1)).to_rfc3339();
        let result = sqlx::query(
            "DELETE FROM oauth_codes WHERE expires_at < ? OR (used_at IS NOT NULL AND used_at < ?)",
        )
        .bind(&now)
        .bind(&one_hour_ago)
        .execute(&self.pool)
        .await?;
        stats.codes_removed = result.rows_affected();

        if stats.tokens_removed > 0 || stats.sessions_removed > 0 || stats.codes_removed > 0 {
            info!(
                "Auth cleanup: {} tokens, {} sessions, {} codes removed",
                stats.tokens_removed, stats.sessions_removed, stats.codes_removed
            );
        }

        Ok(stats)
    }

    /// Returns statistics about the auth store.
    pub async fn stats(&self) -> AuthResult<AuthStoreStats> {
        let clients: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM oauth_clients")
            .fetch_one(&self.pool)
            .await?;

        let active_tokens: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM oauth_tokens WHERE revoked_at IS NULL AND expires_at > ?",
        )
        .bind(Utc::now().to_rfc3339())
        .fetch_one(&self.pool)
        .await?;

        let active_sessions: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM oauth_sessions WHERE expires_at > ?")
                .bind(Utc::now().to_rfc3339())
                .fetch_one(&self.pool)
                .await?;

        Ok(AuthStoreStats {
            clients: clients as u64,
            active_tokens: active_tokens as u64,
            active_sessions: active_sessions as u64,
        })
    }
}

/// Statistics about the auth store.
#[derive(Debug, Clone, Default)]
pub struct AuthStoreStats {
    /// Number of registered clients
    pub clients: u64,
    /// Number of valid (non-expired, non-revoked) tokens
    pub active_tokens: u64,
    /// Number of active (non-expired) sessions
    pub active_sessions: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::TokenType;
    use tempfile::tempdir;

    async fn create_test_store() -> AuthStore {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test_auth.db");
        let store = AuthStore::new(&db_path).await.unwrap();
        store.init_schema().await.unwrap();
        store
    }

    #[tokio::test]
    async fn test_client_crud() {
        let store = create_test_store().await;

        let client = OAuthClient {
            client_id: "test_client".to_string(),
            client_name: "Test Client".to_string(),
            redirect_uris: vec!["http://localhost:3000/callback".to_string()],
            grant_types: vec!["authorization_code".to_string()],
            response_types: vec!["code".to_string()],
            token_endpoint_auth_method: "none".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // Create
        store.register_client(&client).await.unwrap();

        // Read
        let fetched = store.get_client("test_client").await.unwrap();
        assert!(fetched.is_some());
        let fetched = fetched.unwrap();
        assert_eq!(fetched.client_name, "Test Client");

        // List
        let clients = store.list_clients().await.unwrap();
        assert_eq!(clients.len(), 1);

        // Delete
        let deleted = store.delete_client("test_client").await.unwrap();
        assert!(deleted);

        let fetched = store.get_client("test_client").await.unwrap();
        assert!(fetched.is_none());
    }

    #[tokio::test]
    async fn test_token_validation() {
        let store = create_test_store().await;

        // Create a client first
        let client = OAuthClient {
            client_id: "test_client".to_string(),
            client_name: "Test".to_string(),
            redirect_uris: vec![],
            grant_types: vec![],
            response_types: vec![],
            token_endpoint_auth_method: "none".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        store.register_client(&client).await.unwrap();

        // Create a token
        let token = OAuthToken {
            token_id: "token_1".to_string(),
            token_type: TokenType::Access,
            client_id: "test_client".to_string(),
            session_id: None,
            scope: Some("read write".to_string()),
            issued_at: Utc::now(),
            expires_at: Utc::now() + Duration::hours(1),
            revoked_at: None,
        };
        let token_value = "test_token_value_123";
        store.store_token(&token, token_value).await.unwrap();

        // Validate
        let validated = store.validate_token(token_value).await.unwrap();
        assert!(validated.is_some());
        let validated = validated.unwrap();
        assert_eq!(validated.client_id, "test_client");
        assert_eq!(validated.scope, Some("read write".to_string()));

        // Invalid token should fail
        let invalid = store.validate_token("wrong_token").await.unwrap();
        assert!(invalid.is_none());

        // Revoke and verify
        store.revoke_token(token_value).await.unwrap();
        let revoked = store.validate_token(token_value).await.unwrap();
        assert!(revoked.is_none());
    }

    #[tokio::test]
    async fn test_authorization_code_flow() {
        let store = create_test_store().await;

        // Create a client
        let client = OAuthClient {
            client_id: "test_client".to_string(),
            client_name: "Test".to_string(),
            redirect_uris: vec!["http://localhost/callback".to_string()],
            grant_types: vec!["authorization_code".to_string()],
            response_types: vec!["code".to_string()],
            token_endpoint_auth_method: "none".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        store.register_client(&client).await.unwrap();

        // Create an auth code
        let code = AuthorizationCode {
            client_id: "test_client".to_string(),
            redirect_uri: "http://localhost/callback".to_string(),
            scope: Some("read".to_string()),
            code_challenge: Some("E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM".to_string()),
            code_challenge_method: Some("S256".to_string()),
            created_at: Utc::now(),
            expires_at: Utc::now() + Duration::minutes(10),
            used_at: None,
        };
        let code_value = "test_code_123";
        store.store_code(&code, code_value).await.unwrap();

        // Consume the code
        let consumed = store.consume_code(code_value).await.unwrap();
        assert!(consumed.is_some());
        let consumed = consumed.unwrap();
        assert_eq!(consumed.client_id, "test_client");

        // Second attempt should fail (already used)
        let second = store.consume_code(code_value).await.unwrap();
        assert!(second.is_none());
    }
}
