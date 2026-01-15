//! Work item management services.
//!
//! Provides shared business logic for work item CRUD operations,
//! used by both the REST API and MCP server.

use kix_projects::SharedEventBus;
use kix_store::{WorkItemRecord, ProjectStore};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use crate::error::{ServiceError, ServiceResult};
use crate::Pagination;

// =============================================================================
// TYPES
// =============================================================================

/// Work item summary for list operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkItemSummary {
    pub id: String,
    pub project_id: String,
    pub number: u32,
    pub title: String,
    pub body: Option<String>,
    pub state: String,
    pub labels: Vec<String>,
    pub assignees: Vec<String>,
    pub priority: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    // Board fields
    pub item_type: String,
    pub parent_id: Option<String>,
    pub position: i64,
    pub board_column: String,
    pub story_points: Option<i64>,
    pub epic_color: Option<String>,
}

impl From<&WorkItemRecord> for WorkItemSummary {
    fn from(item: &WorkItemRecord) -> Self {
        let priority = item.priority.map(|p| match p {
            1 => "critical".to_string(),
            2 => "high".to_string(),
            3 => "medium".to_string(),
            _ => "low".to_string(),
        });

        Self {
            id: item.id.clone(),
            project_id: item.project_id.clone(),
            number: item.number as u32,
            title: item.title.clone(),
            body: item.body.clone(),
            state: item.state.clone(),
            labels: item.labels_vec(),
            assignees: item.assignees_vec(),
            priority,
            created_at: item.created_at.clone(),
            updated_at: item.updated_at.clone(),
            // Board fields
            item_type: item.item_type.clone(),
            parent_id: item.parent_id.clone(),
            position: item.position,
            board_column: item.board_column.clone(),
            story_points: item.story_points,
            epic_color: item.epic_color.clone(),
        }
    }
}

/// Work item list with pagination.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkItemList {
    pub items: Vec<WorkItemSummary>,
    pub total: usize,
    pub has_more: bool,
}

/// Filter options for listing work items.
#[derive(Debug, Clone, Default)]
pub struct WorkItemFilters {
    pub state: Option<String>,
    pub labels: Option<Vec<String>>,
    pub assignee: Option<String>,
    pub search: Option<String>,
    pub item_type: Option<String>,
    pub board_column: Option<String>,
}

/// Data for creating a new work item.
#[derive(Debug, Clone)]
pub struct CreateWorkItemData {
    pub title: String,
    pub body: Option<String>,
    pub labels: Option<Vec<String>>,
    pub assignees: Option<Vec<String>>,
    pub item_type: Option<String>,
    pub parent_id: Option<String>,
    pub board_column: Option<String>,
    pub story_points: Option<i64>,
    pub epic_color: Option<String>,
}

/// Updates to apply to a work item.
#[derive(Debug, Clone, Default)]
pub struct WorkItemUpdates {
    pub title: Option<String>,
    pub body: Option<String>,
    pub state: Option<String>,
    pub labels: Option<Vec<String>>,
    pub assignees: Option<Vec<String>>,
    pub item_type: Option<String>,
    pub parent_id: Option<String>,
    pub board_column: Option<String>,
    pub story_points: Option<i64>,
    pub epic_color: Option<String>,
}

/// Result of work item creation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateWorkItemResult {
    pub item_id: String,
    pub number: u32,
    pub title: String,
}

// =============================================================================
// SERVICE FUNCTIONS
// =============================================================================

/// List work items for a project with optional filtering and pagination.
///
/// # Arguments
/// * `store` - The ProjectStore instance
/// * `project_id_or_slug` - Project ID or slug
/// * `filters` - Filter options
/// * `pagination` - Limit and offset
///
/// # Returns
/// List of work items with pagination info
pub async fn list_work_items(
    store: &Arc<RwLock<ProjectStore>>,
    project_id_or_slug: &str,
    filters: WorkItemFilters,
    pagination: Pagination,
) -> ServiceResult<WorkItemList> {
    info!(
        "List work items for project {} (limit: {}, offset: {})",
        project_id_or_slug, pagination.limit, pagination.offset
    );

    let store_guard = store.read().await;

    // Get project to validate it exists
    let project = store_guard
        .get_project(project_id_or_slug)
        .await
        .map_err(ServiceError::Store)?
        .ok_or_else(|| ServiceError::not_found("Project", project_id_or_slug))?;

    // List work items with state filter
    let items = store_guard
        .list_work_items(
            &project.id,
            filters.state.as_deref(),
            pagination.limit,
            pagination.offset,
        )
        .await
        .map_err(ServiceError::Store)?;

    let total = items.len();
    let has_more = items.len() >= pagination.limit;

    let summaries: Vec<WorkItemSummary> = items.iter().map(WorkItemSummary::from).collect();

    Ok(WorkItemList {
        items: summaries,
        total,
        has_more,
    })
}

