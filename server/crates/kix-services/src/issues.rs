//! Issue management services.
//!
//! Provides shared business logic for issue CRUD operations with GitHub
//! integration, used by both the REST API and MCP server.

use kix_projects::github::{GitHubRestClient, models as github_models};
use kix_projects::{SharedEventBus, TokenService, ProjectTokenType};
use kix_store::{IssueRecord, ProjectStore};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use crate::error::{ServiceError, ServiceResult};
use crate::Pagination;

// =============================================================================
// TYPES
// =============================================================================

/// Issue summary for list operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueSummary {
    pub id: String,
    pub project_id: String,
    pub number: u32,
    pub title: String,
    pub body: Option<String>,
    pub state: String,
    pub labels: Vec<String>,
    pub assignees: Vec<String>,
    pub priority: Option<String>,
    pub github_number: Option<u32>,
    pub github_url: Option<String>,
    pub source: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<&IssueRecord> for IssueSummary {
    fn from(issue: &IssueRecord) -> Self {
        let priority = issue.priority.map(|p| match p {
            1 => "critical".to_string(),
            2 => "high".to_string(),
            3 => "medium".to_string(),
            _ => "low".to_string(),
        });

        Self {
            id: issue.id.clone(),
            project_id: issue.project_id.clone(),
            number: issue.number as u32,
            title: issue.title.clone(),
            body: issue.body.clone(),
            state: issue.state.clone(),
            labels: issue.labels_vec(),
            assignees: issue.assignees_vec(),
            priority,
            github_number: issue.github_number.map(|n| n as u32),
            github_url: issue.github_url.clone(),
            source: issue.source.clone(),
            created_at: issue.created_at.clone(),
            updated_at: issue.updated_at.clone(),
        }
    }
}

/// Issue list with pagination.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueList {
    pub issues: Vec<IssueSummary>,
    pub total: usize,
    pub has_more: bool,
}

/// Filter options for listing issues.
#[derive(Debug, Clone, Default)]
pub struct IssueFilters {
    pub state: Option<String>,
    pub labels: Option<Vec<String>>,
    pub assignee: Option<String>,
    pub search: Option<String>,
}

/// Data for creating a new issue.
#[derive(Debug, Clone)]
pub struct CreateIssueData {
    pub title: String,
    pub body: Option<String>,
    pub labels: Option<Vec<String>>,
    pub assignees: Option<Vec<String>>,
}

/// Updates to apply to an issue.
#[derive(Debug, Clone, Default)]
pub struct IssueUpdates {
    pub title: Option<String>,
    pub body: Option<String>,
    pub state: Option<String>,
    pub labels: Option<Vec<String>>,
    pub assignees: Option<Vec<String>>,
}

/// Options for issue operations.
#[derive(Debug, Clone, Default)]
pub struct IssueOptions {
    /// Push changes to GitHub (default: true if GitHub is configured)
    pub push_to_github: bool,
}

/// Options for deleting an issue.
#[derive(Debug, Clone, Default)]
pub struct DeleteIssueOptions {
    /// Close issue on GitHub (default: true if GitHub is configured)
    pub close_on_github: bool,
}

/// Result of issue creation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateIssueResult {
    pub issue_id: String,
    pub number: u32,
    pub title: String,
    pub github_number: Option<u32>,
    pub github_url: Option<String>,
}

/// Result of issue deletion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteIssueResult {
    pub github_closed: bool,
}

// =============================================================================
// SERVICE FUNCTIONS
// =============================================================================

