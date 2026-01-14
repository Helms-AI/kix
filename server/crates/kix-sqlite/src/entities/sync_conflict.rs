//! Sync conflict entity for SeaORM.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// SyncConflict entity representing the `sync_conflicts` table.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "sync_conflicts")]
pub struct Model {
    /// Unique conflict identifier (UUID)
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,

    /// FK to issue (unique - only one active conflict per issue)
    #[sea_orm(unique)]
    pub issue_id: String,

    /// FK to project
    pub project_id: String,

    /// Detection timestamp (RFC3339)
    pub detected_at: String,

    /// Conflict type: 'content', 'state', 'deletion'
    pub conflict_type: String,

    /// JSON: local issue state snapshot
    #[sea_orm(column_type = "Text")]
    pub local_snapshot: String,

    /// JSON: GitHub issue state snapshot
    #[sea_orm(column_type = "Text")]
    pub github_snapshot: String,

    /// JSON: last known synchronized state
    #[sea_orm(column_type = "Text")]
    pub last_common_snapshot: Option<String>,

    /// JSON array: affected fields ['title', 'body', etc.]
    pub affected_fields: String,

    /// Whether auto resolution was attempted
    pub auto_resolution_attempted: Option<i64>,

    /// JSON: attempted resolution details
    #[sea_orm(column_type = "Text")]
    pub auto_resolution_result: Option<String>,

    /// JSON: user's chosen resolution
    #[sea_orm(column_type = "Text")]
    pub manual_resolution: Option<String>,

    /// Resolution timestamp (RFC3339)
    pub resolved_at: Option<String>,

    /// User ID or 'auto'
    pub resolved_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::issue::Entity",
        from = "Column::IssueId",
        to = "super::issue::Column::Id",
        on_delete = "Cascade"
    )]
    Issue,
    #[sea_orm(
        belongs_to = "super::project::Entity",
        from = "Column::ProjectId",
        to = "super::project::Column::Id",
        on_delete = "Cascade"
    )]
    Project,
}

impl Related<super::issue::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Issue.def()
    }
}

impl Related<super::project::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Project.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Check if conflict is resolved.
    pub fn is_resolved(&self) -> bool {
        self.resolved_at.is_some()
    }

    /// Check if conflict is a content conflict.
    pub fn is_content_conflict(&self) -> bool {
        self.conflict_type == "content"
    }

    /// Check if conflict is a state conflict.
    pub fn is_state_conflict(&self) -> bool {
        self.conflict_type == "state"
    }

    /// Check if conflict is a deletion conflict.
    pub fn is_deletion_conflict(&self) -> bool {
        self.conflict_type == "deletion"
    }

    /// Get affected fields as Vec<String>.
    pub fn affected_fields_vec(&self) -> Vec<String> {
        serde_json::from_str(&self.affected_fields).unwrap_or_default()
    }

    /// Check if auto resolution was attempted.
    pub fn auto_resolution_was_attempted(&self) -> bool {
        self.auto_resolution_attempted.unwrap_or(0) > 0
    }
}