/// Get a work item by ID or number.
///
/// # Arguments
/// * `store` - The ProjectStore instance
/// * `project_id_or_slug` - Project ID or slug
/// * `item_id_or_number` - Work item ID or number
///
/// # Returns
/// Work item details
pub async fn get_work_item(
    store: &Arc<RwLock<ProjectStore>>,
    project_id_or_slug: &str,
    item_id_or_number: &str,
) -> ServiceResult<WorkItemSummary> {
    info!(
        "Get work item {} in project {}",
        item_id_or_number, project_id_or_slug
    );

    let store_guard = store.read().await;

    // Get project to validate it exists
    let project = store_guard
        .get_project(project_id_or_slug)
        .await
        .map_err(ServiceError::Store)?
        .ok_or_else(|| ServiceError::not_found("Project", project_id_or_slug))?;

    // Try to parse as item number, otherwise treat as ID
    let item = if let Ok(num) = item_id_or_number.parse::<u32>() {
        store_guard
            .get_work_item_by_number(&project.id, num)
            .await
            .map_err(ServiceError::Store)?
    } else {
        store_guard
            .get_work_item(item_id_or_number)
            .await
            .map_err(ServiceError::Store)?
    };

    let item = item.ok_or_else(|| ServiceError::not_found("WorkItem", item_id_or_number))?;

    Ok(WorkItemSummary::from(&item))
}

/// Create a new work item in a project.
///
/// # Arguments
/// * `store` - The ProjectStore instance
/// * `event_bus` - Optional event bus for notifications
/// * `project_id_or_slug` - Project ID or slug
/// * `data` - Work item data
///
/// # Returns
/// Created work item result
pub async fn create_work_item(
    store: &Arc<RwLock<ProjectStore>>,
    event_bus: Option<&SharedEventBus>,
    project_id_or_slug: &str,
    data: CreateWorkItemData,
) -> ServiceResult<CreateWorkItemResult> {
    info!("Create work item '{}' in project {}", data.title, project_id_or_slug);

    let store_guard = store.write().await;

    // Get project to validate it exists
    let project = store_guard
        .get_project(project_id_or_slug)
        .await
        .map_err(ServiceError::Store)?
        .ok_or_else(|| ServiceError::not_found("Project", project_id_or_slug))?;

    // Get next work item number
    let number = store_guard
        .next_work_item_number(&project.id)
        .await
        .map_err(ServiceError::Store)?;

    // Create work item record
    let mut item = WorkItemRecord::new(project.id.clone(), number, data.title.clone());

    if let Some(body) = data.body {
        item = item.with_body(body);
    }
    if let Some(labels) = data.labels {
        item = item.with_labels(labels);
    }
    if let Some(assignees) = data.assignees {
        item.set_assignees(assignees);
    }
    if let Some(item_type) = data.item_type {
        item.item_type = item_type;
    }
    if let Some(parent_id) = data.parent_id {
        item.parent_id = Some(parent_id);
    }
    if let Some(board_column) = data.board_column {
        item.board_column = board_column;
    }
    if let Some(story_points) = data.story_points {
        item.story_points = Some(story_points);
    }
    if let Some(epic_color) = data.epic_color {
        item.epic_color = Some(epic_color);
    }

    let item_id = item.id.clone();
    let item_title = item.title.clone();

    store_guard
        .create_work_item(&item)
        .await
        .map_err(ServiceError::Store)?;

    drop(store_guard);

    // Emit event
    if let Some(bus) = event_bus {
        bus.issue_created(&project.id, &item_id, &item_title);
    }

    Ok(CreateWorkItemResult {
        item_id,
        number,
        title: item_title,
    })
}

