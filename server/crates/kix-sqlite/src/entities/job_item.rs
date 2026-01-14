//! Job item entity for SeaORM.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// JobItem entity representing the `job_items` table.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "job_items")]
pub struct Model {
    /// Unique item identifier (UUID)
    #[sea_orm(primary_key, auto_increment = false)]
    pub item_id: String,

    /// FK to jobs table
    pub job_id: String,

    /// URL or file path
    pub item_path: String,

    /// Item type: "page" or "file"
    pub item_type: String,

    /// Status: "completed", "error", or "skipped"
    pub status: String,

    /// Parent URL (for crawled pages)
    pub parent_url: Option<String>,

    /// Crawl depth
    pub depth: i64,

    /// Discovery timestamp (RFC3339)
    pub discovered_at: String,

    /// Start timestamp (RFC3339)
    pub started_at: Option<String>,

    /// Completion timestamp (RFC3339)
    pub completed_at: Option<String>,

    /// Number of chunks created
    pub chunks_created: i64,

    /// Number of embeddings generated
    pub embeddings_generated: i64,

    /// Duration in milliseconds
    pub duration_ms: i64,

    /// Error message if failed
    pub error_message: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::job::Entity",
        from = "Column::JobId",
        to = "super::job::Column::JobId",
        on_delete = "Cascade"
    )]
    Job,
}

impl Related<super::job::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Job.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Check if item is completed.
    pub fn is_completed(&self) -> bool {
        self.status == "completed"
    }

    /// Check if item has error.
    pub fn is_error(&self) -> bool {
        self.status == "error"
    }

    /// Check if item was skipped.
    pub fn is_skipped(&self) -> bool {
        self.status == "skipped"
    }
}
