//! Job entity for SeaORM.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// Job entity representing the `jobs` table.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "jobs")]
pub struct Model {
    /// Unique job identifier (UUID)
    #[sea_orm(primary_key, auto_increment = false)]
    pub job_id: String,

    /// Job type: "url", "file_upload", or "reindex"
    pub job_type: String,

    /// Status: "completed", "failed", or "cancelled"
    pub status: String,

    /// Creation timestamp (RFC3339)
    pub created_at: String,

    /// Start timestamp (RFC3339)
    pub started_at: Option<String>,

    /// Completion timestamp (RFC3339)
    pub completed_at: String,

    /// Source URL for URL jobs
    pub source_url: Option<String>,

    /// Source domain
    pub source_domain: Option<String>,

    /// Job configuration as JSON
    #[sea_orm(column_type = "Text")]
    pub config: String,

    /// Number of items processed
    pub items_processed: i64,

    /// Number of items discovered
    pub items_discovered: i64,

    /// Number of chunks created
    pub chunks_created: i64,

    /// Number of embeddings generated
    pub embeddings_generated: i64,

    /// Number of errors
    pub error_count: i64,

    /// Duration in milliseconds
    pub duration_ms: i64,

    /// Processing rate (items/second)
    pub processing_rate: f64,

    /// Error messages as JSON array
    pub errors: Option<String>,

    /// Code extraction statistics as JSON
    /// Contains: total_code_blocks, pages_with_code, languages, patterns_matched, validation
    #[sea_orm(column_type = "Text", nullable)]
    pub code_extraction_stats: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::job_item::Entity")]
    JobItems,
}

impl Related<super::job_item::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::JobItems.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Check if job is completed.
    pub fn is_completed(&self) -> bool {
        self.status == "completed"
    }

    /// Check if job failed.
    pub fn is_failed(&self) -> bool {
        self.status == "failed"
    }

    /// Check if job is cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.status == "cancelled"
    }

    /// Get errors as Vec<String>.
    pub fn errors_vec(&self) -> Vec<String> {
        self.errors
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default()
    }
}
