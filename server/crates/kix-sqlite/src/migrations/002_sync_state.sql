-- ===========================================================================
-- KIX SQLite Schema - Issue Sync State Migration
-- Adds bidirectional GitHub sync tracking and conflict resolution
-- ===========================================================================

-- Enable foreign key constraints
PRAGMA foreign_keys = ON;

-- ===========================================================================
-- Issue Sync State Table
-- Tracks synchronization state between local and GitHub issues
-- ===========================================================================

CREATE TABLE IF NOT EXISTS issue_sync_state (
    issue_id TEXT PRIMARY KEY REFERENCES issues(id) ON DELETE CASCADE,
    content_hash TEXT NOT NULL,          -- SHA-256 hash of canonical issue content
    last_sync_hash TEXT NOT NULL,        -- Content hash at last successful sync (merge base)
    local_version INTEGER NOT NULL DEFAULT 1,  -- Incremented on each local modification
    github_version INTEGER,              -- GitHub updated_at as Unix timestamp
    github_etag TEXT,                    -- GitHub ETag for efficient change detection
    sync_status TEXT NOT NULL DEFAULT 'pending',  -- 'synced', 'local_ahead', 'github_ahead', 'conflict', 'pending'
    last_sync_at TEXT NOT NULL,          -- RFC3339 timestamp of last sync
    last_sync_direction TEXT,            -- 'push', 'pull', 'merge', null for new
    last_sync_result TEXT,               -- JSON: {pushed_fields, pulled_fields, merged_fields}
    conflict_count INTEGER DEFAULT 0,    -- Number of times this issue had conflicts
    created_at TEXT NOT NULL,            -- RFC3339
    updated_at TEXT NOT NULL             -- RFC3339
);

-- Indexes for efficient sync operations
CREATE INDEX IF NOT EXISTS idx_sync_state_status ON issue_sync_state(sync_status);
CREATE INDEX IF NOT EXISTS idx_sync_state_updated ON issue_sync_state(updated_at);
CREATE INDEX IF NOT EXISTS idx_sync_state_sync_at ON issue_sync_state(last_sync_at);

-- ===========================================================================
-- Sync History Table
-- Audit log of all sync operations for debugging and rollback
-- ===========================================================================

CREATE TABLE IF NOT EXISTS sync_history (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    sync_type TEXT NOT NULL,             -- 'full', 'incremental', 'single_issue'
    direction TEXT NOT NULL,             -- 'pull', 'push', 'bidirectional'
    started_at TEXT NOT NULL,            -- RFC3339
    completed_at TEXT,                   -- RFC3339
    status TEXT NOT NULL,                -- 'running', 'success', 'partial', 'failed'
    stats TEXT,                          -- JSON: sync statistics
    issues_pulled INTEGER DEFAULT 0,
    issues_pushed INTEGER DEFAULT 0,
    issues_merged INTEGER DEFAULT 0,
    conflicts_resolved INTEGER DEFAULT 0,
    conflicts_failed INTEGER DEFAULT 0,
    errors TEXT                          -- JSON array of error messages
);

CREATE INDEX IF NOT EXISTS idx_sync_history_project ON sync_history(project_id);
CREATE INDEX IF NOT EXISTS idx_sync_history_status ON sync_history(status);
CREATE INDEX IF NOT EXISTS idx_sync_history_completed ON sync_history(completed_at DESC);

-- ===========================================================================
-- Sync Conflict Resolution Table
-- Stores unresolved conflicts for manual review (future feature)
-- ===========================================================================

CREATE TABLE IF NOT EXISTS sync_conflicts (
    id TEXT PRIMARY KEY,
    issue_id TEXT NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    detected_at TEXT NOT NULL,           -- RFC3339
    conflict_type TEXT NOT NULL,         -- 'content', 'state', 'deletion'
    local_snapshot TEXT NOT NULL,        -- JSON: local issue state
    github_snapshot TEXT NOT NULL,       -- JSON: GitHub issue state
    last_common_snapshot TEXT,           -- JSON: last known synchronized state
    affected_fields TEXT NOT NULL,       -- JSON array: ['title', 'body', etc.]
    auto_resolution_attempted INTEGER DEFAULT 0,
    auto_resolution_result TEXT,         -- JSON: attempted resolution details
    manual_resolution TEXT,               -- JSON: user's chosen resolution
    resolved_at TEXT,                    -- RFC3339
    resolved_by TEXT,                    -- User ID or 'auto'
    UNIQUE(issue_id)                    -- Only one active conflict per issue
);

CREATE INDEX IF NOT EXISTS idx_conflicts_project ON sync_conflicts(project_id);
CREATE INDEX IF NOT EXISTS idx_conflicts_resolved ON sync_conflicts(resolved_at);
CREATE INDEX IF NOT EXISTS idx_conflicts_issue ON sync_conflicts(issue_id);

-- ===========================================================================
-- Add sync tracking columns to issues table if not present
-- ===========================================================================

-- Add content_hash column for change detection
-- Note: In production, this would be done via proper migration
-- ALTER TABLE issues ADD COLUMN content_hash TEXT;
-- ALTER TABLE issues ADD COLUMN local_version INTEGER DEFAULT 1;

-- ===========================================================================
-- Triggers for automatic sync state management
-- ===========================================================================

-- Initialize sync state when new issue is created
CREATE TRIGGER IF NOT EXISTS issue_sync_state_init
AFTER INSERT ON issues
WHEN NEW.github_number IS NOT NULL  -- Only for GitHub-linked issues
BEGIN
    INSERT INTO issue_sync_state (
        issue_id,
        content_hash,
        last_sync_hash,
        local_version,
        sync_status,
        last_sync_at,
        created_at,
        updated_at
    ) VALUES (
        NEW.id,
        '', -- Will be calculated by application
        '', -- Will be calculated by application
        1,
        'pending',
        NEW.created_at,
        NEW.created_at,
        NEW.created_at
    );
END;

-- Update sync state version on issue modification
CREATE TRIGGER IF NOT EXISTS issue_sync_state_version_update
AFTER UPDATE ON issues
WHEN EXISTS (SELECT 1 FROM issue_sync_state WHERE issue_id = NEW.id)
    AND (OLD.title != NEW.title
        OR OLD.body != NEW.body
        OR OLD.state != NEW.state
        OR OLD.labels != NEW.labels
        OR OLD.assignees != NEW.assignees)
BEGIN
    UPDATE issue_sync_state
    SET local_version = local_version + 1,
        sync_status = CASE
            WHEN sync_status = 'synced' THEN 'local_ahead'
            WHEN sync_status = 'github_ahead' THEN 'conflict'
            ELSE sync_status
        END,
        updated_at = NEW.updated_at
    WHERE issue_id = NEW.id;
END;

-- Clean up sync state when issue is deleted
-- (CASCADE DELETE handles this, but adding for clarity)
CREATE TRIGGER IF NOT EXISTS issue_sync_state_cleanup
AFTER DELETE ON issues
BEGIN
    DELETE FROM issue_sync_state WHERE issue_id = OLD.id;
    DELETE FROM sync_conflicts WHERE issue_id = OLD.id;
END;