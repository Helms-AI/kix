//! Page entity for SeaORM.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// Page entity representing the `pages` table.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "pages")]
pub struct Model {
    /// Unique page identifier (UUID)
    #[sea_orm(primary_key, auto_increment = false)]
    pub page_id: String,

    /// FK to the source entry
    pub source_id: String,

    /// Original URL
    pub url: String,

    /// Page title
    pub title: Option<String>,

    /// Full markdown content
    #[sea_orm(column_type = "Text")]
    pub full_content: String,

    /// SHA256 hash for deduplication
    pub content_hash: String,

    /// Content length in characters
    pub content_length: i64,

    /// Number of code blocks
    pub code_block_count: i64,

    /// Additional metadata as JSON
    pub metadata: Option<String>,

    /// Crawl time in milliseconds
    pub crawl_time_ms: Option<i64>,

    /// Creation timestamp (RFC3339)
    pub created_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::entry::Entity",
        from = "Column::SourceId",
        to = "super::entry::Column::Id",
        on_delete = "Cascade"
    )]
    Entry,
}

impl Related<super::entry::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Entry.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Count code blocks in content using ``` markers.
    pub fn code_block_count_actual(&self) -> usize {
        self.full_content.matches("```").count() / 2
    }
}
