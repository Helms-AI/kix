//! Work item CRUD operations for SQLite.
//!
//! Provides storage for Kanban board work items: epics, stories, tasks, subtasks, bugs.

use crate::error::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

/// Work item record for SQLite storage.
#[derive(Clone, Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct WorkItemRecord {
    /// Unique work item identifier (UUID)
    pub id: String,

    /// FK to parent project
    pub project_id: String,

    /// Local work item number (within project)
    pub number: i64,

    /// Work item title
    pub title: String,

    /// Work item body (markdown)
    pub body: Option<String>,

    /// State ("open" or "closed")
    pub state: String,

    /// Labels as JSON array
    pub labels: Option<String>,

    /// Assignees as JSON array
    pub assignees: Option<String>,

    /// Priority (1-5, 1=highest)
    pub priority: Option<i64>,

    /// Creation timestamp (RFC3339)
    pub created_at: String,

    /// Last update timestamp (RFC3339)
    pub updated_at: String,

    /// Close timestamp (RFC3339)
    pub closed_at: Option<String>,

    // =========================================================================
    // Board-related fields
    // =========================================================================

    /// Item type: 'epic', 'story', 'task', 'subtask', 'bug'
    #[sqlx(rename = "item_type")]
    pub item_type: String,

    /// Parent work item ID (for hierarchy)
    pub parent_id: Option<String>,

    /// Position within board column (for drag-drop ordering)
    pub position: i64,

    /// Board column: 'backlog', 'todo', 'in_progress', 'in_review', 'testing', 'done'
    pub board_column: String,

    /// Story points for estimation
    pub story_points: Option<i64>,

    /// Epic color (hex without #, e.g., "4f46e5")
    pub epic_color: Option<String>,
}

impl WorkItemRecord {
    /// Create a new work item.
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
            created_at: now.clone(),
            updated_at: now,
            closed_at: None,
            // Board fields
            item_type: "task".to_string(),
            parent_id: None,
            position: 0,
            board_column: "backlog".to_string(),
            story_points: None,
            epic_color: None,
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

    /// Check if work item is open.
    pub fn is_open(&self) -> bool {
        self.state == "open"
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

    /// Set item type (builder pattern).
    pub fn with_item_type(mut self, item_type: impl Into<String>) -> Self {
        self.item_type = item_type.into();
        self
    }

    /// Set parent work item ID (builder pattern).
    pub fn with_parent(mut self, parent_id: impl Into<String>) -> Self {
        self.parent_id = Some(parent_id.into());
        self
    }

    /// Set board column (builder pattern).
    pub fn with_board_column(mut self, column: impl Into<String>) -> Self {
        self.board_column = column.into();
        self
    }

    /// Set position (builder pattern).
    pub fn with_position(mut self, position: i64) -> Self {
        self.position = position;
        self
    }

    /// Set story points (builder pattern).
    pub fn with_story_points(mut self, points: i64) -> Self {
        self.story_points = Some(points);
        self
    }

    /// Set epic color (builder pattern).
    pub fn with_epic_color(mut self, color: impl Into<String>) -> Self {
        self.epic_color = Some(color.into());
        self
    }

    /// Check if this is an epic.
    pub fn is_epic(&self) -> bool {
        self.item_type == "epic"
    }

    /// Check if this work item can contain a child of the given type.
    /// Hierarchy rules (flexible - any type can be created independently):
    ///   Epic -> Story, Bug, Task
    ///   Story -> Task, Subtask
    ///   Task -> Subtask
    ///   Bug -> Subtask
    ///   Subtask -> (none)
    pub fn can_contain(&self, child_type: &str) -> bool {
        match self.item_type.as_str() {
            "epic" => matches!(child_type, "story" | "bug" | "task"),
            "story" => matches!(child_type, "task" | "subtask"),
            "task" => matches!(child_type, "subtask"),
            "bug" => matches!(child_type, "subtask"),
            "subtask" => false,
            _ => false,
        }
    }
}

/// Insert a new work item.
pub async fn insert_work_item(pool: &SqlitePool, item: &WorkItemRecord) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO work_items (
            id, project_id, number, title, body, state,
            labels, assignees, priority,
            created_at, updated_at, closed_at,
            item_type, parent_id, position, board_column,
            story_points, epic_color
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&item.id)
    .bind(&item.project_id)
    .bind(item.number)
    .bind(&item.title)
    .bind(&item.body)
    .bind(&item.state)
    .bind(&item.labels)
    .bind(&item.assignees)
    .bind(item.priority)
    .bind(&item.created_at)
    .bind(&item.updated_at)
    .bind(&item.closed_at)
    // Board fields
    .bind(&item.item_type)
    .bind(&item.parent_id)
    .bind(item.position)
    .bind(&item.board_column)
    .bind(item.story_points)
    .bind(&item.epic_color)
    .execute(pool)
    .await?;