/// List issues for a project with optional filtering and pagination.
///
/// # Arguments
/// * `store` - The ProjectStore instance
/// * `project_id_or_slug` - Project ID or slug
/// * `filters` - Filter options
/// * `pagination` - Limit and offset
///
/// # Returns
/// List of issues with pagination info
pub async fn list_issues(
    store: &Arc<RwLock<ProjectStore>>,
    project_id_or_slug: &str,
    filters: IssueFilters,
    pagination: Pagination,
) -> ServiceResult<IssueList> {
    info!(
        "List issues for project {} (limit: {}, offset: {})",
        project_id_or_slug, pagination.limit, pagination.offset
    );

    let store_guard = store.read().await;

    // Get project to validate it exists
    let project = store_guard
        .get_project(project_id_or_slug)
        .await
        .map_err(ServiceError::Store)?
        .ok_or_else(|| ServiceError::not_found("Project", project_id_or_slug))?;

    // List issues with state filter
    let issues = store_guard
        .list_issues(
            &project.id,
            filters.state.as_deref(),
            pagination.limit,
            pagination.offset,
        )
        .await
        .map_err(ServiceError::Store)?;

    let total = issues.len();
    let has_more = issues.len() >= pagination.limit;

    let summaries: Vec<IssueSummary> = issues.iter().map(IssueSummary::from).collect();

    Ok(IssueList {
        issues: summaries,
        total,
        has_more,
    })
}

/// Get an issue by ID or number.
///
/// # Arguments
/// * `store` - The ProjectStore instance
/// * `project_id_or_slug` - Project ID or slug
/// * `issue_id_or_number` - Issue ID or number
///
/// # Returns
/// Issue details
pub async fn get_issue(
    store: &Arc<RwLock<ProjectStore>>,
    project_id_or_slug: &str,
    issue_id_or_number: &str,
) -> ServiceResult<IssueSummary> {
    info!(
        "Get issue {} in project {}",
        issue_id_or_number, project_id_or_slug
    );

    let store_guard = store.read().await;

    // Get project to validate it exists
    let project = store_guard
        .get_project(project_id_or_slug)
        .await
        .map_err(ServiceError::Store)?
        .ok_or_else(|| ServiceError::not_found("Project", project_id_or_slug))?;

    // Try to parse as issue number, otherwise treat as ID
    let issue = if let Ok(num) = issue_id_or_number.parse::<u32>() {
        store_guard
            .get_issue_by_number(&project.id, num)
            .await
            .map_err(ServiceError::Store)?
    } else {
        store_guard
            .get_issue(issue_id_or_number)
            .await
            .map_err(ServiceError::Store)?
    };

    let issue = issue.ok_or_else(|| ServiceError::not_found("Issue", issue_id_or_number))?;

    Ok(IssueSummary::from(&issue))
}

