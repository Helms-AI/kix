//! Issue entity for SeaORM.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// Issue entity representing the `issues` table.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "issues")]
pub struct Model {
    /// Unique issue identifier (UUID)
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,

    /// FK to parent project
    pub project_id: String,

    /// Local issue number (within project)
    pub number: i64,

    /// Issue title
    pub title: String,

    /// Issue body (markdown)
    #[sea_orm(column_type = "Text")]
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

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::project::Entity",
        from = "Column::ProjectId",
        to = "super::project::Column::Id",
        on_delete = "Cascade"
    )]
    Project,
    #[sea_orm(has_one = "super::sync_state::Entity")]
    SyncState,
    #[sea_orm(has_one = "super::sync_conflict::Entity")]
    SyncConflict,
}

impl Related<super::project::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Project.def()
    }
}

impl Related<super::sync_state::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SyncState.def()
    }
}

impl Related<super::sync_conflict::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SyncConflict.def()
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

    /// Check if issue is open.
    pub fn is_open(&self) -> bool {
        self.state == "open"
    }

    /// Check if issue is from GitHub.
    pub fn is_github(&self) -> bool {
        self.source == "github"
    }
}
