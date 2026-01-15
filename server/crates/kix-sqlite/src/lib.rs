//! KIX SQLite Store
//!
//! SQLite storage for structured data in the hybrid KIX architecture.
//! Vector embeddings are stored separately; all other data is stored here.
//!
//! ## Tables
//!
//! - `entries` - Document metadata
//! - `pages` - Full page content for RAG context
//! - `projects` - Project management
//! - `work_items` - Work item tracking (epics, stories, tasks, etc.)
//! - `project_entries` - Knowledge links
//! - `board_config` - Kanban board configuration
//! - `jobs` - Job history
//! - `job_items` - Per-item job details
//!
//! ## Architecture
//!
//! This crate uses a hybrid approach:
//! - **SeaORM** for type-safe CRUD operations
//! - **Tantivy** for full-text search (via kix-search crate)
//! - **sqlite-vec** for vector search (via kix-vectors crate)

pub mod entries;
pub mod entities;
pub mod error;
pub mod jobs;
pub mod links;
pub mod pages;
pub mod pool;
pub mod projects;
pub mod work_items;

// Note: Full-text search is now provided by kix-search crate (Tantivy).
// The deprecated search module has been removed.

pub use error::{Result, SqliteError};
pub use pool::{create_pool, create_sea_orm_connection, run_migrations, DbInfo};

// Re-export record types
pub use entries::EntryRecord;
pub use jobs::{JobItemRecord, JobRecord};
pub use links::ProjectEntryRecord;
pub use pages::PageRecord;
pub use projects::ProjectRecord;
pub use work_items::WorkItemRecord;

use sea_orm::DatabaseConnection;
use sqlx::SqlitePool;
use std::path::Path;
use tracing::info;

/// SQLite store for structured data.
///
/// This is the main interface for SQLite operations in KIX.
/// It manages the connection pool and provides access to all tables.
///
/// ## Architecture
///
/// The store uses sqlx for SQL queries and SeaORM for type-safe CRUD:
/// - `pool` - sqlx pool for raw SQL queries
/// - `db` - SeaORM connection for ORM operations
///
/// ## Full-Text Search
///
/// Full-text search is provided by Tantivy via the `kix-search` crate.
/// See `kix_search::SearchEngine` for search operations.
#[derive(Clone)]
pub struct SqliteStore {
    pool: SqlitePool,
    db: DatabaseConnection,
}

impl SqliteStore {
    /// Create a new SQLite store at the given path.
    ///
    /// Creates the database file and parent directories if they don't exist.
    /// Runs all migrations to ensure the schema is up to date.
    pub async fn new(db_path: &Path) -> Result<Self> {
        let pool = pool::create_pool(db_path).await?;
        pool::run_migrations(&pool).await?;
        let db = pool::create_sea_orm_connection(db_path).await?;
        info!("SQLite store initialized at: {}", db_path.display());
        Ok(Self { pool, db })
    }

    /// Create a store from existing connections (for testing or sharing).
    pub fn from_connections(pool: SqlitePool, db: DatabaseConnection) -> Self {
        Self { pool, db }
    }

    /// Create a store from an existing pool (for testing or sharing pools).
    /// Note: This creates a new SeaORM connection using the pool's URL.
    #[deprecated(note = "Use from_connections() instead for full control")]
    pub fn from_pool(pool: SqlitePool) -> Self {
        // This is a legacy method - we can't easily extract the path from SqlitePool
        // For now, create without SeaORM (will panic if SeaORM operations are used)
        Self {
            pool,
            db: DatabaseConnection::Disconnected,
        }
    }

    /// Get a reference to the sqlx connection pool.
    /// Used for FTS5 queries and raw SQL operations.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Get a reference to the SeaORM database connection.
    /// Used for type-safe ORM operations.
    pub fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    /// Get database statistics.
    pub async fn info(&self) -> Result<DbInfo> {
        pool::get_db_info(&self.pool).await
    }

