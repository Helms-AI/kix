//! GitHub token entity for SeaORM.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// GithubToken entity representing the `github_tokens` table.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "github_tokens")]
pub struct Model {
    /// Scope: "global" or "project:{uuid}" - serves as primary key
    #[sea_orm(primary_key, auto_increment = false)]
    pub scope: String,

    /// Encrypted token data as JSON (ciphertext + nonce)
    pub encrypted_token: String,

    /// Last 4 characters (for display only)
    pub token_suffix: Option<String>,

    /// Token format prefix (ghp_, gho_, github_pat_)
    pub token_format: Option<String>,

    /// Last verification timestamp (RFC3339)
    pub verified_at: Option<String>,

    /// GitHub username when verified
    pub verified_user: Option<String>,

    /// Creation timestamp (RFC3339)
    pub created_at: String,

    /// Last update timestamp (RFC3339)
    pub updated_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Check if this is a global token.
    pub fn is_global(&self) -> bool {
        self.scope == "global"
    }

    /// Check if this is a project-scoped token.
    pub fn is_project_scoped(&self) -> bool {
        self.scope.starts_with("project:")
    }

    /// Get the project ID if this is a project-scoped token.
    pub fn project_id(&self) -> Option<&str> {
        self.scope.strip_prefix("project:")
    }
}
