//! Project management services.
//!
//! Provides shared business logic for project CRUD operations used by
//! both the REST API and MCP server.

use kix_projects::SharedEventBus;
use kix_store::projects::ProjectStore;
use kix_store::ProjectRecord;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use crate::error::{ServiceError, ServiceResult};
use crate::Pagination;

// =============================================================================
// TYPES
// =============================================================================

/// Project summary for list operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub color: Option<String>,
    pub github_owner: String,
    pub github_repo: String,
    pub has_github: bool,
    pub archived: bool,
    pub created_at: String,
}

impl From<ProjectRecord> for ProjectSummary {
    fn from(p: ProjectRecord) -> Self {
        let has_github = p.has_github();
        let github_owner = p.github_owner().unwrap_or_default().to_string();
        let github_repo = p.github_repo().unwrap_or_default().to_string();
        let archived = p.is_archived();
        Self {
            id: p.id,
            name: p.name,
            slug: p.slug,
            description: p.description,
            color: p.color,
            github_owner,
            github_repo,
            has_github,
            archived,
            created_at: p.created_at,
        }
    }
}

/// Project list with pagination.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectList {
    pub projects: Vec<ProjectSummary>,
    pub total: usize,
    pub has_more: bool,
}

/// Project statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectStats {
    pub open_issues: usize,
    pub closed_issues: usize,
    pub total_issues: usize,
    pub linked_entries: usize,
}

/// Detailed project information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectDetail {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub color: Option<String>,
    pub github_owner: String,
    pub github_repo: String,
    pub github_url: Option<String>,
    pub has_github: bool,
    pub archived: bool,
    pub created_at: String,
    pub updated_at: String,
    pub stats: Option<ProjectStats>,
}

/// Filter options for listing projects.
#[derive(Debug, Clone, Default)]
pub struct ProjectFilters {
    pub include_archived: bool,
}

/// Updates to apply to a project.
#[derive(Debug, Clone, Default)]
pub struct ProjectUpdates {
    pub name: Option<String>,
    pub description: Option<String>,
    pub color: Option<String>,
    pub archived: Option<bool>,
}

/// Options for deleting a project.
#[derive(Debug, Clone, Default)]
pub struct DeleteProjectOptions {
    pub delete_issues: bool,
}

/// Result of project deletion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteProjectResult {
    pub issues_deleted: usize,
    pub entries_unlinked: usize,
}

// =============================================================================
// SERVICE FUNCTIONS
// =============================================================================

/// List all projects with optional filtering and pagination.
///
/// # Arguments
/// * `store` - The ProjectStore instance
/// * `filters` - Filter options (include_archived, etc.)
/// * `pagination` - Limit and offset
///
/// # Returns
/// List of project summaries with pagination info
pub async fn list_projects(
    store: &Arc<RwLock<ProjectStore>>,
    filters: ProjectFilters,
    pagination: Pagination,
) -> ServiceResult<ProjectList> {
    info!(
        "List projects (archived: {}, limit: {}, offset: {})",
        filters.include_archived, pagination.limit, pagination.offset
    );

    let store_guard = store.read().await;
    let projects = store_guard
        .list_projects(filters.include_archived)
        .await
        .map_err(ServiceError::Store)?;

    let total = projects.len();
    let paginated: Vec<ProjectSummary> = projects
        .into_iter()
        .skip(pagination.offset)
        .take(pagination.limit)
        .map(ProjectSummary::from)
        .collect();
    let has_more = total > pagination.offset + pagination.limit;

    Ok(ProjectList {
        projects: paginated,
        total,
        has_more,
    })
}

/// Get a project by ID or slug with optional stats.
///
/// # Arguments
/// * `store` - The ProjectStore instance
/// * `id_or_slug` - Project ID or slug
/// * `include_stats` - Whether to include issue counts
///
/// # Returns
/// Detailed project information
pub async fn get_project(
    store: &Arc<RwLock<ProjectStore>>,
    id_or_slug: &str,
    include_stats: bool,
) -> ServiceResult<ProjectDetail> {
    info!("Get project: {} (stats: {})", id_or_slug, include_stats);

    let store_guard = store.read().await;
    let project = store_guard
        .get_project(id_or_slug)
        .await
        .map_err(ServiceError::Store)?
        .ok_or_else(|| ServiceError::not_found("Project", id_or_slug))?;

    // Get stats if requested
    let stats = if include_stats {
        let issues = store_guard
            .list_issues(&project.id, None, 10000, 0)
            .await
            .unwrap_or_default();
        let open_count = issues.iter().filter(|i| i.state == "open").count();
        let closed_count = issues.len() - open_count;
        let entries = store_guard
            .list_project_entries(&project.id)
            .await
            .unwrap_or_default();
        Some(ProjectStats {
            open_issues: open_count,
            closed_issues: closed_count,
            total_issues: issues.len(),
            linked_entries: entries.len(),
        })
    } else {
        None
    };

    let github_owner = project.github_owner().unwrap_or_default().to_string();
    let github_repo = project.github_repo().unwrap_or_default().to_string();
    let has_github = project.has_github();
    let github_url = if has_github {
        Some(format!("https://github.com/{}/{}", github_owner, github_repo))
    } else {
        None
    };
    let archived = project.is_archived();

    Ok(ProjectDetail {
        id: project.id,
        name: project.name,
        slug: project.slug,
        description: project.description,
        color: project.color,
        github_owner,
        github_repo,
        github_url,
        has_github,
        archived,
        created_at: project.created_at,
        updated_at: project.updated_at,
        stats,
    })
}

