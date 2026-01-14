//! Knowledge linking services.
//!
//! Provides shared business logic for linking knowledge entries to projects,
//! used by both the REST API and MCP server.

use kix_projects::SharedEventBus;
use kix_store::{ProjectEntryRecord, ProjectStore};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use crate::error::{ServiceError, ServiceResult};

// =============================================================================
// TYPES
// =============================================================================

/// Linked entry information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkedEntry {
    pub link_id: String,
    pub project_id: String,
    pub entry_id: String,
    pub relevance: Option<f64>,
    pub notes: Option<String>,
    pub linked_at: String,
}

/// List of linked entries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkedEntryList {
    pub entries: Vec<LinkedEntry>,
    pub total: usize,
}

/// Filter options for listing linked entries.
#[derive(Debug, Clone, Default)]
pub struct LinkedEntryFilters {
    pub entry_type: Option<String>,
}

/// Result of link operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkResult {
    pub link_id: String,
    pub project_id: String,
    pub entry_id: String,
}

/// Options for search within project scope.
#[derive(Debug, Clone, Default)]
pub struct ProjectSearchOptions {
    /// Search type: "all", "issues", or "knowledge"
    pub search_type: Option<String>,
    /// Include closed issues
    pub include_closed: bool,
    /// Maximum results
    pub limit: usize,
}

/// Result from project-scoped search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSearchResult {
    pub issues: Vec<IssueSearchHit>,
    pub entries: Vec<EntrySearchHit>,
}

/// Issue search hit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueSearchHit {
    pub id: String,
    pub number: u32,
    pub title: String,
    pub state: String,
    pub score: f32,
}

/// Entry search hit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntrySearchHit {
    pub id: String,
    pub title: String,
    pub entry_type: String,
    pub score: f32,
}

// =============================================================================
// SERVICE FUNCTIONS
// =============================================================================

/// Link a knowledge entry to a project.
///
/// # Arguments
/// * `store` - The ProjectStore instance
/// * `event_bus` - Optional event bus for notifications
/// * `project_id_or_slug` - Project ID or slug
/// * `entry_id` - Knowledge entry ID
/// * `relevance` - Optional relevance score (0.0 to 1.0)
/// * `notes` - Optional notes about the link
///
/// # Returns
/// Link result
pub async fn link_entry(
    store: &Arc<RwLock<ProjectStore>>,
    event_bus: Option<&SharedEventBus>,
    project_id_or_slug: &str,
    entry_id: &str,
    relevance: Option<f64>,
    notes: Option<&str>,
) -> ServiceResult<LinkResult> {
    info!(
        "Link entry {} to project {}",
        entry_id, project_id_or_slug
    );

    let store_guard = store.write().await;

    // Get project to validate it exists
    let project = store_guard
        .get_project(project_id_or_slug)
        .await
        .map_err(ServiceError::Store)?
        .ok_or_else(|| ServiceError::not_found("Project", project_id_or_slug))?;

    // Create link record
    let mut link = ProjectEntryRecord::new(&project.id, entry_id);
    if let Some(r) = relevance {
        link = link.with_relevance(r);
    }
    if let Some(n) = notes {
        link = link.with_notes(n);
    }
    let link_id = link.id.clone();

    // Store the link
    store_guard
        .link_entry(&link)
        .await
        .map_err(ServiceError::Store)?;

    drop(store_guard);

    // Emit event
    if let Some(bus) = event_bus {
        bus.entry_linked(&project.id, entry_id, &link_id);
    }

    Ok(LinkResult {
        link_id,
        project_id: project.id,
        entry_id: entry_id.to_string(),
    })
}

/// Unlink a knowledge entry from a project.
///
/// # Arguments
/// * `store` - The ProjectStore instance
/// * `event_bus` - Optional event bus for notifications
/// * `project_id_or_slug` - Project ID or slug
/// * `entry_id` - Knowledge entry ID
///
/// # Returns
/// Success or error
pub async fn unlink_entry(
    store: &Arc<RwLock<ProjectStore>>,
    event_bus: Option<&SharedEventBus>,
    project_id_or_slug: &str,
    entry_id: &str,
) -> ServiceResult<()> {
    info!(
        "Unlink entry {} from project {}",
        entry_id, project_id_or_slug
    );

    let store_guard = store.write().await;

    // Get project to validate it exists
    let project = store_guard
        .get_project(project_id_or_slug)
        .await
        .map_err(ServiceError::Store)?
        .ok_or_else(|| ServiceError::not_found("Project", project_id_or_slug))?;

    // Unlink the entry
    let removed = store_guard
        .unlink_entry(&project.id, entry_id)
        .await
        .map_err(ServiceError::Store)?;

    if !removed {
        return Err(ServiceError::not_found("Link", entry_id));
    }

    drop(store_guard);

    // Emit event
    if let Some(bus) = event_bus {
        bus.entry_unlinked(&project.id, entry_id);
    }

    Ok(())
}

/// List all entries linked to a project.
///
/// # Arguments
/// * `store` - The ProjectStore instance
/// * `project_id_or_slug` - Project ID or slug
/// * `filters` - Optional filters
///
/// # Returns
/// List of linked entries
pub async fn list_linked_entries(
    store: &Arc<RwLock<ProjectStore>>,
    project_id_or_slug: &str,
    _filters: LinkedEntryFilters,
) -> ServiceResult<LinkedEntryList> {
    info!("List linked entries for project {}", project_id_or_slug);

    let store_guard = store.read().await;

    // Get project to validate it exists
    let project = store_guard
        .get_project(project_id_or_slug)
        .await
        .map_err(ServiceError::Store)?
        .ok_or_else(|| ServiceError::not_found("Project", project_id_or_slug))?;

    // Get linked entries
    let links = store_guard
        .list_project_entries(&project.id)
        .await
        .map_err(ServiceError::Store)?;

    let total = links.len();
    let entries: Vec<LinkedEntry> = links
        .into_iter()
        .map(|link| LinkedEntry {
            link_id: link.id,
            project_id: link.project_id,
            entry_id: link.entry_id,
            relevance: link.relevance,
            notes: link.notes,
            linked_at: link.linked_at,
        })
        .collect();

    Ok(LinkedEntryList { entries, total })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linked_entry() {
        let entry = LinkedEntry {
            link_id: "link-1".to_string(),
            project_id: "project-1".to_string(),
            entry_id: "entry-1".to_string(),
            relevance: Some(0.8f64),
            notes: Some("Important reference".to_string()),
            linked_at: "2024-01-01T00:00:00Z".to_string(),
        };
        assert_eq!(entry.relevance, Some(0.8f64));
    }

    #[test]
    fn test_project_search_options_default() {
        let opts = ProjectSearchOptions::default();
        assert!(opts.search_type.is_none());
        assert!(!opts.include_closed);
        assert_eq!(opts.limit, 0);
    }

    #[test]
    fn test_link_result() {
        let result = LinkResult {
            link_id: "link-123".to_string(),
            project_id: "project-1".to_string(),
            entry_id: "entry-1".to_string(),
        };
        assert!(!result.link_id.is_empty());
    }
}
