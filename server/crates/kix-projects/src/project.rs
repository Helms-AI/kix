//! Project data model and storage operations.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A project is a user-created workspace for organizing knowledge and issues.
///
/// Projects require a GitHub repository connection for issues and project board functionality.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Project {
    /// Unique identifier (UUID)
    pub id: String,

    /// Human-readable name (unique, used for reference)
    pub name: String,

    /// URL-safe slug (auto-generated from name)
    pub slug: String,

    /// Short description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// GitHub repository configuration (required)
    pub github: GitHubConfig,

    /// Project color for UI (hex without #, e.g., "4f46e5")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,

    /// Whether project is archived
    #[serde(default)]
    pub archived: bool,

    /// Cached counts for quick display
    #[serde(default)]
    pub stats: ProjectStats,

    /// Created timestamp
    pub created_at: DateTime<Utc>,

    /// Updated timestamp
    pub updated_at: DateTime<Utc>,
}

/// GitHub repository configuration for a project.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitHubConfig {
    /// Repository owner (e.g., "anthropics")
    pub owner: String,

    /// Repository name (e.g., "claude-code")
    pub repo: String,

    /// Repository node ID (for GraphQL operations)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository_id: Option<String>,

    /// GitHub Project V2 configuration (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_v2: Option<GitHubProjectV2Config>,

    /// Sync settings
    #[serde(default)]
    pub sync: GitHubSyncConfig,

    /// Last successful sync timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_synced_at: Option<DateTime<Utc>>,

    /// Last sync error (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

/// GitHub Project V2 board configuration.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitHubProjectV2Config {
    /// GitHub Project node ID (e.g., "PVT_...")
    pub project_id: String,

    /// Project number (human-readable)
    pub project_number: u32,

    /// Project URL
    pub url: String,

    /// Status field ID for updating item status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_field_id: Option<String>,

    /// Status options mapped to field option IDs
    #[serde(default)]
    pub status_options: Vec<StatusOption>,

    /// Custom field IDs for additional fields (Priority, Sprint, etc.)
    #[serde(default)]
    pub custom_fields: Vec<CustomFieldConfig>,
}

/// A status option in a GitHub Project.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StatusOption {
    /// Option name (e.g., "Todo", "In Progress", "Done")
    pub name: String,

    /// GitHub option ID
    pub option_id: String,
}

/// A custom field in a GitHub Project.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CustomFieldConfig {
    /// Field name (e.g., "Priority", "Sprint")
    pub name: String,

    /// GitHub field ID
    pub field_id: String,

    /// Field type
    pub field_type: ProjectFieldType,

    /// Options for single-select fields
    #[serde(default)]
    pub options: Vec<StatusOption>,
}

/// GitHub Project field types.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProjectFieldType {
    Text,
    Number,
    Date,
    SingleSelect,
    Iteration,
}

/// GitHub sync configuration.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct GitHubSyncConfig {
    /// Enable automatic sync
    #[serde(default)]
    pub auto_sync: bool,

    /// Sync interval in minutes (0 = manual only)
    #[serde(default)]
    pub interval_minutes: u32,

    /// Issue labels to sync (empty = all)
    #[serde(default)]
    pub labels: Vec<String>,

    /// Issue states to sync
    #[serde(default)]
    pub states: Vec<IssueState>,
}

/// Issue state.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum IssueState {
    Open,
    Closed,
}

/// Cached project statistics for quick display.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ProjectStats {
    /// Number of open issues
    #[serde(default)]
    pub open_issues: u32,

    /// Number of closed issues
    #[serde(default)]
    pub closed_issues: u32,

    /// Number of linked knowledge entries
    #[serde(default)]
    pub entry_count: u32,
}

impl Project {
    /// Create a new project with required fields.
    pub fn new(name: String, github_owner: String, github_repo: String) -> Self {
        let slug = slugify(&name);
        let now = Utc::now();

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            slug,
            description: None,
            github: GitHubConfig {
                owner: github_owner,
                repo: github_repo,
                repository_id: None,
                project_v2: None,
                sync: GitHubSyncConfig::default(),
                last_synced_at: None,
                last_error: None,
            },
            color: None,
            archived: false,
            stats: ProjectStats::default(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Get the full GitHub repository string (owner/repo).
    pub fn github_repo_full(&self) -> String {
        format!("{}/{}", self.github.owner, self.github.repo)
    }
}

/// Convert a string to a URL-safe slug.
fn slugify(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_new() {
        let project = Project::new(
            "My Test Project".to_string(),
            "anthropics".to_string(),
            "claude-code".to_string(),
        );

        assert!(!project.id.is_empty());
        assert_eq!(project.name, "My Test Project");
        assert_eq!(project.slug, "my-test-project");
        assert_eq!(project.github.owner, "anthropics");
        assert_eq!(project.github.repo, "claude-code");
        assert!(!project.archived);
    }

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("My Project"), "my-project");
        assert_eq!(slugify("Test  Project 123"), "test-project-123");
        assert_eq!(slugify("Special!@#$%^&*()"), "special");
        assert_eq!(slugify("   spaced   "), "spaced");
    }

    #[test]
    fn test_github_repo_full() {
        let project = Project::new(
            "Test".to_string(),
            "owner".to_string(),
            "repo".to_string(),
        );
        assert_eq!(project.github_repo_full(), "owner/repo");
    }
}