    /// Close both connection pools.
    pub async fn close(&self) {
        self.pool.close().await;
        self.db.clone().close().await.ok();
    }

    // =========================================================================
    // Entry Operations
    // =========================================================================

    /// Insert a new entry.
    pub async fn insert_entry(&self, entry: &EntryRecord) -> Result<()> {
        entries::insert_entry(&self.pool, entry).await
    }

    /// Insert multiple entries.
    pub async fn insert_entries(&self, entries: &[EntryRecord]) -> Result<()> {
        for entry in entries {
            self.insert_entry(entry).await?;
        }
        Ok(())
    }

    /// Get an entry by ID.
    pub async fn get_entry(&self, id: &str) -> Result<Option<EntryRecord>> {
        entries::get_entry(&self.pool, id).await
    }

    /// Get an entry by slug.
    pub async fn get_entry_by_slug(&self, slug: &str) -> Result<Option<EntryRecord>> {
        entries::get_entry_by_slug(&self.pool, slug).await
    }

    /// Check if an entry exists by source hash.
    pub async fn entry_exists_by_hash(&self, source_hash: &str) -> Result<bool> {
        entries::exists_by_hash(&self.pool, source_hash).await
    }

    /// Delete an entry by ID.
    pub async fn delete_entry(&self, id: &str) -> Result<bool> {
        entries::delete_entry(&self.pool, id).await
    }