    Ok(())
}

/// Get a work item by ID.
pub async fn get_work_item(pool: &SqlitePool, id: &str) -> Result<Option<WorkItemRecord>> {
    let item = sqlx::query_as::<_, WorkItemRecord>("SELECT * FROM work_items WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;

    Ok(item)
}

/// Get a work item by project and number.
pub async fn get_work_item_by_number(
    pool: &SqlitePool,
    project_id: &str,
    number: u32,
) -> Result<Option<WorkItemRecord>> {
    let item = sqlx::query_as::<_, WorkItemRecord>(
        "SELECT * FROM work_items WHERE project_id = ? AND number = ?",
    )
    .bind(project_id)
    .bind(number as i64)
    .fetch_optional(pool)
    .await?;

    Ok(item)
}

/// Update a work item.
pub async fn update_work_item(pool: &SqlitePool, item: &WorkItemRecord) -> Result<bool> {
    let result = sqlx::query(
        r#"
        UPDATE work_items SET
            title = ?, body = ?, state = ?, labels = ?,
            assignees = ?, priority = ?,
            updated_at = ?, closed_at = ?,
            item_type = ?, parent_id = ?, position = ?, board_column = ?,
            story_points = ?, epic_color = ?
        WHERE id = ?
        "#,
    )
    .bind(&item.title)
    .bind(&item.body)
    .bind(&item.state)
    .bind(&item.labels)
    .bind(&item.assignees)
    .bind(item.priority)
    .bind(Utc::now().to_rfc3339())
    .bind(&item.closed_at)
    // Board fields
    .bind(&item.item_type)
    .bind(&item.parent_id)
    .bind(item.position)
    .bind(&item.board_column)
    .bind(item.story_points)
    .bind(&item.epic_color)
    .bind(&item.id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// Delete a work item by ID.
pub async fn delete_work_item(pool: &SqlitePool, id: &str) -> Result<bool> {
    let result = sqlx::query("DELETE FROM work_items WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected() > 0)
}

/// List work items for a project.
pub async fn list_work_items(
    pool: &SqlitePool,
    project_id: &str,
    state: Option<&str>,
    limit: usize,
    offset: usize,
) -> Result<Vec<WorkItemRecord>> {
    let query = match state {
        Some(s) => {
            sqlx::query_as::<_, WorkItemRecord>(
                "SELECT * FROM work_items WHERE project_id = ? AND state = ? ORDER BY number DESC LIMIT ? OFFSET ?",
            )
            .bind(project_id)
            .bind(s)
            .bind(limit as i64)
            .bind(offset as i64)
        }
        None => {
            sqlx::query_as::<_, WorkItemRecord>(
                "SELECT * FROM work_items WHERE project_id = ? ORDER BY number DESC LIMIT ? OFFSET ?",
            )
            .bind(project_id)
            .bind(limit as i64)
            .bind(offset as i64)
        }
    };

    let items = query.fetch_all(pool).await?;
    Ok(items)
}

/// Get the next work item number for a project.
pub async fn next_work_item_number(pool: &SqlitePool, project_id: &str) -> Result<u32> {
    let max: (Option<i64>,) =
        sqlx::query_as("SELECT MAX(number) FROM work_items WHERE project_id = ?")
            .bind(project_id)
            .fetch_one(pool)
            .await?;

    Ok(max.0.unwrap_or(0) as u32 + 1)
}

/// Count work items for a project.
pub async fn work_item_count(pool: &SqlitePool, project_id: &str) -> Result<usize> {
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM work_items WHERE project_id = ?")
        .bind(project_id)
        .fetch_one(pool)
        .await?;

    Ok(count.0 as usize)
}

// =========================================================================
// Board-specific operations
// =========================================================================

/// List work items for board view (grouped by column and sorted by position).
pub async fn list_work_items_for_board(
    pool: &SqlitePool,
    project_id: &str,
    item_type: Option<&str>,
) -> Result<Vec<WorkItemRecord>> {
    let query = match item_type {
        Some(t) => {
            sqlx::query_as::<_, WorkItemRecord>(
                r#"
                SELECT * FROM work_items
                WHERE project_id = ? AND item_type = ?
                ORDER BY board_column, position ASC
                "#,
            )
            .bind(project_id)
            .bind(t)
        }
        None => {
            sqlx::query_as::<_, WorkItemRecord>(
                r#"
                SELECT * FROM work_items
                WHERE project_id = ?
                ORDER BY board_column, position ASC
                "#,
            )
            .bind(project_id)
        }
    };

    let items = query.fetch_all(pool).await?;
    Ok(items)
}

/// Update work item position (for drag-drop reordering).
pub async fn update_work_item_position(
    pool: &SqlitePool,
    item_id: &str,
    board_column: &str,
    position: i64,
) -> Result<bool> {
    let result = sqlx::query(
        r#"
        UPDATE work_items SET
            board_column = ?,
            position = ?,
            updated_at = ?
        WHERE id = ?
        "#,
    )
    .bind(board_column)
    .bind(position)
    .bind(Utc::now().to_rfc3339())
    .bind(item_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// Reorder work items in a column after a card is moved.
/// Shifts positions to make room for the new card.
pub async fn shift_positions(
    pool: &SqlitePool,
    project_id: &str,
    board_column: &str,
    from_position: i64,
    shift: i64, // +1 to shift down, -1 to shift up
) -> Result<()> {
    if shift > 0 {
        // Shift positions down (increase) for inserts
        sqlx::query(
            r#"
            UPDATE work_items SET
                position = position + 1
            WHERE project_id = ? AND board_column = ? AND position >= ?
            "#,
        )
        .bind(project_id)
        .bind(board_column)
        .bind(from_position)
        .execute(pool)
        .await?;
    } else {
        // Shift positions up (decrease) for removals
        sqlx::query(
            r#"
            UPDATE work_items SET
                position = position - 1
            WHERE project_id = ? AND board_column = ? AND position > ?
            "#,
        )
        .bind(project_id)
        .bind(board_column)
        .bind(from_position)
        .execute(pool)
        .await?;
    }
    Ok(())
}

/// Get child work items for a parent (for hierarchy display).
pub async fn get_child_work_items(
    pool: &SqlitePool,
    parent_id: &str,
) -> Result<Vec<WorkItemRecord>> {
    let items = sqlx::query_as::<_, WorkItemRecord>(
        "SELECT * FROM work_items WHERE parent_id = ? ORDER BY position ASC",
    )
    .bind(parent_id)
    .fetch_all(pool)
    .await?;

    Ok(items)
}

/// Count work items by column for a project (for column headers).
pub async fn count_work_items_by_column(
    pool: &SqlitePool,
    project_id: &str,
) -> Result<Vec<(String, i64)>> {
    let counts: Vec<(String, i64)> = sqlx::query_as(
        r#"
        SELECT board_column, COUNT(*) as count
        FROM work_items
        WHERE project_id = ?
        GROUP BY board_column
        "#,
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;

    Ok(counts)
}

/// Get the next position for a column (for new card placement).
pub async fn next_position_in_column(
    pool: &SqlitePool,
    project_id: &str,
    board_column: &str,
) -> Result<i64> {
    let max: (Option<i64>,) = sqlx::query_as(
        "SELECT MAX(position) FROM work_items WHERE project_id = ? AND board_column = ?",
    )
    .bind(project_id)
    .bind(board_column)
    .fetch_one(pool)
    .await?;

    Ok(max.0.unwrap_or(-1) + 1)
}

// =========================================================================
// Epic-based board operations
// =========================================================================

/// List all Epics for a project, ordered by position.
/// Used for Epic-based swimlane board view.
pub async fn list_epics_for_project(
    pool: &SqlitePool,
    project_id: &str,
) -> Result<Vec<WorkItemRecord>> {
    let items = sqlx::query_as::<_, WorkItemRecord>(
        r#"
        SELECT * FROM work_items
        WHERE project_id = ? AND item_type = 'epic'
        ORDER BY position ASC
        "#,
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;

    Ok(items)
}

/// Compute the root Epic ID for all descendants in a project.
/// Walks the parent_id chain up to find the Epic ancestor.
/// Returns a HashMap mapping item_id -> root_epic_id.
/// Items with no Epic ancestor are not included in the result.
pub async fn compute_epic_assignments(
    pool: &SqlitePool,
    project_id: &str,
) -> Result<std::collections::HashMap<String, String>> {
    // Load all work items for the project
    let items = sqlx::query_as::<_, WorkItemRecord>(
        "SELECT * FROM work_items WHERE project_id = ?",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;

    // Build a lookup map: id -> item
    let item_map: std::collections::HashMap<String, &WorkItemRecord> =
        items.iter().map(|item| (item.id.clone(), item)).collect();

    // Build a parent lookup map: id -> parent_id
    let parent_map: std::collections::HashMap<String, Option<String>> = items
        .iter()
        .map(|item| (item.id.clone(), item.parent_id.clone()))
        .collect();

    // Compute root epic for each item
    let mut epic_assignments: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();

    for item in &items {
        // Skip Epics themselves - they don't need assignment
        if item.item_type == "epic" {
            continue;
        }

        // Walk up the parent chain to find the root Epic
        let mut current_id = item.id.clone();
        let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();

        loop {
            // Prevent infinite loops
            if visited.contains(&current_id) {
                break;
            }
            visited.insert(current_id.clone());

            // Get the current item
            let current = match item_map.get(&current_id) {
                Some(i) => *i,
                None => break,
            };

            // If this is an Epic, we found the root
            if current.item_type == "epic" {
                epic_assignments.insert(item.id.clone(), current.id.clone());
                break;
            }

            // Move to parent
            match parent_map.get(&current_id) {
                Some(Some(parent_id)) => current_id = parent_id.clone(),
                _ => break, // No parent, item is unassigned
            }
        }
    }

    Ok(epic_assignments)
}

/// Update Epic swimlane position (for reordering).
pub async fn update_epic_position(
    pool: &SqlitePool,
    epic_id: &str,
    new_position: i64,
) -> Result<bool> {
    let result = sqlx::query(
        r#"
        UPDATE work_items SET
            position = ?,
            updated_at = ?
        WHERE id = ? AND item_type = 'epic'
        "#,
    )
    .bind(new_position)
    .bind(Utc::now().to_rfc3339())
    .bind(epic_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// Shift Epic positions when reordering swimlanes.
pub async fn shift_epic_positions(
    pool: &SqlitePool,
    project_id: &str,
    from_position: i64,
    to_position: i64,
) -> Result<()> {
    if from_position < to_position {
        // Moving down: shift items between (from, to] up by 1
        sqlx::query(
            r#"
            UPDATE work_items SET
                position = position - 1
            WHERE project_id = ? AND item_type = 'epic'
                AND position > ? AND position <= ?
            "#,
        )
        .bind(project_id)
        .bind(from_position)
        .bind(to_position)
        .execute(pool)
        .await?;
    } else {
        // Moving up: shift items between [to, from) down by 1
        sqlx::query(
            r#"
            UPDATE work_items SET
                position = position + 1
            WHERE project_id = ? AND item_type = 'epic'
                AND position >= ? AND position < ?
            "#,
        )
        .bind(project_id)
        .bind(to_position)
        .bind(from_position)
        .execute(pool)
        .await?;
    }
    Ok(())
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
        let mut project = ProjectRecord::new("Test Project");
        project.id = id.to_string();
        insert_project(pool, &project).await.unwrap();
    }

    #[tokio::test]
    async fn test_insert_and_get_work_item() {
        let (pool, _temp_dir) = setup_test_db().await;
        create_test_project(&pool, "proj-1").await;

        let item = WorkItemRecord::new("proj-1", 1, "First Work Item")
            .with_body("This is the body")
            .with_priority(2);

        insert_work_item(&pool, &item).await.unwrap();

        let retrieved = get_work_item(&pool, &item.id).await.unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.title, "First Work Item");
        assert_eq!(retrieved.priority, Some(2));
    }

    #[tokio::test]
    async fn test_get_work_item_by_number() {
        let (pool, _temp_dir) = setup_test_db().await;
        create_test_project(&pool, "proj-2").await;

        let item = WorkItemRecord::new("proj-2", 42, "Work Item 42");
        insert_work_item(&pool, &item).await.unwrap();

        let retrieved = get_work_item_by_number(&pool, "proj-2", 42).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().title, "Work Item 42");
    }

    #[tokio::test]
    async fn test_next_work_item_number() {
        let (pool, _temp_dir) = setup_test_db().await;
        create_test_project(&pool, "proj-3").await;

        // First item should be 1
        assert_eq!(next_work_item_number(&pool, "proj-3").await.unwrap(), 1);

        // Add some items
        for i in 1..=5 {
            let item = WorkItemRecord::new("proj-3", i, format!("Work Item {}", i));
            insert_work_item(&pool, &item).await.unwrap();
        }

        // Next should be 6
        assert_eq!(next_work_item_number(&pool, "proj-3").await.unwrap(), 6);
    }

    #[tokio::test]
    async fn test_list_work_items_by_state() {
        let (pool, _temp_dir) = setup_test_db().await;
        create_test_project(&pool, "proj-4").await;

        // Create open and closed items
        for i in 1..=5 {
            let mut item = WorkItemRecord::new("proj-4", i, format!("Work Item {}", i));
            if i % 2 == 0 {
                item.state = "closed".to_string();
            }
            insert_work_item(&pool, &item).await.unwrap();
        }

        let open = list_work_items(&pool, "proj-4", Some("open"), 100, 0).await.unwrap();
        let closed = list_work_items(&pool, "proj-4", Some("closed"), 100, 0).await.unwrap();
        let all = list_work_items(&pool, "proj-4", None, 100, 0).await.unwrap();

        assert_eq!(open.len(), 3);
        assert_eq!(closed.len(), 2);
        assert_eq!(all.len(), 5);
    }

    #[tokio::test]
    async fn test_labels_and_assignees() {
        let (pool, _temp_dir) = setup_test_db().await;
        create_test_project(&pool, "proj-5").await;

        let mut item = WorkItemRecord::new("proj-5", 1, "Labeled Item");
        item.set_labels(vec!["bug".to_string(), "priority".to_string()]);
        item.set_assignees(vec!["alice".to_string(), "bob".to_string()]);
        insert_work_item(&pool, &item).await.unwrap();

        let retrieved = get_work_item(&pool, &item.id).await.unwrap().unwrap();
        assert_eq!(retrieved.labels_vec(), vec!["bug", "priority"]);
        assert_eq!(retrieved.assignees_vec(), vec!["alice", "bob"]);
    }

    #[tokio::test]
    async fn test_item_type_and_hierarchy() {
        let (pool, _temp_dir) = setup_test_db().await;
        create_test_project(&pool, "proj-6").await;

        // Create an epic
        let epic = WorkItemRecord::new("proj-6", 1, "Epic")
            .with_item_type("epic")
            .with_epic_color("4f46e5");
        insert_work_item(&pool, &epic).await.unwrap();

        // Create a task under the epic
        let task = WorkItemRecord::new("proj-6", 2, "Task")
            .with_item_type("task")
            .with_parent(epic.id.clone());
        insert_work_item(&pool, &task).await.unwrap();

        // Verify epic
        let retrieved_epic = get_work_item(&pool, &epic.id).await.unwrap().unwrap();
        assert!(retrieved_epic.is_epic());
        assert_eq!(retrieved_epic.epic_color, Some("4f46e5".to_string()));

        // Verify child
        let children = get_child_work_items(&pool, &epic.id).await.unwrap();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].title, "Task");
    }
}
