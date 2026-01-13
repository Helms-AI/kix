//! Auth error types for the kix-auth crate.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use thiserror::Error;

/// Errors that can occur during authentication and authorization operations.
#[derive(Error, Debug)]
pub enum AuthError {
    /// Database connection or query error
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Client not found during lookup
    #[error("Client not found: {0}")]
    ClientNotFound(String),

    /// Token validation failed
    #[error("Invalid token")]
    InvalidToken,

    /// Token has expired
    #[error("Token expired")]
    TokenExpired,

    /// Token has been revoked
    #[error("Token revoked")]
    TokenRevoked,

    /// Authorization code not found or already used
    #[error("Invalid authorization code")]
    InvalidAuthCode,

    /// PKCE verification failed
    #[error("PKCE verification failed")]
    PkceVerificationFailed,

    /// Invalid redirect URI
    #[error("Invalid redirect URI: {0}")]
    InvalidRedirectUri(String),

    /// Missing required parameter
    #[error("Missing required parameter: {0}")]
    MissingParameter(String),

    /// Invalid grant type
    #[error("Unsupported grant type: {0}")]
    UnsupportedGrantType(String),

    /// Invalid request format
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    /// Session not found
    #[error("Session not found: {0}")]
    SessionNotFound(String),

    /// Internal server error
    #[error("Internal error: {0}")]
    Internal(String),
}

impl AuthError {
    /// Convert error to OAuth 2.0 error code.
    pub fn oauth_error_code(&self) -> &'static str {
        match self {
            AuthError::Database(_) => "server_error",
            AuthError::ClientNotFound(_) => "invalid_client",
            AuthError::InvalidToken => "invalid_token",
            AuthError::TokenExpired => "invalid_token",
            AuthError::TokenRevoked => "invalid_token",
            AuthError::InvalidAuthCode => "invalid_grant",
            AuthError::PkceVerificationFailed => "invalid_grant",
            AuthError::InvalidRedirectUri(_) => "invalid_request",
            AuthError::MissingParameter(_) => "invalid_request",
            AuthError::UnsupportedGrantType(_) => "unsupported_grant_type",
            AuthError::InvalidRequest(_) => "invalid_request",
            AuthError::SessionNotFound(_) => "invalid_grant",
            AuthError::Internal(_) => "server_error",
        }
    }

    /// Get HTTP status code for this error.
    pub fn status_code(&self) -> StatusCode {
        match self {
            AuthError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AuthError::ClientNotFound(_) => StatusCode::UNAUTHORIZED,
            AuthError::InvalidToken => StatusCode::UNAUTHORIZED,
            AuthError::TokenExpired => StatusCode::UNAUTHORIZED,
            AuthError::TokenRevoked => StatusCode::UNAUTHORIZED,
            AuthError::InvalidAuthCode => StatusCode::BAD_REQUEST,
            AuthError::PkceVerificationFailed => StatusCode::BAD_REQUEST,
            AuthError::InvalidRedirectUri(_) => StatusCode::BAD_REQUEST,
            AuthError::MissingParameter(_) => StatusCode::BAD_REQUEST,
            AuthError::UnsupportedGrantType(_) => StatusCode::BAD_REQUEST,
            AuthError::InvalidRequest(_) => StatusCode::BAD_REQUEST,
            AuthError::SessionNotFound(_) => StatusCode::BAD_REQUEST,
            AuthError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

/// OAuth 2.0 error response body.
#[derive(Debug, serde::Serialize)]
pub struct OAuthErrorResponse {
    pub error: &'static str,
    pub error_description: String,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let body = OAuthErrorResponse {
            error: self.oauth_error_code(),
            error_description: self.to_string(),
        };

        (status, axum::Json(body)).into_response()
    }
}

/// Result type alias for auth operations.
pub type AuthResult<T> = Result<T, AuthError>;