/// Update a work item.
///
/// # Arguments
/// * `store` - The ProjectStore instance
/// * `event_bus` - Optional event bus for notifications
/// * `project_id_or_slug` - Project ID or slug
/// * `item_id_or_number` - Work item ID or number
/// * `updates` - Updates to apply
///
/// # Returns
/// Updated work item summary
pub async fn update_work_item(
    store: &Arc<RwLock<ProjectStore>>,
    event_bus: Option<&SharedEventBus>,
    project_id_or_slug: &str,
    item_id_or_number: &str,
    updates: WorkItemUpdates,
) -> ServiceResult<WorkItemSummary> {
    info!(
        "Update work item {} in project {}",
        item_id_or_number, project_id_or_slug
    );

    let store_guard = store.write().await;

    // Get project
    let project = store_guard
        .get_project(project_id_or_slug)
        .await
        .map_err(ServiceError::Store)?
        .ok_or_else(|| ServiceError::not_found("Project", project_id_or_slug))?;

    // Get work item
    let item_result = if let Ok(num) = item_id_or_number.parse::<u32>() {
        store_guard.get_work_item_by_number(&project.id, num).await
    } else {
        store_guard.get_work_item(item_id_or_number).await
    };

    let mut item = item_result
        .map_err(ServiceError::Store)?
        .ok_or_else(|| ServiceError::not_found("WorkItem", item_id_or_number))?;

    let was_open = item.state != "closed";

    // Apply updates
    if let Some(title) = updates.title {
        item.title = title;
    }
    if let Some(body) = updates.body {
        item.body = Some(body);
    }
    if let Some(state) = updates.state {
        item.state = state;
    }
    if let Some(labels) = updates.labels {
        item.set_labels(labels);
    }
    if let Some(assignees) = updates.assignees {
        item.set_assignees(assignees);
    }
    if let Some(item_type) = updates.item_type {
        item.item_type = item_type;
    }
    if let Some(parent_id) = updates.parent_id {
        item.parent_id = Some(parent_id);
    }
    if let Some(board_column) = updates.board_column {
        item.board_column = board_column;
    }
    if let Some(story_points) = updates.story_points {
        item.story_points = Some(story_points);
    }
    if let Some(epic_color) = updates.epic_color {
        item.epic_color = Some(epic_color);
    }

    item.updated_at = chrono::Utc::now().to_rfc3339();
    let is_now_closed = item.state == "closed";
    let item_id = item.id.clone();
    let project_id = project.id.clone();

    store_guard
        .update_work_item(&item)
        .await
        .map_err(ServiceError::Store)?;

    drop(store_guard);

    // Emit appropriate event
    if let Some(bus) = event_bus {
        if was_open && is_now_closed {
            bus.issue_closed(&project_id, &item_id);
        } else if !was_open && !is_now_closed {
            bus.issue_reopened(&project_id, &item_id);
        } else {
            bus.issue_updated(&project_id, &item_id);
        }
    }

    Ok(WorkItemSummary::from(&item))
}

/// Delete a work item.
///
/// # Arguments
/// * `store` - The ProjectStore instance
/// * `event_bus` - Optional event bus for notifications
/// * `project_id_or_slug` - Project ID or slug
/// * `item_id_or_number` - Work item ID or number
///
/// # Returns
/// Whether deletion succeeded
pub async fn delete_work_item(
    store: &Arc<RwLock<ProjectStore>>,
    event_bus: Option<&SharedEventBus>,
    project_id_or_slug: &str,
    item_id_or_number: &str,
) -> ServiceResult<bool> {
    info!(
        "Delete work item {} from project {}",
        item_id_or_number, project_id_or_slug
    );

    let store_guard = store.write().await;

    // Get project
    let project = store_guard
        .get_project(project_id_or_slug)
        .await
        .map_err(ServiceError::Store)?
        .ok_or_else(|| ServiceError::not_found("Project", project_id_or_slug))?;

    // Get work item
    let item_result = if let Ok(num) = item_id_or_number.parse::<u32>() {
        store_guard.get_work_item_by_number(&project.id, num).await
    } else {
        store_guard.get_work_item(item_id_or_number).await
    };

    let item = item_result
        .map_err(ServiceError::Store)?
        .ok_or_else(|| ServiceError::not_found("WorkItem", item_id_or_number))?;

    // Delete locally
    let item_id = item.id.clone();
    let project_id = project.id.clone();

    store_guard
        .delete_work_item(&item_id)
        .await
        .map_err(ServiceError::Store)?;

    drop(store_guard);

    // Emit event
    if let Some(bus) = event_bus {
        bus.issue_deleted(&project_id, &item_id);
    }

    Ok(true)
}

