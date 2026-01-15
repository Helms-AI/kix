//! Project entity for SeaORM.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// Project entity representing the `projects` table.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "projects")]
pub struct Model {
    /// Unique project identifier (UUID)
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,

    /// Display name
    #[sea_orm(unique)]
    pub name: String,

    /// URL-friendly identifier
    #[sea_orm(unique)]
    pub slug: String,

    /// Project description
    pub description: Option<String>,

    /// Hex color for UI (e.g., "#3b82f6")
    pub color: Option<String>,

    /// Whether the project is archived (0 or 1)
    pub archived: i64,

    /// Creation timestamp (RFC3339)
    pub created_at: String,

    /// Last update timestamp (RFC3339)
    pub updated_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::work_item::Entity")]
    WorkItems,
    #[sea_orm(has_many = "super::project_entry::Entity")]
    ProjectEntries,
}

impl Related<super::work_item::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::WorkItems.def()
    }
}

impl Related<super::project_entry::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ProjectEntries.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Check if project is archived.
    pub fn is_archived(&self) -> bool {
        self.archived != 0
    }
}
