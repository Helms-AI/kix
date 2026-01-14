//! GitHub integration services.
//!
//! Provides shared business logic for GitHub token management and API operations
//! used by both the REST API and MCP server.

use kix_projects::github::models::AuthenticatedUser;
use kix_projects::github::GitHubRestClient;
use kix_projects::{ProjectTokenType, TokenService};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;

use crate::error::{ServiceError, ServiceResult};

// =============================================================================
// TYPES
// =============================================================================

/// Token scope for setting/getting tokens.
#[derive(Debug, Clone)]
pub enum TokenScope {
    /// Global token (used as fallback)
    Global,
    /// Project-specific token
    Project(String),
}

/// Token status information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenStatus {
    pub has_token: bool,
    pub token_type: String,
    pub token_suffix: Option<String>,
    pub verified_user: Option<String>,
}

/// Result of setting a token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetTokenResult {
    pub success: bool,
    pub verified_user: Option<String>,
}

/// Verification result for repo access.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyAccessResult {
    pub has_access: bool,
    pub can_push: bool,
    pub can_pull: bool,
}

/// Organization item (includes user as first item).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrgItem {
    pub login: String,
    pub avatar_url: String,
    #[serde(rename = "type")]
    pub org_type: String,
    pub description: Option<String>,
}

/// Repository item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoItem {
    pub name: String,
    pub full_name: String,
    pub description: Option<String>,
    pub private: bool,
}

/// Repository list result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoListResult {
    pub repos: Vec<RepoItem>,
    pub has_more: bool,
}

