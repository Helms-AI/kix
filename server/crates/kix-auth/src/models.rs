//! Data models for OAuth 2.1 authentication.
//!
//! These models represent the core entities persisted in SQLite:
//! - `OAuthClient`: Registered OAuth clients (RFC 7591)
//! - `OAuthToken`: Access and refresh tokens
//! - `OAuthSession`: Client sessions for token binding
//! - `AuthorizationCode`: Authorization codes for auth code flow

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// OAuth 2.0 Client (RFC 7591 Dynamic Client Registration).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthClient {
    /// Unique client identifier
    pub client_id: String,

    /// Human-readable client name
    pub client_name: String,

    /// Allowed redirect URIs
    pub redirect_uris: Vec<String>,

    /// Allowed grant types (e.g., ["authorization_code", "refresh_token"])
    pub grant_types: Vec<String>,

    /// Allowed response types (e.g., ["code"])
    pub response_types: Vec<String>,

    /// Token endpoint authentication method (typically "none" for public clients)
    pub token_endpoint_auth_method: String,

    /// When the client was registered
    pub created_at: DateTime<Utc>,

    /// Last update time
    pub updated_at: DateTime<Utc>,
}

impl OAuthClient {
    /// Check if a redirect URI is valid for this client.
    pub fn is_valid_redirect_uri(&self, uri: &str) -> bool {
        self.redirect_uris.iter().any(|allowed| allowed == uri)
    }

    /// Check if a grant type is allowed for this client.
    pub fn is_valid_grant_type(&self, grant_type: &str) -> bool {
        self.grant_types.iter().any(|allowed| allowed == grant_type)
    }
}

/// Type of OAuth token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenType {
    /// Short-lived access token for API requests
    Access,
    /// Long-lived refresh token for obtaining new access tokens
    Refresh,
}

impl TokenType {
    /// Convert to database string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            TokenType::Access => "access",
            TokenType::Refresh => "refresh",
        }
    }

    /// Parse from database string representation.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "access" => Some(TokenType::Access),
            "refresh" => Some(TokenType::Refresh),
            _ => None,
        }
    }
}

/// OAuth access or refresh token.
///
/// Note: The actual token value is never stored; only its SHA-256 hash.
#[derive(Debug, Clone)]
pub struct OAuthToken {
    /// Unique token identifier (UUID)
    pub token_id: String,

    /// Type of token (access or refresh)
    pub token_type: TokenType,

    /// Client that owns this token
    pub client_id: String,

    /// Session this token belongs to (for revocation)
    pub session_id: Option<String>,

    /// Granted scopes (space-separated)
    pub scope: Option<String>,

    /// When the token was issued
    pub issued_at: DateTime<Utc>,

    /// When the token expires
    pub expires_at: DateTime<Utc>,

    /// When the token was revoked (None if still valid)
    pub revoked_at: Option<DateTime<Utc>>,
}

impl OAuthToken {
    /// Check if the token is expired.
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// Check if the token is revoked.
    pub fn is_revoked(&self) -> bool {
        self.revoked_at.is_some()
    }

    /// Check if the token is valid (not expired and not revoked).
    pub fn is_valid(&self) -> bool {
        !self.is_expired() && !self.is_revoked()
    }
}

/// Client session for token binding and management.
#[derive(Debug, Clone)]
pub struct OAuthSession {
    /// Unique session identifier (UUID)
    pub session_id: String,

    /// Client that owns this session
    pub client_id: String,

    /// When the session was created
    pub created_at: DateTime<Utc>,

    /// Last activity timestamp
    pub last_activity: DateTime<Utc>,

    /// When the session expires
    pub expires_at: DateTime<Utc>,

    /// Optional metadata (JSON)
    pub metadata: Option<serde_json::Value>,
}

impl OAuthSession {
    /// Check if the session is expired.
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }
}

/// Authorization code for auth code flow.
///
/// Note: The actual code value is never stored; only its SHA-256 hash.
#[derive(Debug, Clone)]
pub struct AuthorizationCode {
    /// Client that requested authorization
    pub client_id: String,

    /// Redirect URI specified in the request
    pub redirect_uri: String,

    /// Requested scopes (space-separated)
    pub scope: Option<String>,

    /// PKCE code challenge
    pub code_challenge: Option<String>,

    /// PKCE code challenge method (always "S256")
    pub code_challenge_method: Option<String>,

    /// When the code was created
    pub created_at: DateTime<Utc>,

    /// When the code expires (typically 10 minutes)
    pub expires_at: DateTime<Utc>,

    /// When the code was used (None if not yet exchanged)
    pub used_at: Option<DateTime<Utc>>,
}

impl AuthorizationCode {
    /// Check if the code is expired.
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// Check if the code has been used.
    pub fn is_used(&self) -> bool {
        self.used_at.is_some()
    }

