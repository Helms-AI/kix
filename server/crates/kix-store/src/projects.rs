//! Project store wrapper for backward compatibility.
//!
//! This module provides a `ProjectStore` that wraps `SqliteStore`
//! for backward compatibility with existing code.

use crate::error::StoreError;
use kix_sqlite::{WorkItemRecord, ProjectEntryRecord, ProjectRecord, SqliteStore};
use std::path::Path;

// Re-export record types for convenience
pub use kix_sqlite::{WorkItemRecord as WorkItem, ProjectRecord as Project};

/// Project store wrapper.
///
/// Wraps `SqliteStore` to provide project-specific operations.
#[derive(Clone)]
pub struct ProjectStore {
    sqlite: SqliteStore,
    #[allow(dead_code)]
    embedding_dim: usize,
}

impl ProjectStore {
    /// Create a new project store at the given SQLite database path.
    pub async fn new(db_path: &str, embedding_dim: usize) -> Result<Self, StoreError> {
        let path = Path::new(db_path);
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                StoreError::Database(format!("Failed to create parent directory: {}", e))
            })?;
        }

        let sqlite = SqliteStore::new(path).await.map_err(|e| {
            StoreError::Database(format!("Failed to create SQLite store: {}", e))
        })?;

        Ok(Self { sqlite, embedding_dim })
    }

    /// Create from existing SqliteStore.
    pub fn from_sqlite(sqlite: SqliteStore, embedding_dim: usize) -> Self {
        Self { sqlite, embedding_dim }
    }

    /// Initialize tables (no-op as SqliteStore handles migrations).
    pub async fn init_tables(&mut self) -> Result<(), StoreError> {
        // Tables are created during SqliteStore::new() via migrations
        Ok(())
    }

    // =========================================================================
    // Project Operations
    // =========================================================================

    /// Create a new project.
    pub async fn create_project(&self, project: &ProjectRecord) -> Result<(), StoreError> {
        self.sqlite.insert_project(project).await.map_err(|e| {
            StoreError::Database(format!("Failed to create project: {}", e))
        })
    }

    /// Get a project by ID.
    pub async fn get_project(&self, id: &str) -> Result<Option<ProjectRecord>, StoreError> {
        self.sqlite.get_project(id).await.map_err(|e| {
            StoreError::Database(format!("Failed to get project: {}", e))
        })
    }

    /// Get a project by slug.
    pub async fn get_project_by_slug(&self, slug: &str) -> Result<Option<ProjectRecord>, StoreError> {
        self.sqlite.get_project_by_slug(slug).await.map_err(|e| {
            StoreError::Database(format!("Failed to get project by slug: {}", e))
        })
    }

    /// Update a project.
    pub async fn update_project(&self, project: &ProjectRecord) -> Result<bool, StoreError> {
        self.sqlite.update_project(project).await.map_err(|e| {
            StoreError::Database(format!("Failed to update project: {}", e))
        })
    }

    /// Delete a project and cascade to work items and links.
    pub async fn delete_project(&self, id: &str) -> Result<bool, StoreError> {
        self.sqlite.delete_project(id).await.map_err(|e| {
            StoreError::Database(format!("Failed to delete project: {}", e))
        })
    }

    /// List all projects.
    pub async fn list_projects(&self, include_archived: bool) -> Result<Vec<ProjectRecord>, StoreError> {
        self.sqlite.list_projects(include_archived).await.map_err(|e| {
            StoreError::Database(format!("Failed to list projects: {}", e))
        })
    }

    // =========================================================================
    // Work Item Operations
    // =========================================================================

    /// Create a new work item.
    pub async fn create_work_item(&self, item: &WorkItemRecord) -> Result<(), StoreError> {
        self.sqlite.insert_work_item(item).await.map_err(|e| {
            StoreError::Database(format!("Failed to create work item: {}", e))
        })
    }

    /// Get a work item by ID.
    pub async fn get_work_item(&self, id: &str) -> Result<Option<WorkItemRecord>, StoreError> {
        self.sqlite.get_work_item(id).await.map_err(|e| {
            StoreError::Database(format!("Failed to get work item: {}", e))
        })
    }

    /// Get a work item by project and number.
    pub async fn get_work_item_by_number(
        &self,
        project_id: &str,
        number: u32,
    ) -> Result<Option<WorkItemRecord>, StoreError> {
        self.sqlite.get_work_item_by_number(project_id, number).await.map_err(|e| {
            StoreError::Database(format!("Failed to get work item by number: {}", e))
        })
    }

    /// Update a work item.
    pub async fn update_work_item(&self, item: &WorkItemRecord) -> Result<bool, StoreError> {
        self.sqlite.update_work_item(item).await.map_err(|e| {
            StoreError::Database(format!("Failed to update work item: {}", e))
        })
    }

    /// Delete a work item.
    pub async fn delete_work_item(&self, id: &str) -> Result<bool, StoreError> {
        self.sqlite.delete_work_item(id).await.map_err(|e| {
            StoreError::Database(format!("Failed to delete work item: {}", e))
        })
    }

    /// List work items for a project.
    pub async fn list_work_items(
        &self,
        project_id: &str,
        state: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<WorkItemRecord>, StoreError> {
        self.sqlite.list_work_items(project_id, state, limit, offset).await.map_err(|e| {
            StoreError::Database(format!("Failed to list work items: {}", e))
        })
    }

    /// Get the next work item number for a project.
    pub async fn next_work_item_number(&self, project_id: &str) -> Result<u32, StoreError> {
        self.sqlite.next_work_item_number(project_id).await.map_err(|e| {
            StoreError::Database(format!("Failed to get next work item number: {}", e))
        })
    }

    /// Count work items in a project.
    pub async fn work_item_count(&self, project_id: &str) -> Result<usize, StoreError> {
        self.sqlite.work_item_count(project_id).await.map_err(|e| {
            StoreError::Database(format!("Failed to count work items: {}", e))
        })
    }

    // =========================================================================
    // Board Operations
    // =========================================================================

    /// List work items for board view (grouped by column and sorted by position).
    pub async fn list_work_items_for_board(
        &self,
        project_id: &str,
        item_type: Option<&str>,
    ) -> Result<Vec<WorkItemRecord>, StoreError> {
        self.sqlite.list_work_items_for_board(project_id, item_type).await.map_err(|e| {
            StoreError::Database(format!("Failed to list work items for board: {}", e))
        })
    }

    /// Update work item position (for drag-drop reordering).
    pub async fn update_work_item_position(
        &self,
        item_id: &str,
        board_column: &str,
        position: i64,
    ) -> Result<bool, StoreError> {
        self.sqlite.update_work_item_position(item_id, board_column, position).await.map_err(|e| {
            StoreError::Database(format!("Failed to update work item position: {}", e))
        })
    }

    /// Shift positions in a column to make room for a card.
    pub async fn shift_positions(
        &self,
        project_id: &str,
        board_column: &str,
        from_position: i64,
        shift: i64,
    ) -> Result<(), StoreError> {
        self.sqlite.shift_positions(project_id, board_column, from_position, shift).await.map_err(|e| {
            StoreError::Database(format!("Failed to shift positions: {}", e))
        })
    }

    /// Get child work items for a parent.
    pub async fn get_child_work_items(&self, parent_id: &str) -> Result<Vec<WorkItemRecord>, StoreError> {
        self.sqlite.get_child_work_items(parent_id).await.map_err(|e| {
            StoreError::Database(format!("Failed to get child work items: {}", e))
        })
    }

    /// Count work items by column for a project.
    pub async fn count_work_items_by_column(&self, project_id: &str) -> Result<Vec<(String, i64)>, StoreError> {
        self.sqlite.count_work_items_by_column(project_id).await.map_err(|e| {
            StoreError::Database(format!("Failed to count work items by column: {}", e))
        })
    }

    /// Get the next position for a column.
    pub async fn next_position_in_column(
        &self,
        project_id: &str,
        board_column: &str,
    ) -> Result<i64, StoreError> {
        self.sqlite.next_position_in_column(project_id, board_column).await.map_err(|e| {
            StoreError::Database(format!("Failed to get next position: {}", e))
        })
    }

    // =========================================================================
    // Epic-based Board Operations
    // =========================================================================

    /// List all Epics for a project, ordered by position.
    pub async fn list_epics_for_project(&self, project_id: &str) -> Result<Vec<WorkItemRecord>, StoreError> {
        self.sqlite.list_epics_for_project(project_id).await.map_err(|e| {
            StoreError::Database(format!("Failed to list epics: {}", e))
        })
    }

    /// Compute Epic assignments for all items in a project.
    /// Returns a map of item_id -> root_epic_id.
    pub async fn compute_epic_assignments(&self, project_id: &str) -> Result<std::collections::HashMap<String, String>, StoreError> {
        self.sqlite.compute_epic_assignments(project_id).await.map_err(|e| {
            StoreError::Database(format!("Failed to compute epic assignments: {}", e))
        })
    }

    /// Update Epic swimlane position.
    pub async fn update_epic_position(&self, epic_id: &str, new_position: i64) -> Result<bool, StoreError> {
        self.sqlite.update_epic_position(epic_id, new_position).await.map_err(|e| {
            StoreError::Database(format!("Failed to update epic position: {}", e))
        })
    }

    /// Shift Epic positions when reordering swimlanes.
    pub async fn shift_epic_positions(
        &self,
        project_id: &str,
        from_position: i64,
        to_position: i64,
    ) -> Result<(), StoreError> {
        self.sqlite.shift_epic_positions(project_id, from_position, to_position).await.map_err(|e| {
            StoreError::Database(format!("Failed to shift epic positions: {}", e))
        })
    }

    // =========================================================================
    // Project Entry Link Operations
    // =========================================================================

    /// Link an entry to a project.
    pub async fn link_entry(&self, link: &ProjectEntryRecord) -> Result<(), StoreError> {
        self.sqlite.link_entry(link).await.map_err(|e| {
            StoreError::Database(format!("Failed to link entry: {}", e))
        })
    }

    /// Unlink an entry from a project.
    pub async fn unlink_entry(&self, project_id: &str, entry_id: &str) -> Result<bool, StoreError> {
        self.sqlite.unlink_entry(project_id, entry_id).await.map_err(|e| {
            StoreError::Database(format!("Failed to unlink entry: {}", e))
        })
    }

    /// List all entries linked to a project.
    pub async fn list_project_entries(&self, project_id: &str) -> Result<Vec<ProjectEntryRecord>, StoreError> {
        self.sqlite.list_project_entries(project_id).await.map_err(|e| {
            StoreError::Database(format!("Failed to list project entries: {}", e))
        })
    }

    /// Check if an entry is linked to a project.
    pub async fn is_entry_linked(&self, project_id: &str, entry_id: &str) -> Result<bool, StoreError> {
        self.sqlite.is_entry_linked(project_id, entry_id).await.map_err(|e| {
            StoreError::Database(format!("Failed to check entry link: {}", e))
        })
    }

    /// Close the store.
    pub async fn close(&self) {
        self.sqlite.close().await;
    }
}
