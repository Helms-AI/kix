-- ===========================================================================
-- KIX SQLite Schema - Initial Migration
-- Hybrid Architecture: SQLite for structured data, LanceDB for vectors
-- ===========================================================================

-- Enable foreign key constraints
PRAGMA foreign_keys = ON;

-- ===========================================================================
-- Core Knowledge Tables
-- ===========================================================================

-- Entries table (document metadata)
CREATE TABLE IF NOT EXISTS entries (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    description TEXT,
    content TEXT,
    tags TEXT,                    -- JSON array
    collection_ids TEXT,          -- JSON array
    entry_type TEXT NOT NULL,
    source_type TEXT NOT NULL,
    source_path TEXT NOT NULL,
    source_domain TEXT,
    slug TEXT NOT NULL,
    source_hash TEXT NOT NULL,
    created_at TEXT NOT NULL,     -- RFC3339
    updated_at TEXT NOT NULL      -- RFC3339
);

CREATE INDEX IF NOT EXISTS idx_entries_slug ON entries(slug);
CREATE INDEX IF NOT EXISTS idx_entries_entry_type ON entries(entry_type);
CREATE INDEX IF NOT EXISTS idx_entries_source_domain ON entries(source_domain);
CREATE INDEX IF NOT EXISTS idx_entries_source_hash ON entries(source_hash);
CREATE INDEX IF NOT EXISTS idx_entries_created_at ON entries(created_at);

-- Pages table (full content for RAG context)
CREATE TABLE IF NOT EXISTS pages (
    page_id TEXT PRIMARY KEY,
    source_id TEXT NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
    url TEXT NOT NULL,
    title TEXT,
    full_content TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    content_length INTEGER NOT NULL,
    code_block_count INTEGER NOT NULL DEFAULT 0,
    metadata TEXT,                -- JSON
    crawl_time_ms INTEGER,
    created_at TEXT NOT NULL      -- RFC3339
);

CREATE INDEX IF NOT EXISTS idx_pages_source_id ON pages(source_id);
CREATE INDEX IF NOT EXISTS idx_pages_content_hash ON pages(content_hash);
CREATE INDEX IF NOT EXISTS idx_pages_url ON pages(url);

-- ===========================================================================
-- Project Management Tables
-- ===========================================================================

-- Projects table
CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    slug TEXT NOT NULL UNIQUE,
    description TEXT,
    color TEXT,
    github_config TEXT NOT NULL,  -- JSON (GitHubConfig)
    archived INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_projects_slug ON projects(slug);
CREATE INDEX IF NOT EXISTS idx_projects_archived ON projects(archived);

-- Issues table (metadata only, vectors stored in LanceDB)
CREATE TABLE IF NOT EXISTS issues (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    number INTEGER NOT NULL,
    title TEXT NOT NULL,
    body TEXT,
    state TEXT NOT NULL DEFAULT 'open',
    labels TEXT,                  -- JSON array
    assignees TEXT,               -- JSON array
    priority INTEGER,
    github_number INTEGER,
    github_node_id TEXT,
    github_url TEXT,
    github_project_item_id TEXT,
    source TEXT NOT NULL DEFAULT 'local',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    closed_at TEXT,
    synced_at TEXT,
    UNIQUE(project_id, number)
);

CREATE INDEX IF NOT EXISTS idx_issues_project_id ON issues(project_id);
CREATE INDEX IF NOT EXISTS idx_issues_state ON issues(state);
CREATE INDEX IF NOT EXISTS idx_issues_github_number ON issues(github_number);
CREATE INDEX IF NOT EXISTS idx_issues_number ON issues(project_id, number);

-- Project entries (knowledge links)
CREATE TABLE IF NOT EXISTS project_entries (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    entry_id TEXT NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
    notes TEXT,
    relevance REAL,
    linked_at TEXT NOT NULL,
    UNIQUE(project_id, entry_id)
);

CREATE INDEX IF NOT EXISTS idx_project_entries_project ON project_entries(project_id);
CREATE INDEX IF NOT EXISTS idx_project_entries_entry ON project_entries(entry_id);

-- ===========================================================================
-- Token Storage (Encrypted)
-- ===========================================================================