/// Get child work items for a parent work item.
///
/// # Arguments
/// * `store` - The ProjectStore instance
/// * `project_id_or_slug` - Project ID or slug
/// * `parent_id` - Parent work item ID
///
/// # Returns
/// List of child work items
pub async fn get_child_work_items(
    store: &Arc<RwLock<ProjectStore>>,
    project_id_or_slug: &str,
    parent_id: &str,
) -> ServiceResult<Vec<WorkItemSummary>> {
    info!(
        "Get child work items for {} in project {}",
        parent_id, project_id_or_slug
    );

    let store_guard = store.read().await;

    // Get project to validate it exists
    let _project = store_guard
        .get_project(project_id_or_slug)
        .await
        .map_err(ServiceError::Store)?
        .ok_or_else(|| ServiceError::not_found("Project", project_id_or_slug))?;

    // Get child work items
    let children = store_guard
        .get_child_work_items(parent_id)
        .await
        .map_err(ServiceError::Store)?;

    Ok(children.iter().map(WorkItemSummary::from).collect())
}

/// Move a work item to a different board column.
///
/// # Arguments
/// * `store` - The ProjectStore instance
/// * `event_bus` - Optional event bus for notifications
/// * `project_id_or_slug` - Project ID or slug
/// * `item_id_or_number` - Work item ID or number
/// * `to_column` - Target board column
/// * `to_position` - Target position in column
///
/// # Returns
/// Updated work item summary
pub async fn move_card(
    store: &Arc<RwLock<ProjectStore>>,
    event_bus: Option<&SharedEventBus>,
    project_id_or_slug: &str,
    item_id_or_number: &str,
    to_column: &str,
    to_position: Option<i64>,
) -> ServiceResult<WorkItemSummary> {
    info!(
        "Move work item {} to column {} in project {}",
        item_id_or_number, to_column, project_id_or_slug
    );

    let store_guard = store.write().await;

    // Get project
    let project = store_guard
        .get_project(project_id_or_slug)
        .await
        .map_err(ServiceError::Store)?
        .ok_or_else(|| ServiceError::not_found("Project", project_id_or_slug))?;

    // Get work item
    let item_result = if let Ok(num) = item_id_or_number.parse::<u32>() {
        store_guard.get_work_item_by_number(&project.id, num).await
    } else {
        store_guard.get_work_item(item_id_or_number).await
    };

    let mut item = item_result
        .map_err(ServiceError::Store)?
        .ok_or_else(|| ServiceError::not_found("WorkItem", item_id_or_number))?;

    // Determine position
    let position = if let Some(pos) = to_position {
        pos
    } else {
        // Get next position in target column
        store_guard
            .next_position_in_column(&project.id, to_column)
            .await
            .map_err(ServiceError::Store)?
    };

    // Update work item
    item.board_column = to_column.to_string();
    item.position = position;
    item.updated_at = chrono::Utc::now().to_rfc3339();

    let item_id = item.id.clone();
    let project_id = project.id.clone();

    store_guard
        .update_work_item(&item)
        .await
        .map_err(ServiceError::Store)?;

    drop(store_guard);

    // Emit event
    if let Some(bus) = event_bus {
        bus.issue_updated(&project_id, &item_id);
    }

    Ok(WorkItemSummary::from(&item))
}

// =============================================================================
// BOARD TYPES
// =============================================================================

/// Board column information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardColumn {
    pub id: String,
    pub name: String,
    pub display_name: String,
}

impl BoardColumn {
    fn from_id(id: &str) -> Self {
        let display_name = match id {
            "backlog" => "Backlog",
            "todo" => "To Do",
            "in_progress" => "In Progress",
            "in_review" => "In Review",
            "testing" => "Testing",
            "done" => "Done",
            _ => id,
        };
        Self {
            id: id.to_string(),
            name: id.to_string(),
            display_name: display_name.to_string(),
        }
    }
}

/// Board swimlane with items by column.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardSwimlane {
    pub item_type: String,
    pub items_by_column: std::collections::HashMap<String, Vec<WorkItemSummary>>,
}

