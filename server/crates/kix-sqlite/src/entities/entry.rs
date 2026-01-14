//! Entry entity for SeaORM.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// Entry entity representing the `entries` table.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "entries")]
pub struct Model {
    /// Unique entry identifier (UUID)
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,

    /// Entry title
    pub title: String,

    /// Short description/summary
    pub description: Option<String>,

    /// Full content text
    #[sea_orm(column_type = "Text")]
    pub content: Option<String>,

    /// Tags as JSON array string
    pub tags: Option<String>,

    /// Collection IDs as JSON array string
    pub collection_ids: Option<String>,

    /// Entry type (document, article, pdf, code, etc.)
    pub entry_type: String,

    /// Source type (url, file_upload, direct_input)
    pub source_type: String,

    /// Original file/URL path
    pub source_path: String,

    /// Domain for filtering (e.g., "docs.example.com")
    pub source_domain: Option<String>,

    /// URL-friendly identifier
    pub slug: String,

    /// Content hash for deduplication
    pub source_hash: String,

    /// Creation timestamp (RFC3339)
    pub created_at: String,

    /// Last update timestamp (RFC3339)
    pub updated_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::page::Entity")]
    Pages,
    #[sea_orm(has_many = "super::project_entry::Entity")]
    ProjectEntries,
}

impl Related<super::page::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Pages.def()
    }
}

impl Related<super::project_entry::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ProjectEntries.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Get tags as Vec<String>.
    pub fn tags_vec(&self) -> Vec<String> {
        self.tags
            .as_ref()
            .and_then(|t| serde_json::from_str(t).ok())
            .unwrap_or_default()
    }

    /// Get collection IDs as Vec<String>.
    pub fn collection_ids_vec(&self) -> Vec<String> {
        self.collection_ids
            .as_ref()
            .and_then(|t| serde_json::from_str(t).ok())
            .unwrap_or_default()
    }
}
