//! GitHub REST API client for issue operations.
//!
//! This module provides a client for interacting with GitHub's REST API,
//! specifically for issue management operations.

use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT};
use reqwest::Client;
use serde_json::Value;
use tracing::{debug, info};

use crate::error::ProjectError;
use crate::github::models::{
    AuthenticatedUser, CreateIssueRequest, GitHubIssue, GitHubLabel, GitHubOrg, GitHubRepository,
    SearchReposResult, UpdateIssueRequest,
};

/// GitHub REST API base URL.
const GITHUB_API_BASE: &str = "https://api.github.com";

/// User agent for API requests.
const USER_AGENT_VALUE: &str = "kix-projects/0.1.0";

/// GitHub REST API client.
#[derive(Clone)]
pub struct GitHubRestClient {
    client: Client,
    token: String,
}

impl GitHubRestClient {
    /// Create a new GitHub REST client with the given personal access token.
    pub fn new(token: impl Into<String>) -> Result<Self, ProjectError> {
        let token = token.into();

        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));
        headers.insert(ACCEPT, HeaderValue::from_static("application/vnd.github+json"));
        headers.insert(
            "X-GitHub-Api-Version",
            HeaderValue::from_static("2022-11-28"),
        );

        let client = Client::builder()
            .default_headers(headers)
            .build()
            .map_err(|e| ProjectError::GitHub(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self { client, token })
    }

    /// Add authorization header to a request.
    fn auth_header(&self) -> HeaderValue {
        HeaderValue::from_str(&format!("Bearer {}", self.token))
            .unwrap_or_else(|_| HeaderValue::from_static(""))
    }

    // ========================================================================
    // Repository Operations
    // ========================================================================

    /// Get repository information.
    pub async fn get_repository(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<GitHubRepository, ProjectError> {
        let url = format!("{}/repos/{}/{}", GITHUB_API_BASE, owner, repo);

        let response = self
            .client
            .get(&url)
            .header(AUTHORIZATION, self.auth_header())
            .send()
            .await
            .map_err(|e| ProjectError::GitHub(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ProjectError::GitHub(format!(
                "Failed to get repository: {} - {}",
                status, body
            )));
        }

        response
            .json()
            .await
            .map_err(|e| ProjectError::GitHub(format!("Failed to parse repository: {}", e)))
    }

    /// Verify that the token has access to the repository.
    pub async fn verify_access(&self, owner: &str, repo: &str) -> Result<bool, ProjectError> {
        match self.get_repository(owner, repo).await {
            Ok(_) => Ok(true),
            Err(ProjectError::GitHub(msg)) if msg.contains("404") => Ok(false),
            Err(e) => Err(e),
        }
    }

    // ========================================================================
    // Issue Operations
    // ========================================================================

    /// List issues for a repository.
    pub async fn list_issues(
        &self,
        owner: &str,
        repo: &str,
        state: Option<&str>,
        per_page: Option<u32>,
        page: Option<u32>,
    ) -> Result<Vec<GitHubIssue>, ProjectError> {
        let mut url = format!("{}/repos/{}/{}/issues", GITHUB_API_BASE, owner, repo);

        let mut params = vec![];
        if let Some(s) = state {
            params.push(format!("state={}", s));
        }
        if let Some(pp) = per_page {
            params.push(format!("per_page={}", pp.min(100)));
        }
        if let Some(p) = page {
            params.push(format!("page={}", p));
        }
        // Filter out pull requests (GitHub API returns PRs in issues endpoint)
        params.push("filter=all".to_string());

        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }

        debug!("Fetching issues from: {}", url);

        let response = self
            .client
            .get(&url)
            .header(AUTHORIZATION, self.auth_header())
            .send()
            .await
            .map_err(|e| ProjectError::GitHub(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ProjectError::GitHub(format!(
                "Failed to list issues: {} - {}",
                status, body
            )));
        }

        let issues: Vec<Value> = response
            .json()
            .await
            .map_err(|e| ProjectError::GitHub(format!("Failed to parse issues: {}", e)))?;

        // Filter out pull requests
        let issues: Vec<GitHubIssue> = issues
            .into_iter()
            .filter(|v| v.get("pull_request").is_none())
            .filter_map(|v| serde_json::from_value(v).ok())
            .collect();

        info!("Fetched {} issues from {}/{}", issues.len(), owner, repo);
        Ok(issues)
    }

    /// Get a single issue by number.
    pub async fn get_issue(
        &self,
        owner: &str,
        repo: &str,
        issue_number: u32,
    ) -> Result<GitHubIssue, ProjectError> {
        let url = format!(
            "{}/repos/{}/{}/issues/{}",
            GITHUB_API_BASE, owner, repo, issue_number
        );

        let response = self
            .client
            .get(&url)
            .header(AUTHORIZATION, self.auth_header())
            .send()
            .await
            .map_err(|e| ProjectError::GitHub(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ProjectError::GitHub(format!(
                "Failed to get issue #{}: {} - {}",
                issue_number, status, body
            )));
        }

        response
            .json()
            .await
            .map_err(|e| ProjectError::GitHub(format!("Failed to parse issue: {}", e)))
    }

    /// Create a new issue.
    pub async fn create_issue(
        &self,
        owner: &str,
        repo: &str,
        request: &CreateIssueRequest,
    ) -> Result<GitHubIssue, ProjectError> {
        let url = format!("{}/repos/{}/{}/issues", GITHUB_API_BASE, owner, repo);

        let response = self
            .client
            .post(&url)
            .header(AUTHORIZATION, self.auth_header())
            .json(request)
            .send()
            .await
            .map_err(|e| ProjectError::GitHub(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ProjectError::GitHub(format!(
                "Failed to create issue: {} - {}",
                status, body
            )));
        }

        let issue: GitHubIssue = response
            .json()
            .await
            .map_err(|e| ProjectError::GitHub(format!("Failed to parse issue: {}", e)))?;

        info!(
            "Created issue #{}: {} in {}/{}",
            issue.number, issue.title, owner, repo
        );
        Ok(issue)
    }

    /// Update an existing issue.
    pub async fn update_issue(
        &self,
        owner: &str,
        repo: &str,
        issue_number: u32,
        request: &UpdateIssueRequest,
    ) -> Result<GitHubIssue, ProjectError> {
        let url = format!(
            "{}/repos/{}/{}/issues/{}",
            GITHUB_API_BASE, owner, repo, issue_number
        );

        let response = self
            .client
            .patch(&url)
            .header(AUTHORIZATION, self.auth_header())
            .json(request)
            .send()
            .await
            .map_err(|e| ProjectError::GitHub(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ProjectError::GitHub(format!(
                "Failed to update issue #{}: {} - {}",
                issue_number, status, body
            )));
        }

        let issue: GitHubIssue = response
            .json()
            .await
            .map_err(|e| ProjectError::GitHub(format!("Failed to parse issue: {}", e)))?;

        info!(
            "Updated issue #{}: {} in {}/{}",
            issue.number, issue.title, owner, repo
        );
        Ok(issue)
    }

    /// Close an issue.
    pub async fn close_issue(
        &self,
        owner: &str,
        repo: &str,
        issue_number: u32,
    ) -> Result<GitHubIssue, ProjectError> {
        self.update_issue(
            owner,
            repo,
            issue_number,
            &UpdateIssueRequest {
                title: None,
                body: None,
                state: Some("closed".to_string()),
                labels: None,
                assignees: None,
            },
        )
        .await
    }

    /// Reopen an issue.
    pub async fn reopen_issue(
        &self,
        owner: &str,
        repo: &str,
        issue_number: u32,
    ) -> Result<GitHubIssue, ProjectError> {
        self.update_issue(
            owner,
            repo,
            issue_number,
            &UpdateIssueRequest {
                title: None,
                body: None,
                state: Some("open".to_string()),
                labels: None,
                assignees: None,
            },
        )
        .await
    }

    // ========================================================================
    // Label Operations
    // ========================================================================

    /// List labels for a repository.
    pub async fn list_labels(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<Vec<GitHubLabel>, ProjectError> {
        let url = format!("{}/repos/{}/{}/labels", GITHUB_API_BASE, owner, repo);

        let response = self
            .client
            .get(&url)
            .header(AUTHORIZATION, self.auth_header())
            .send()
            .await
            .map_err(|e| ProjectError::GitHub(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ProjectError::GitHub(format!(
                "Failed to list labels: {} - {}",
                status, body
            )));
        }

        response
            .json()
            .await
            .map_err(|e| ProjectError::GitHub(format!("Failed to parse labels: {}", e)))
    }

    /// Create a new label.
    pub async fn create_label(
        &self,
        owner: &str,
        repo: &str,
        name: &str,
        color: &str,
        description: Option<&str>,
    ) -> Result<GitHubLabel, ProjectError> {
        let url = format!("{}/repos/{}/{}/labels", GITHUB_API_BASE, owner, repo);

        let mut body = serde_json::json!({
            "name": name,
            "color": color.trim_start_matches('#'),
        });

        if let Some(desc) = description {
            body["description"] = serde_json::Value::String(desc.to_string());
        }

        let response = self
            .client
            .post(&url)
            .header(AUTHORIZATION, self.auth_header())
            .json(&body)
            .send()
            .await
            .map_err(|e| ProjectError::GitHub(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ProjectError::GitHub(format!(
                "Failed to create label: {} - {}",
                status, body
            )));
        }

        response
            .json()
            .await
            .map_err(|e| ProjectError::GitHub(format!("Failed to parse label: {}", e)))
    }

    // ========================================================================
    // User & Organization Operations
    // ========================================================================

    /// Get the authenticated user's information.
    pub async fn get_authenticated_user(&self) -> Result<AuthenticatedUser, ProjectError> {
        let url = format!("{}/user", GITHUB_API_BASE);

        let response = self
            .client
            .get(&url)
            .header(AUTHORIZATION, self.auth_header())
            .send()
            .await
            .map_err(|e| ProjectError::GitHub(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ProjectError::GitHub(format!(
                "Failed to get authenticated user: {} - {}",
                status, body
            )));
        }

        response
            .json()
            .await
            .map_err(|e| ProjectError::GitHub(format!("Failed to parse user: {}", e)))
    }

    /// List organizations for the authenticated user.
    pub async fn list_user_orgs(&self) -> Result<Vec<GitHubOrg>, ProjectError> {
        let url = format!("{}/user/orgs", GITHUB_API_BASE);

        let response = self
            .client
            .get(&url)
            .header(AUTHORIZATION, self.auth_header())
            .send()
            .await
            .map_err(|e| ProjectError::GitHub(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ProjectError::GitHub(format!(
                "Failed to list organizations: {} - {}",
                status, body
            )));
        }

        response
            .json()
            .await
            .map_err(|e| ProjectError::GitHub(format!("Failed to parse organizations: {}", e)))
    }

    /// List repositories for a user.
    pub async fn list_user_repos(
        &self,
        username: &str,
        per_page: Option<u32>,
        page: Option<u32>,
    ) -> Result<Vec<GitHubRepository>, ProjectError> {
        let mut url = format!("{}/users/{}/repos", GITHUB_API_BASE, username);

        let mut params = vec!["sort=updated".to_string()];
        if let Some(pp) = per_page {
            params.push(format!("per_page={}", pp.min(100)));
        }
        if let Some(p) = page {
            params.push(format!("page={}", p));
        }

        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }

        debug!("Fetching user repos from: {}", url);

        let response = self
            .client
            .get(&url)
            .header(AUTHORIZATION, self.auth_header())
            .send()
            .await
            .map_err(|e| ProjectError::GitHub(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ProjectError::GitHub(format!(
                "Failed to list user repos: {} - {}",
                status, body
            )));
        }

        let repos: Vec<GitHubRepository> = response
            .json()
            .await
            .map_err(|e| ProjectError::GitHub(format!("Failed to parse repos: {}", e)))?;

        info!("Fetched {} repos for user {}", repos.len(), username);
        Ok(repos)
    }

    /// List repositories for an organization.
    pub async fn list_org_repos(
        &self,
        org: &str,
        per_page: Option<u32>,
        page: Option<u32>,
    ) -> Result<Vec<GitHubRepository>, ProjectError> {
        let mut url = format!("{}/orgs/{}/repos", GITHUB_API_BASE, org);

        let mut params = vec!["sort=updated".to_string()];
        if let Some(pp) = per_page {
            params.push(format!("per_page={}", pp.min(100)));
        }
        if let Some(p) = page {
            params.push(format!("page={}", p));
        }

        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }

        debug!("Fetching org repos from: {}", url);

        let response = self
            .client
            .get(&url)
            .header(AUTHORIZATION, self.auth_header())
            .send()
            .await
            .map_err(|e| ProjectError::GitHub(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ProjectError::GitHub(format!(
                "Failed to list org repos: {} - {}",
                status, body
            )));
        }

        let repos: Vec<GitHubRepository> = response
            .json()
            .await
            .map_err(|e| ProjectError::GitHub(format!("Failed to parse repos: {}", e)))?;

        info!("Fetched {} repos for org {}", repos.len(), org);
        Ok(repos)
    }

    /// Search repositories by owner and optional query.
    pub async fn search_repos(
        &self,
        owner: &str,
        query: Option<&str>,
        per_page: Option<u32>,
        page: Option<u32>,
    ) -> Result<SearchReposResult, ProjectError> {
        // Build search query: user:{owner} or org:{owner} + optional text
        let q = if let Some(q) = query {
            format!("user:{} {} in:name", owner, q)
        } else {
            format!("user:{}", owner)
        };

        let mut url = format!("{}/search/repositories?q={}", GITHUB_API_BASE, urlencoding::encode(&q));

        if let Some(pp) = per_page {
            url.push_str(&format!("&per_page={}", pp.min(100)));
        }
        if let Some(p) = page {
            url.push_str(&format!("&page={}", p));
        }
        url.push_str("&sort=updated");

        debug!("Searching repos: {}", url);

        let response = self
            .client
            .get(&url)
            .header(AUTHORIZATION, self.auth_header())
            .send()
            .await
            .map_err(|e| ProjectError::GitHub(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ProjectError::GitHub(format!(
                "Failed to search repos: {} - {}",
                status, body
            )));
        }

        let result: SearchReposResult = response
            .json()
            .await
            .map_err(|e| ProjectError::GitHub(format!("Failed to parse search result: {}", e)))?;

        info!("Found {} repos matching query", result.total_count);
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = GitHubRestClient::new("test_token");
        assert!(client.is_ok());
    }

    #[test]
    fn test_create_issue_request_serialization() {
        let request = CreateIssueRequest {
            title: "Test Issue".to_string(),
            body: Some("Test body".to_string()),
            labels: vec!["bug".to_string()],
            assignees: vec![],
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("Test Issue"));
        assert!(json.contains("bug"));
    }

    #[test]
    fn test_update_issue_request_serialization() {
        let request = UpdateIssueRequest {
            title: Some("Updated Title".to_string()),
            body: None,
            state: Some("closed".to_string()),
            labels: None,
            assignees: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("Updated Title"));
        assert!(json.contains("closed"));
        // Ensure None fields are not included
        assert!(!json.contains("body"));
    }
}
