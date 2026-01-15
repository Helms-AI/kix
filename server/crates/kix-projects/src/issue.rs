//! Work item data model and parameters.
//!
//! Note: This file is named issue.rs for backward compatibility,
//! but represents work items (epic, story, task, subtask, bug).

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::project::{BoardColumn, IssueType};

/// Issue state (open or closed).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum IssueState {
    #[default]
    Open,
    Closed,
}

impl IssueState {
    /// Convert from string representation.
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "open" => Some(IssueState::Open),
            "closed" => Some(IssueState::Closed),
            _ => None,
        }
    }

    /// Convert to string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            IssueState::Open => "open",
            IssueState::Closed => "closed",
        }
    }
}

impl std::fmt::Display for IssueState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A work item in a project.
///
/// Work items support a hierarchy:
/// - Epic → Story, Bug, Task
/// - Story → Task, Subtask
/// - Task → Subtask
/// - Bug → Subtask
/// - Subtask → (none)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Issue {
    /// Unique identifier (UUID)
    pub id: String,

    /// Parent project ID
    pub project_id: String,

    /// Work item number within project (auto-incremented)
    pub number: u32,

    /// Work item title
    pub title: String,

    /// Work item body (markdown)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,

    /// Work item state (open or closed)
    pub state: IssueState,

    /// Labels
    #[serde(default)]
    pub labels: Vec<String>,

    /// Priority (1-5, 1=highest)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<u8>,

    /// Assignees (usernames)
    #[serde(default)]
    pub assignees: Vec<String>,

    /// Created timestamp
    pub created_at: DateTime<Utc>,

    /// Updated timestamp
    pub updated_at: DateTime<Utc>,

    /// Closed timestamp (if closed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub closed_at: Option<DateTime<Utc>>,

    // =========================================================================
    // Board/Hierarchy fields
    // =========================================================================

    /// Work item type (epic, story, task, subtask, bug)
    #[serde(default)]
    pub issue_type: IssueType,

    /// Parent work item ID (for hierarchy)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,

    /// Position within board column (for drag-drop ordering)
    #[serde(default)]
    pub position: i64,

    /// Board column (backlog, todo, in_progress, in_review, testing, done)
    #[serde(default)]
    pub board_column: BoardColumn,

    /// Story points for estimation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub story_points: Option<u32>,

    /// Epic color (hex without #, e.g., "4f46e5")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub epic_color: Option<String>,
}

impl Issue {
    /// Create a new work item.
    pub fn new(project_id: String, number: u32, title: String) -> Self {
        let now = Utc::now();

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            project_id,
            number,
            title,
            body: None,
            state: IssueState::Open,
            labels: Vec::new(),
            priority: None,
            assignees: Vec::new(),
            created_at: now,
            updated_at: now,
            closed_at: None,
            issue_type: IssueType::Task,
            parent_id: None,
            position: 0,
            board_column: BoardColumn::Backlog,
            story_points: None,
            epic_color: None,
        }
    }

    /// Create a new epic.
    pub fn new_epic(project_id: String, number: u32, title: String, color: Option<String>) -> Self {
        let mut issue = Self::new(project_id, number, title);
        issue.issue_type = IssueType::Epic;
        issue.epic_color = color;
        issue
    }

    /// Check if this is an epic.
    pub fn is_epic(&self) -> bool {
        self.issue_type == IssueType::Epic
    }

    /// Check if this work item can contain a child of the given type.
    pub fn can_contain(&self, child_type: IssueType) -> bool {
        self.issue_type.can_contain(child_type)
    }

    /// Close the work item.
    pub fn close(&mut self) {
        self.state = IssueState::Closed;
        self.closed_at = Some(Utc::now());
        self.updated_at = Utc::now();
    }

    /// Reopen the work item.
    pub fn reopen(&mut self) {
        self.state = IssueState::Open;
        self.closed_at = None;
        self.updated_at = Utc::now();
    }

    /// Move to a different board column.
    pub fn move_to_column(&mut self, column: BoardColumn, position: i64) {
        self.board_column = column;
        self.position = position;
        self.updated_at = Utc::now();
    }

    /// Set parent work item.
    pub fn set_parent(&mut self, parent_id: Option<String>) {
        self.parent_id = parent_id;
        self.updated_at = Utc::now();
    }

    /// Set with body (builder pattern).
    pub fn with_body(mut self, body: impl Into<String>) -> Self {
        self.body = Some(body.into());
        self
    }

    /// Set with issue type (builder pattern).
    pub fn with_type(mut self, issue_type: IssueType) -> Self {
        self.issue_type = issue_type;
        self
    }

    /// Set with labels (builder pattern).
    pub fn with_labels(mut self, labels: Vec<String>) -> Self {
        self.labels = labels;
        self
    }

    /// Set with parent (builder pattern).
    pub fn with_parent(mut self, parent_id: impl Into<String>) -> Self {
        self.parent_id = Some(parent_id.into());
        self
    }

    /// Set with story points (builder pattern).
    pub fn with_story_points(mut self, points: u32) -> Self {
        self.story_points = Some(points);
        self
    }
}