/// Full board view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardView {
    pub columns: Vec<BoardColumn>,
    pub swimlanes: Vec<BoardSwimlane>,
    pub column_counts: std::collections::HashMap<String, usize>,
    pub total_items: usize,
}

/// Column counts for board stats.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnCounts {
    pub counts: std::collections::HashMap<String, usize>,
    pub total: usize,
}

// =============================================================================
// BOARD SERVICE FUNCTIONS
// =============================================================================

/// Get the full board view for a project.
///
/// # Arguments
/// * `store` - The ProjectStore instance
/// * `project_id_or_slug` - Project ID or slug
/// * `item_type_filter` - Optional filter by item type
///
/// # Returns
/// Full board view with swimlanes and columns
pub async fn get_board(
    store: &Arc<RwLock<ProjectStore>>,
    project_id_or_slug: &str,
    item_type_filter: Option<&str>,
) -> ServiceResult<BoardView> {
    info!("Get board for project {}", project_id_or_slug);

    let store_guard = store.read().await;

    // Get project to validate it exists
    let project = store_guard
        .get_project(project_id_or_slug)
        .await
        .map_err(ServiceError::Store)?
        .ok_or_else(|| ServiceError::not_found("Project", project_id_or_slug))?;

    // Get all work items for board (open and closed)
    let items = store_guard
        .list_work_items(&project.id, None, 10000, 0)
        .await
        .map_err(ServiceError::Store)?;

    // Define columns in workflow order
    let column_ids = ["backlog", "todo", "in_progress", "in_review", "testing", "done"];
    let columns: Vec<BoardColumn> = column_ids.iter().map(|id| BoardColumn::from_id(id)).collect();

    // Define swimlane types
    let swimlane_types = ["epic", "story", "task", "subtask", "bug"];

    // Build swimlanes with items organized by column
    let mut swimlanes = Vec::new();
    let mut column_counts: std::collections::HashMap<String, usize> = column_ids
        .iter()
        .map(|id| (id.to_string(), 0))
        .collect();
    let mut total_items = 0;

    for swimlane_type in &swimlane_types {
        let mut items_by_column: std::collections::HashMap<String, Vec<WorkItemSummary>> =
            column_ids.iter().map(|id| (id.to_string(), Vec::new())).collect();

        for item in &items {
            // Apply item type filter if specified
            if let Some(filter) = item_type_filter {
                if item.item_type != filter {
                    continue;
                }
            }

            if item.item_type == *swimlane_type {
                let column = &item.board_column;
                if let Some(col_items) = items_by_column.get_mut(column) {
                    col_items.push(WorkItemSummary::from(item));
                    *column_counts.get_mut(column).unwrap() += 1;
                    total_items += 1;
                }
            }
        }

        // Sort items within each column by position
        for col_items in items_by_column.values_mut() {
            col_items.sort_by_key(|item| item.position);
        }

        swimlanes.push(BoardSwimlane {
            item_type: swimlane_type.to_string(),
            items_by_column,
        });
    }

    Ok(BoardView {
        columns,
        swimlanes,
        column_counts,
        total_items,
    })
}

/// Get column counts for a project board.
///
/// # Arguments
/// * `store` - The ProjectStore instance
/// * `project_id_or_slug` - Project ID or slug
///
/// # Returns
/// Column counts
pub async fn get_column_counts(
    store: &Arc<RwLock<ProjectStore>>,
    project_id_or_slug: &str,
) -> ServiceResult<ColumnCounts> {
    info!("Get column counts for project {}", project_id_or_slug);

    let store_guard = store.read().await;

    // Get project to validate it exists
    let project = store_guard
        .get_project(project_id_or_slug)
        .await
        .map_err(ServiceError::Store)?
        .ok_or_else(|| ServiceError::not_found("Project", project_id_or_slug))?;

    // Get column counts
    let counts_vec = store_guard
        .count_work_items_by_column(&project.id)
        .await
        .map_err(ServiceError::Store)?;

    // Convert Vec<(String, i64)> to HashMap<String, usize>
    let counts: std::collections::HashMap<String, usize> = counts_vec
        .into_iter()
        .map(|(k, v)| (k, v as usize))
        .collect();
    let total: usize = counts.values().sum();

    Ok(ColumnCounts { counts, total })
}

// =============================================================================
// EPIC-BASED BOARD TYPES
// =============================================================================