CREATE TABLE IF NOT EXISTS github_tokens (
    scope TEXT PRIMARY KEY,       -- 'global' or 'project:{uuid}'
    encrypted_token TEXT NOT NULL,-- JSON: {ciphertext, nonce}
    token_suffix TEXT,
    token_format TEXT,
    verified_at TEXT,
    verified_user TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- ===========================================================================
-- Job History Tables
-- ===========================================================================

CREATE TABLE IF NOT EXISTS jobs (
    job_id TEXT PRIMARY KEY,
    job_type TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT NOT NULL,
    source_url TEXT,
    source_domain TEXT,
    config TEXT NOT NULL,         -- JSON
    items_processed INTEGER NOT NULL DEFAULT 0,
    items_discovered INTEGER NOT NULL DEFAULT 0,
    chunks_created INTEGER NOT NULL DEFAULT 0,
    embeddings_generated INTEGER NOT NULL DEFAULT 0,
    error_count INTEGER NOT NULL DEFAULT 0,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    processing_rate REAL NOT NULL DEFAULT 0.0,
    errors TEXT                   -- JSON array
);

CREATE INDEX IF NOT EXISTS idx_jobs_status ON jobs(status);
CREATE INDEX IF NOT EXISTS idx_jobs_job_type ON jobs(job_type);
CREATE INDEX IF NOT EXISTS idx_jobs_completed_at ON jobs(completed_at);
CREATE INDEX IF NOT EXISTS idx_jobs_source_domain ON jobs(source_domain);

CREATE TABLE IF NOT EXISTS job_items (
    item_id TEXT PRIMARY KEY,
    job_id TEXT NOT NULL REFERENCES jobs(job_id) ON DELETE CASCADE,
    item_path TEXT NOT NULL,
    item_type TEXT NOT NULL,
    status TEXT NOT NULL,
    parent_url TEXT,
    depth INTEGER NOT NULL DEFAULT 0,
    discovered_at TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT,
    chunks_created INTEGER NOT NULL DEFAULT 0,
    embeddings_generated INTEGER NOT NULL DEFAULT 0,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    error_message TEXT
);

CREATE INDEX IF NOT EXISTS idx_job_items_job_id ON job_items(job_id);
CREATE INDEX IF NOT EXISTS idx_job_items_status ON job_items(status);

-- ===========================================================================
-- Full-Text Search (FTS5) Virtual Tables
-- ===========================================================================

-- FTS for entries (title, description, content)
CREATE VIRTUAL TABLE IF NOT EXISTS entries_fts USING fts5(
    id UNINDEXED,
    title,
    description,
    content,
    content='entries',
    content_rowid='rowid'
);

-- FTS for pages (title, full_content)
CREATE VIRTUAL TABLE IF NOT EXISTS pages_fts USING fts5(
    page_id UNINDEXED,
    title,
    full_content,
    content='pages',
    content_rowid='rowid'
);

-- FTS for issues (title, body)
CREATE VIRTUAL TABLE IF NOT EXISTS issues_fts USING fts5(
    id UNINDEXED,
    title,
    body,
    content='issues',
    content_rowid='rowid'
);

-- ===========================================================================
-- FTS Sync Triggers
-- ===========================================================================

-- Entries FTS triggers
CREATE TRIGGER IF NOT EXISTS entries_ai AFTER INSERT ON entries BEGIN
    INSERT INTO entries_fts(rowid, id, title, description, content)
    VALUES (NEW.rowid, NEW.id, NEW.title, NEW.description, NEW.content);
END;

CREATE TRIGGER IF NOT EXISTS entries_ad AFTER DELETE ON entries BEGIN
    INSERT INTO entries_fts(entries_fts, rowid, id, title, description, content)
    VALUES ('delete', OLD.rowid, OLD.id, OLD.title, OLD.description, OLD.content);
END;

CREATE TRIGGER IF NOT EXISTS entries_au AFTER UPDATE ON entries BEGIN
    INSERT INTO entries_fts(entries_fts, rowid, id, title, description, content)
    VALUES ('delete', OLD.rowid, OLD.id, OLD.title, OLD.description, OLD.content);
    INSERT INTO entries_fts(rowid, id, title, description, content)
    VALUES (NEW.rowid, NEW.id, NEW.title, NEW.description, NEW.content);
END;

-- Pages FTS triggers
CREATE TRIGGER IF NOT EXISTS pages_ai AFTER INSERT ON pages BEGIN
    INSERT INTO pages_fts(rowid, page_id, title, full_content)
    VALUES (NEW.rowid, NEW.page_id, NEW.title, NEW.full_content);
END;

CREATE TRIGGER IF NOT EXISTS pages_ad AFTER DELETE ON pages BEGIN
    INSERT INTO pages_fts(pages_fts, rowid, page_id, title, full_content)
    VALUES ('delete', OLD.rowid, OLD.page_id, OLD.title, OLD.full_content);
END;

CREATE TRIGGER IF NOT EXISTS pages_au AFTER UPDATE ON pages BEGIN
    INSERT INTO pages_fts(pages_fts, rowid, page_id, title, full_content)
    VALUES ('delete', OLD.rowid, OLD.page_id, OLD.title, OLD.full_content);
    INSERT INTO pages_fts(rowid, page_id, title, full_content)
    VALUES (NEW.rowid, NEW.page_id, NEW.title, NEW.full_content);
END;

-- Issues FTS triggers
CREATE TRIGGER IF NOT EXISTS issues_ai AFTER INSERT ON issues BEGIN
    INSERT INTO issues_fts(rowid, id, title, body)
    VALUES (NEW.rowid, NEW.id, NEW.title, NEW.body);
END;

CREATE TRIGGER IF NOT EXISTS issues_ad AFTER DELETE ON issues BEGIN
    INSERT INTO issues_fts(issues_fts, rowid, id, title, body)
    VALUES ('delete', OLD.rowid, OLD.id, OLD.title, OLD.body);
END;

CREATE TRIGGER IF NOT EXISTS issues_au AFTER UPDATE ON issues BEGIN
    INSERT INTO issues_fts(issues_fts, rowid, id, title, body)
    VALUES ('delete', OLD.rowid, OLD.id, OLD.title, OLD.body);
    INSERT INTO issues_fts(rowid, id, title, body)
    VALUES (NEW.rowid, NEW.id, NEW.title, NEW.body);
END;
