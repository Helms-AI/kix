# API Key Authentication for KIX MCP Server

## Overview

Add simple API key authentication as an alternative to OAuth for local development and single-user deployments. When configured, the server validates a static API key from request headers instead of requiring the full OAuth flow.

## Problem Statement

The current OAuth implementation:
- Requires client registration, authorization codes, PKCE verification
- Can fail after server restarts due to token expiration or state mismatches
- Is overkill for local development where the user trusts all connections

API key authentication provides a simpler alternative that:
- Works reliably across server restarts
- Requires minimal client configuration
- Is easy to set up for local development

## Design Goals

1. **Simple**: Single environment variable to enable
2. **Secure**: Keys are never logged, compared in constant time
3. **Flexible**: Works alongside OAuth (not a replacement)
4. **Compatible**: Works with Claude Code, Claude Desktop, and other MCP clients

## Authentication Modes

The server will support three authentication modes:

| Mode | Trigger | Behavior |
|------|---------|----------|
| **None** | No `KIX_API_KEY` env var, `require_auth=false` | All requests allowed |
| **API Key** | `KIX_API_KEY` env var is set | Validate header, reject if invalid |
| **OAuth** | `require_auth=true`, no API key | Full OAuth flow required |

Priority: API Key > OAuth > None

## Implementation Spec

### 1. Environment Variable

```bash
# Enable API key authentication
export KIX_API_KEY="your-secret-key-here"

# Start server - API key auth is now active
./run.sh
```

### 2. Accepted Headers

The server should accept the API key in either format:

```
Authorization: Bearer <api-key>
X-API-Key: <api-key>
```

### 3. New Middleware: `api_key_middleware`

**Location:** `kix-auth/src/middleware.rs`

```rust
/// API key authentication middleware.
///
/// If `KIX_API_KEY` env var is set, validates the request header.
/// If not set, passes through to next middleware (OAuth or none).
pub async fn api_key_middleware(
    State(auth): State<AuthState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    // 1. Check if API key auth is configured
    let expected_key = match &auth.config.api_key {
        Some(key) => key,
        None => return next.run(request).await, // No API key configured, skip
    };

    // 2. Extract key from headers (Bearer or X-API-Key)
    let provided_key = extract_api_key(&request);

    // 3. Validate (constant-time comparison)
    match provided_key {
        Some(key) if constant_time_eq(key, expected_key) => {
            next.run(request).await
        }
        Some(_) => {
            // Invalid key
            unauthorized_response("Invalid API key")
        }
        None => {
            // No key provided
            unauthorized_response("API key required")
        }
    }
}

fn extract_api_key(request: &Request<Body>) -> Option<&str> {
    // Try Authorization: Bearer <key>
    if let Some(auth) = request.headers().get(AUTHORIZATION) {
        if let Ok(value) = auth.to_str() {
            if value.starts_with("Bearer ") {
                return Some(&value[7..]);
            }
        }
    }

    // Try X-API-Key: <key>
    if let Some(key) = request.headers().get("X-API-Key") {
        return key.to_str().ok();
    }

    None
}
```

### 4. Config Changes

**Location:** `kix-auth/src/middleware.rs`

```rust
#[derive(Clone, Debug)]
pub struct AuthConfig {
    // ... existing fields ...

    /// Static API key for simple authentication.
    /// If set, API key auth takes precedence over OAuth.
    /// Read from KIX_API_KEY environment variable.
    pub api_key: Option<String>,
}

impl AuthConfig {
    pub fn from_env() -> Self {
        Self {
            api_key: std::env::var("KIX_API_KEY").ok(),
            require_auth: std::env::var("KIX_REQUIRE_AUTH")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(false),
            ..Default::default()
        }
    }
}
```

### 5. Router Changes

**Location:** `kix-cli/src/main.rs`

```rust
// Create auth config from environment
let auth_config = AuthConfig::from_env();

// Log auth mode
match (&auth_config.api_key, auth_config.require_auth) {
    (Some(_), _) => info!("API key authentication enabled"),
    (None, true) => info!("OAuth authentication required"),
    (None, false) => info!("Authentication disabled (open access)"),
}

let auth_state = AuthState::with_config(Arc::new(auth_store), auth_config);

// Apply API key middleware BEFORE OAuth endpoints
let mcp_app = axum::Router::new()
    .nest_service("/mcp", mcp_service)
    .layer(axum::middleware::from_fn_with_state(
        auth_state.clone(),
        api_key_middleware,
    ))
    // OAuth endpoints (only used if API key not configured)
    .route("/.well-known/oauth-authorization-server", get(oauth_metadata))
    // ... other OAuth routes ...
    .with_state(auth_state);
```

### 6. Security Considerations

1. **Constant-time comparison**: Use `subtle::ConstantTimeEq` or similar to prevent timing attacks
2. **No logging of keys**: Never log the API key value, only "API key validated" or "API key invalid"
3. **HTTPS recommended**: Warn if API key auth is used over HTTP in production
4. **Key generation**: Provide a helper command to generate secure keys

```bash
# Generate a secure API key
kix generate-api-key
# Output: kix_ak_7f3d8a2b1c9e4f5a6b7c8d9e0f1a2b3c
```

### 7. Client Configuration

**Claude Code:**
```bash
claude mcp add --transport http kix http://localhost:3002/mcp \
  --header "Authorization: Bearer ${KIX_API_KEY}"
```

**Claude Desktop (`claude_desktop_config.json`):**
```json
{
  "mcpServers": {
    "kix": {
      "command": "npx",
      "args": [
        "mcp-remote@latest",
        "http://localhost:3002/mcp",
        "--header",
        "Authorization: Bearer ${KIX_API_KEY}"
      ],
      "env": {
        "KIX_API_KEY": "your-api-key-here"
      }
    }
  }
}
```