/// Statistics for an Epic swimlane.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpicStats {
    pub total_items: usize,
    pub total_story_points: i64,
    pub items_by_column: std::collections::HashMap<String, usize>,
}

/// Data for an Epic swimlane containing all its descendants.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpicSwimlaneData {
    /// The Epic work item itself
    pub epic: WorkItemSummary,
    /// All descendants organized by board column
    pub items_by_column: std::collections::HashMap<String, Vec<WorkItemSummary>>,
    /// Aggregated statistics
    pub stats: EpicStats,
}

/// Epic-based board view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpicBoardView {
    /// Board columns configuration
    pub columns: Vec<BoardColumn>,
    /// Epic swimlanes (ordered by position)
    pub epics: Vec<EpicSwimlaneData>,
    /// Items not assigned to any Epic
    pub unassigned: std::collections::HashMap<String, Vec<WorkItemSummary>>,
    /// Column counts across all items
    pub column_counts: std::collections::HashMap<String, usize>,
    /// Total item count
    pub total_items: usize,
}

// =============================================================================
// EPIC-BASED BOARD SERVICE FUNCTIONS
// =============================================================================

/// Get the Epic-based board view for a project.
///
/// Returns a board organized by Epic swimlanes, where each Epic contains
/// all its descendant items (Stories, Tasks, Subtasks, Bugs) organized by column.
///
/// # Arguments
/// * `store` - The ProjectStore instance
/// * `project_id_or_slug` - Project ID or slug
///
/// # Returns
/// Epic-based board view with swimlanes and columns
pub async fn get_epic_board(
    store: &Arc<RwLock<ProjectStore>>,
    project_id_or_slug: &str,
) -> ServiceResult<EpicBoardView> {
    info!("Get Epic-based board for project {}", project_id_or_slug);

    let store_guard = store.read().await;

    // Get project to validate it exists
    let project = store_guard
        .get_project(project_id_or_slug)
        .await
        .map_err(ServiceError::Store)?
        .ok_or_else(|| ServiceError::not_found("Project", project_id_or_slug))?;

    // Define columns in workflow order
    let column_ids = ["backlog", "todo", "in_progress", "in_review", "testing", "done"];
    let columns: Vec<BoardColumn> = column_ids.iter().map(|id| BoardColumn::from_id(id)).collect();

    // Get all Epics ordered by position
    let epics = store_guard
        .list_epics_for_project(&project.id)
        .await
        .map_err(ServiceError::Store)?;

    // Compute Epic assignments for all items
    let epic_assignments = store_guard
        .compute_epic_assignments(&project.id)
        .await
        .map_err(ServiceError::Store)?;

    // Get all non-Epic work items
    let all_items = store_guard
        .list_work_items(&project.id, None, 10000, 0)
        .await
        .map_err(ServiceError::Store)?;

    let non_epic_items: Vec<_> = all_items
        .iter()
        .filter(|item| item.item_type != "epic")
        .collect();

    // Initialize column counts
    let mut column_counts: std::collections::HashMap<String, usize> = column_ids
        .iter()
        .map(|id| (id.to_string(), 0))
        .collect();
    let mut total_items = 0;

    // Build Epic swimlanes
    let mut epic_swimlanes = Vec::new();

    for epic in &epics {
        // Initialize columns for this Epic
        let mut items_by_column: std::collections::HashMap<String, Vec<WorkItemSummary>> =
            column_ids.iter().map(|id| (id.to_string(), Vec::new())).collect();

        let mut epic_total_items = 0;
        let mut epic_story_points: i64 = 0;
        let mut epic_items_by_column: std::collections::HashMap<String, usize> = column_ids
            .iter()
            .map(|id| (id.to_string(), 0))
            .collect();

        // Find all items assigned to this Epic
        for item in &non_epic_items {
            if let Some(assigned_epic_id) = epic_assignments.get(&item.id) {
                if assigned_epic_id == &epic.id {
                    let column = &item.board_column;
                    if let Some(col_items) = items_by_column.get_mut(column) {
                        col_items.push(WorkItemSummary::from(*item));
                        *column_counts.get_mut(column).unwrap() += 1;
                        *epic_items_by_column.get_mut(column).unwrap() += 1;
                        epic_total_items += 1;
                        total_items += 1;
                        if let Some(sp) = item.story_points {
                            epic_story_points += sp;
                        }
                    }
                }
            }
        }

        // Sort items within each column by position
        for col_items in items_by_column.values_mut() {
            col_items.sort_by_key(|item| item.position);
        }

        epic_swimlanes.push(EpicSwimlaneData {
            epic: WorkItemSummary::from(epic),
            items_by_column,
            stats: EpicStats {
                total_items: epic_total_items,
                total_story_points: epic_story_points,
                items_by_column: epic_items_by_column,
            },
        });
    }

    // Build unassigned swimlane (items with no Epic ancestor)
    let mut unassigned: std::collections::HashMap<String, Vec<WorkItemSummary>> =
        column_ids.iter().map(|id| (id.to_string(), Vec::new())).collect();

    for item in &non_epic_items {
        if !epic_assignments.contains_key(&item.id) {
            let column = &item.board_column;
            if let Some(col_items) = unassigned.get_mut(column) {
                col_items.push(WorkItemSummary::from(*item));
                *column_counts.get_mut(column).unwrap() += 1;
                total_items += 1;
            }
        }
    }

    // Sort unassigned items by position
    for col_items in unassigned.values_mut() {
        col_items.sort_by_key(|item| item.position);
    }

    Ok(EpicBoardView {
        columns,
        epics: epic_swimlanes,
        unassigned,
        column_counts,
        total_items,
    })
}