/// Parameters for creating a work item.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct CreateIssueParams {
    /// Work item title
    pub title: String,

    /// Work item body (markdown)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,

    /// Work item type (default: task)
    #[serde(default)]
    pub issue_type: IssueType,

    /// Parent work item ID (for hierarchy)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,

    /// Labels to add
    #[serde(default)]
    pub labels: Vec<String>,

    /// Priority (1-5, 1=highest)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<u8>,

    /// Board column (default: backlog)
    #[serde(default)]
    pub board_column: BoardColumn,

    /// Story points for estimation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub story_points: Option<u32>,

    /// Epic color (for epics only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub epic_color: Option<String>,

    /// Assignees (usernames)
    #[serde(default)]
    pub assignees: Vec<String>,
}

/// Parameters for updating a work item.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct UpdateIssueParams {
    /// New title
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// New body
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,

    /// New state
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<IssueState>,

    /// New issue type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issue_type: Option<IssueType>,

    /// New parent ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,

    /// New labels (replaces existing)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<Vec<String>>,

    /// New priority
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<u8>,

    /// New assignees
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignees: Option<Vec<String>>,

    /// New board column
    #[serde(skip_serializing_if = "Option::is_none")]
    pub board_column: Option<BoardColumn>,

    /// New story points
    #[serde(skip_serializing_if = "Option::is_none")]
    pub story_points: Option<u32>,

    /// New epic color
    #[serde(skip_serializing_if = "Option::is_none")]
    pub epic_color: Option<String>,
}

/// Filters for listing work items.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
pub struct IssueFilters {
    /// Filter by state
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<IssueState>,

    /// Filter by issue type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issue_type: Option<IssueType>,

    /// Filter by parent ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,

    /// Filter by labels (any match)
    #[serde(default)]
    pub labels: Vec<String>,

    /// Filter by priority
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<u8>,

    /// Filter by assignee
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,

    /// Filter by board column
    #[serde(skip_serializing_if = "Option::is_none")]
    pub board_column: Option<BoardColumn>,

    /// Maximum results
    #[serde(default = "default_limit")]
    pub limit: usize,

    /// Offset for pagination
    #[serde(default)]
    pub offset: usize,
}

fn default_limit() -> usize {
    50
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_issue_new() {
        let issue = Issue::new(
            "proj_123".to_string(),
            1,
            "Test Issue".to_string(),
        );

        assert!(!issue.id.is_empty());
        assert_eq!(issue.project_id, "proj_123");
        assert_eq!(issue.number, 1);
        assert_eq!(issue.title, "Test Issue");
        assert_eq!(issue.state, IssueState::Open);
        assert_eq!(issue.issue_type, IssueType::Task);
        assert_eq!(issue.board_column, BoardColumn::Backlog);
    }

    #[test]
    fn test_issue_close_reopen() {
        let mut issue = Issue::new(
            "proj_123".to_string(),
            1,
            "Test".to_string(),
        );

        assert_eq!(issue.state, IssueState::Open);
        assert!(issue.closed_at.is_none());

        issue.close();
        assert_eq!(issue.state, IssueState::Closed);
        assert!(issue.closed_at.is_some());

        issue.reopen();
        assert_eq!(issue.state, IssueState::Open);
        assert!(issue.closed_at.is_none());
    }

    #[test]
    fn test_epic_creation() {
        let epic = Issue::new_epic(
            "proj_123".to_string(),
            1,
            "Auth Epic".to_string(),
            Some("4f46e5".to_string()),
        );

        assert!(epic.is_epic());
        assert_eq!(epic.issue_type, IssueType::Epic);
        assert_eq!(epic.epic_color, Some("4f46e5".to_string()));
    }

    #[test]
    fn test_issue_hierarchy() {
        let epic = Issue::new_epic(
            "proj_123".to_string(),
            1,
            "Epic".to_string(),
            None,
        );

        // Epic can contain Story, Bug, Task
        assert!(epic.can_contain(IssueType::Story));
        assert!(epic.can_contain(IssueType::Bug));
        assert!(epic.can_contain(IssueType::Task));
        assert!(!epic.can_contain(IssueType::Subtask));

        // Task can only contain Subtask
        let task = Issue::new("proj_123".to_string(), 2, "Task".to_string());
        assert!(task.can_contain(IssueType::Subtask));
        assert!(!task.can_contain(IssueType::Task));
    }

    #[test]
    fn test_move_to_column() {
        let mut issue = Issue::new(
            "proj_123".to_string(),
            1,
            "Test".to_string(),
        );

        assert_eq!(issue.board_column, BoardColumn::Backlog);
        assert_eq!(issue.position, 0);

        issue.move_to_column(BoardColumn::InProgress, 5);
        assert_eq!(issue.board_column, BoardColumn::InProgress);
        assert_eq!(issue.position, 5);
    }

    #[test]
    fn test_issue_state_from_str() {
        assert_eq!(IssueState::from_str("open"), Some(IssueState::Open));
        assert_eq!(IssueState::from_str("CLOSED"), Some(IssueState::Closed));
        assert_eq!(IssueState::from_str("unknown"), None);
    }
}
