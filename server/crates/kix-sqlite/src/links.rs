//! Project-Entry link operations for SQLite.
//!
//! Links knowledge entries to projects for project-scoped search.

use crate::error::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

/// Project entry link record.
#[derive(Clone, Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProjectEntryRecord {
    /// Unique link identifier (UUID)
    pub id: String,

    /// FK to project
    pub project_id: String,

    /// FK to entry
    pub entry_id: String,

    /// User notes about the link
    pub notes: Option<String>,

    /// Relevance score (0.0 - 1.0)
    pub relevance: Option<f64>,

    /// When the link was created (RFC3339)
    pub linked_at: String,
}

impl ProjectEntryRecord {
    /// Create a new project-entry link.
    pub fn new(project_id: impl Into<String>, entry_id: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            project_id: project_id.into(),
            entry_id: entry_id.into(),
            notes: None,
            relevance: None,
            linked_at: Utc::now().to_rfc3339(),
        }
    }

    /// Set notes.
    pub fn with_notes(mut self, notes: impl Into<String>) -> Self {
        self.notes = Some(notes.into());
        self
    }

    /// Set relevance score.
    pub fn with_relevance(mut self, relevance: f64) -> Self {
        self.relevance = Some(relevance.clamp(0.0, 1.0));
        self
    }
}

/// Link an entry to a project.
pub async fn link_entry(pool: &SqlitePool, link: &ProjectEntryRecord) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO project_entries (id, project_id, entry_id, notes, relevance, linked_at)
        VALUES (?, ?, ?, ?, ?, ?)
        ON CONFLICT(project_id, entry_id) DO UPDATE SET
            notes = excluded.notes,
            relevance = excluded.relevance,
            linked_at = excluded.linked_at
        "#,
    )
    .bind(&link.id)
    .bind(&link.project_id)
    .bind(&link.entry_id)
    .bind(&link.notes)
    .bind(link.relevance)
    .bind(&link.linked_at)
    .execute(pool)
    .await?;

    Ok(())
}

/// Unlink an entry from a project.
pub async fn unlink_entry(pool: &SqlitePool, project_id: &str, entry_id: &str) -> Result<bool> {
    let result = sqlx::query("DELETE FROM project_entries WHERE project_id = ? AND entry_id = ?")
        .bind(project_id)
        .bind(entry_id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected() > 0)
}

/// List all entries linked to a project.
pub async fn list_project_entries(
    pool: &SqlitePool,
    project_id: &str,
) -> Result<Vec<ProjectEntryRecord>> {
    let links = sqlx::query_as::<_, ProjectEntryRecord>(
        "SELECT * FROM project_entries WHERE project_id = ? ORDER BY linked_at DESC",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;

    Ok(links)
}

/// Check if an entry is linked to a project.
pub async fn is_linked(pool: &SqlitePool, project_id: &str, entry_id: &str) -> Result<bool> {
    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM project_entries WHERE project_id = ? AND entry_id = ?",
    )
    .bind(project_id)
    .bind(entry_id)
    .fetch_one(pool)
    .await?;

    Ok(count.0 > 0)
}

/// Get a specific link.
pub async fn get_link(
    pool: &SqlitePool,
    project_id: &str,
    entry_id: &str,
) -> Result<Option<ProjectEntryRecord>> {
    let link = sqlx::query_as::<_, ProjectEntryRecord>(
        "SELECT * FROM project_entries WHERE project_id = ? AND entry_id = ?",
    )
    .bind(project_id)
    .bind(entry_id)
    .fetch_optional(pool)
    .await?;

    Ok(link)
}

