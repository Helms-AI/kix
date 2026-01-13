//! # kix-auth
//!
//! OAuth 2.1 authentication and authorization for the KIX MCP server.
//!
//! This crate provides:
//! - Persistent storage for OAuth clients, tokens, and sessions
//! - Axum middleware for Bearer token validation
//! - OAuth 2.1 endpoint handlers (register, authorize, token, revoke)
//! - PKCE (S256) support for secure authorization code flow
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                     kix-auth                                 │
//! ├─────────────────────────────────────────────────────────────┤
//! │  handlers.rs    → OAuth endpoints (/oauth/*)                │
//! │  middleware.rs  → Axum Bearer token validation              │
//! │  store.rs       → SQLite persistence layer                  │
//! │  models.rs      → OAuthClient, OAuthToken, OAuthSession     │
//! │  token.rs       → Token generation and hashing              │
//! │  pkce.rs        → PKCE S256 validation                      │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use kix_auth::{AuthStore, AuthState, AuthConfig};
//! use std::sync::Arc;
//!
//! // Initialize auth store
//! let auth_store = AuthStore::new("./data/auth.db").await?;
//! auth_store.init_schema().await?;
//!
//! // Create shared auth state
//! let auth_state = AuthState {
//!     store: Arc::new(auth_store),
//!     config: AuthConfig::default(),
//! };
//!
//! // Add middleware and handlers to your Axum router
//! let app = Router::new()
//!     .route("/oauth/register", post(handlers::oauth_register))
//!     .layer(middleware::from_fn_with_state(auth_state.clone(), middleware::auth_middleware))
//!     .with_state(auth_state);
//! ```

pub mod error;
pub mod handlers;
pub mod middleware;
pub mod models;
pub mod pkce;
pub mod store;
pub mod token;

// Re-export main types for convenience
pub use error::AuthError;
pub use middleware::{auth_middleware, AuthConfig, AuthState};
pub use models::{OAuthClient, OAuthSession, OAuthToken, TokenType, ValidatedToken};
pub use store::AuthStore;
