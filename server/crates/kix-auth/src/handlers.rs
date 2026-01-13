//! OAuth 2.1 endpoint handlers.
//!
//! Implements the OAuth authorization server endpoints:
//! - `/.well-known/oauth-authorization-server` - Server metadata (RFC 8414)
//! - `/.well-known/oauth-protected-resource` - Resource metadata (RFC 9728)
//! - `/oauth/register` - Dynamic client registration (RFC 7591)
//! - `/oauth/authorize` - Authorization endpoint
//! - `/oauth/token` - Token endpoint
//! - `/oauth/revoke` - Token revocation endpoint

use axum::{
    extract::{Host, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
    Form, Json,
};
use chrono::Utc;
use serde_json::json;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::error::AuthError;
use crate::middleware::AuthState;
use crate::models::{
    AuthorizationCode, AuthorizeParams, ClientRegistrationRequest, ClientRegistrationResponse,
    OAuthClient, OAuthSession, OAuthToken, RevokeParams, TokenParams, TokenResponse, TokenType,
};
use crate::pkce::{verify_pkce, CHALLENGE_METHOD_S256};
use crate::token::{generate_auth_code, generate_client_id, generate_token};

/// Helper to get the effective issuer URL from request headers.
fn get_issuer(headers: &HeaderMap, host: &str) -> String {
    // Check for X-Forwarded-Proto and X-Forwarded-Host (behind proxy)
    let proto = headers
        .get("x-forwarded-proto")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("http");

    let effective_host = headers
        .get("x-forwarded-host")
        .and_then(|h| h.to_str().ok())
        .unwrap_or(host);

    format!("{}://{}", proto, effective_host)
}

// ==================== Metadata Endpoints ====================

/// GET /.well-known/oauth-authorization-server
///
/// Returns OAuth 2.0 Authorization Server Metadata (RFC 8414).
pub async fn oauth_metadata(
    State(_auth): State<AuthState>,
    Host(host): Host,
    headers: HeaderMap,
) -> Json<serde_json::Value> {
    let issuer = get_issuer(&headers, &host);

    Json(json!({
        "issuer": issuer,
        "authorization_endpoint": format!("{}/oauth/authorize", issuer),
        "token_endpoint": format!("{}/oauth/token", issuer),
        "registration_endpoint": format!("{}/oauth/register", issuer),
        "revocation_endpoint": format!("{}/oauth/revoke", issuer),
        "response_types_supported": ["code"],
        "grant_types_supported": ["authorization_code", "refresh_token"],
        "code_challenge_methods_supported": [CHALLENGE_METHOD_S256],
        "token_endpoint_auth_methods_supported": ["none"],
        "service_documentation": "https://github.com/kon1790/eip",
    }))
}

/// GET /.well-known/oauth-protected-resource
///
/// Returns OAuth 2.0 Protected Resource Metadata (RFC 9728).
pub async fn oauth_resource_metadata(
    Host(host): Host,
    headers: HeaderMap,
) -> Json<serde_json::Value> {
    let issuer = get_issuer(&headers, &host);

    Json(json!({
        "resource": issuer,
        "authorization_servers": [issuer],
        "bearer_methods_supported": ["header"],
        "resource_documentation": "https://github.com/kon1790/eip",
    }))
}

// ==================== Client Registration ====================

/// POST /oauth/register
///
/// Dynamic client registration (RFC 7591).
pub async fn oauth_register(
    State(auth): State<AuthState>,
    Json(req): Json<ClientRegistrationRequest>,
) -> Result<Json<ClientRegistrationResponse>, AuthError> {
    if !auth.config.allow_registration {
        return Err(AuthError::InvalidRequest(
            "Client registration is disabled".to_string(),
        ));
    }

    // Validate redirect URIs
    if req.redirect_uris.is_empty() {
        return Err(AuthError::MissingParameter("redirect_uris".to_string()));
    }

    for uri in &req.redirect_uris {
        // Basic validation - must be a valid URL
        if url::Url::parse(uri).is_err() {
            return Err(AuthError::InvalidRedirectUri(uri.clone()));
        }
    }

    // Generate client ID
    let client_id = generate_client_id();
    let now = Utc::now();

    let client = OAuthClient {
        client_id: client_id.clone(),
        client_name: req.client_name.clone(),
        redirect_uris: req.redirect_uris.clone(),
        grant_types: req.grant_types.clone(),
        response_types: req.response_types.clone(),
        token_endpoint_auth_method: req.token_endpoint_auth_method.clone(),
        created_at: now,
        updated_at: now,
    };

    auth.store.register_client(&client).await?;

    info!("Registered new client: {} ({})", client.client_name, client_id);

    Ok(Json(ClientRegistrationResponse {
        client_id,
        client_name: req.client_name,
        redirect_uris: req.redirect_uris,
        grant_types: req.grant_types,
        response_types: req.response_types,
        token_endpoint_auth_method: req.token_endpoint_auth_method,
        client_id_issued_at: now.timestamp(),
    }))
}

// ==================== Authorization Endpoint ====================

/// GET /oauth/authorize
///
/// Authorization endpoint - initiates the authorization code flow.
pub async fn oauth_authorize(
    State(auth): State<AuthState>,
    Query(params): Query<AuthorizeParams>,
) -> Result<Response, AuthError> {
    // Validate response_type
    if params.response_type != "code" {
        return Err(AuthError::InvalidRequest(format!(
            "Unsupported response_type: {}",
            params.response_type
        )));
    }

    // Validate client
    let client = auth
        .store
        .get_client(&params.client_id)
        .await?
        .ok_or_else(|| AuthError::ClientNotFound(params.client_id.clone()))?;

    // Validate redirect_uri
    if !client.is_valid_redirect_uri(&params.redirect_uri) {
        return Err(AuthError::InvalidRedirectUri(params.redirect_uri.clone()));
    }

    // Validate PKCE (required per MCP 2025 spec)
    if params.code_challenge.is_none() {
        return Err(AuthError::MissingParameter("code_challenge".to_string()));
    }

    let challenge_method = params
        .code_challenge_method
        .as_deref()
        .unwrap_or(CHALLENGE_METHOD_S256);

    if challenge_method != CHALLENGE_METHOD_S256 {
        return Err(AuthError::InvalidRequest(format!(
            "Unsupported code_challenge_method: {}",
            challenge_method
        )));
    }

    // Generate authorization code
    let code_value = generate_auth_code();
    let now = Utc::now();

    let code = AuthorizationCode {
        client_id: params.client_id.clone(),
        redirect_uri: params.redirect_uri.clone(),
        scope: params.scope.clone(),
        code_challenge: params.code_challenge.clone(),
        code_challenge_method: Some(CHALLENGE_METHOD_S256.to_string()),
        created_at: now,
        expires_at: now + auth.config.auth_code_ttl,
        used_at: None,
    };

    auth.store.store_code(&code, &code_value).await?;

    debug!(
        "Generated authorization code for client {} (expires: {})",
        params.client_id, code.expires_at
    );

    // Build redirect URL with code
    let mut redirect_url =
        url::Url::parse(&params.redirect_uri).map_err(|_| AuthError::InvalidRedirectUri(params.redirect_uri.clone()))?;

    redirect_url
        .query_pairs_mut()
        .append_pair("code", &code_value);

    if let Some(state) = &params.state {
        redirect_url.query_pairs_mut().append_pair("state", state);
    }

    Ok(Redirect::to(redirect_url.as_str()).into_response())
}

// ==================== Token Endpoint ====================

/// POST /oauth/token
///
/// Token endpoint - exchanges authorization codes for tokens or refreshes tokens.
pub async fn oauth_token(
    State(auth): State<AuthState>,
    Form(params): Form<TokenParams>,
) -> Result<Json<TokenResponse>, AuthError> {
    match params.grant_type.as_str() {
        "authorization_code" => handle_authorization_code_grant(&auth, &params).await,
        "refresh_token" => handle_refresh_token_grant(&auth, &params).await,
        _ => Err(AuthError::UnsupportedGrantType(params.grant_type.clone())),
    }
}

async fn handle_authorization_code_grant(
    auth: &AuthState,
    params: &TokenParams,
) -> Result<Json<TokenResponse>, AuthError> {
    // Validate required parameters
    let code_value = params
        .code
        .as_ref()
        .ok_or_else(|| AuthError::MissingParameter("code".to_string()))?;

    let redirect_uri = params
        .redirect_uri
        .as_ref()
        .ok_or_else(|| AuthError::MissingParameter("redirect_uri".to_string()))?;

    let code_verifier = params
        .code_verifier
        .as_ref()
        .ok_or_else(|| AuthError::MissingParameter("code_verifier".to_string()))?;

    // Consume the authorization code
    let code = auth
        .store
        .consume_code(code_value)
        .await?
        .ok_or(AuthError::InvalidAuthCode)?;

    // Validate redirect_uri matches
    if &code.redirect_uri != redirect_uri {
        warn!(
            "Redirect URI mismatch: expected {}, got {}",
            code.redirect_uri, redirect_uri
        );
        return Err(AuthError::InvalidRedirectUri(redirect_uri.clone()));
    }

    // Verify PKCE
    let challenge = code
        .code_challenge
        .as_ref()
        .ok_or(AuthError::PkceVerificationFailed)?;
    let method = code
        .code_challenge_method
        .as_ref()
        .ok_or(AuthError::PkceVerificationFailed)?;

    if !verify_pkce(code_verifier, challenge, method) {
        warn!("PKCE verification failed for client {}", code.client_id);
        return Err(AuthError::PkceVerificationFailed);
    }

    // Create a session
    let session_id = Uuid::new_v4().to_string();
    let now = Utc::now();

    let session = OAuthSession {
        session_id: session_id.clone(),
        client_id: code.client_id.clone(),
        created_at: now,
        last_activity: now,
        expires_at: now + auth.config.session_ttl,
        metadata: None,
    };

    auth.store.create_session(&session).await?;

    // Generate tokens
    let access_token_value = generate_token();
    let refresh_token_value = generate_token();

    let access_token = OAuthToken {
        token_id: Uuid::new_v4().to_string(),
        token_type: TokenType::Access,
        client_id: code.client_id.clone(),
        session_id: Some(session_id.clone()),
        scope: code.scope.clone(),
        issued_at: now,
        expires_at: now + auth.config.access_token_ttl,
        revoked_at: None,
    };

    let refresh_token = OAuthToken {
        token_id: Uuid::new_v4().to_string(),
        token_type: TokenType::Refresh,
        client_id: code.client_id.clone(),
        session_id: Some(session_id),
        scope: code.scope.clone(),
        issued_at: now,
        expires_at: now + auth.config.refresh_token_ttl,
        revoked_at: None,
    };

    auth.store
        .store_token(&access_token, &access_token_value)
        .await?;
    auth.store
        .store_token(&refresh_token, &refresh_token_value)
        .await?;

    info!(
        "Issued tokens for client {} (session: {})",
        code.client_id, access_token.session_id.as_ref().unwrap()
    );

    Ok(Json(TokenResponse {
        access_token: access_token_value,
        token_type: "Bearer".to_string(),
        expires_in: auth.config.access_token_ttl.num_seconds(),
        refresh_token: Some(refresh_token_value),
        scope: code.scope,
    }))
}

async fn handle_refresh_token_grant(
    auth: &AuthState,
    params: &TokenParams,
) -> Result<Json<TokenResponse>, AuthError> {
    let refresh_token_value = params
        .refresh_token
        .as_ref()
        .ok_or_else(|| AuthError::MissingParameter("refresh_token".to_string()))?;

    // Validate refresh token
    let validated = auth
        .store
        .validate_token(refresh_token_value)
        .await?
        .ok_or(AuthError::InvalidToken)?;

    // Update session activity if bound to a session
    if let Some(session_id) = &validated.session_id {
        auth.store.update_session_activity(session_id).await?;
    }

    // Generate new access token
    let now = Utc::now();
    let access_token_value = generate_token();

    let access_token = OAuthToken {
        token_id: Uuid::new_v4().to_string(),
        token_type: TokenType::Access,
        client_id: validated.client_id.clone(),
        session_id: validated.session_id.clone(),
        scope: validated.scope.clone(),
        issued_at: now,
        expires_at: now + auth.config.access_token_ttl,
        revoked_at: None,
    };

    auth.store
        .store_token(&access_token, &access_token_value)
        .await?;

    debug!("Refreshed access token for client {}", validated.client_id);

    Ok(Json(TokenResponse {
        access_token: access_token_value,
        token_type: "Bearer".to_string(),
        expires_in: auth.config.access_token_ttl.num_seconds(),
        refresh_token: None, // Don't rotate refresh token by default
        scope: validated.scope,
    }))
}

// ==================== Token Revocation ====================

/// POST /oauth/revoke
///
/// Token revocation endpoint (RFC 7009).
pub async fn oauth_revoke(
    State(auth): State<AuthState>,
    Form(params): Form<RevokeParams>,
) -> StatusCode {
    // Try to revoke the token
    match auth.store.revoke_token(&params.token).await {
        Ok(revoked) => {
            if revoked {
                debug!("Revoked token");
            } else {
                debug!("Token not found or already revoked");
            }
            // Per RFC 7009, always return 200 OK
            StatusCode::OK
        }
        Err(e) => {
            warn!("Token revocation error: {}", e);
            // Per RFC 7009, return 200 OK even on errors for the token holder
            StatusCode::OK
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_issuer_simple() {
        let headers = HeaderMap::new();
        let issuer = get_issuer(&headers, "localhost:3000");
        assert_eq!(issuer, "http://localhost:3000");
    }

    #[test]
    fn test_get_issuer_with_proxy() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-proto", "https".parse().unwrap());
        headers.insert("x-forwarded-host", "api.example.com".parse().unwrap());

        let issuer = get_issuer(&headers, "localhost:3000");
        assert_eq!(issuer, "https://api.example.com");
    }
}