/// List all projects an entry is linked to.
pub async fn list_entry_projects(
    pool: &SqlitePool,
    entry_id: &str,
) -> Result<Vec<ProjectEntryRecord>> {
    let links = sqlx::query_as::<_, ProjectEntryRecord>(
        "SELECT * FROM project_entries WHERE entry_id = ? ORDER BY linked_at DESC",
    )
    .bind(entry_id)
    .fetch_all(pool)
    .await?;

    Ok(links)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entries::{insert_entry, EntryRecord};
    use crate::pool::{create_pool, run_migrations};
    use crate::projects::{insert_project, ProjectRecord};
    use tempfile::TempDir;

    async fn setup_test_db() -> (SqlitePool, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let pool = create_pool(&db_path).await.unwrap();
        run_migrations(&pool).await.unwrap();
        (pool, temp_dir)
    }

    async fn create_test_data(pool: &SqlitePool) -> (String, String) {
        let mut project = ProjectRecord::new_local("Test Project");
        project.id = "proj-1".to_string();
        insert_project(pool, &project).await.unwrap();

        let entry = EntryRecord::new(
            "entry-1",
            "Test Entry",
            "document",
            "url",
            "https://example.com",
            "hash",
        );
        insert_entry(pool, &entry).await.unwrap();

        (project.id, entry.id)
    }

    #[tokio::test]
    async fn test_link_entry() {
        let (pool, _temp_dir) = setup_test_db().await;
        let (project_id, entry_id) = create_test_data(&pool).await;

        let link = ProjectEntryRecord::new(&project_id, &entry_id)
            .with_notes("Important reference")
            .with_relevance(0.95);

        link_entry(&pool, &link).await.unwrap();

        assert!(is_linked(&pool, &project_id, &entry_id).await.unwrap());

        let retrieved = get_link(&pool, &project_id, &entry_id).await.unwrap().unwrap();
        assert_eq!(retrieved.notes, Some("Important reference".to_string()));
        assert!((retrieved.relevance.unwrap() - 0.95).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_unlink_entry() {
        let (pool, _temp_dir) = setup_test_db().await;
        let (project_id, entry_id) = create_test_data(&pool).await;

        let link = ProjectEntryRecord::new(&project_id, &entry_id);
        link_entry(&pool, &link).await.unwrap();

        assert!(is_linked(&pool, &project_id, &entry_id).await.unwrap());

        let unlinked = unlink_entry(&pool, &project_id, &entry_id).await.unwrap();
        assert!(unlinked);

        assert!(!is_linked(&pool, &project_id, &entry_id).await.unwrap());
    }

    #[tokio::test]
    async fn test_list_project_entries() {
        let (pool, _temp_dir) = setup_test_db().await;

        // Create project
        let mut project = ProjectRecord::new_local("Project");
        project.id = "proj-list".to_string();
        insert_project(&pool, &project).await.unwrap();

        // Create and link multiple entries
        for i in 0..5 {
            let entry = EntryRecord::new(
                format!("entry-{}", i),
                format!("Entry {}", i),
                "document",
                "url",
                format!("https://example.com/{}", i),
                format!("hash-{}", i),
            );
            insert_entry(&pool, &entry).await.unwrap();

            let link = ProjectEntryRecord::new("proj-list", &entry.id);
            link_entry(&pool, &link).await.unwrap();
        }

        let links = list_project_entries(&pool, "proj-list").await.unwrap();
        assert_eq!(links.len(), 5);
    }

    #[tokio::test]
    async fn test_upsert_link() {
        let (pool, _temp_dir) = setup_test_db().await;
        let (project_id, entry_id) = create_test_data(&pool).await;

        // First link
        let link1 = ProjectEntryRecord::new(&project_id, &entry_id).with_notes("First notes");
        link_entry(&pool, &link1).await.unwrap();

        // Upsert with new notes
        let link2 = ProjectEntryRecord::new(&project_id, &entry_id).with_notes("Updated notes");
        link_entry(&pool, &link2).await.unwrap();

        // Should still be one link, but with updated notes
        let links = list_project_entries(&pool, &project_id).await.unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].notes, Some("Updated notes".to_string()));
    }
}
