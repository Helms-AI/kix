//! Issue data model and storage operations.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::project::IssueState;

/// A project issue (local or synced from GitHub).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Issue {
    /// Unique identifier (UUID)
    pub id: String,

    /// Parent project ID
    pub project_id: String,

    /// Issue number within project (auto-incremented)
    pub number: u32,

    /// Issue title
    pub title: String,

    /// Issue body (markdown)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,

    /// Issue state
    pub state: IssueState,

    /// Labels
    #[serde(default)]
    pub labels: Vec<String>,

    /// Priority (1-5, 1=highest)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<u8>,

    /// Assignees (usernames)
    #[serde(default)]
    pub assignees: Vec<String>,

    /// GitHub issue number (if synced)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github_number: Option<u32>,

    /// GitHub issue node ID (for GraphQL operations)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github_node_id: Option<String>,

    /// GitHub issue URL (if synced)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github_url: Option<String>,

    /// GitHub Project item ID (if added to a project board)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github_project_item_id: Option<String>,

    /// Source of the issue
    pub source: IssueSource,

    /// Created timestamp
    pub created_at: DateTime<Utc>,

    /// Updated timestamp
    pub updated_at: DateTime<Utc>,

    /// Closed timestamp (if closed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub closed_at: Option<DateTime<Utc>>,

    /// Sync timestamp (last synced with GitHub)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub synced_at: Option<DateTime<Utc>>,

    /// Content hash for change detection
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,

    /// Local version number (incremented on each change)
    #[serde(default = "default_version")]
    pub local_version: i64,
}

/// Issue source.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IssueSource {
    /// Created locally in Kix
    Local,
    /// Synced from GitHub
    GitHub,
    /// Created via MCP tool
    Mcp,
}

impl Issue {
    /// Create a new local issue.
    pub fn new(project_id: String, number: u32, title: String) -> Self {
        let now = Utc::now();

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            project_id,
            number,
            title,
            body: None,
            state: IssueState::Open,
            labels: Vec::new(),
            priority: None,
            assignees: Vec::new(),
            github_number: None,
            github_node_id: None,
            github_url: None,
            github_project_item_id: None,
            source: IssueSource::Local,
            created_at: now,
            updated_at: now,
            closed_at: None,
            synced_at: None,
            content_hash: None,
            local_version: 1,
        }
    }

    /// Create an issue from GitHub sync.
    pub fn from_github(
        project_id: String,
        number: u32,
        github_issue: GitHubIssueData,
    ) -> Self {
        let now = Utc::now();

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            project_id,
            number,
            title: github_issue.title,
            body: github_issue.body,
            state: github_issue.state,
            labels: github_issue.labels,
            priority: None, // Local field, not from GitHub
            assignees: github_issue.assignees,
            github_number: Some(github_issue.number),
            github_node_id: Some(github_issue.node_id),
            github_url: Some(github_issue.url),
            github_project_item_id: None,
            source: IssueSource::GitHub,
            created_at: github_issue.created_at,
            updated_at: now,
            closed_at: github_issue.closed_at,
            synced_at: Some(now),
            content_hash: None, // Will be calculated after creation
            local_version: 1,
        }
    }

    /// Check if this issue is synced from GitHub.
    pub fn is_github_synced(&self) -> bool {
        self.github_number.is_some()
    }

    /// Close the issue.
    pub fn close(&mut self) {
        self.state = IssueState::Closed;
        self.closed_at = Some(Utc::now());
        self.updated_at = Utc::now();
        self.increment_version();
    }

    /// Reopen the issue.
    pub fn reopen(&mut self) {
        self.state = IssueState::Open;
        self.closed_at = None;
        self.updated_at = Utc::now();
        self.increment_version();
    }

    /// Increment the local version number.
    pub fn increment_version(&mut self) {
        self.local_version += 1;
    }

    /// Update the content hash.
    pub fn update_content_hash(&mut self) {
        use crate::sync_state::ContentHasher;
        self.content_hash = Some(ContentHasher::calculate_hash(self));
    }

    /// Mark as modified and update tracking fields.
    pub fn mark_modified(&mut self) {
        self.updated_at = Utc::now();
        self.increment_version();
        self.update_content_hash();
    }

    /// Update from sync operation.
    pub fn update_from_sync(&mut self, github_updated_at: Option<DateTime<Utc>>) {
        self.synced_at = Some(Utc::now());
        if let Some(updated) = github_updated_at {
            // Don't increment local version for sync updates
            self.updated_at = updated;
        }
        self.update_content_hash();
    }
}