/// Create a new issue in a project.
///
/// When GitHub is configured with a token, the issue is created on GitHub first.
/// If GitHub creation fails, the operation fails.
///
/// # Arguments
/// * `store` - The ProjectStore instance
/// * `token_service` - Token service for GitHub token management
/// * `event_bus` - Optional event bus for notifications
/// * `project_id_or_slug` - Project ID or slug
/// * `data` - Issue data
/// * `options` - Creation options
///
/// # Returns
/// Created issue result
pub async fn create_issue(
    store: &Arc<RwLock<ProjectStore>>,
    token_service: Option<&Arc<TokenService>>,
    event_bus: Option<&SharedEventBus>,
    project_id_or_slug: &str,
    data: CreateIssueData,
    options: IssueOptions,
) -> ServiceResult<CreateIssueResult> {
    info!("Create issue '{}' in project {}", data.title, project_id_or_slug);

    let store_guard = store.write().await;

    // Get project to validate it exists
    let project = store_guard
        .get_project(project_id_or_slug)
        .await
        .map_err(ServiceError::Store)?
        .ok_or_else(|| ServiceError::not_found("Project", project_id_or_slug))?;

    let github_owner = project.github_owner().unwrap_or_default();
    let github_repo = project.github_repo().unwrap_or_default();
    let has_github_config = !github_owner.is_empty() && !github_repo.is_empty();

    // Check if token is configured (only if we have token service)
    let has_token = if let Some(ts) = token_service {
        match ts.get_project_token_type(&project.id).await {
            Ok(ProjectTokenType::NotConfigured) => false,
            Ok(_) => true,
            Err(_) => false,
        }
    } else {
        false
    };

    // If GitHub is configured and we want to push, create there FIRST
    if has_github_config && has_token && options.push_to_github {
        let ts = token_service.unwrap();

        // Get token
        let token = ts
            .get_token_for_project(&project.id)
            .await
            .map_err(|e| ServiceError::GitHub(format!("Failed to get token: {}", e)))?;

        // Create GitHub client
        let client = GitHubRestClient::new(&token)
            .map_err(|e| ServiceError::GitHub(format!("Failed to create client: {}", e)))?;

        // Build GitHub create request
        let github_req = github_models::CreateIssueRequest {
            title: data.title.clone(),
            body: data.body.clone(),
            labels: data.labels.clone().unwrap_or_default(),
            assignees: data.assignees.clone().unwrap_or_default(),
        };

        // Create on GitHub FIRST
        let gh_issue = client
            .create_issue(&github_owner, &github_repo, &github_req)
            .await
            .map_err(|e| ServiceError::GitHub(format!("Failed to create on GitHub: {}", e)))?;

        info!(
            "Created GitHub issue #{} for project {}",
            gh_issue.number, project.id
        );

        // GitHub succeeded - create locally with GitHub's number
        let mut issue = IssueRecord::new(project.id.clone(), gh_issue.number, data.title.clone());
        if let Some(body) = data.body {
            issue = issue.with_body(body);
        }
        if let Some(labels) = data.labels {
            issue = issue.with_labels(labels);
        }
        if let Some(assignees) = data.assignees {
            issue.set_assignees(assignees);
        }
        issue.github_number = Some(gh_issue.number as i64);
        issue.github_node_id = Some(gh_issue.node_id.clone());
        issue.github_url = Some(gh_issue.html_url.clone());
        issue.source = "github".to_string();

        let issue_id = issue.id.clone();
        let issue_title = issue.title.clone();
        let github_url = issue.github_url.clone();

        store_guard
            .create_issue(&issue)
            .await
            .map_err(ServiceError::Store)?;

        drop(store_guard);

        // Emit event
        if let Some(bus) = event_bus {
            bus.issue_created(&project.id, &issue_id, &issue_title);
        }

        Ok(CreateIssueResult {
            issue_id,
            number: gh_issue.number,
            title: issue_title,
            github_number: Some(gh_issue.number),
            github_url,
        })
    } else {
        // No GitHub config - create locally only
        let number = store_guard
            .next_issue_number(&project.id)
            .await
            .map_err(ServiceError::Store)?;

        let mut issue = IssueRecord::new(project.id.clone(), number, data.title.clone());
        if let Some(body) = data.body {
            issue = issue.with_body(body);
        }
        if let Some(labels) = data.labels {
            issue = issue.with_labels(labels);
        }
        if let Some(assignees) = data.assignees {
            issue.set_assignees(assignees);
        }

        let issue_id = issue.id.clone();
        let issue_title = issue.title.clone();

        store_guard
            .create_issue(&issue)
            .await
            .map_err(ServiceError::Store)?;

        drop(store_guard);

        // Emit event
        if let Some(bus) = event_bus {
            bus.issue_created(&project.id, &issue_id, &issue_title);
        }

        Ok(CreateIssueResult {
            issue_id,
            number,
            title: issue_title,
            github_number: None,
            github_url: None,
        })
    }
}