    /// List all entries with optional filters.
    pub async fn list_entries(
        &self,
        entry_type: Option<&str>,
        source_domain: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<EntryRecord>> {
        entries::list_entries(&self.pool, entry_type, source_domain, limit, offset).await
    }

    /// Count entries.
    pub async fn entry_count(&self) -> Result<usize> {
        entries::count(&self.pool).await
    }

    // =========================================================================
    // Page Operations
    // =========================================================================

    /// Insert a new page.
    pub async fn insert_page(&self, page: &PageRecord) -> Result<()> {
        pages::insert_page(&self.pool, page).await
    }

    /// Get a page by ID.
    pub async fn get_page(&self, page_id: &str) -> Result<Option<PageRecord>> {
        pages::get_page(&self.pool, page_id).await
    }

    /// Get all pages for a source/entry.
    pub async fn get_pages_by_source(&self, source_id: &str) -> Result<Vec<PageRecord>> {
        pages::get_pages_by_source(&self.pool, source_id).await
    }

    /// Check if a page exists by content hash.
    pub async fn page_exists_by_hash(&self, content_hash: &str) -> Result<bool> {
        pages::exists_by_hash(&self.pool, content_hash).await
    }

    /// Delete a page by ID.
    pub async fn delete_page(&self, page_id: &str) -> Result<bool> {
        pages::delete_page(&self.pool, page_id).await
    }

    /// Delete all pages for a source.
    pub async fn delete_pages_by_source(&self, source_id: &str) -> Result<usize> {
        pages::delete_by_source(&self.pool, source_id).await
    }

    /// Count pages.
    pub async fn page_count(&self) -> Result<usize> {
        pages::count(&self.pool).await
    }

    // =========================================================================
    // Project Operations
    // =========================================================================

    /// Insert a new project.
    pub async fn insert_project(&self, project: &ProjectRecord) -> Result<()> {
        projects::insert_project(&self.pool, project).await
    }

    /// Get a project by ID.
    pub async fn get_project(&self, id: &str) -> Result<Option<ProjectRecord>> {
        projects::get_project(&self.pool, id).await
    }

    /// Get a project by slug.
    pub async fn get_project_by_slug(&self, slug: &str) -> Result<Option<ProjectRecord>> {
        projects::get_project_by_slug(&self.pool, slug).await
    }

    /// Update a project.
    pub async fn update_project(&self, project: &ProjectRecord) -> Result<bool> {
        projects::update_project(&self.pool, project).await
    }

    /// Delete a project by ID (cascades to issues and links).
    pub async fn delete_project(&self, id: &str) -> Result<bool> {
        projects::delete_project(&self.pool, id).await
    }

    /// List all projects.
    pub async fn list_projects(&self, include_archived: bool) -> Result<Vec<ProjectRecord>> {
        projects::list_projects(&self.pool, include_archived).await
    }

    // =========================================================================
    // Work Item Operations
    // =========================================================================

    /// Insert a new work item.
    pub async fn insert_work_item(&self, item: &WorkItemRecord) -> Result<()> {
        work_items::insert_work_item(&self.pool, item).await
    }

    /// Get a work item by ID.
    pub async fn get_work_item(&self, id: &str) -> Result<Option<WorkItemRecord>> {
        work_items::get_work_item(&self.pool, id).await
    }

    /// Get a work item by project and number.
    pub async fn get_work_item_by_number(
        &self,
        project_id: &str,
        number: u32,
    ) -> Result<Option<WorkItemRecord>> {
        work_items::get_work_item_by_number(&self.pool, project_id, number).await
    }

    /// Update a work item.
    pub async fn update_work_item(&self, item: &WorkItemRecord) -> Result<bool> {
        work_items::update_work_item(&self.pool, item).await
    }

    /// Delete a work item by ID.
    pub async fn delete_work_item(&self, id: &str) -> Result<bool> {
        work_items::delete_work_item(&self.pool, id).await
    }

    /// List work items for a project.
    pub async fn list_work_items(
        &self,
        project_id: &str,
        state: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<WorkItemRecord>> {
        work_items::list_work_items(&self.pool, project_id, state, limit, offset).await
    }

    /// Get the next work item number for a project.
    pub async fn next_work_item_number(&self, project_id: &str) -> Result<u32> {
        work_items::next_work_item_number(&self.pool, project_id).await
    }

    // =========================================================================
    // Board Operations
    // =========================================================================

    /// List work items for board view (grouped by column and sorted by position).
    pub async fn list_work_items_for_board(
        &self,
        project_id: &str,
        item_type: Option<&str>,
    ) -> Result<Vec<WorkItemRecord>> {
        work_items::list_work_items_for_board(&self.pool, project_id, item_type).await
    }

    /// Update work item position (for drag-drop reordering).
    pub async fn update_work_item_position(
        &self,
        item_id: &str,
        board_column: &str,
        position: i64,
    ) -> Result<bool> {
        work_items::update_work_item_position(&self.pool, item_id, board_column, position).await
    }

    /// Shift positions in a column to make room for a card.
    pub async fn shift_positions(
        &self,
        project_id: &str,
        board_column: &str,
        from_position: i64,
        shift: i64,
    ) -> Result<()> {
        work_items::shift_positions(&self.pool, project_id, board_column, from_position, shift).await
    }

    /// Get child work items for a parent.
    pub async fn get_child_work_items(&self, parent_id: &str) -> Result<Vec<WorkItemRecord>> {
        work_items::get_child_work_items(&self.pool, parent_id).await
    }

    /// Count work items by column for a project.
    pub async fn count_work_items_by_column(&self, project_id: &str) -> Result<Vec<(String, i64)>> {
        work_items::count_work_items_by_column(&self.pool, project_id).await
    }

    /// Get the next position for a column.
    pub async fn next_position_in_column(
        &self,
        project_id: &str,
        board_column: &str,
    ) -> Result<i64> {
        work_items::next_position_in_column(&self.pool, project_id, board_column).await
    }

    // =========================================================================
    // Epic-based Board Operations
    // =========================================================================

    /// List all Epics for a project, ordered by position.
    pub async fn list_epics_for_project(&self, project_id: &str) -> Result<Vec<WorkItemRecord>> {
        work_items::list_epics_for_project(&self.pool, project_id).await
    }

    /// Compute Epic assignments for all items in a project.
    /// Returns a map of item_id -> root_epic_id.
    pub async fn compute_epic_assignments(&self, project_id: &str) -> Result<std::collections::HashMap<String, String>> {
        work_items::compute_epic_assignments(&self.pool, project_id).await
    }

    /// Update Epic swimlane position.
    pub async fn update_epic_position(&self, epic_id: &str, new_position: i64) -> Result<bool> {
        work_items::update_epic_position(&self.pool, epic_id, new_position).await
    }

    /// Shift Epic positions when reordering swimlanes.
    pub async fn shift_epic_positions(
        &self,
        project_id: &str,
        from_position: i64,
        to_position: i64,
    ) -> Result<()> {
        work_items::shift_epic_positions(&self.pool, project_id, from_position, to_position).await
    }

    // =========================================================================
    // Project Entry Link Operations
    // =========================================================================

    /// Link an entry to a project.
    pub async fn link_entry(&self, link: &ProjectEntryRecord) -> Result<()> {
        links::link_entry(&self.pool, link).await
    }

    /// Unlink an entry from a project.
    pub async fn unlink_entry(&self, project_id: &str, entry_id: &str) -> Result<bool> {
        links::unlink_entry(&self.pool, project_id, entry_id).await
    }

    /// List all entries linked to a project.
    pub async fn list_project_entries(&self, project_id: &str) -> Result<Vec<ProjectEntryRecord>> {
        links::list_project_entries(&self.pool, project_id).await
    }

    /// Check if an entry is linked to a project.
    pub async fn is_entry_linked(&self, project_id: &str, entry_id: &str) -> Result<bool> {
        links::is_linked(&self.pool, project_id, entry_id).await
    }

    // =========================================================================
    // Job Operations
    // =========================================================================

    /// Insert a job record.
    pub async fn insert_job(&self, job: &JobRecord) -> Result<()> {
        jobs::insert_job(&self.pool, job).await
    }

    /// Get a job by ID.
    pub async fn get_job(&self, job_id: &str) -> Result<Option<JobRecord>> {
        jobs::get_job(&self.pool, job_id).await
    }

    /// List jobs with optional status filter.
    pub async fn list_jobs(
        &self,
        status: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<JobRecord>> {
        jobs::list_jobs(&self.pool, status, limit, offset).await
    }

    /// Insert job items.
    pub async fn insert_job_items(&self, items: &[JobItemRecord]) -> Result<()> {
        jobs::insert_job_items(&self.pool, items).await
    }

    /// Get job items for a job.
    pub async fn get_job_items(&self, job_id: &str) -> Result<Vec<JobItemRecord>> {
        jobs::get_job_items(&self.pool, job_id).await
    }

    // =========================================================================
    // Utility Operations
    // =========================================================================

    /// Count work items for a project.
    pub async fn work_item_count(&self, project_id: &str) -> Result<usize> {
        work_items::work_item_count(&self.pool, project_id).await
    }

    /// Clear all data from all tables (use with caution!).
    ///
    /// This permanently deletes all data from entries, pages, projects,
    /// work_items, project_entries, board_config, jobs, and job_items tables.
    pub async fn clear_all(&self) -> Result<()> {
        // Delete in order respecting foreign key constraints
        sqlx::query("DELETE FROM job_items").execute(&self.pool).await?;
        sqlx::query("DELETE FROM jobs").execute(&self.pool).await?;
        sqlx::query("DELETE FROM board_config").execute(&self.pool).await?;
        sqlx::query("DELETE FROM project_entries").execute(&self.pool).await?;
        sqlx::query("DELETE FROM work_items").execute(&self.pool).await?;
        sqlx::query("DELETE FROM projects").execute(&self.pool).await?;
        sqlx::query("DELETE FROM pages").execute(&self.pool).await?;
        sqlx::query("DELETE FROM entries").execute(&self.pool).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_sqlite_store_creation() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let store = SqliteStore::new(&db_path).await.unwrap();

        let info = store.info().await.unwrap();
        assert!(info.table_count > 0);

        store.close().await;
    }
}
