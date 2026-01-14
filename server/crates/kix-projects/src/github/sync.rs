//! GitHub sync service for synchronizing issues and projects.
//!
//! This module provides bidirectional sync between Kix projects and GitHub.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::error::ProjectError;
use crate::github::models::{CreateIssueRequest, GitHubIssue, UpdateIssueRequest};
use crate::github::{GitHubGraphQLClient, GitHubRestClient};
use crate::issue::IssueSource;
use crate::project::IssueState;

/// Sync direction for GitHub synchronization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncDirection {
    /// Only pull changes from GitHub
    Pull,
    /// Only push changes to GitHub
    Push,
    /// Bidirectional sync (default)
    Bidirectional,
}

impl Default for SyncDirection {
    fn default() -> Self {
        SyncDirection::Bidirectional
    }
}

/// Configuration for sync operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfig {
    /// Sync direction
    #[serde(default)]
    pub direction: SyncDirection,

    /// Include closed issues
    #[serde(default = "default_true")]
    pub include_closed: bool,

    /// Labels to filter by (empty = all)
    #[serde(default)]
    pub labels_filter: Vec<String>,

    /// Maximum issues to sync
    #[serde(default = "default_max_issues")]
    pub max_issues: u32,
}

fn default_true() -> bool {
    true
}

fn default_max_issues() -> u32 {
    100
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            direction: SyncDirection::Bidirectional,
            include_closed: true,
            labels_filter: vec![],
            max_issues: 100,
        }
    }
}

/// Result of a sync operation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncResult {
    /// Number of issues pulled from GitHub
    pub issues_pulled: u32,

    /// Number of issues pushed to GitHub
    pub issues_pushed: u32,

    /// Number of issues updated
    pub issues_updated: u32,

    /// Number of issues that failed to sync
    pub issues_failed: u32,

    /// Error messages for failed items
    pub errors: Vec<String>,

    /// Timestamp of sync completion
    pub synced_at: chrono::DateTime<Utc>,
}

/// Information about an issue for sync.
#[derive(Debug, Clone)]
pub struct IssueInfo {
    /// Local issue ID
    pub id: String,
    /// Issue title
    pub title: String,
    /// Issue body
    pub body: Option<String>,
    /// Issue state
    pub state: IssueState,
    /// Labels
    pub labels: Vec<String>,
    /// Assignees
    pub assignees: Vec<String>,
    /// GitHub issue number (if synced)
    pub github_number: Option<u32>,
    /// GitHub node ID
    pub github_node_id: Option<String>,
    /// Source
    pub source: IssueSource,
}

/// GitHub sync service.
pub struct GitHubSyncService {
    rest_client: GitHubRestClient,
    graphql_client: GitHubGraphQLClient,
}

impl GitHubSyncService {
    /// Create a new sync service.
    pub fn new(token: impl Into<String>) -> Result<Self, ProjectError> {
        let token = token.into();
        let rest_client = GitHubRestClient::new(&token)?;
        let graphql_client = GitHubGraphQLClient::new(&token)?;

        Ok(Self {
            rest_client,
            graphql_client,
        })
    }

    /// Get the REST client.
    pub fn rest(&self) -> &GitHubRestClient {
        &self.rest_client
    }

    /// Get the GraphQL client.
    pub fn graphql(&self) -> &GitHubGraphQLClient {
        &self.graphql_client
    }

    /// Verify access to a repository.
    pub async fn verify_access(&self, owner: &str, repo: &str) -> Result<bool, ProjectError> {
        self.rest_client.verify_access(owner, repo).await
    }

    /// Pull issues from GitHub.
    pub async fn pull_issues(
        &self,
        owner: &str,
        repo: &str,
        config: &SyncConfig,
    ) -> Result<Vec<GitHubIssue>, ProjectError> {
        let state = if config.include_closed {
            Some("all")
        } else {
            Some("open")
        };

        // Fetch issues with pagination
        let mut all_issues = Vec::new();
        let mut page = 1;
        let per_page = 100;

        loop {
            let issues = self
                .rest_client
                .list_issues(owner, repo, state, Some(per_page), Some(page))
                .await?;

            let count = issues.len();
            all_issues.extend(issues);

            if count < per_page as usize || all_issues.len() >= config.max_issues as usize {
                break;
            }
            page += 1;
        }

        // Truncate to max_issues
        all_issues.truncate(config.max_issues as usize);

        // Filter by labels if specified
        if !config.labels_filter.is_empty() {
            all_issues.retain(|issue| {
                issue
                    .labels
                    .iter()
                    .any(|l| config.labels_filter.contains(&l.name))
            });
        }

        info!(
            "Pulled {} issues from {}/{}",
            all_issues.len(),
            owner,
            repo
        );
        Ok(all_issues)
    }