/// Update an issue.
///
/// When GitHub is configured and the issue exists on GitHub, updates there first.
/// If GitHub update fails, the operation fails.
///
/// # Arguments
/// * `store` - The ProjectStore instance
/// * `token_service` - Token service for GitHub token management
/// * `event_bus` - Optional event bus for notifications
/// * `project_id_or_slug` - Project ID or slug
/// * `issue_id_or_number` - Issue ID or number
/// * `updates` - Updates to apply
/// * `options` - Update options
///
/// # Returns
/// Updated issue summary
pub async fn update_issue(
    store: &Arc<RwLock<ProjectStore>>,
    token_service: Option<&Arc<TokenService>>,
    event_bus: Option<&SharedEventBus>,
    project_id_or_slug: &str,
    issue_id_or_number: &str,
    updates: IssueUpdates,
    options: IssueOptions,
) -> ServiceResult<IssueSummary> {
    info!(
        "Update issue {} in project {}",
        issue_id_or_number, project_id_or_slug
    );

    let store_guard = store.write().await;

    // Get project
    let project = store_guard
        .get_project(project_id_or_slug)
        .await
        .map_err(ServiceError::Store)?
        .ok_or_else(|| ServiceError::not_found("Project", project_id_or_slug))?;

    // Get issue
    let issue_result = if let Ok(num) = issue_id_or_number.parse::<u32>() {
        store_guard.get_issue_by_number(&project.id, num).await
    } else {
        store_guard.get_issue(issue_id_or_number).await
    };

    let mut issue = issue_result
        .map_err(ServiceError::Store)?
        .ok_or_else(|| ServiceError::not_found("Issue", issue_id_or_number))?;

    let was_open = issue.state != "closed";
    let github_owner = project.github_owner().unwrap_or_default();
    let github_repo = project.github_repo().unwrap_or_default();
    let has_github_config = !github_owner.is_empty() && !github_repo.is_empty();

    // Check if issue has a GitHub number
    let github_number = issue.github_number.map(|n| n as u32);

    // Check token
    let has_token = if let Some(ts) = token_service {
        match ts.get_project_token_type(&project.id).await {
            Ok(ProjectTokenType::NotConfigured) => false,
            Ok(_) => true,
            Err(_) => false,
        }
    } else {
        false
    };

    // If GitHub configured and issue exists on GitHub, update there FIRST
    if has_github_config && github_number.is_some() && has_token && options.push_to_github {
        let ts = token_service.unwrap();
        let gh_num = github_number.unwrap();

        let token = ts
            .get_token_for_project(&project.id)
            .await
            .map_err(|e| ServiceError::GitHub(format!("Failed to get token: {}", e)))?;

        let client = GitHubRestClient::new(&token)
            .map_err(|e| ServiceError::GitHub(format!("Failed to create client: {}", e)))?;

        // Build GitHub update request
        let github_req = github_models::UpdateIssueRequest {
            title: updates.title.clone(),
            body: updates.body.clone(),
            state: updates.state.clone(),
            labels: updates.labels.clone(),
            assignees: updates.assignees.clone(),
        };

        // Update on GitHub FIRST
        let gh_issue = client
            .update_issue(&github_owner, &github_repo, gh_num, &github_req)
            .await
            .map_err(|e| ServiceError::GitHub(format!("Failed to update on GitHub: {}", e)))?;

        info!(
            "Updated GitHub issue #{} for project {}",
            gh_num, project.id
        );

        // Update local issue with data from GitHub response
        issue.title = gh_issue.title;
        issue.body = gh_issue.body;
        issue.state = gh_issue.state.clone();
        issue.set_labels(gh_issue.labels.iter().map(|l| l.name.clone()).collect());
        issue.set_assignees(gh_issue.assignees.iter().map(|u| u.login.clone()).collect());
    } else {
        // No GitHub - update locally only
        if let Some(title) = updates.title {
            issue.title = title;
        }
        if let Some(body) = updates.body {
            issue.body = Some(body);
        }
        if let Some(state) = updates.state {
            issue.state = state;
        }
        if let Some(labels) = updates.labels {
            issue.set_labels(labels);
        }
        if let Some(assignees) = updates.assignees {
            issue.set_assignees(assignees);
        }
    }

    issue.updated_at = chrono::Utc::now().to_rfc3339();
    let is_now_closed = issue.state == "closed";
    let issue_id = issue.id.clone();
    let project_id = project.id.clone();

    store_guard
        .update_issue(&issue)
        .await
        .map_err(ServiceError::Store)?;

    drop(store_guard);

    // Emit appropriate event
    if let Some(bus) = event_bus {
        if was_open && is_now_closed {
            bus.issue_closed(&project_id, &issue_id);
        } else if !was_open && !is_now_closed {
            bus.issue_reopened(&project_id, &issue_id);
        } else {
            bus.issue_updated(&project_id, &issue_id);
        }
    }

    Ok(IssueSummary::from(&issue))
}

