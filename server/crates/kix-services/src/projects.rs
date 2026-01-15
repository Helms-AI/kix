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
    pub archived: bool,
    pub created_at: String,
}

impl From<ProjectRecord> for ProjectSummary {
    fn from(p: ProjectRecord) -> Self {
        let archived = p.is_archived();
        Self {
            id: p.id,
            name: p.name,
            slug: p.slug,
            description: p.description,
            color: p.color,
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
    pub open_items: usize,
    pub closed_items: usize,
    pub total_items: usize,
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
    pub delete_items: bool,
}

/// Result of project deletion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteProjectResult {
    pub items_deleted: usize,
    pub entries_unlinked: usize,
}

/// Data for creating a new project.
#[derive(Debug, Clone)]
pub struct CreateProjectData {
    pub name: String,
    pub description: Option<String>,
    pub color: Option<String>,
}

/// Result of project creation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProjectResult {
    pub project_id: String,
    pub name: String,
    pub slug: String,
}

// =============================================================================
// SERVICE FUNCTIONS
// =============================================================================

/// Create a new project.
///
/// # Arguments
/// * `store` - The ProjectStore instance
/// * `event_bus` - Optional event bus for notifications
/// * `data` - Project creation data
///
/// # Returns
/// Created project result
pub async fn create_project(
    store: &Arc<RwLock<ProjectStore>>,
    event_bus: Option<&SharedEventBus>,
    data: CreateProjectData,
) -> ServiceResult<CreateProjectResult> {
    info!("Create project: {}", data.name);

    // Create project record
    let mut project = ProjectRecord::new(data.name.clone());

    if let Some(description) = data.description {
        project.description = Some(description);
    }
    if let Some(color) = data.color {
        project.color = Some(color);
    }

    let project_id = project.id.clone();
    let project_name = project.name.clone();
    let project_slug = project.slug.clone();

    // Store the project
    let store_guard = store.write().await;
    store_guard
        .create_project(&project)
        .await
        .map_err(ServiceError::Store)?;
    drop(store_guard);

    // Emit event
    if let Some(bus) = event_bus {
        bus.project_created(&project_id, &project_name);
    }

    Ok(CreateProjectResult {
        project_id,
        name: project_name,
        slug: project_slug,
    })
}

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
/// * `include_stats` - Whether to include work item counts
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
        let items = store_guard
            .list_work_items(&project.id, None, 10000, 0)
            .await
            .unwrap_or_default();
        let open_count = items.iter().filter(|i| i.state == "open").count();
        let closed_count = items.len() - open_count;
        let entries = store_guard
            .list_project_entries(&project.id)
            .await
            .unwrap_or_default();
        Some(ProjectStats {
            open_items: open_count,
            closed_items: closed_count,
            total_items: items.len(),
            linked_entries: entries.len(),
        })
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

/// Delete a project and optionally its work items.
///
/// Note: Only deletes local data.
///
/// # Arguments
/// * `store` - The ProjectStore instance
/// * `event_bus` - Optional event bus for notifications
/// * `id_or_slug` - Project ID or slug
/// * `options` - Delete options (whether to delete work items)
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
        "Delete project: {} (delete_items: {})",
        id_or_slug, options.delete_items
    );

    let store_guard = store.read().await;
    let project = store_guard
        .get_project(id_or_slug)
        .await
        .map_err(ServiceError::Store)?
        .ok_or_else(|| ServiceError::not_found("Project", id_or_slug))?;
    drop(store_guard);

    let project_id = project.id.clone();
    let mut items_deleted = 0;
    let mut entries_unlinked = 0;

    let store_guard = store.write().await;

    // Delete work items if requested
    if options.delete_items {
        let items = store_guard
            .list_work_items(&project_id, None, 10000, 0)
            .await
            .unwrap_or_default();
        for item in &items {
            let _ = store_guard.delete_work_item(&item.id).await;
            items_deleted += 1;
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
        items_deleted,
        entries_unlinked,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_summary_from_record() {
        let record = ProjectRecord::new("Test Project".to_string());
        let summary = ProjectSummary::from(record);
        assert_eq!(summary.name, "Test Project");
        assert!(!summary.archived);
    }

    #[test]
    fn test_project_filters_default() {
        let filters = ProjectFilters::default();
        assert!(!filters.include_archived);
    }
}