    /// Push a local issue to GitHub.
    pub async fn push_issue(
        &self,
        owner: &str,
        repo: &str,
        issue: &IssueInfo,
    ) -> Result<GitHubIssue, ProjectError> {
        if let Some(github_number) = issue.github_number {
            // Update existing issue
            let request = UpdateIssueRequest {
                title: Some(issue.title.clone()),
                body: issue.body.clone(),
                state: Some(match issue.state {
                    IssueState::Open => "open".to_string(),
                    IssueState::Closed => "closed".to_string(),
                }),
                labels: Some(issue.labels.clone()),
                assignees: Some(issue.assignees.clone()),
            };

            self.rest_client
                .update_issue(owner, repo, github_number, &request)
                .await
        } else {
            // Create new issue
            let request = CreateIssueRequest {
                title: issue.title.clone(),
                body: issue.body.clone(),
                labels: issue.labels.clone(),
                assignees: issue.assignees.clone(),
            };

            self.rest_client.create_issue(owner, repo, &request).await
        }
    }

    /// Sync issues bidirectionally.
    ///
    /// This is a simplified sync that:
    /// 1. Pulls all issues from GitHub
    /// 2. Returns them for the caller to reconcile with local state
    pub async fn sync_issues(
        &self,
        owner: &str,
        repo: &str,
        config: &SyncConfig,
        local_issues: &[IssueInfo],
    ) -> Result<SyncResult, ProjectError> {
        let mut result = SyncResult::default();

        // Pull from GitHub
        let github_issues = self.pull_issues(owner, repo, config).await?;
        result.issues_pulled = github_issues.len() as u32;

        // Push local-only issues to GitHub (if bidirectional or push)
        if config.direction != SyncDirection::Pull {
            for local_issue in local_issues {
                // Skip issues that came from GitHub
                if local_issue.source == IssueSource::GitHub {
                    continue;
                }

                match self.push_issue(owner, repo, local_issue).await {
                    Ok(_) => {
                        result.issues_pushed += 1;
                    }
                    Err(e) => {
                        result.issues_failed += 1;
                        result
                            .errors
                            .push(format!("Failed to push '{}': {}", local_issue.title, e));
                        warn!("Failed to push issue '{}': {}", local_issue.title, e);
                    }
                }
            }
        }

        result.synced_at = Utc::now();
        info!(
            "Sync complete: pulled={}, pushed={}, failed={}",
            result.issues_pulled, result.issues_pushed, result.issues_failed
        );

        Ok(result)
    }

    // ========================================================================
    // Projects V2 Operations
    // ========================================================================

    /// Add an issue to a GitHub Project V2.
    pub async fn add_issue_to_project(
        &self,
        project_node_id: &str,
        issue_node_id: &str,
    ) -> Result<String, ProjectError> {
        self.graphql_client
            .add_issue_to_project(project_node_id, issue_node_id)
            .await
    }

    /// Update an item's status in a Project V2.
    pub async fn update_item_status(
        &self,
        project_node_id: &str,
        item_id: &str,
        status_field_id: &str,
        status_option_id: &str,
    ) -> Result<(), ProjectError> {
        self.graphql_client
            .update_item_field(project_node_id, item_id, status_field_id, status_option_id)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_config_defaults() {
        let config = SyncConfig::default();
        assert_eq!(config.direction, SyncDirection::Bidirectional);
        assert!(config.include_closed);
        assert!(config.labels_filter.is_empty());
        assert_eq!(config.max_issues, 100);
    }

    #[test]
    fn test_sync_result_defaults() {
        let result = SyncResult::default();
        assert_eq!(result.issues_pulled, 0);
        assert_eq!(result.issues_pushed, 0);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_sync_direction_serialization() {
        let dir = SyncDirection::Bidirectional;
        let json = serde_json::to_string(&dir).unwrap();
        assert_eq!(json, "\"bidirectional\"");

        let dir: SyncDirection = serde_json::from_str("\"pull\"").unwrap();
        assert_eq!(dir, SyncDirection::Pull);
    }
}
