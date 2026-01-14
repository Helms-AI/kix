//! Project-Entry link entity for SeaORM.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// ProjectEntry entity representing the `project_entries` link table.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "project_entries")]
pub struct Model {
    /// Unique link identifier (UUID)
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,

    /// FK to project
    pub project_id: String,

    /// FK to entry
    pub entry_id: String,

    /// Relevance score (0.0 to 1.0)
    pub relevance: Option<f64>,

    /// Additional notes about the link
    pub notes: Option<String>,

    /// Link creation timestamp (RFC3339)
    pub linked_at: String,
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
    #[sea_orm(
        belongs_to = "super::entry::Entity",
        from = "Column::EntryId",
        to = "super::entry::Column::Id",
        on_delete = "Cascade"
    )]
    Entry,
}

impl Related<super::project::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Project.def()
    }
}

impl Related<super::entry::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Entry.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
