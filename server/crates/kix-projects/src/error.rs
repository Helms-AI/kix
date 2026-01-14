//! Error types for the kix-projects crate.

use thiserror::Error;

/// Errors that can occur in project management operations.
#[derive(Debug, Error)]
pub enum ProjectError {
    /// Project not found by ID or name
    #[error("Project not found: {0}")]
    ProjectNotFound(String),

    /// Project already exists with the same name
    #[error("Project already exists: {0}")]
    ProjectExists(String),

    /// Issue not found
    #[error("Issue not found: project={project}, issue={issue}")]
    IssueNotFound { project: String, issue: String },

    /// GitHub repository required but not configured
    #[error("GitHub repository required for this operation")]
    GitHubRepoRequired,

    /// GitHub token not found
    #[error("GitHub token not found for project: {0}")]
    GitHubTokenNotFound(String),

    /// GitHub API error
    #[error("GitHub API error: {0}")]
    GitHubApi(String),

    /// GitHub GraphQL error
    #[error("GitHub GraphQL error: {message}")]
    GitHubGraphQL { message: String },

    /// General GitHub error (REST or GraphQL)
    #[error("GitHub error: {0}")]
    GitHub(String),

    /// Token storage error
    #[error("Token storage error: {0}")]
    TokenStorage(String),

    /// Invalid project template
    #[error("Invalid project template: {0}")]
    InvalidTemplate(String),

    /// Storage error
    #[error("Storage error: {0}")]
    Storage(String),

    /// Validation error
    #[error("Validation error: {0}")]
    Validation(String),

    /// Entry not found
    #[error("Entry not found: {0}")]
    EntryNotFound(String),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<anyhow::Error> for ProjectError {
    fn from(err: anyhow::Error) -> Self {
        ProjectError::Internal(err.to_string())
    }
}

impl From<reqwest::Error> for ProjectError {
    fn from(err: reqwest::Error) -> Self {
        ProjectError::GitHubApi(err.to_string())
    }
}

impl From<serde_json::Error> for ProjectError {
    fn from(err: serde_json::Error) -> Self {
        ProjectError::Internal(format!("JSON error: {}", err))
    }
}
