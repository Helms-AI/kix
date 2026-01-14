//! Issue sync state entity for SeaORM.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// IssueSyncState entity representing the `issue_sync_state` table.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "issue_sync_state")]
pub struct Model {
    /// Issue ID (PK, FK to issues)
    #[sea_orm(primary_key, auto_increment = false)]
    pub issue_id: String,

    /// SHA-256 hash of canonical issue content
    pub content_hash: String,

    /// Content hash at last successful sync (merge base)
    pub last_sync_hash: String,

    /// Incremented on each local modification
    pub local_version: i64,

    /// GitHub updated_at as Unix timestamp
    pub github_version: Option<i64>,

    /// GitHub ETag for efficient change detection
    pub github_etag: Option<String>,

    /// Sync status: 'synced', 'local_ahead', 'github_ahead', 'conflict', 'pending'
    pub sync_status: String,

    /// RFC3339 timestamp of last sync
    pub last_sync_at: String,

    /// Sync direction: 'push', 'pull', 'merge', null for new
    pub last_sync_direction: Option<String>,

    /// JSON: {pushed_fields, pulled_fields, merged_fields}
    pub last_sync_result: Option<String>,

    /// Number of times this issue had conflicts
    pub conflict_count: Option<i64>,

    /// Creation timestamp (RFC3339)
    pub created_at: String,

    /// Last update timestamp (RFC3339)
    pub updated_at: String,
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
}

impl Related<super::issue::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Issue.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Check if synced.
    pub fn is_synced(&self) -> bool {
        self.sync_status == "synced"
    }

    /// Check if local is ahead.
    pub fn is_local_ahead(&self) -> bool {
        self.sync_status == "local_ahead"
    }

    /// Check if GitHub is ahead.
    pub fn is_github_ahead(&self) -> bool {
        self.sync_status == "github_ahead"
    }

    /// Check if in conflict.
    pub fn is_conflict(&self) -> bool {
        self.sync_status == "conflict"
    }

    /// Check if pending initial sync.
    pub fn is_pending(&self) -> bool {
        self.sync_status == "pending"
    }
}
