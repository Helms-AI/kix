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

    /// GitHub configuration as JSON
    pub github_config: String,

    /// Whether the project is archived (0 or 1)
    pub archived: i64,

    /// Creation timestamp (RFC3339)
    pub created_at: String,

    /// Last update timestamp (RFC3339)
    pub updated_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::issue::Entity")]
    Issues,
    #[sea_orm(has_many = "super::project_entry::Entity")]
    ProjectEntries,
    #[sea_orm(has_many = "super::sync_history::Entity")]
    SyncHistory,
    #[sea_orm(has_many = "super::sync_conflict::Entity")]
    SyncConflicts,
}

impl Related<super::issue::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Issues.def()
    }
}

impl Related<super::project_entry::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ProjectEntries.def()
    }
}

impl Related<super::sync_history::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SyncHistory.def()
    }
}

impl Related<super::sync_conflict::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SyncConflicts.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Get GitHub config as JSON value.
    pub fn github_config_json(&self) -> Option<serde_json::Value> {
        serde_json::from_str(&self.github_config).ok()
    }

    /// Get GitHub owner.
    pub fn github_owner(&self) -> Option<String> {
        self.github_config_json()?
            .get("owner")?
            .as_str()
            .map(String::from)
    }

    /// Get GitHub repo.
    pub fn github_repo(&self) -> Option<String> {
        self.github_config_json()?
            .get("repo")?
            .as_str()
            .map(String::from)
    }

    /// Check if project has GitHub integration.
    pub fn has_github(&self) -> bool {
        self.github_owner().is_some() && self.github_repo().is_some()
    }

    /// Check if project is archived.
    pub fn is_archived(&self) -> bool {
        self.archived != 0
    }
}