/// Data extracted from a GitHub issue for creating a local issue.
#[derive(Debug, Clone)]
pub struct GitHubIssueData {
    pub number: u32,
    pub node_id: String,
    pub title: String,
    pub body: Option<String>,
    pub state: IssueState,
    pub labels: Vec<String>,
    pub assignees: Vec<String>,
    pub url: String,
    pub created_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
}

/// Parameters for creating an issue.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct CreateIssueParams {
    /// Issue title
    pub title: String,

    /// Issue body (markdown)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,

    /// Labels to add
    #[serde(default)]
    pub labels: Vec<String>,

    /// Priority (1-5, 1=highest)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<u8>,

    /// Also create on GitHub (default: true)
    #[serde(default = "default_true")]
    pub create_on_github: bool,

    /// Add to GitHub Project board (default: true if project has board)
    #[serde(default = "default_true")]
    pub add_to_project: bool,
}

fn default_true() -> bool {
    true
}

fn default_version() -> i64 {
    1
}

/// Parameters for updating an issue.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct UpdateIssueParams {
    /// New title
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// New body
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,

    /// New state
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<IssueState>,

    /// New labels (replaces existing)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<Vec<String>>,

    /// New priority
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<u8>,

    /// Also update on GitHub (default: true for synced issues)
    #[serde(default = "default_true")]
    pub sync_to_github: bool,
}

/// Filters for listing issues.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
pub struct IssueFilters {
    /// Filter by state
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<IssueState>,

    /// Filter by labels (any match)
    #[serde(default)]
    pub labels: Vec<String>,

    /// Filter by priority
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<u8>,

    /// Filter by source
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<IssueSource>,

    /// Maximum results
    #[serde(default = "default_limit")]
    pub limit: usize,

    /// Offset for pagination
    #[serde(default)]
    pub offset: usize,
}

fn default_limit() -> usize {
    50
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_issue_new() {
        let issue = Issue::new(
            "proj_123".to_string(),
            1,
            "Test Issue".to_string(),
        );

        assert!(!issue.id.is_empty());
        assert_eq!(issue.project_id, "proj_123");
        assert_eq!(issue.number, 1);
        assert_eq!(issue.title, "Test Issue");
        assert_eq!(issue.state, IssueState::Open);
        assert_eq!(issue.source, IssueSource::Local);
        assert!(!issue.is_github_synced());
    }

    #[test]
    fn test_issue_close_reopen() {
        let mut issue = Issue::new(
            "proj_123".to_string(),
            1,
            "Test".to_string(),
        );

        assert_eq!(issue.state, IssueState::Open);
        assert!(issue.closed_at.is_none());

        issue.close();
        assert_eq!(issue.state, IssueState::Closed);
        assert!(issue.closed_at.is_some());

        issue.reopen();
        assert_eq!(issue.state, IssueState::Open);
        assert!(issue.closed_at.is_none());
    }

    #[test]
    fn test_issue_from_github() {
        let github_data = GitHubIssueData {
            number: 42,
            node_id: "I_123".to_string(),
            title: "GitHub Issue".to_string(),
            body: Some("Body text".to_string()),
            state: IssueState::Open,
            labels: vec!["bug".to_string()],
            assignees: vec!["user1".to_string()],
            url: "https://github.com/owner/repo/issues/42".to_string(),
            created_at: Utc::now(),
            closed_at: None,
        };

        let issue = Issue::from_github("proj_123".to_string(), 1, github_data);

        assert!(issue.is_github_synced());
        assert_eq!(issue.github_number, Some(42));
        assert_eq!(issue.source, IssueSource::GitHub);
        assert_eq!(issue.labels, vec!["bug".to_string()]);
    }
}
