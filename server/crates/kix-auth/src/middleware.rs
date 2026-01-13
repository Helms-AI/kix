//! Axum middleware for Bearer token authentication.
//!
//! Provides request-level authentication by validating Bearer tokens
//! against the AuthStore.

use axum::{
    body::Body,
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use chrono::Duration;
use std::sync::Arc;
use tracing::{debug, warn};

use crate::models::ValidatedToken;
use crate::store::AuthStore;

/// Configuration for authentication behavior.
#[derive(Clone, Debug)]
pub struct AuthConfig {
    /// Access token lifetime (default: 1 hour)
    pub access_token_ttl: Duration,

    /// Refresh token lifetime (default: 7 days)
    pub refresh_token_ttl: Duration,

    /// Session lifetime (default: 30 days)
    pub session_ttl: Duration,

    /// Authorization code lifetime (default: 10 minutes)
    pub auth_code_ttl: Duration,

    /// Whether authentication is required for requests (default: false for dev)
    pub require_auth: bool,

    /// Allow dynamic client registration (default: true)
    pub allow_registration: bool,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            access_token_ttl: Duration::hours(1),
            refresh_token_ttl: Duration::days(7),
            session_ttl: Duration::days(30),
            auth_code_ttl: Duration::minutes(10),
            require_auth: false, // Safe default for development
            allow_registration: true,
        }
    }
}

impl AuthConfig {
    /// Creates a config with authentication required.
    pub fn production() -> Self {
        Self {
            require_auth: true,
            ..Default::default()
        }
    }
}

/// Shared authentication state for Axum handlers and middleware.
#[derive(Clone)]
pub struct AuthState {
    /// The authentication store (wrapped in Arc for sharing)
    pub store: Arc<AuthStore>,

    /// Authentication configuration
    pub config: AuthConfig,
}

impl AuthState {
    /// Creates a new auth state with the given store and default config.
    pub fn new(store: Arc<AuthStore>) -> Self {
        Self {
            store,
            config: AuthConfig::default(),
        }
    }

    /// Creates a new auth state with the given store and config.
    pub fn with_config(store: Arc<AuthStore>, config: AuthConfig) -> Self {
        Self { store, config }
    }
}

/// Request extension containing validated token information.
///
/// Added to the request extensions by the auth middleware when
/// authentication succeeds.
#[derive(Clone, Debug)]
pub struct AuthenticatedClient {
    /// The validated token claims
    pub token: ValidatedToken,
}

/// Axum middleware that validates Bearer tokens.
///
/// If `require_auth` is true in the config, requests without valid
/// Bearer tokens will receive a 401 Unauthorized response.
///
/// If `require_auth` is false, unauthenticated requests will pass through
/// but without `AuthenticatedClient` in extensions.
///
/// # Usage
///
/// ```rust,ignore
/// use axum::{Router, middleware};
/// use kix_auth::{AuthState, auth_middleware};
///
/// let app = Router::new()
///     .route("/api/protected", get(handler))
///     .layer(middleware::from_fn_with_state(auth_state, auth_middleware));
/// ```
pub async fn auth_middleware(
    State(auth): State<AuthState>,
    mut request: Request<Body>,
    next: Next,
) -> Response {
    // Extract Bearer token from Authorization header
    let auth_header = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok());

    let token_value = match auth_header {
        Some(h) if h.starts_with("Bearer ") => Some(&h[7..]),
        _ => None,
    };

    // Try to validate the token if present
    let validated = if let Some(token) = token_value {
        match auth.store.validate_token(token).await {
            Ok(Some(v)) => {
                debug!("Authenticated request from client: {}", v.client_id);
                Some(v)
            }
            Ok(None) => {
                debug!("Invalid or expired token");
                None
            }
            Err(e) => {
                warn!("Token validation error: {}", e);
                None
            }
        }
    } else {
        None
    };

    // If we have a validated token, add it to request extensions
    if let Some(token) = validated.clone() {
        request.extensions_mut().insert(AuthenticatedClient { token });
    }

    // Check if authentication is required
    if auth.config.require_auth && validated.is_none() {
        return unauthorized_response();
    }

    // Continue with the request
    next.run(request).await
}

/// Creates a 401 Unauthorized response with WWW-Authenticate header.
fn unauthorized_response() -> Response {
    let body = serde_json::json!({
        "error": "invalid_token",
        "error_description": "Bearer token required"
    });

    (
        StatusCode::UNAUTHORIZED,
        [
            (header::WWW_AUTHENTICATE, "Bearer"),
            (header::CONTENT_TYPE, "application/json"),
        ],
        serde_json::to_string(&body).unwrap(),
    )
        .into_response()
}

/// Middleware that requires authentication (always returns 401 if no valid token).
///
/// Use this for endpoints that must always be protected.
pub async fn require_auth_middleware(
    State(auth): State<AuthState>,
    mut request: Request<Body>,
    next: Next,
) -> Response {
    // Extract Bearer token
    let auth_header = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok());

    let token_value = match auth_header {
        Some(h) if h.starts_with("Bearer ") => &h[7..],
        _ => return unauthorized_response(),
    };

    // Validate token
    match auth.store.validate_token(token_value).await {
        Ok(Some(validated)) => {
            debug!("Authenticated request from client: {}", validated.client_id);
            request.extensions_mut().insert(AuthenticatedClient { token: validated });
            next.run(request).await
        }
        Ok(None) => {
            debug!("Invalid or expired token");
            unauthorized_response()
        }
        Err(e) => {
            warn!("Token validation error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(header::CONTENT_TYPE, "application/json")],
                serde_json::json!({
                    "error": "server_error",
                    "error_description": "Token validation failed"
                })
                .to_string(),
            )
                .into_response()
        }
    }
}

/// Extracts the authenticated client from request extensions.
///
/// Returns `None` if the request is not authenticated.
pub fn get_authenticated_client(request: &Request<Body>) -> Option<&AuthenticatedClient> {
    request.extensions().get::<AuthenticatedClient>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_config_default() {
        let config = AuthConfig::default();
        assert!(!config.require_auth);
        assert!(config.allow_registration);
        assert_eq!(config.access_token_ttl, Duration::hours(1));
    }

    #[test]
    fn test_auth_config_production() {
        let config = AuthConfig::production();
        assert!(config.require_auth);
    }
}
