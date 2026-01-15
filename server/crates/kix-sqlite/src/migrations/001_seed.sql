-- ===========================================================================
-- KIX SQLite Schema - Clean Seed Migration
-- No GitHub integration, work_items instead of issues
-- ===========================================================================

PRAGMA foreign_keys = ON;

-- ===========================================================================
-- Core Knowledge Tables
-- ===========================================================================

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
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_entries_slug ON entries(slug);
CREATE INDEX IF NOT EXISTS idx_entries_entry_type ON entries(entry_type);
CREATE INDEX IF NOT EXISTS idx_entries_source_domain ON entries(source_domain);
CREATE INDEX IF NOT EXISTS idx_entries_source_hash ON entries(source_hash);
CREATE INDEX IF NOT EXISTS idx_entries_created_at ON entries(created_at);

CREATE TABLE IF NOT EXISTS pages (
    page_id TEXT PRIMARY KEY,
    source_id TEXT NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
    url TEXT NOT NULL,
    title TEXT,
    full_content TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    content_length INTEGER NOT NULL,
    code_block_count INTEGER NOT NULL DEFAULT 0,
    metadata TEXT,
    crawl_time_ms INTEGER,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_pages_source_id ON pages(source_id);
CREATE INDEX IF NOT EXISTS idx_pages_content_hash ON pages(content_hash);
CREATE INDEX IF NOT EXISTS idx_pages_url ON pages(url);

-- ===========================================================================
-- Project Management Tables (No GitHub)
-- ===========================================================================

CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    slug TEXT NOT NULL UNIQUE,
    description TEXT,
    color TEXT,
    archived INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_projects_slug ON projects(slug);
CREATE INDEX IF NOT EXISTS idx_projects_archived ON projects(archived);

-- Work Items table (formerly "issues")
-- Supports hierarchy: epic -> story/bug -> task -> subtask
CREATE TABLE IF NOT EXISTS work_items (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    number INTEGER NOT NULL,
    title TEXT NOT NULL,
    body TEXT,
    state TEXT NOT NULL DEFAULT 'open',
    labels TEXT,                  -- JSON array
    assignees TEXT,               -- JSON array
    priority INTEGER,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    closed_at TEXT,
    -- Hierarchy fields
    item_type TEXT NOT NULL DEFAULT 'task',  -- epic, story, task, subtask, bug
    parent_id TEXT REFERENCES work_items(id) ON DELETE SET NULL,
    -- Board fields
    position INTEGER NOT NULL DEFAULT 0,
    board_column TEXT NOT NULL DEFAULT 'backlog',
    story_points INTEGER,
    epic_color TEXT,              -- Hex color for epic identification
    UNIQUE(project_id, number)
);

CREATE INDEX IF NOT EXISTS idx_work_items_project_id ON work_items(project_id);
CREATE INDEX IF NOT EXISTS idx_work_items_state ON work_items(state);
CREATE INDEX IF NOT EXISTS idx_work_items_number ON work_items(project_id, number);
CREATE INDEX IF NOT EXISTS idx_work_items_item_type ON work_items(item_type);
CREATE INDEX IF NOT EXISTS idx_work_items_parent_id ON work_items(parent_id);
CREATE INDEX IF NOT EXISTS idx_work_items_board_column ON work_items(board_column);
CREATE INDEX IF NOT EXISTS idx_work_items_position ON work_items(project_id, board_column, position);

-- Project knowledge links
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

-- Board configuration (per-project settings)
CREATE TABLE IF NOT EXISTS board_config (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL UNIQUE REFERENCES projects(id) ON DELETE CASCADE,
    columns TEXT NOT NULL DEFAULT '["backlog","todo","in_progress","in_review","testing","done"]',
    swimlane_field TEXT NOT NULL DEFAULT 'item_type',
    wip_limits TEXT,
    board_default INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_board_config_project ON board_config(project_id);

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
    config TEXT NOT NULL,
    items_processed INTEGER NOT NULL DEFAULT 0,
    items_discovered INTEGER NOT NULL DEFAULT 0,
    chunks_created INTEGER NOT NULL DEFAULT 0,
    embeddings_generated INTEGER NOT NULL DEFAULT 0,
    error_count INTEGER NOT NULL DEFAULT 0,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    processing_rate REAL NOT NULL DEFAULT 0.0,
    errors TEXT,
    code_extraction_stats TEXT    -- JSON
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
