//! Project store wrapper for backward compatibility.
//!
//! This module provides a `ProjectStore` that wraps `SqliteStore`
//! for backward compatibility with existing code.

use crate::error::StoreError;
use kix_sqlite::{IssueRecord, ProjectEntryRecord, ProjectRecord, SqliteStore};
use std::path::Path;

// Re-export record types for convenience
pub use kix_sqlite::{IssueRecord as Issue, ProjectRecord as Project};

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

    /// Delete a project and cascade to issues and links.
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
    // Issue Operations
    // =========================================================================

    /// Create a new issue.
    pub async fn create_issue(&self, issue: &IssueRecord) -> Result<(), StoreError> {
        self.sqlite.insert_issue(issue).await.map_err(|e| {
            StoreError::Database(format!("Failed to create issue: {}", e))
        })
    }

    /// Get an issue by ID.
    pub async fn get_issue(&self, id: &str) -> Result<Option<IssueRecord>, StoreError> {
        self.sqlite.get_issue(id).await.map_err(|e| {
            StoreError::Database(format!("Failed to get issue: {}", e))
        })
    }

    /// Get an issue by project and number.
    pub async fn get_issue_by_number(
        &self,
        project_id: &str,
        number: u32,
    ) -> Result<Option<IssueRecord>, StoreError> {
        self.sqlite.get_issue_by_number(project_id, number).await.map_err(|e| {
            StoreError::Database(format!("Failed to get issue by number: {}", e))
        })
    }

    /// Get an issue by project and GitHub number.
    pub async fn get_issue_by_github_number(
        &self,
        project_id: &str,
        github_number: u32,
    ) -> Result<Option<IssueRecord>, StoreError> {
        self.sqlite.get_issue_by_github_number(project_id, github_number).await.map_err(|e| {
            StoreError::Database(format!("Failed to get issue by GitHub number: {}", e))
        })
    }

    /// Update an issue.
    pub async fn update_issue(&self, issue: &IssueRecord) -> Result<bool, StoreError> {
        self.sqlite.update_issue(issue).await.map_err(|e| {
            StoreError::Database(format!("Failed to update issue: {}", e))
        })
    }

    /// Delete an issue.
    pub async fn delete_issue(&self, id: &str) -> Result<bool, StoreError> {
        self.sqlite.delete_issue(id).await.map_err(|e| {
            StoreError::Database(format!("Failed to delete issue: {}", e))
        })
    }

    /// List issues for a project.
    pub async fn list_issues(
        &self,
        project_id: &str,
        state: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<IssueRecord>, StoreError> {
        self.sqlite.list_issues(project_id, state, limit, offset).await.map_err(|e| {
            StoreError::Database(format!("Failed to list issues: {}", e))
        })
    }

    /// Get the next issue number for a project.
    pub async fn next_issue_number(&self, project_id: &str) -> Result<u32, StoreError> {
        self.sqlite.next_issue_number(project_id).await.map_err(|e| {
            StoreError::Database(format!("Failed to get next issue number: {}", e))
        })
    }

    /// Count issues in a project.
    pub async fn issue_count(&self, project_id: &str) -> Result<usize, StoreError> {
        self.sqlite.issue_count(project_id).await.map_err(|e| {
            StoreError::Database(format!("Failed to count issues: {}", e))
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