/// Reorder an Epic swimlane to a new position.
///
/// # Arguments
/// * `store` - The ProjectStore instance
/// * `event_bus` - Optional event bus for notifications
/// * `project_id_or_slug` - Project ID or slug
/// * `epic_id` - Epic ID to reorder
/// * `to_position` - Target position
///
/// # Returns
/// Whether reordering succeeded
pub async fn reorder_epic(
    store: &Arc<RwLock<ProjectStore>>,
    event_bus: Option<&SharedEventBus>,
    project_id_or_slug: &str,
    epic_id: &str,
    to_position: i64,
) -> ServiceResult<bool> {
    info!(
        "Reorder Epic {} to position {} in project {}",
        epic_id, to_position, project_id_or_slug
    );

    let store_guard = store.write().await;

    // Get project to validate it exists
    let project = store_guard
        .get_project(project_id_or_slug)
        .await
        .map_err(ServiceError::Store)?
        .ok_or_else(|| ServiceError::not_found("Project", project_id_or_slug))?;

    // Get the Epic to reorder
    let epic = store_guard
        .get_work_item(epic_id)
        .await
        .map_err(ServiceError::Store)?
        .ok_or_else(|| ServiceError::not_found("Epic", epic_id))?;

    // Verify it's an Epic and belongs to this project
    if epic.item_type != "epic" {
        return Err(ServiceError::bad_request("Work item is not an Epic"));
    }
    if epic.project_id != project.id {
        return Err(ServiceError::bad_request("Epic does not belong to this project"));
    }

    let from_position = epic.position;

    // Shift other Epics to make room
    store_guard
        .shift_epic_positions(&project.id, from_position, to_position)
        .await
        .map_err(ServiceError::Store)?;

    // Update the Epic's position
    store_guard
        .update_epic_position(epic_id, to_position)
        .await
        .map_err(ServiceError::Store)?;

    drop(store_guard);

    // Emit event
    if let Some(bus) = event_bus {
        bus.issue_updated(&project.id, epic_id);
    }

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_work_item_summary_from_record() {
        let record = WorkItemRecord::new("project-1".to_string(), 1, "Test Item".to_string());
        let summary = WorkItemSummary::from(&record);
        assert_eq!(summary.title, "Test Item");
        assert_eq!(summary.number, 1);
        assert_eq!(summary.state, "open");
    }

    #[test]
    fn test_work_item_filters_default() {
        let filters = WorkItemFilters::default();
        assert!(filters.state.is_none());
        assert!(filters.labels.is_none());
    }

    #[test]
    fn test_create_work_item_data() {
        let data = CreateWorkItemData {
            title: "New Item".to_string(),
            body: Some("Description".to_string()),
            labels: Some(vec!["bug".to_string()]),
            assignees: None,
            item_type: Some("task".to_string()),
            parent_id: None,
            board_column: None,
            story_points: None,
            epic_color: None,
        };
        assert_eq!(data.title, "New Item");
        assert!(data.body.is_some());
    }
}
