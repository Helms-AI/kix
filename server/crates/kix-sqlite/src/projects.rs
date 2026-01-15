//! Project CRUD operations for SQLite.

use crate::error::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

/// Project record for SQLite storage.
#[derive(Clone, Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProjectRecord {
    /// Unique project identifier (UUID)
    pub id: String,

    /// Display name
    pub name: String,

    /// URL-friendly identifier
    pub slug: String,

    /// Project description
    pub description: Option<String>,

    /// Hex color for UI (e.g., "#3b82f6")
    pub color: Option<String>,

    /// Whether the project is archived (0 or 1)
    pub archived: i64,

    /// Creation timestamp (RFC3339)
    pub created_at: String,

    /// Last update timestamp (RFC3339)
    pub updated_at: String,
}

impl ProjectRecord {
    /// Create a new project.
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        let slug = slugify(&name);
        let now = Utc::now().to_rfc3339();

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            slug,
            description: None,
            color: None,
            archived: 0,
            created_at: now.clone(),
            updated_at: now,
        }
    }

    /// Check if project is archived.
    pub fn is_archived(&self) -> bool {
        self.archived != 0
    }

    /// Set description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set color.
    pub fn with_color(mut self, color: impl Into<String>) -> Self {
        self.color = Some(color.into());
        self
    }
}

/// Create a URL-friendly slug from a string.
fn slugify(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Insert a new project.
pub async fn insert_project(pool: &SqlitePool, project: &ProjectRecord) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO projects (
            id, name, slug, description, color,
            archived, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&project.id)
    .bind(&project.name)
    .bind(&project.slug)
    .bind(&project.description)
    .bind(&project.color)
    .bind(project.archived)
    .bind(&project.created_at)
    .bind(&project.updated_at)
    .execute(pool)
    .await?;

    Ok(())
}

/// Get a project by ID.
pub async fn get_project(pool: &SqlitePool, id: &str) -> Result<Option<ProjectRecord>> {
    let project = sqlx::query_as::<_, ProjectRecord>("SELECT * FROM projects WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;

    Ok(project)
}

/// Get a project by slug.
pub async fn get_project_by_slug(pool: &SqlitePool, slug: &str) -> Result<Option<ProjectRecord>> {
    let project = sqlx::query_as::<_, ProjectRecord>("SELECT * FROM projects WHERE slug = ?")
        .bind(slug)
        .fetch_optional(pool)
        .await?;

    Ok(project)
}

/// Update a project.
pub async fn update_project(pool: &SqlitePool, project: &ProjectRecord) -> Result<bool> {
    let result = sqlx::query(
        r#"
        UPDATE projects SET
            name = ?, slug = ?, description = ?, color = ?,
            archived = ?, updated_at = ?
        WHERE id = ?
        "#,
    )
    .bind(&project.name)
    .bind(&project.slug)
    .bind(&project.description)
    .bind(&project.color)
    .bind(project.archived)
    .bind(Utc::now().to_rfc3339())
    .bind(&project.id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// Delete a project by ID.
pub async fn delete_project(pool: &SqlitePool, id: &str) -> Result<bool> {
    let result = sqlx::query("DELETE FROM projects WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected() > 0)
}

/// List all projects.
pub async fn list_projects(pool: &SqlitePool, include_archived: bool) -> Result<Vec<ProjectRecord>> {
    let query = if include_archived {
        "SELECT * FROM projects ORDER BY created_at DESC"
    } else {
        "SELECT * FROM projects WHERE archived = 0 ORDER BY created_at DESC"
    };

    let projects = sqlx::query_as::<_, ProjectRecord>(query)
        .fetch_all(pool)
        .await?;

    Ok(projects)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::{create_pool, run_migrations};
    use tempfile::TempDir;

    async fn setup_test_db() -> (SqlitePool, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let pool = create_pool(&db_path).await.unwrap();
        run_migrations(&pool).await.unwrap();
        (pool, temp_dir)
    }

    #[tokio::test]
    async fn test_insert_and_get_project() {
        let (pool, _temp_dir) = setup_test_db().await;

        let project = ProjectRecord::new("My Project")
            .with_description("A test project")
            .with_color("#3b82f6");

        insert_project(&pool, &project).await.unwrap();

        let retrieved = get_project(&pool, &project.id).await.unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.name, "My Project");
        assert_eq!(retrieved.description, Some("A test project".to_string()));
    }

    #[tokio::test]
    async fn test_get_project_by_slug() {
        let (pool, _temp_dir) = setup_test_db().await;

        let project = ProjectRecord::new("Test Project");
        insert_project(&pool, &project).await.unwrap();

        let retrieved = get_project_by_slug(&pool, "test-project").await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "Test Project");
    }

    #[tokio::test]
    async fn test_list_projects_archived() {
        let (pool, _temp_dir) = setup_test_db().await;

        // Insert active project
        let active = ProjectRecord::new("Active Project");
        insert_project(&pool, &active).await.unwrap();

        // Insert archived project
        let mut archived = ProjectRecord::new("Archived Project");
        archived.archived = 1;
        insert_project(&pool, &archived).await.unwrap();

        // List without archived
        let active_only = list_projects(&pool, false).await.unwrap();
        assert_eq!(active_only.len(), 1);
        assert_eq!(active_only[0].name, "Active Project");

        // List with archived
        let all = list_projects(&pool, true).await.unwrap();
        assert_eq!(all.len(), 2);
    }
}