    /// Check if the code is valid (not expired and not used).
    pub fn is_valid(&self) -> bool {
        !self.is_expired() && !self.is_used()
    }
}

/// Result of validating an access token.
///
/// Contains the essential claims for authorization decisions.
#[derive(Debug, Clone)]
pub struct ValidatedToken {
    /// Client ID that owns the token
    pub client_id: String,

    /// Session ID (if bound to a session)
    pub session_id: Option<String>,

    /// Granted scopes
    pub scope: Option<String>,

    /// When the token expires
    pub expires_at: DateTime<Utc>,
}

/// Request body for client registration (RFC 7591).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientRegistrationRequest {
    /// Human-readable client name
    pub client_name: String,

    /// Redirect URIs for authorization callbacks
    pub redirect_uris: Vec<String>,

    /// Grant types the client will use (default: ["authorization_code", "refresh_token"])
    #[serde(default = "default_grant_types")]
    pub grant_types: Vec<String>,

    /// Response types the client will use (default: ["code"])
    #[serde(default = "default_response_types")]
    pub response_types: Vec<String>,

    /// Token endpoint auth method (default: "none" for public clients)
    #[serde(default = "default_auth_method")]
    pub token_endpoint_auth_method: String,
}

fn default_grant_types() -> Vec<String> {
    vec!["authorization_code".to_string(), "refresh_token".to_string()]
}

fn default_response_types() -> Vec<String> {
    vec!["code".to_string()]
}

fn default_auth_method() -> String {
    "none".to_string()
}

/// Response body for client registration (RFC 7591).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientRegistrationResponse {
    /// Assigned client identifier
    pub client_id: String,

    /// Client name (echoed back)
    pub client_name: String,

    /// Registered redirect URIs
    pub redirect_uris: Vec<String>,

    /// Allowed grant types
    pub grant_types: Vec<String>,

    /// Allowed response types
    pub response_types: Vec<String>,

    /// Token endpoint auth method
    pub token_endpoint_auth_method: String,

    /// When the client was registered
    pub client_id_issued_at: i64,
}

/// Query parameters for authorization request.
#[derive(Debug, Clone, Deserialize)]
pub struct AuthorizeParams {
    /// Response type (must be "code")
    pub response_type: String,

    /// Client identifier
    pub client_id: String,

    /// Redirect URI
    pub redirect_uri: String,

    /// Requested scopes (space-separated)
    pub scope: Option<String>,

    /// CSRF protection state
    pub state: Option<String>,

    /// PKCE code challenge
    pub code_challenge: Option<String>,

    /// PKCE code challenge method (must be "S256")
    pub code_challenge_method: Option<String>,
}

/// Form parameters for token request.
#[derive(Debug, Clone, Deserialize)]
pub struct TokenParams {
    /// Grant type ("authorization_code" or "refresh_token")
    pub grant_type: String,

    /// Authorization code (for authorization_code grant)
    pub code: Option<String>,

    /// Redirect URI (for authorization_code grant)
    pub redirect_uri: Option<String>,

    /// PKCE code verifier (for authorization_code grant)
    pub code_verifier: Option<String>,

    /// Refresh token (for refresh_token grant)
    pub refresh_token: Option<String>,

    /// Client identifier (optional, may be in Authorization header)
    pub client_id: Option<String>,
}

/// Response body for token request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenResponse {
    /// Access token
    pub access_token: String,

    /// Token type (always "Bearer")
    pub token_type: String,

    /// Time until expiration in seconds
    pub expires_in: i64,

    /// Refresh token (if granted)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,

    /// Granted scopes (if different from requested)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

/// Form parameters for token revocation.
#[derive(Debug, Clone, Deserialize)]
pub struct RevokeParams {
    /// Token to revoke
    pub token: String,

    /// Token type hint ("access_token" or "refresh_token")
    #[serde(default)]
    pub token_type_hint: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_type_conversion() {
        assert_eq!(TokenType::Access.as_str(), "access");
        assert_eq!(TokenType::Refresh.as_str(), "refresh");
        assert_eq!(TokenType::from_str("access"), Some(TokenType::Access));
        assert_eq!(TokenType::from_str("refresh"), Some(TokenType::Refresh));
        assert_eq!(TokenType::from_str("invalid"), None);
    }

    #[test]
    fn test_client_redirect_uri_validation() {
        let client = OAuthClient {
            client_id: "test".to_string(),
            client_name: "Test".to_string(),
            redirect_uris: vec![
                "http://localhost:3000/callback".to_string(),
                "https://example.com/callback".to_string(),
            ],
            grant_types: vec!["authorization_code".to_string()],
            response_types: vec!["code".to_string()],
            token_endpoint_auth_method: "none".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert!(client.is_valid_redirect_uri("http://localhost:3000/callback"));
        assert!(client.is_valid_redirect_uri("https://example.com/callback"));
        assert!(!client.is_valid_redirect_uri("https://evil.com/callback"));
    }
}
