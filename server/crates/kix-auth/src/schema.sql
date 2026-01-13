-- KIX Auth SQLite Schema
-- OAuth 2.1 client, token, and session persistence

-- OAuth Clients (RFC 7591 Dynamic Client Registration)
CREATE TABLE IF NOT EXISTS oauth_clients (
    client_id TEXT PRIMARY KEY NOT NULL,
    client_name TEXT NOT NULL,
    redirect_uris TEXT NOT NULL,  -- JSON array of allowed redirect URIs
    grant_types TEXT NOT NULL,    -- JSON array of allowed grant types
    response_types TEXT NOT NULL, -- JSON array of allowed response types
    token_endpoint_auth_method TEXT NOT NULL DEFAULT 'none',
    created_at TEXT NOT NULL,     -- ISO 8601 timestamp
    updated_at TEXT NOT NULL      -- ISO 8601 timestamp
);

-- Index for listing clients by creation date
CREATE INDEX IF NOT EXISTS idx_clients_created ON oauth_clients(created_at);

-- OAuth Tokens (Access and Refresh tokens)
-- Note: Raw token values are NEVER stored; only SHA-256 hashes
CREATE TABLE IF NOT EXISTS oauth_tokens (
    token_id TEXT PRIMARY KEY NOT NULL,      -- UUID for internal reference
    token_hash TEXT NOT NULL UNIQUE,         -- SHA-256 hash of actual token
    token_type TEXT NOT NULL,                -- 'access' or 'refresh'
    client_id TEXT NOT NULL,                 -- FK to oauth_clients
    session_id TEXT,                         -- FK to oauth_sessions (optional)
    scope TEXT,                              -- Space-separated scopes
    issued_at TEXT NOT NULL,                 -- ISO 8601 timestamp
    expires_at TEXT NOT NULL,                -- ISO 8601 timestamp
    revoked_at TEXT,                         -- ISO 8601 timestamp (NULL if valid)
    FOREIGN KEY (client_id) REFERENCES oauth_clients(client_id) ON DELETE CASCADE,
    FOREIGN KEY (session_id) REFERENCES oauth_sessions(session_id) ON DELETE SET NULL
);

-- Index for fast token lookup by hash
CREATE INDEX IF NOT EXISTS idx_tokens_hash ON oauth_tokens(token_hash);
-- Index for finding expired tokens (cleanup)
CREATE INDEX IF NOT EXISTS idx_tokens_expires ON oauth_tokens(expires_at);
-- Index for finding tokens by client (revocation)
CREATE INDEX IF NOT EXISTS idx_tokens_client ON oauth_tokens(client_id);
-- Index for finding tokens by session (revocation)
CREATE INDEX IF NOT EXISTS idx_tokens_session ON oauth_tokens(session_id);

-- OAuth Sessions (for token binding and revocation)
CREATE TABLE IF NOT EXISTS oauth_sessions (
    session_id TEXT PRIMARY KEY NOT NULL,   -- UUID
    client_id TEXT NOT NULL,                -- FK to oauth_clients
    created_at TEXT NOT NULL,               -- ISO 8601 timestamp
    last_activity TEXT NOT NULL,            -- ISO 8601 timestamp
    expires_at TEXT NOT NULL,               -- ISO 8601 timestamp
    metadata TEXT,                          -- JSON for additional session data
    FOREIGN KEY (client_id) REFERENCES oauth_clients(client_id) ON DELETE CASCADE
);

-- Index for finding expired sessions (cleanup)
CREATE INDEX IF NOT EXISTS idx_sessions_expires ON oauth_sessions(expires_at);
-- Index for finding sessions by client
CREATE INDEX IF NOT EXISTS idx_sessions_client ON oauth_sessions(client_id);

-- Authorization Codes (for auth code flow)
-- Note: Raw code values are NEVER stored; only SHA-256 hashes
CREATE TABLE IF NOT EXISTS oauth_codes (
    code_hash TEXT PRIMARY KEY NOT NULL,    -- SHA-256 hash of actual code
    client_id TEXT NOT NULL,                -- FK to oauth_clients
    redirect_uri TEXT NOT NULL,             -- Must match on token exchange
    scope TEXT,                             -- Space-separated scopes
    code_challenge TEXT,                    -- PKCE S256 challenge
    code_challenge_method TEXT,             -- Must be 'S256' if present
    created_at TEXT NOT NULL,               -- ISO 8601 timestamp
    expires_at TEXT NOT NULL,               -- ISO 8601 timestamp
    used_at TEXT,                           -- ISO 8601 timestamp (NULL if unused)
    FOREIGN KEY (client_id) REFERENCES oauth_clients(client_id) ON DELETE CASCADE
);

-- Index for finding expired codes (cleanup)
CREATE INDEX IF NOT EXISTS idx_codes_expires ON oauth_codes(expires_at);