**Environment file (`.env`):**
```bash
KIX_API_KEY=kix_ak_7f3d8a2b1c9e4f5a6b7c8d9e0f1a2b3c
```

## Files to Modify

| File | Changes |
|------|---------|
| `kix-auth/src/middleware.rs` | Add `api_key` to `AuthConfig`, add `api_key_middleware` |
| `kix-auth/src/lib.rs` | Export `api_key_middleware` |
| `kix-auth/Cargo.toml` | Add `subtle` crate for constant-time comparison |
| `kix-cli/src/main.rs` | Use `AuthConfig::from_env()`, apply API key middleware |

## New Files

| File | Purpose |
|------|---------|
| `kix-auth/src/api_key.rs` | API key extraction and validation utilities |

## Testing

### Unit Tests

```rust
#[test]
fn test_extract_api_key_bearer() {
    let mut headers = HeaderMap::new();
    headers.insert(AUTHORIZATION, "Bearer test-key".parse().unwrap());
    assert_eq!(extract_api_key_from_headers(&headers), Some("test-key"));
}

#[test]
fn test_extract_api_key_x_header() {
    let mut headers = HeaderMap::new();
    headers.insert("X-API-Key", "test-key".parse().unwrap());
    assert_eq!(extract_api_key_from_headers(&headers), Some("test-key"));
}

#[test]
fn test_api_key_validation() {
    let config = AuthConfig {
        api_key: Some("secret-key".to_string()),
        ..Default::default()
    };
    assert!(validate_api_key(&config, "secret-key"));
    assert!(!validate_api_key(&config, "wrong-key"));
}
```

### Integration Test

```bash
# Start server with API key
KIX_API_KEY=test-key ./run.sh &

# Test with valid key - should succeed
curl -H "Authorization: Bearer test-key" http://localhost:3002/mcp/health
# Expected: 200 OK

# Test with invalid key - should fail
curl -H "Authorization: Bearer wrong-key" http://localhost:3002/mcp/health
# Expected: 401 Unauthorized

# Test without key - should fail
curl http://localhost:3002/mcp/health
# Expected: 401 Unauthorized

# Test without API key configured (restart server without env var)
./run.sh &
curl http://localhost:3002/mcp/health
# Expected: 200 OK (open access)
```

## Migration Path

1. **No breaking changes**: Existing OAuth flow continues to work
2. **Opt-in**: API key auth only activates when `KIX_API_KEY` is set
3. **Gradual adoption**: Users can switch from OAuth to API key at their own pace

## Future Enhancements

1. **Multiple API keys**: Support comma-separated keys or key file
2. **Key rotation**: Support for primary/secondary keys during rotation
3. **Rate limiting per key**: Track and limit requests per API key
4. **Key scopes**: Restrict certain keys to read-only operations

## Implementation Checklist

### Phase 1: Dependencies & Setup
- [ ] Add `subtle` crate to `kix-auth/Cargo.toml` for constant-time comparison
- [ ] Create `kix-auth/src/api_key.rs` file

### Phase 2: API Key Module (`kix-auth/src/api_key.rs`)
- [ ] Implement `extract_api_key_from_headers()` function
  - [ ] Support `Authorization: Bearer <key>` format
  - [ ] Support `X-API-Key: <key>` format
- [ ] Implement `validate_api_key()` with constant-time comparison
- [ ] Add unit tests for header extraction
- [ ] Add unit tests for validation

### Phase 3: Config Changes (`kix-auth/src/middleware.rs`)
- [ ] Add `api_key: Option<String>` field to `AuthConfig`
- [ ] Update `AuthConfig::from_env()` to read `KIX_API_KEY` env var
- [ ] Add logging for auth mode detection

### Phase 4: Middleware (`kix-auth/src/middleware.rs`)
- [ ] Implement `api_key_middleware()` function
  - [ ] Check if API key is configured
  - [ ] Extract key from request headers
  - [ ] Validate with constant-time comparison
  - [ ] Return 401 with appropriate message on failure
  - [ ] Pass through to next middleware if no API key configured
- [ ] Export `api_key_middleware` from `kix-auth/src/lib.rs`

### Phase 5: Router Integration (`kix-cli/src/main.rs`)
- [ ] Update auth config initialization to use `AuthConfig::from_env()`
- [ ] Add auth mode logging (API Key / OAuth / None)
- [ ] Apply `api_key_middleware` layer to MCP router
- [ ] Ensure middleware order: API Key → OAuth → Handler

### Phase 6: Testing
- [ ] Unit test: Bearer token extraction
- [ ] Unit test: X-API-Key header extraction
- [ ] Unit test: Constant-time validation
- [ ] Integration test: Valid API key → 200 OK
- [ ] Integration test: Invalid API key → 401 Unauthorized
- [ ] Integration test: Missing API key (when required) → 401 Unauthorized
- [ ] Integration test: No API key configured → Open access (200 OK)

### Phase 7: Documentation
- [ ] Update README with API key configuration
- [ ] Add example `.env` file with `KIX_API_KEY`
- [ ] Document Claude Code configuration
- [ ] Document Claude Desktop configuration

### Phase 8: Optional Enhancements
- [ ] Add `kix generate-api-key` CLI command
- [ ] Add HTTPS warning for production use
- [ ] Consider rate limiting per API key (future)

## References

- [LiteLLM MCP Auth](https://docs.litellm.ai/docs/mcp) - API key and Bearer token support
- [mcp-front](https://github.com/stainless-api/mcp-front) - Simple Bearer token configuration
- [MCP Authorization Spec](https://modelcontextprotocol.io/specification/2025-06-18/basic/authorization)
