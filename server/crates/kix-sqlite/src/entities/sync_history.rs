//! Sync history entity for SeaORM.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// SyncHistory entity representing the `sync_history` table.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "sync_history")]
pub struct Model {
    /// Unique sync history identifier (UUID)
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,

    /// FK to project
    pub project_id: String,

    /// Sync type: 'full', 'incremental', 'single_issue'
    pub sync_type: String,

    /// Direction: 'pull', 'push', 'bidirectional'
    pub direction: String,

    /// Start timestamp (RFC3339)
    pub started_at: String,

    /// Completion timestamp (RFC3339)
    pub completed_at: Option<String>,

    /// Status: 'running', 'success', 'partial', 'failed'
    pub status: String,

    /// JSON: sync statistics
    pub stats: Option<String>,

    /// Number of issues pulled from GitHub
    pub issues_pulled: Option<i64>,

    /// Number of issues pushed to GitHub
    pub issues_pushed: Option<i64>,

    /// Number of issues merged
    pub issues_merged: Option<i64>,

    /// Number of conflicts resolved
    pub conflicts_resolved: Option<i64>,

    /// Number of conflicts that failed resolution
    pub conflicts_failed: Option<i64>,

    /// JSON array of error messages
    pub errors: Option<String>,
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
}

impl Related<super::project::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Project.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Check if sync is running.
    pub fn is_running(&self) -> bool {
        self.status == "running"
    }

    /// Check if sync succeeded.
    pub fn is_success(&self) -> bool {
        self.status == "success"
    }

    /// Check if sync partially succeeded.
    pub fn is_partial(&self) -> bool {
        self.status == "partial"
    }

    /// Check if sync failed.
    pub fn is_failed(&self) -> bool {
        self.status == "failed"
    }

    /// Get errors as Vec<String>.
    pub fn errors_vec(&self) -> Vec<String> {
        self.errors
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default()
    }
}