/// Update a project's properties.
///
/// # Arguments
/// * `store` - The ProjectStore instance
/// * `event_bus` - Optional event bus for notifications
/// * `id_or_slug` - Project ID or slug
/// * `updates` - Fields to update
///
/// # Returns
/// Updated project summary
pub async fn update_project(
    store: &Arc<RwLock<ProjectStore>>,
    _event_bus: Option<&SharedEventBus>,
    id_or_slug: &str,
    updates: ProjectUpdates,
) -> ServiceResult<ProjectSummary> {
    info!("Update project: {}", id_or_slug);

    let store_guard = store.read().await;
    let mut project = store_guard
        .get_project(id_or_slug)
        .await
        .map_err(ServiceError::Store)?
        .ok_or_else(|| ServiceError::not_found("Project", id_or_slug))?;
    drop(store_guard);

    // Apply updates
    if let Some(name) = updates.name {
        project.name = name;
    }
    if let Some(description) = updates.description {
        project.description = Some(description);
    }
    if let Some(color) = updates.color {
        project.color = Some(color);
    }
    if let Some(archived) = updates.archived {
        project.archived = if archived { 1 } else { 0 };
    }

    // Update timestamp
    project.updated_at = chrono::Utc::now().to_rfc3339();

    // Save
    let store_guard = store.write().await;
    store_guard
        .update_project(&project)
        .await
        .map_err(ServiceError::Store)?;

    // TODO: Emit event if event_bus is provided
    // if let Some(bus) = event_bus {
    //     bus.emit(ProjectEvent::ProjectUpdated { ... });
    // }

    Ok(ProjectSummary::from(project))
}

/// Delete a project and optionally its issues.
///
/// Note: Does NOT delete from GitHub - only local data.
///
/// # Arguments
/// * `store` - The ProjectStore instance
/// * `event_bus` - Optional event bus for notifications
/// * `id_or_slug` - Project ID or slug
/// * `options` - Delete options (whether to delete issues)
///
/// # Returns
/// Deletion statistics
pub async fn delete_project(
    store: &Arc<RwLock<ProjectStore>>,
    _event_bus: Option<&SharedEventBus>,
    id_or_slug: &str,
    options: DeleteProjectOptions,
) -> ServiceResult<DeleteProjectResult> {
    info!(
        "Delete project: {} (delete_issues: {})",
        id_or_slug, options.delete_issues
    );

    let store_guard = store.read().await;
    let project = store_guard
        .get_project(id_or_slug)
        .await
        .map_err(ServiceError::Store)?
        .ok_or_else(|| ServiceError::not_found("Project", id_or_slug))?;
    drop(store_guard);

    let project_id = project.id.clone();
    let mut issues_deleted = 0;
    let mut entries_unlinked = 0;

    let store_guard = store.write().await;

    // Delete issues if requested
    if options.delete_issues {
        let issues = store_guard
            .list_issues(&project_id, None, 10000, 0)
            .await
            .unwrap_or_default();
        for issue in &issues {
            let _ = store_guard.delete_issue(&issue.id).await;
            issues_deleted += 1;
        }
    }

    // Unlink entries
    let entries = store_guard
        .list_project_entries(&project_id)
        .await
        .unwrap_or_default();
    for entry in &entries {
        let _ = store_guard
            .unlink_entry(&project_id, &entry.entry_id)
            .await;
        entries_unlinked += 1;
    }

    // Delete the project
    store_guard
        .delete_project(&project_id)
        .await
        .map_err(ServiceError::Store)?;

    // TODO: Emit event if event_bus is provided

    Ok(DeleteProjectResult {
        issues_deleted,
        entries_unlinked,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_summary_from_record() {
        let record = ProjectRecord::new_local("Test Project".to_string());
        let summary = ProjectSummary::from(record);
        assert_eq!(summary.name, "Test Project");
        assert!(!summary.has_github);
    }

    #[test]
    fn test_project_filters_default() {
        let filters = ProjectFilters::default();
        assert!(!filters.include_archived);
    }
}
