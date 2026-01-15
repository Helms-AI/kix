//! Project data model and storage operations.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A project is a user-created workspace for organizing knowledge and work items.
///
/// Projects are local-only containers for work items with Kanban board functionality.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Project {
    /// Unique identifier (UUID)
    pub id: String,

    /// Human-readable name (unique, used for reference)
    pub name: String,

    /// URL-safe slug (auto-generated from name)
    pub slug: String,

    /// Short description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Project color for UI (hex without #, e.g., "4f46e5")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,

    /// Whether project is archived
    #[serde(default)]
    pub archived: bool,

    /// Cached counts for quick display
    #[serde(default)]
    pub stats: ProjectStats,

    /// Created timestamp
    pub created_at: DateTime<Utc>,

    /// Updated timestamp
    pub updated_at: DateTime<Utc>,
}

/// Issue type for hierarchy enforcement.
///
/// Hierarchy rules (flexible - any type can be created independently):
/// - Epic → Story, Bug, Task
/// - Story → Task, Subtask
/// - Task → Subtask
/// - Bug → Subtask
/// - Subtask → (none)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum IssueType {
    /// Top-level container for related work
    Epic,
    /// User story or feature request
    Story,
    /// Implementation task
    #[default]
    Task,
    /// Sub-task under a task or story
    Subtask,
    /// Bug report or defect
    Bug,
}

impl IssueType {
    /// Check if this issue type can contain a child of the given type.
    pub fn can_contain(&self, child: IssueType) -> bool {
        match self {
            IssueType::Epic => matches!(child, IssueType::Story | IssueType::Bug | IssueType::Task),
            IssueType::Story => matches!(child, IssueType::Task | IssueType::Subtask),
            IssueType::Task => matches!(child, IssueType::Subtask),
            IssueType::Bug => matches!(child, IssueType::Subtask),
            IssueType::Subtask => false,
        }
    }

    /// Get all valid child types for this issue type.
    pub fn valid_children(&self) -> Vec<IssueType> {
        match self {
            IssueType::Epic => vec![IssueType::Story, IssueType::Bug, IssueType::Task],
            IssueType::Story => vec![IssueType::Task, IssueType::Subtask],
            IssueType::Task => vec![IssueType::Subtask],
            IssueType::Bug => vec![IssueType::Subtask],
            IssueType::Subtask => vec![],
        }
    }

    /// Get all valid parent types for this issue type.
    pub fn valid_parents(&self) -> Vec<IssueType> {
        match self {
            IssueType::Epic => vec![], // Epics have no parents
            IssueType::Story => vec![IssueType::Epic],
            IssueType::Task => vec![IssueType::Epic, IssueType::Story],
            IssueType::Subtask => vec![IssueType::Story, IssueType::Task, IssueType::Bug],
            IssueType::Bug => vec![IssueType::Epic],
        }
    }

    /// Convert from string representation.
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "epic" => Some(IssueType::Epic),
            "story" => Some(IssueType::Story),
            "task" => Some(IssueType::Task),
            "subtask" => Some(IssueType::Subtask),
            "bug" => Some(IssueType::Bug),
            _ => None,
        }
    }

    /// Convert to string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            IssueType::Epic => "epic",
            IssueType::Story => "story",
            IssueType::Task => "task",
            IssueType::Subtask => "subtask",
            IssueType::Bug => "bug",
        }
    }
}

impl std::fmt::Display for IssueType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Board column representing workflow state.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum BoardColumn {
    #[default]
    Backlog,
    Todo,
    InProgress,
    InReview,
    Testing,
    Done,
}

impl BoardColumn {
    /// All columns in workflow order.
    pub fn all() -> Vec<BoardColumn> {
        vec![
            BoardColumn::Backlog,
            BoardColumn::Todo,
            BoardColumn::InProgress,
            BoardColumn::InReview,
            BoardColumn::Testing,
            BoardColumn::Done,
        ]
    }

    /// Convert from string representation.
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "backlog" => Some(BoardColumn::Backlog),
            "todo" => Some(BoardColumn::Todo),
            "in_progress" => Some(BoardColumn::InProgress),
            "in_review" => Some(BoardColumn::InReview),
            "testing" => Some(BoardColumn::Testing),
            "done" => Some(BoardColumn::Done),
            _ => None,
        }
    }

    /// Convert to string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            BoardColumn::Backlog => "backlog",
            BoardColumn::Todo => "todo",
            BoardColumn::InProgress => "in_progress",
            BoardColumn::InReview => "in_review",
            BoardColumn::Testing => "testing",
            BoardColumn::Done => "done",
        }
    }

    /// Get display name for the column.
    pub fn display_name(&self) -> &'static str {
        match self {
            BoardColumn::Backlog => "Backlog",
            BoardColumn::Todo => "To Do",
            BoardColumn::InProgress => "In Progress",
            BoardColumn::InReview => "In Review",
            BoardColumn::Testing => "Testing",
            BoardColumn::Done => "Done",
        }
    }
}

