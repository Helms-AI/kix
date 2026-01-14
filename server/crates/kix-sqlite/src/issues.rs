//! Issue CRUD operations for SQLite.
//!
//! Note: Issue vectors are stored in LanceDB, only metadata is stored here.

use crate::error::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

/// Issue record for SQLite storage.
///
/// Vectors for semantic search are stored in LanceDB separately.
#[derive(Clone, Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct IssueRecord {
    /// Unique issue identifier (UUID)
    pub id: String,

    /// FK to parent project
    pub project_id: String,

    /// Local issue number (within project)
    pub number: i64,

    /// Issue title
    pub title: String,

    /// Issue body (markdown)
    pub body: Option<String>,

    /// Issue state ("open" or "closed")
    pub state: String,

    /// Labels as JSON array
    pub labels: Option<String>,

    /// Assignees as JSON array
    pub assignees: Option<String>,

    /// Priority (1-5, 1=highest)
    pub priority: Option<i64>,

    /// GitHub issue number (if synced)
    pub github_number: Option<i64>,

    /// GitHub GraphQL node ID
    pub github_node_id: Option<String>,

    /// GitHub issue URL
    pub github_url: Option<String>,

    /// GitHub Project V2 item ID
    pub github_project_item_id: Option<String>,

    /// Source: "local" or "github"
    pub source: String,

    /// Creation timestamp (RFC3339)
    pub created_at: String,

    /// Last update timestamp (RFC3339)
    pub updated_at: String,

    /// Close timestamp (RFC3339)
    pub closed_at: Option<String>,

    /// Last sync timestamp (RFC3339)
    pub synced_at: Option<String>,
}

impl IssueRecord {
    /// Create a new local issue.
    pub fn new(project_id: impl Into<String>, number: u32, title: impl Into<String>) -> Self {
        let now = Utc::now().to_rfc3339();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            project_id: project_id.into(),
            number: number as i64,
            title: title.into(),
            body: None,
            state: "open".to_string(),
            labels: None,
            assignees: None,
            priority: None,
            github_number: None,
            github_node_id: None,
            github_url: None,
            github_project_item_id: None,
            source: "local".to_string(),
            created_at: now.clone(),
            updated_at: now,
            closed_at: None,
            synced_at: None,
        }
    }

    /// Get labels as Vec<String>.
    pub fn labels_vec(&self) -> Vec<String> {
        self.labels
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default()
    }

    /// Set labels from Vec<String>.
    pub fn set_labels(&mut self, labels: Vec<String>) {
        self.labels = Some(serde_json::to_string(&labels).unwrap_or_else(|_| "[]".to_string()));
    }

    /// Get assignees as Vec<String>.
    pub fn assignees_vec(&self) -> Vec<String> {
        self.assignees
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default()
    }

    /// Set assignees from Vec<String>.
    pub fn set_assignees(&mut self, assignees: Vec<String>) {
        self.assignees = Some(serde_json::to_string(&assignees).unwrap_or_else(|_| "[]".to_string()));
    }

    /// Check if issue is open.
    pub fn is_open(&self) -> bool {
        self.state == "open"
    }

    /// Check if issue is from GitHub.
    pub fn is_github(&self) -> bool {
        self.source == "github"
    }

    /// Set body.
    pub fn with_body(mut self, body: impl Into<String>) -> Self {
        self.body = Some(body.into());
        self
    }

    /// Set priority.
    pub fn with_priority(mut self, priority: u8) -> Self {
        self.priority = Some(priority as i64);
        self
    }

    /// Set labels (builder pattern).
    pub fn with_labels(mut self, labels: Vec<String>) -> Self {
        self.set_labels(labels);
        self
    }

    /// Set assignees (builder pattern).
    pub fn with_assignees(mut self, assignees: Vec<String>) -> Self {
        self.set_assignees(assignees);
        self
    }
}

