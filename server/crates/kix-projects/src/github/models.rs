//! Data models for GitHub API responses.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::project::IssueState;

/// GitHub issue from REST API.
#[derive(Debug, Clone, Deserialize)]
pub struct GitHubIssue {
    /// Issue ID
    pub id: u64,

    /// Issue node ID (for GraphQL)
    pub node_id: String,

    /// Issue number
    pub number: u32,

    /// Issue title
    pub title: String,

    /// Issue body (markdown)
    pub body: Option<String>,

    /// Issue state ("open" or "closed")
    pub state: String,

    /// Issue labels
    #[serde(default)]
    pub labels: Vec<GitHubLabel>,

    /// Issue assignees
    #[serde(default)]
    pub assignees: Vec<GitHubUser>,

    /// Issue URL
    pub html_url: String,

    /// Created at
    pub created_at: DateTime<Utc>,

    /// Updated at
    pub updated_at: DateTime<Utc>,

    /// Closed at
    pub closed_at: Option<DateTime<Utc>>,
}

impl GitHubIssue {
    /// Convert GitHub state string to IssueState enum.
    pub fn get_state(&self) -> IssueState {
        if self.state == "closed" {
            IssueState::Closed
        } else {
            IssueState::Open
        }
    }

    /// Get label names.
    pub fn get_label_names(&self) -> Vec<String> {
        self.labels.iter().map(|l| l.name.clone()).collect()
    }

    /// Get assignee logins.
    pub fn get_assignee_logins(&self) -> Vec<String> {
        self.assignees.iter().map(|a| a.login.clone()).collect()
    }
}

/// GitHub label.
#[derive(Debug, Clone, Deserialize)]
pub struct GitHubLabel {
    /// Label ID
    pub id: u64,

    /// Label name
    pub name: String,

    /// Label color (hex without #)
    pub color: String,

    /// Label description
    pub description: Option<String>,
}

/// GitHub user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubUser {
    /// User ID
    pub id: u64,

    /// Username
    pub login: String,

    /// Avatar URL
    pub avatar_url: String,

    /// Profile URL
    pub html_url: String,
}

/// Authenticated GitHub user (from /user endpoint).
/// Includes additional fields not in the basic user response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticatedUser {
    /// User ID
    pub id: u64,

    /// Username
    pub login: String,

    /// Display name
    pub name: Option<String>,

    /// Avatar URL
    pub avatar_url: String,

    /// Profile URL
    pub html_url: String,

    /// Email (may be null if private)
    pub email: Option<String>,

    /// Account type
    #[serde(rename = "type")]
    pub user_type: String,
}

/// GitHub organization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubOrg {
    /// Organization ID
    pub id: u64,

    /// Organization login/name
    pub login: String,

    /// Avatar URL
    pub avatar_url: String,

    /// Description
    pub description: Option<String>,
}

/// GitHub repository.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubRepository {
    /// Repository ID
    pub id: u64,

    /// Repository node ID (for GraphQL)
    pub node_id: String,

    /// Repository name
    pub name: String,

    /// Full name (owner/repo)
    pub full_name: String,

    /// Owner
    pub owner: GitHubUser,

    /// Repository URL
    pub html_url: String,

    /// Default branch
    pub default_branch: String,

    /// Repository description
    #[serde(default)]
    pub description: Option<String>,

    /// Whether the repository is private
    #[serde(default)]
    pub private: bool,

    /// Star count
    #[serde(default)]
    pub stargazers_count: u32,
}

/// Search repositories result.
#[derive(Debug, Clone, Deserialize)]
pub struct SearchReposResult {
    /// Total count of matching repositories
    pub total_count: u32,

    /// Whether results are incomplete (GitHub rate limiting)
    pub incomplete_results: bool,

    /// Repository items
    pub items: Vec<GitHubRepository>,
}

/// GitHub Project V2 (from GraphQL).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubProjectV2 {
    /// Project node ID
    pub id: String,

    /// Project number
    pub number: u32,

    /// Project title
    pub title: String,

    /// Project URL
    pub url: String,
}

/// GitHub Project V2 field (from GraphQL).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubProjectField {
    /// Field ID
    pub id: String,

    /// Field name
    pub name: String,

    /// Field type
    #[serde(default)]
    pub field_type: String,

    /// Options for single-select fields
    #[serde(default)]
    pub options: Vec<GitHubFieldOption>,
}

/// GitHub Project V2 field option.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubFieldOption {
    /// Option ID
    pub id: String,

    /// Option name
    pub name: String,
}

/// GitHub Project V2 item (from GraphQL).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubProjectItem {
    /// Item ID
    pub id: String,

    /// Field values
    #[serde(default)]
    pub field_values: Vec<GitHubFieldValue>,

    /// Content (issue or draft)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<GitHubProjectItemContent>,
}

/// GitHub Project V2 item content.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum GitHubProjectItemContent {
    Issue {
        id: String,
        number: u32,
        title: String,
        state: String,
    },
    DraftIssue {
        title: String,
        body: Option<String>,
    },
    PullRequest {
        id: String,
        number: u32,
        title: String,
    },
}

/// GitHub Project V2 field value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubFieldValue {
    /// Field name
    pub field_name: String,

    /// Value (text, number, or option name)
    pub value: String,
}

/// GitHub GraphQL error.
#[derive(Debug, Clone, Deserialize)]
pub struct GitHubGraphQLError {
    /// Error message
    pub message: String,

    /// Error path
    #[serde(default)]
    pub path: Vec<String>,

    /// Error locations
    #[serde(default)]
    pub locations: Vec<GitHubErrorLocation>,
}

/// GitHub GraphQL error location.
#[derive(Debug, Clone, Deserialize)]
pub struct GitHubErrorLocation {
    /// Line number
    pub line: u32,

    /// Column number
    pub column: u32,
}

/// Create issue request body.
#[derive(Debug, Clone, Serialize)]
pub struct CreateIssueRequest {
    /// Issue title
    pub title: String,

    /// Issue body
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,

    /// Labels to add
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,

    /// Assignees
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub assignees: Vec<String>,
}

/// Update issue request body.
#[derive(Debug, Clone, Serialize)]
pub struct UpdateIssueRequest {
    /// New title
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// New body
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,

    /// New state ("open" or "closed")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,

    /// New labels
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<Vec<String>>,

    /// New assignees
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignees: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_github_issue_get_state() {
        let mut issue = create_test_issue();
        issue.state = "open".to_string();
        assert_eq!(issue.get_state(), IssueState::Open);

        issue.state = "closed".to_string();
        assert_eq!(issue.get_state(), IssueState::Closed);
    }

    #[test]
    fn test_github_issue_get_labels() {
        let mut issue = create_test_issue();
        issue.labels = vec![
            GitHubLabel {
                id: 1,
                name: "bug".to_string(),
                color: "ff0000".to_string(),
                description: None,
            },
            GitHubLabel {
                id: 2,
                name: "urgent".to_string(),
                color: "ff9900".to_string(),
                description: None,
            },
        ];

        let names = issue.get_label_names();
        assert_eq!(names, vec!["bug".to_string(), "urgent".to_string()]);
    }

    fn create_test_issue() -> GitHubIssue {
        GitHubIssue {
            id: 1,
            node_id: "I_123".to_string(),
            number: 1,
            title: "Test".to_string(),
            body: None,
            state: "open".to_string(),
            labels: vec![],
            assignees: vec![],
            html_url: "https://github.com/test/test/issues/1".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            closed_at: None,
        }
    }
}