/// Delete an issue.
///
/// When GitHub is configured and the issue exists on GitHub, closes it there first
/// (GitHub API doesn't allow deleting issues).
///
/// # Arguments
/// * `store` - The ProjectStore instance
/// * `token_service` - Token service for GitHub token management
/// * `event_bus` - Optional event bus for notifications
/// * `project_id_or_slug` - Project ID or slug
/// * `issue_id_or_number` - Issue ID or number
/// * `options` - Delete options
///
/// # Returns
/// Deletion result
pub async fn delete_issue(
    store: &Arc<RwLock<ProjectStore>>,
    token_service: Option<&Arc<TokenService>>,
    event_bus: Option<&SharedEventBus>,
    project_id_or_slug: &str,
    issue_id_or_number: &str,
    options: DeleteIssueOptions,
) -> ServiceResult<DeleteIssueResult> {
    info!(
        "Delete issue {} from project {}",
        issue_id_or_number, project_id_or_slug
    );

    let store_guard = store.write().await;

    // Get project
    let project = store_guard
        .get_project(project_id_or_slug)
        .await
        .map_err(ServiceError::Store)?
        .ok_or_else(|| ServiceError::not_found("Project", project_id_or_slug))?;

    // Get issue
    let issue_result = if let Ok(num) = issue_id_or_number.parse::<u32>() {
        store_guard.get_issue_by_number(&project.id, num).await
    } else {
        store_guard.get_issue(issue_id_or_number).await
    };

    let issue = issue_result
        .map_err(ServiceError::Store)?
        .ok_or_else(|| ServiceError::not_found("Issue", issue_id_or_number))?;

    let github_owner = project.github_owner().unwrap_or_default();
    let github_repo = project.github_repo().unwrap_or_default();
    let has_github_config = !github_owner.is_empty() && !github_repo.is_empty();

    // Check if issue has a GitHub number
    let github_number = issue.github_number.map(|n| n as u32);

    // Check token
    let has_token = if let Some(ts) = token_service {
        match ts.get_project_token_type(&project.id).await {
            Ok(ProjectTokenType::NotConfigured) => false,
            Ok(_) => true,
            Err(_) => false,
        }
    } else {
        false
    };

    let mut github_closed = false;

    // If GitHub configured and issue exists on GitHub, close there FIRST
    if has_github_config && github_number.is_some() && has_token && options.close_on_github {
        let ts = token_service.unwrap();
        let gh_num = github_number.unwrap();

        let token = ts
            .get_token_for_project(&project.id)
            .await
            .map_err(|e| ServiceError::GitHub(format!("Failed to get token: {}", e)))?;

        let client = GitHubRestClient::new(&token)
            .map_err(|e| ServiceError::GitHub(format!("Failed to create client: {}", e)))?;

        // Close on GitHub (GitHub doesn't allow deleting issues)
        client
            .close_issue(&github_owner, &github_repo, gh_num)
            .await
            .map_err(|e| ServiceError::GitHub(format!("Failed to close on GitHub: {}", e)))?;

        info!(
            "Closed GitHub issue #{} for project {}",
            gh_num, project.id
        );
        github_closed = true;
    }

    // Delete locally
    let issue_id = issue.id.clone();
    let project_id = project.id.clone();

    store_guard
        .delete_issue(&issue_id)
        .await
        .map_err(ServiceError::Store)?;

    drop(store_guard);

    // Emit event
    if let Some(bus) = event_bus {
        bus.issue_deleted(&project_id, &issue_id);
    }

    Ok(DeleteIssueResult { github_closed })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_issue_summary_from_record() {
        let record = IssueRecord::new("project-1".to_string(), 1, "Test Issue".to_string());
        let summary = IssueSummary::from(&record);
        assert_eq!(summary.title, "Test Issue");
        assert_eq!(summary.number, 1);
        assert_eq!(summary.state, "open");
    }

    #[test]
    fn test_issue_filters_default() {
        let filters = IssueFilters::default();
        assert!(filters.state.is_none());
        assert!(filters.labels.is_none());
    }

    #[test]
    fn test_create_issue_data() {
        let data = CreateIssueData {
            title: "New Issue".to_string(),
            body: Some("Description".to_string()),
            labels: Some(vec!["bug".to_string()]),
            assignees: None,
        };
        assert_eq!(data.title, "New Issue");
        assert!(data.body.is_some());
    }
}