/// Query parameters for repository search.
#[derive(Debug, Clone, Default)]
pub struct RepoSearchQuery {
    pub owner: String,
    pub search: Option<String>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

// =============================================================================
// TOKEN MANAGEMENT
// =============================================================================

/// Set a GitHub token (global or project-specific).
///
/// # Arguments
/// * `token_service` - Token service for token management
/// * `scope` - Token scope (global or project)
/// * `token` - The GitHub PAT
/// * `verify_repo` - Optional (owner, repo) to verify access
///
/// # Returns
/// Result with verification info
pub async fn set_token(
    token_service: &Arc<TokenService>,
    scope: TokenScope,
    token: &str,
    verify_repo: Option<(&str, &str)>,
) -> ServiceResult<SetTokenResult> {
    match scope {
        TokenScope::Global => {
            info!("Setting global GitHub token");
            let result = token_service
                .set_global_token(token, true)
                .await
                .map_err(|e| ServiceError::GitHub(format!("Failed to set global token: {}", e)))?;

            Ok(SetTokenResult {
                success: true,
                verified_user: result.verified_user,
            })
        }
        TokenScope::Project(project_id) => {
            info!("Setting GitHub token for project: {}", project_id);
            let result = token_service
                .set_project_token(&project_id, token, verify_repo)
                .await
                .map_err(|e| ServiceError::GitHub(format!("Failed to set project token: {}", e)))?;

            Ok(SetTokenResult {
                success: true,
                verified_user: result.verified_user,
            })
        }
    }
}

/// Get token status for a scope.
///
/// # Arguments
/// * `token_service` - Token service for token management
/// * `scope` - Token scope (global or project)
///
/// # Returns
/// Token status information
pub async fn get_token_status(
    token_service: &Arc<TokenService>,
    scope: TokenScope,
) -> ServiceResult<TokenStatus> {
    match scope {
        TokenScope::Global => {
            let has_token = token_service
                .has_global_token()
                .await
                .map_err(|e| ServiceError::GitHub(format!("Failed to check global token: {}", e)))?;

            let (token_suffix, verified_user) = if has_token {
                let info = token_service.get_global_token_info().await.ok().flatten();
                (
                    info.as_ref().and_then(|t| t.token_suffix.clone()),
                    info.as_ref().and_then(|t| t.verified_user.clone()),
                )
            } else {
                (None, None)
            };

            Ok(TokenStatus {
                has_token,
                token_type: "global".to_string(),
                token_suffix,
                verified_user,
            })
        }
        TokenScope::Project(project_id) => {
            let token_type = token_service
                .get_project_token_type(&project_id)
                .await
                .map_err(|e| {
                    ServiceError::GitHub(format!("Failed to get project token type: {}", e))
                })?;

            let (has_token, type_str) = match token_type {
                ProjectTokenType::NotConfigured => (false, "not_configured"),
                ProjectTokenType::ProjectSpecific { .. } => (true, "project_specific"),
                ProjectTokenType::UsingGlobal { .. } => (true, "global_fallback"),
            };

            Ok(TokenStatus {
                has_token,
                token_type: type_str.to_string(),
                token_suffix: None, // Could be enhanced to get this
                verified_user: None, // Could be enhanced to get this
            })
        }
    }
}

/// Get decrypted token for a project (project-specific or global fallback).
///
/// # Arguments
/// * `token_service` - Token service for token management
/// * `project_id` - Project ID
///
/// # Returns
/// Decrypted token string
pub async fn get_token_for_project(
    token_service: &Arc<TokenService>,
    project_id: &str,
) -> ServiceResult<String> {
    token_service
        .get_token_for_project(project_id)
        .await
        .map_err(|e| ServiceError::GitHub(format!("Failed to get token: {}", e)))
}

// =============================================================================
// GITHUB API OPERATIONS
// =============================================================================

/// Get the authenticated GitHub user.
///
/// # Arguments
/// * `token_service` - Token service for token management
/// * `scope` - Token scope to use
///
/// # Returns
/// GitHub user information
pub async fn get_authenticated_user(
    token_service: &Arc<TokenService>,
    scope: TokenScope,
) -> ServiceResult<AuthenticatedUser> {
    let token = get_token_by_scope(token_service, &scope).await?;

    let client = GitHubRestClient::new(&token)
        .map_err(|e| ServiceError::GitHub(format!("Failed to create client: {}", e)))?;

    client
        .get_authenticated_user()
        .await
        .map_err(|e| ServiceError::GitHub(format!("Failed to get user: {}", e)))
}

/// List organizations for the authenticated user.
///
/// Returns the user as the first "org" item, followed by actual organizations.
///
/// # Arguments
/// * `token_service` - Token service for token management
/// * `scope` - Token scope to use
///
/// # Returns
/// List of organizations (with user first)
pub async fn list_organizations(
    token_service: &Arc<TokenService>,
    scope: TokenScope,
) -> ServiceResult<Vec<OrgItem>> {
    let token = get_token_by_scope(token_service, &scope).await?;

    let client = GitHubRestClient::new(&token)
        .map_err(|e| ServiceError::GitHub(format!("Failed to create client: {}", e)))?;

    // Get user info first
    let user = client
        .get_authenticated_user()
        .await
        .map_err(|e| ServiceError::GitHub(format!("Failed to get user: {}", e)))?;

    // Start with user as first "org"
    let mut orgs = vec![OrgItem {
        login: user.login.clone(),
        avatar_url: user.avatar_url.clone(),
        org_type: "user".to_string(),
        description: user.name.clone(),
    }];

    // Get user's organizations
    match client.list_user_orgs().await {
        Ok(github_orgs) => {
            for org in github_orgs {
                orgs.push(OrgItem {
                    login: org.login,
                    avatar_url: org.avatar_url,
                    org_type: "org".to_string(),
                    description: org.description,
                });
            }
        }
        Err(e) => {
            // Log but don't fail - return user only
            tracing::warn!("Failed to list organizations: {}", e);
        }
    }

    Ok(orgs)
}

/// Search repositories for an owner.
///
/// # Arguments
/// * `token_service` - Token service for token management
/// * `scope` - Token scope to use
/// * `query` - Search query parameters
///
/// # Returns
/// List of repositories with pagination info
pub async fn search_repositories(
    token_service: &Arc<TokenService>,
    scope: TokenScope,
    query: RepoSearchQuery,
) -> ServiceResult<RepoListResult> {
    let token = get_token_by_scope(token_service, &scope).await?;

    let client = GitHubRestClient::new(&token)
        .map_err(|e| ServiceError::GitHub(format!("Failed to create client: {}", e)))?;

    let per_page = query.per_page.unwrap_or(50);
    let page = query.page.unwrap_or(1);

    let result = client
        .search_repos(&query.owner, query.search.as_deref(), Some(per_page), Some(page))
        .await
        .map_err(|e| ServiceError::GitHub(format!("Failed to search repos: {}", e)))?;

    let repos: Vec<RepoItem> = result
        .items
        .into_iter()
        .map(|r| RepoItem {
            name: r.name,
            full_name: r.full_name,
            description: r.description,
            private: r.private,
        })
        .collect();

    let has_more = result.total_count > (page * per_page);

    Ok(RepoListResult { repos, has_more })
}

/// Verify access to a repository.
///
/// # Arguments
/// * `token_service` - Token service for token management
/// * `scope` - Token scope to use (or specific token)
/// * `owner` - Repository owner
/// * `repo` - Repository name
/// * `explicit_token` - Optional explicit token to use instead of scope
///
/// # Returns
/// Access verification result
pub async fn verify_repo_access(
    token_service: &Arc<TokenService>,
    scope: TokenScope,
    owner: &str,
    repo: &str,
    explicit_token: Option<&str>,
) -> ServiceResult<VerifyAccessResult> {
    let token = if let Some(t) = explicit_token {
        t.to_string()
    } else {
        get_token_by_scope(token_service, &scope).await?
    };

    let client = GitHubRestClient::new(&token)
        .map_err(|e| ServiceError::GitHub(format!("Failed to create client: {}", e)))?;

    match client.verify_access(owner, repo).await {
        Ok(has_access) => Ok(VerifyAccessResult {
            has_access,
            can_push: has_access, // Simplified - could be enhanced
            can_pull: has_access,
        }),
        Err(_) => Ok(VerifyAccessResult {
            has_access: false,
            can_push: false,
            can_pull: false,
        }),
    }
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Get token by scope.
async fn get_token_by_scope(
    token_service: &Arc<TokenService>,
    scope: &TokenScope,
) -> ServiceResult<String> {
    match scope {
        TokenScope::Global => token_service
            .get_global_token_decrypted()
            .await
            .map_err(|e| ServiceError::Auth(format!("No global token: {}", e))),
        TokenScope::Project(project_id) => token_service
            .get_token_for_project(project_id)
            .await
            .map_err(|e| ServiceError::Auth(format!("No token for project: {}", e))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_scope() {
        let global = TokenScope::Global;
        let project = TokenScope::Project("project-1".to_string());

        match global {
            TokenScope::Global => (),
            _ => panic!("Expected Global"),
        }

        match project {
            TokenScope::Project(id) => assert_eq!(id, "project-1"),
            _ => panic!("Expected Project"),
        }
    }

    #[test]
    fn test_token_status() {
        let status = TokenStatus {
            has_token: true,
            token_type: "global".to_string(),
            token_suffix: Some("abc123".to_string()),
            verified_user: Some("testuser".to_string()),
        };
        assert!(status.has_token);
        assert_eq!(status.token_type, "global");
    }

    #[test]
    fn test_repo_search_query_default() {
        let query = RepoSearchQuery::default();
        assert!(query.owner.is_empty());
        assert!(query.search.is_none());
    }

    #[test]
    fn test_verify_access_result() {
        let result = VerifyAccessResult {
            has_access: true,
            can_push: true,
            can_pull: true,
        };
        assert!(result.has_access);
    }
}