impl std::fmt::Display for BoardColumn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Cached project statistics for quick display.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ProjectStats {
    /// Number of open issues
    #[serde(default)]
    pub open_issues: u32,

    /// Number of closed issues
    #[serde(default)]
    pub closed_issues: u32,

    /// Number of linked knowledge entries
    #[serde(default)]
    pub entry_count: u32,
}

impl Project {
    /// Create a new project with required fields.
    pub fn new(name: String) -> Self {
        let slug = slugify(&name);
        let now = Utc::now();

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            slug,
            description: None,
            color: None,
            archived: false,
            stats: ProjectStats::default(),
            created_at: now,
            updated_at: now,
        }
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

/// Convert a string to a URL-safe slug.
fn slugify(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_new() {
        let project = Project::new("My Test Project".to_string());

        assert!(!project.id.is_empty());
        assert_eq!(project.name, "My Test Project");
        assert_eq!(project.slug, "my-test-project");
        assert!(!project.archived);
    }

    #[test]
    fn test_project_with_description() {
        let project = Project::new("Test".to_string())
            .with_description("A test project")
            .with_color("3b82f6");

        assert_eq!(project.description, Some("A test project".to_string()));
        assert_eq!(project.color, Some("3b82f6".to_string()));
    }

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("My Project"), "my-project");
        assert_eq!(slugify("Test  Project 123"), "test-project-123");
        assert_eq!(slugify("Special!@#$%^&*()"), "special");
        assert_eq!(slugify("   spaced   "), "spaced");
    }

    #[test]
    fn test_issue_type_hierarchy() {
        // Epic can contain Story, Bug, and Task
        assert!(IssueType::Epic.can_contain(IssueType::Story));
        assert!(IssueType::Epic.can_contain(IssueType::Bug));
        assert!(IssueType::Epic.can_contain(IssueType::Task));
        assert!(!IssueType::Epic.can_contain(IssueType::Subtask));

        // Story can contain Task and Subtask
        assert!(IssueType::Story.can_contain(IssueType::Task));
        assert!(IssueType::Story.can_contain(IssueType::Subtask));
        assert!(!IssueType::Story.can_contain(IssueType::Epic));
        assert!(!IssueType::Story.can_contain(IssueType::Bug));

        // Task can only contain Subtask
        assert!(IssueType::Task.can_contain(IssueType::Subtask));
        assert!(!IssueType::Task.can_contain(IssueType::Task));

        // Bug can only contain Subtask
        assert!(IssueType::Bug.can_contain(IssueType::Subtask));
        assert!(!IssueType::Bug.can_contain(IssueType::Task));

        // Subtask cannot contain anything
        assert!(!IssueType::Subtask.can_contain(IssueType::Subtask));
        assert!(!IssueType::Subtask.can_contain(IssueType::Task));
    }

    #[test]
    fn test_issue_type_from_str() {
        assert_eq!(IssueType::from_str("epic"), Some(IssueType::Epic));
        assert_eq!(IssueType::from_str("STORY"), Some(IssueType::Story));
        assert_eq!(IssueType::from_str("Task"), Some(IssueType::Task));
        assert_eq!(IssueType::from_str("subtask"), Some(IssueType::Subtask));
        assert_eq!(IssueType::from_str("bug"), Some(IssueType::Bug));
        assert_eq!(IssueType::from_str("unknown"), None);
    }

    #[test]
    fn test_board_column() {
        // All columns in order
        let columns = BoardColumn::all();
        assert_eq!(columns.len(), 6);
        assert_eq!(columns[0], BoardColumn::Backlog);
        assert_eq!(columns[5], BoardColumn::Done);

        // String conversion
        assert_eq!(BoardColumn::from_str("in_progress"), Some(BoardColumn::InProgress));
        assert_eq!(BoardColumn::InProgress.as_str(), "in_progress");
        assert_eq!(BoardColumn::InProgress.display_name(), "In Progress");
    }
}
