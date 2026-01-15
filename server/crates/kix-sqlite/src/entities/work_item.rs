//! Work item entity for SeaORM.
//!
//! Represents work items in the Kanban board: epics, stories, tasks, subtasks, bugs.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// Work item entity representing the `work_items` table.
///
/// Work items can be: epic, story, task, subtask, or bug.
/// They support hierarchical relationships (parent/child).
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "work_items")]
pub struct Model {
    /// Unique work item identifier (UUID)
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,

    /// FK to parent project
    pub project_id: String,

    /// Local work item number (within project)
    pub number: i64,

    /// Work item title
    pub title: String,

    /// Work item body (markdown)
    #[sea_orm(column_type = "Text")]
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
    #[sea_orm(column_name = "item_type", default_value = "task")]
    pub item_type: String,

    /// Parent work item ID (for hierarchy)
    pub parent_id: Option<String>,

    /// Position within board column (for drag-drop ordering)
    #[sea_orm(default_value = "0")]
    pub position: i64,

    /// Board column: 'backlog', 'todo', 'in_progress', 'in_review', 'testing', 'done'
    #[sea_orm(default_value = "backlog")]
    pub board_column: String,

    /// Story points for estimation
    pub story_points: Option<i64>,

    /// Epic color (hex without #, e.g., "4f46e5")
    pub epic_color: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::project::Entity",
        from = "Column::ProjectId",
        to = "super::project::Column::Id",
        on_delete = "Cascade"
    )]
    Project,
    // Self-referential relation for parent-child hierarchy
    #[sea_orm(
        belongs_to = "Entity",
        from = "Column::ParentId",
        to = "Column::Id",
        on_delete = "SetNull"
    )]
    Parent,
}

impl Related<super::project::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Project.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Get labels as Vec<String>.
    pub fn labels_vec(&self) -> Vec<String> {
        self.labels
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default()
    }

    /// Get assignees as Vec<String>.
    pub fn assignees_vec(&self) -> Vec<String> {
        self.assignees
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default()
    }

    /// Check if work item is open.
    pub fn is_open(&self) -> bool {
        self.state == "open"
    }

    /// Check if this is an epic.
    pub fn is_epic(&self) -> bool {
        self.item_type == "epic"
    }

    /// Check if this work item has a parent.
    pub fn has_parent(&self) -> bool {
        self.parent_id.is_some()
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