/// Insert a new issue.
pub async fn insert_issue(pool: &SqlitePool, issue: &IssueRecord) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO issues (
            id, project_id, number, title, body, state,
            labels, assignees, priority, github_number,
            github_node_id, github_url, github_project_item_id,
            source, created_at, updated_at, closed_at, synced_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&issue.id)
    .bind(&issue.project_id)
    .bind(issue.number)
    .bind(&issue.title)
    .bind(&issue.body)
    .bind(&issue.state)
    .bind(&issue.labels)
    .bind(&issue.assignees)
    .bind(issue.priority)
    .bind(issue.github_number)
    .bind(&issue.github_node_id)
    .bind(&issue.github_url)
    .bind(&issue.github_project_item_id)
    .bind(&issue.source)
    .bind(&issue.created_at)
    .bind(&issue.updated_at)
    .bind(&issue.closed_at)
    .bind(&issue.synced_at)
    .execute(pool)
    .await?;

    Ok(())
}

/// Get an issue by ID.
pub async fn get_issue(pool: &SqlitePool, id: &str) -> Result<Option<IssueRecord>> {
    let issue = sqlx::query_as::<_, IssueRecord>("SELECT * FROM issues WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;

    Ok(issue)
}

/// Get an issue by project and number.
pub async fn get_issue_by_number(
    pool: &SqlitePool,
    project_id: &str,
    number: u32,
) -> Result<Option<IssueRecord>> {
    let issue = sqlx::query_as::<_, IssueRecord>(
        "SELECT * FROM issues WHERE project_id = ? AND number = ?",
    )
    .bind(project_id)
    .bind(number as i64)
    .fetch_optional(pool)
    .await?;

    Ok(issue)
}

/// Get an issue by project and GitHub number.
pub async fn get_issue_by_github_number(
    pool: &SqlitePool,
    project_id: &str,
    github_number: u32,
) -> Result<Option<IssueRecord>> {
    let issue = sqlx::query_as::<_, IssueRecord>(
        "SELECT * FROM issues WHERE project_id = ? AND github_number = ?",
    )
    .bind(project_id)
    .bind(github_number as i64)
    .fetch_optional(pool)
    .await?;

    Ok(issue)
}

/// Update an issue.
pub async fn update_issue(pool: &SqlitePool, issue: &IssueRecord) -> Result<bool> {
    let result = sqlx::query(
        r#"
        UPDATE issues SET
            title = ?, body = ?, state = ?, labels = ?,
            assignees = ?, priority = ?, github_number = ?,
            github_node_id = ?, github_url = ?, github_project_item_id = ?,
            updated_at = ?, closed_at = ?, synced_at = ?
        WHERE id = ?
        "#,
    )
    .bind(&issue.title)
    .bind(&issue.body)
    .bind(&issue.state)
    .bind(&issue.labels)
    .bind(&issue.assignees)
    .bind(issue.priority)
    .bind(issue.github_number)
    .bind(&issue.github_node_id)
    .bind(&issue.github_url)
    .bind(&issue.github_project_item_id)
    .bind(Utc::now().to_rfc3339())
    .bind(&issue.closed_at)
    .bind(&issue.synced_at)
    .bind(&issue.id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// Delete an issue by ID.
pub async fn delete_issue(pool: &SqlitePool, id: &str) -> Result<bool> {
    let result = sqlx::query("DELETE FROM issues WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected() > 0)
}

/// List issues for a project.
pub async fn list_issues(
    pool: &SqlitePool,
    project_id: &str,
    state: Option<&str>,
    limit: usize,
    offset: usize,
) -> Result<Vec<IssueRecord>> {
    let query = match state {
        Some(s) => {
            sqlx::query_as::<_, IssueRecord>(
                "SELECT * FROM issues WHERE project_id = ? AND state = ? ORDER BY number DESC LIMIT ? OFFSET ?",
            )
            .bind(project_id)
            .bind(s)
            .bind(limit as i64)
            .bind(offset as i64)
        }
        None => {
            sqlx::query_as::<_, IssueRecord>(
                "SELECT * FROM issues WHERE project_id = ? ORDER BY number DESC LIMIT ? OFFSET ?",
            )
            .bind(project_id)
            .bind(limit as i64)
            .bind(offset as i64)
        }
    };

    let issues = query.fetch_all(pool).await?;
    Ok(issues)
}

/// Get the next issue number for a project.
pub async fn next_issue_number(pool: &SqlitePool, project_id: &str) -> Result<u32> {
    let max: (Option<i64>,) =
        sqlx::query_as("SELECT MAX(number) FROM issues WHERE project_id = ?")
            .bind(project_id)
            .fetch_one(pool)
            .await?;

    Ok(max.0.unwrap_or(0) as u32 + 1)
}

/// Count issues for a project.
pub async fn issue_count(pool: &SqlitePool, project_id: &str) -> Result<usize> {
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM issues WHERE project_id = ?")
        .bind(project_id)
        .fetch_one(pool)
        .await?;

    Ok(count.0 as usize)
}

#[cfg(test)]
mod tests {
    use super::*;
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

    async fn create_test_project(pool: &SqlitePool, id: &str) {
        let mut project = ProjectRecord::new_local("Test Project");
        project.id = id.to_string();
        insert_project(pool, &project).await.unwrap();
    }

    #[tokio::test]
    async fn test_insert_and_get_issue() {
        let (pool, _temp_dir) = setup_test_db().await;
        create_test_project(&pool, "proj-1").await;

        let issue = IssueRecord::new("proj-1", 1, "First Issue")
            .with_body("This is the body")
            .with_priority(2);

        insert_issue(&pool, &issue).await.unwrap();

        let retrieved = get_issue(&pool, &issue.id).await.unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.title, "First Issue");
        assert_eq!(retrieved.priority, Some(2));
    }

    #[tokio::test]
    async fn test_get_issue_by_number() {
        let (pool, _temp_dir) = setup_test_db().await;
        create_test_project(&pool, "proj-2").await;

        let issue = IssueRecord::new("proj-2", 42, "Issue 42");
        insert_issue(&pool, &issue).await.unwrap();

        let retrieved = get_issue_by_number(&pool, "proj-2", 42).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().title, "Issue 42");
    }

    #[tokio::test]
    async fn test_next_issue_number() {
        let (pool, _temp_dir) = setup_test_db().await;
        create_test_project(&pool, "proj-3").await;

        // First issue should be 1
        assert_eq!(next_issue_number(&pool, "proj-3").await.unwrap(), 1);

        // Add some issues
        for i in 1..=5 {
            let issue = IssueRecord::new("proj-3", i, format!("Issue {}", i));
            insert_issue(&pool, &issue).await.unwrap();
        }

        // Next should be 6
        assert_eq!(next_issue_number(&pool, "proj-3").await.unwrap(), 6);
    }

    #[tokio::test]
    async fn test_list_issues_by_state() {
        let (pool, _temp_dir) = setup_test_db().await;
        create_test_project(&pool, "proj-4").await;

        // Create open and closed issues
        for i in 1..=5 {
            let mut issue = IssueRecord::new("proj-4", i, format!("Issue {}", i));
            if i % 2 == 0 {
                issue.state = "closed".to_string();
            }
            insert_issue(&pool, &issue).await.unwrap();
        }

        let open = list_issues(&pool, "proj-4", Some("open"), 100, 0).await.unwrap();
        let closed = list_issues(&pool, "proj-4", Some("closed"), 100, 0).await.unwrap();
        let all = list_issues(&pool, "proj-4", None, 100, 0).await.unwrap();

        assert_eq!(open.len(), 3);
        assert_eq!(closed.len(), 2);
        assert_eq!(all.len(), 5);
    }

    #[tokio::test]
    async fn test_labels_and_assignees() {
        let (pool, _temp_dir) = setup_test_db().await;
        create_test_project(&pool, "proj-5").await;

        let mut issue = IssueRecord::new("proj-5", 1, "Labeled Issue");
        issue.set_labels(vec!["bug".to_string(), "priority".to_string()]);
        issue.set_assignees(vec!["alice".to_string(), "bob".to_string()]);
        insert_issue(&pool, &issue).await.unwrap();

        let retrieved = get_issue(&pool, &issue.id).await.unwrap().unwrap();
        assert_eq!(retrieved.labels_vec(), vec!["bug", "priority"]);
        assert_eq!(retrieved.assignees_vec(), vec!["alice", "bob"]);
    }
}
