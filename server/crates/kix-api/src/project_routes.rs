//! Project API routes with SSE for real-time events.
//!
//! Provides REST endpoints for project management and a Server-Sent Events
//! endpoint for real-time updates from MCP operations.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
    },
    routing::{get, post},
    Json, Router,
};
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use std::{convert::Infallible, sync::Arc, time::Duration};
use tokio::sync::RwLock;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use tracing::{info, warn};

use kix_projects::{ProjectEvent, ProjectEventType, SharedEventBus};
use kix_services;
use kix_store::{WorkItemRecord, ProjectRecord, ProjectStore};

/// State for project routes.
#[derive(Clone)]
pub struct ProjectState {
    /// Project store
    store: Arc<RwLock<ProjectStore>>,
    /// Event bus for real-time updates
    event_bus: SharedEventBus,
}

impl ProjectState {
    /// Create a new project state.
    pub fn new(
        store: Arc<RwLock<ProjectStore>>,
        event_bus: SharedEventBus,
    ) -> Self {
        Self { store, event_bus }
    }

    /// Get a reference to the project store.
    pub fn store(&self) -> &Arc<RwLock<ProjectStore>> {
        &self.store
    }

    /// Get a reference to the event bus.
    pub fn event_bus(&self) -> &SharedEventBus {
        &self.event_bus
    }
}

/// Create the project routes router.
pub fn create_project_router(state: ProjectState) -> Router {
    Router::new()
        // SSE endpoint for real-time events
        .route("/api/projects/events", get(project_events_sse))
        .route(
            "/api/projects/events/:project_id",
            get(project_events_sse_filtered),
        )
        // REST endpoints
        .route("/api/projects", get(list_projects).post(create_project))
        .route(
            "/api/projects/:id",
            get(get_project).put(update_project).delete(delete_project),
        )
        .route(
            "/api/projects/:id/work-items",
            get(list_work_items).post(create_work_item),
        )
        .route(
            "/api/projects/:id/work-items/:item_id",
            get(get_work_item).put(update_work_item).delete(delete_work_item),
        )
        .route("/api/projects/:id/entries", get(list_project_entries))
        .route(
            "/api/projects/:id/entries/:entry_id",
            post(link_entry).delete(unlink_entry),
        )
        .route("/api/projects/:id/search", get(search_project))
        // Board endpoints
        .route("/api/projects/:id/board", get(get_board))
        .route("/api/projects/:id/board/move", post(move_card))
        .route("/api/projects/:id/board/columns", get(get_column_counts))
        .route("/api/projects/:id/work-items/:item_id/children", get(get_child_work_items))
        .with_state(state)
}

// =============================================================================
// SSE ENDPOINTS
// =============================================================================

/// Subscribe to all project events via SSE.
async fn project_events_sse(
    State(state): State<ProjectState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    info!("New SSE subscriber for all project events");

    let receiver = state.event_bus.subscribe();
    let stream = BroadcastStream::new(receiver)
        .filter_map(|result| {
            match result {
                Ok(event) => Some(Ok(Event::default()
                    .event(event.event_type.to_string())
                    .data(serde_json::to_string(&event).unwrap_or_default())
                    .id(event.id))),
                Err(_) => None, // Skip lagged messages
            }
        });

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(30))
            .text("ping"),
    )
}

/// Subscribe to events for a specific project via SSE.
async fn project_events_sse_filtered(
    State(state): State<ProjectState>,
    Path(project_id): Path<String>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    info!("New SSE subscriber for project: {}", project_id);

    let receiver = state.event_bus.subscribe();
    let stream = BroadcastStream::new(receiver)
        .filter_map(move |result| {
            match result {
                Ok(event) if event.project_id == project_id => {
                    Some(Ok(Event::default()
                        .event(event.event_type.to_string())
                        .data(serde_json::to_string(&event).unwrap_or_default())
                        .id(event.id)))
                }
                Ok(_) => None, // Different project
                Err(_) => None, // Skip lagged messages
            }
        });

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(30))
            .text("ping"),
    )
}

// =============================================================================
// PROJECT REST ENDPOINTS
// =============================================================================

#[derive(Debug, Deserialize)]
struct ListProjectsQuery {
    include_archived: Option<bool>,
    limit: Option<usize>,
    offset: Option<usize>,
}

#[derive(Debug, Serialize)]
struct ProjectListResponse {
    projects: Vec<ProjectSummary>,
    total: usize,
    has_more: bool,
}

#[derive(Debug, Serialize)]
struct ProjectSummary {
    id: String,
    name: String,
    slug: String,
    description: Option<String>,
    color: Option<String>,
    archived: bool,
    created_at: String,
}

async fn list_projects(
    State(state): State<ProjectState>,
    Query(query): Query<ListProjectsQuery>,
) -> impl IntoResponse {
    let include_archived = query.include_archived.unwrap_or(false);
    let limit = query.limit.unwrap_or(50);
    let offset = query.offset.unwrap_or(0);

    let store = state.store.read().await;
    match store.list_projects(include_archived).await {
        Ok(projects) => {
            let total = projects.len();
            let paginated: Vec<_> = projects.into_iter().skip(offset).take(limit).collect();
            let has_more = total > offset + limit;

            let summaries: Vec<ProjectSummary> = paginated
                .into_iter()
                .map(|p| {
                    let is_archived = p.is_archived();
                    ProjectSummary {
                        id: p.id,
                        name: p.name,
                        slug: p.slug,
                        description: p.description,
                        color: p.color,
                        archived: is_archived,
                        created_at: p.created_at,
                    }
                })
                .collect();

            Json(ProjectListResponse {
                projects: summaries,
                total,
                has_more,
            })
            .into_response()
        }
        Err(e) => {
            warn!("Failed to list projects: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
struct CreateProjectRequest {
    name: String,
    description: Option<String>,
    color: Option<String>,
}

async fn create_project(
    State(state): State<ProjectState>,
    Json(req): Json<CreateProjectRequest>,
) -> impl IntoResponse {
    // Create project record
    let mut project = ProjectRecord::new(req.name.clone());

    if let Some(desc) = req.description.clone() {
        project = project.with_description(desc);
    }
    if let Some(color) = req.color.clone() {
        project = project.with_color(color);
    }

    // Store the project
    let store = state.store.write().await;
    match store.create_project(&project).await {
        Ok(_) => {
            drop(store);

            // Emit event
            state.event_bus.project_created(&project.id, &project.name);

            // Build response
            let response = serde_json::json!({
                "success": true,
                "project_id": project.id,
                "name": project.name,
                "slug": project.slug,
            });

            Json(response).into_response()
        }
        Err(e) => {
            warn!("Failed to create project: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

/// Full project response with computed fields.
#[derive(Debug, Serialize)]
struct ProjectResponse {
    id: String,
    name: String,
    slug: String,
    description: Option<String>,
    color: Option<String>,
    archived: bool,
    created_at: String,
    updated_at: String,
}

/// Work item response with properly parsed labels and assignees arrays.
#[derive(Debug, Serialize)]
struct WorkItemResponse {
    id: String,
    project_id: String,
    number: u32,
    title: String,
    body: Option<String>,
    state: String,
    labels: Vec<String>,
    assignees: Vec<String>,
    priority: Option<String>,
    created_at: String,
    updated_at: String,
    // Board fields
    item_type: String,
    parent_id: Option<String>,
    position: i64,
    board_column: String,
    story_points: Option<i64>,
    epic_color: Option<String>,
}

impl From<&WorkItemRecord> for WorkItemResponse {
    fn from(item: &WorkItemRecord) -> Self {
        let priority = item.priority.map(|p| match p {
            1 => "critical".to_string(),
            2 => "high".to_string(),
            3 => "medium".to_string(),
            _ => "low".to_string(),
        });

        WorkItemResponse {
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

async fn get_project(
    State(state): State<ProjectState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let store = state.store.read().await;
    match store.get_project(&id).await {
        Ok(Some(project)) => {
            let archived = project.is_archived();
            let response = ProjectResponse {
                id: project.id,
                name: project.name,
                slug: project.slug,
                description: project.description,
                color: project.color,
                archived,
                created_at: project.created_at,
                updated_at: project.updated_at,
            };
            Json(response).into_response()
        }
        Ok(None) => (StatusCode::NOT_FOUND, "Project not found").into_response(),
        Err(e) => {
            warn!("Failed to get project: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
struct UpdateProjectRequest {
    name: Option<String>,
    description: Option<String>,
    color: Option<String>,
    archived: Option<bool>,
}

async fn update_project(
    State(state): State<ProjectState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateProjectRequest>,
) -> impl IntoResponse {
    let store = state.store.write().await;
    match store.get_project(&id).await {
        Ok(Some(mut project)) => {
            if let Some(name) = req.name {
                project.name = name;
            }
            if let Some(desc) = req.description {
                project.description = Some(desc);
            }
            if let Some(color) = req.color {
                project.color = Some(color);
            }
            if let Some(archived) = req.archived {
                project.archived = if archived { 1 } else { 0 };
            }
            project.updated_at = chrono::Utc::now().to_rfc3339();

            let project_id = project.id.clone();
            match store.update_project(&project).await {
                Ok(_) => {
                    drop(store);
                    state.event_bus.project_updated(&project_id);
                    Json(serde_json::json!({
                        "success": true,
                        "project_id": project_id,
                    }))
                    .into_response()
                }
                Err(e) => {
                    warn!("Failed to update project: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
                }
            }
        }
        Ok(None) => (StatusCode::NOT_FOUND, "Project not found").into_response(),
        Err(e) => {
            warn!("Failed to get project: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

async fn delete_project(
    State(state): State<ProjectState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let store = state.store.write().await;
    match store.get_project(&id).await {
        Ok(Some(project)) => {
            let project_id = project.id.clone();
            match store.delete_project(&project_id).await {
                Ok(_) => {
                    drop(store);
                    state.event_bus.project_deleted(&project_id);
                    Json(serde_json::json!({
                        "success": true,
                    }))
                    .into_response()
                }
                Err(e) => {
                    warn!("Failed to delete project: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
                }
            }
        }
        Ok(None) => (StatusCode::NOT_FOUND, "Project not found").into_response(),
        Err(e) => {
            warn!("Failed to get project: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

// =============================================================================
// WORK ITEM REST ENDPOINTS
// =============================================================================

#[derive(Debug, Deserialize)]
struct ListWorkItemsQuery {
    state: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
}

async fn list_work_items(
    State(state): State<ProjectState>,
    Path(project_id): Path<String>,
    Query(query): Query<ListWorkItemsQuery>,
) -> impl IntoResponse {
    let store = state.store.read().await;

    // Verify project exists
    match store.get_project(&project_id).await {
        Ok(Some(project)) => {
            let state_filter = query.state.as_deref();
            let limit = query.limit.unwrap_or(50);
            let offset = query.offset.unwrap_or(0);
            match store.list_work_items(&project.id, state_filter, limit, offset).await {
                Ok(items) => {
                    let total = items.len();
                    let has_more = items.len() >= limit;

                    // Convert to WorkItemResponse to properly parse labels/assignees
                    let item_responses: Vec<WorkItemResponse> = items.iter()
                        .map(WorkItemResponse::from)
                        .collect();

                    Json(serde_json::json!({
                        "items": item_responses,
                        "total": total,
                        "has_more": has_more,
                    }))
                    .into_response()
                }
                Err(e) => {
                    warn!("Failed to list work items: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
                }
            }
        }
        Ok(None) => (StatusCode::NOT_FOUND, "Project not found").into_response(),
        Err(e) => {
            warn!("Failed to get project: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
struct CreateWorkItemRequest {
    title: String,
    body: Option<String>,
    labels: Option<Vec<String>>,
    assignees: Option<Vec<String>>,
    // Board fields
    item_type: Option<String>,
    parent_id: Option<String>,
    board_column: Option<String>,
    story_points: Option<i64>,
    epic_color: Option<String>,
}

async fn create_work_item(
    State(state): State<ProjectState>,
    Path(project_id): Path<String>,
    Json(req): Json<CreateWorkItemRequest>,
) -> impl IntoResponse {
    let store = state.store.write().await;

    // Verify project exists
    match store.get_project(&project_id).await {
        Ok(Some(project)) => {
            // Get next work item number
            let number = match store.next_work_item_number(&project.id).await {
                Ok(n) => n,
                Err(e) => {
                    warn!("Failed to get next work item number: {}", e);
                    return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
                }
            };

            // Create work item record
            let mut item = WorkItemRecord::new(project.id.clone(), number, req.title.clone());
            if let Some(body) = req.body {
                item = item.with_body(body);
            }
            if let Some(labels) = req.labels {
                item = item.with_labels(labels);
            }
            if let Some(assignees) = req.assignees {
                item.set_assignees(assignees);
            }
            // Board fields
            if let Some(item_type) = req.item_type {
                item.item_type = item_type;
            }
            if let Some(parent_id) = req.parent_id {
                item.parent_id = Some(parent_id);
            }
            if let Some(board_column) = req.board_column {
                item.board_column = board_column;
            }
            if let Some(story_points) = req.story_points {
                item.story_points = Some(story_points);
            }
            if let Some(epic_color) = req.epic_color {
                item.epic_color = Some(epic_color);
            }

            let item_id = item.id.clone();
            let item_title = item.title.clone();
            let project_id_clone = project.id.clone();

            match store.create_work_item(&item).await {
                Ok(_) => {
                    drop(store);
                    state.event_bus.issue_created(&project_id_clone, &item_id, &item_title);
                    Json(serde_json::json!({
                        "success": true,
                        "item_id": item_id,
                        "number": number,
                        "title": item_title,
                    }))
                    .into_response()
                }
                Err(e) => {
                    warn!("Failed to create work item: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
                }
            }
        }
        Ok(None) => (StatusCode::NOT_FOUND, "Project not found").into_response(),
        Err(e) => {
            warn!("Failed to get project: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

async fn get_work_item(
    State(state): State<ProjectState>,
    Path((project_id, item_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let store = state.store.read().await;

    // Try to parse as work item number
    let item = if let Ok(num) = item_id.parse::<u32>() {
        store.get_work_item_by_number(&project_id, num).await
    } else {
        store.get_work_item(&item_id).await
    };

    match item {
        Ok(Some(item)) => {
            // Convert to WorkItemResponse to properly parse labels/assignees
            let response = WorkItemResponse::from(&item);
            Json(response).into_response()
        }
        Ok(None) => (StatusCode::NOT_FOUND, "Work item not found").into_response(),
        Err(e) => {
            warn!("Failed to get work item: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
struct UpdateWorkItemRequest {
    title: Option<String>,
    body: Option<String>,
    state: Option<String>,
    labels: Option<Vec<String>>,
    assignees: Option<Vec<String>>,
    // Board fields
    item_type: Option<String>,
    parent_id: Option<String>,
    board_column: Option<String>,
    story_points: Option<i64>,
    epic_color: Option<String>,
}

async fn update_work_item(
    State(state): State<ProjectState>,
    Path((project_id, item_id)): Path<(String, String)>,
    Json(req): Json<UpdateWorkItemRequest>,
) -> impl IntoResponse {
    let store = state.store.write().await;

    // Verify project exists
    let _project = match store.get_project(&project_id).await {
        Ok(Some(p)) => p,
        Ok(None) => return (StatusCode::NOT_FOUND, "Project not found").into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    // Get work item
    let item_result = if let Ok(num) = item_id.parse::<u32>() {
        store.get_work_item_by_number(&project_id, num).await
    } else {
        store.get_work_item(&item_id).await
    };

    match item_result {
        Ok(Some(mut item)) => {
            let was_open = item.state != "closed";

            // Apply updates
            if let Some(title) = req.title {
                item.title = title;
            }
            if let Some(body) = req.body {
                item.body = Some(body);
            }
            if let Some(state_str) = req.state {
                item.state = state_str;
            }
            if let Some(labels) = req.labels {
                item.set_labels(labels);
            }
            if let Some(assignees) = req.assignees {
                item.set_assignees(assignees);
            }
            // Board fields
            if let Some(item_type) = req.item_type {
                item.item_type = item_type;
            }
            if let Some(parent_id) = req.parent_id {
                item.parent_id = Some(parent_id);
            }
            if let Some(board_column) = req.board_column {
                item.board_column = board_column;
            }
            if let Some(story_points) = req.story_points {
                item.story_points = Some(story_points);
            }
            if let Some(epic_color) = req.epic_color {
                item.epic_color = Some(epic_color);
            }

            item.updated_at = chrono::Utc::now().to_rfc3339();
            let is_now_closed = item.state == "closed";
            let item_id_str = item.id.clone();

            match store.update_work_item(&item).await {
                Ok(_) => {
                    drop(store);
                    // Emit appropriate event
                    if was_open && is_now_closed {
                        state.event_bus.issue_closed(&project_id, &item_id_str);
                    } else if !was_open && !is_now_closed {
                        state.event_bus.issue_reopened(&project_id, &item_id_str);
                    } else {
                        state.event_bus.issue_updated(&project_id, &item_id_str);
                    }
                    Json(serde_json::json!({
                        "success": true,
                        "item_id": item_id_str,
                    }))
                    .into_response()
                }
                Err(e) => {
                    warn!("Failed to update work item: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
                }
            }
        }
        Ok(None) => (StatusCode::NOT_FOUND, "Work item not found").into_response(),
        Err(e) => {
            warn!("Failed to get work item: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

async fn delete_work_item(
    State(state): State<ProjectState>,
    Path((project_id, item_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let store = state.store.write().await;

    // Verify project exists
    let _project = match store.get_project(&project_id).await {
        Ok(Some(p)) => p,
        Ok(None) => return (StatusCode::NOT_FOUND, "Project not found").into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    // Get work item
    let item_result = if let Ok(num) = item_id.parse::<u32>() {
        store.get_work_item_by_number(&project_id, num).await
    } else {
        store.get_work_item(&item_id).await
    };

    match item_result {
        Ok(Some(item)) => {
            let item_id_str = item.id.clone();
            match store.delete_work_item(&item_id_str).await {
                Ok(_) => {
                    drop(store);
                    state.event_bus.issue_deleted(&project_id, &item_id_str);
                    Json(serde_json::json!({
                        "success": true,
                    }))
                    .into_response()
                }
                Err(e) => {
                    warn!("Failed to delete work item: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
                }
            }
        }
        Ok(None) => (StatusCode::NOT_FOUND, "Work item not found").into_response(),
        Err(e) => {
            warn!("Failed to get work item: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

// =============================================================================
// PROJECT ENTRIES ENDPOINTS
// =============================================================================

async fn list_project_entries(
    State(state): State<ProjectState>,
    Path(project_id): Path<String>,
) -> impl IntoResponse {
    let store = state.store.read().await;

    match store.get_project(&project_id).await {
        Ok(Some(project)) => {
            match store.list_project_entries(&project.id).await {
                Ok(entries) => Json(serde_json::json!({
                    "entries": entries,
                    "total": entries.len(),
                }))
                .into_response(),
                Err(e) => {
                    warn!("Failed to list project entries: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
                }
            }
        }
        Ok(None) => (StatusCode::NOT_FOUND, "Project not found").into_response(),
        Err(e) => {
            warn!("Failed to get project: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

// =============================================================================
// BOARD ENDPOINTS
// =============================================================================

/// Board column names in workflow order.
const BOARD_COLUMNS: [&str; 6] = ["backlog", "todo", "in_progress", "in_review", "testing", "done"];

/// Item type swimlanes in display order.
const SWIMLANES: [&str; 5] = ["epic", "story", "task", "subtask", "bug"];

/// Board response with work items organized by column and swimlane.
#[derive(Debug, Serialize)]
struct BoardResponse {
    /// All board columns in workflow order
    columns: Vec<BoardColumnInfo>,
    /// All swimlane types
    swimlanes: Vec<String>,
    /// Work items organized by swimlane -> column -> items
    items_by_swimlane: std::collections::HashMap<String, std::collections::HashMap<String, Vec<WorkItemResponse>>>,
    /// Column counts for quick display
    column_counts: std::collections::HashMap<String, i64>,
    /// Total work item count
    total_items: usize,
}

#[derive(Debug, Serialize)]
struct BoardColumnInfo {
    id: String,
    name: String,
    display_name: String,
}

#[derive(Debug, Deserialize)]
struct GetBoardQuery {
    /// Filter by item type (optional)
    item_type: Option<String>,
}

/// GET /api/projects/:id/board - Get board with all work items organized by column and swimlane.
async fn get_board(
    State(state): State<ProjectState>,
    Path(project_id): Path<String>,
    Query(query): Query<GetBoardQuery>,
) -> impl IntoResponse {
    let store = state.store.read().await;

    // Verify project exists
    match store.get_project(&project_id).await {
        Ok(Some(project)) => {
            // Get work items for board view (sorted by position within columns)
            match store.list_work_items_for_board(&project.id, query.item_type.as_deref()).await {
                Ok(items) => {
                    let total_items = items.len();

                    // Organize work items by swimlane (item_type) and column
                    let mut items_by_swimlane: std::collections::HashMap<String, std::collections::HashMap<String, Vec<WorkItemResponse>>> =
                        std::collections::HashMap::new();

                    // Initialize all swimlanes with empty column maps
                    for swimlane in SWIMLANES.iter() {
                        let mut columns_map: std::collections::HashMap<String, Vec<WorkItemResponse>> =
                            std::collections::HashMap::new();
                        for col in BOARD_COLUMNS.iter() {
                            columns_map.insert(col.to_string(), Vec::new());
                        }
                        items_by_swimlane.insert(swimlane.to_string(), columns_map);
                    }

                    // Populate with actual work items
                    for item in &items {
                        let swimlane = &item.item_type;
                        let column = &item.board_column;

                        if let Some(columns_map) = items_by_swimlane.get_mut(swimlane) {
                            if let Some(column_items) = columns_map.get_mut(column) {
                                column_items.push(WorkItemResponse::from(item));
                            }
                        }
                    }

                    // Get column counts
                    let column_counts: std::collections::HashMap<String, i64> =
                        match store.count_work_items_by_column(&project.id).await {
                            Ok(counts) => counts.into_iter().collect(),
                            Err(_) => std::collections::HashMap::new(),
                        };

                    // Build column info
                    let columns: Vec<BoardColumnInfo> = BOARD_COLUMNS
                        .iter()
                        .map(|col| BoardColumnInfo {
                            id: col.to_string(),
                            name: col.to_string(),
                            display_name: match *col {
                                "backlog" => "Backlog".to_string(),
                                "todo" => "To Do".to_string(),
                                "in_progress" => "In Progress".to_string(),
                                "in_review" => "In Review".to_string(),
                                "testing" => "Testing".to_string(),
                                "done" => "Done".to_string(),
                                _ => col.to_string(),
                            },
                        })
                        .collect();

                    Json(BoardResponse {
                        columns,
                        swimlanes: SWIMLANES.iter().map(|s| s.to_string()).collect(),
                        items_by_swimlane,
                        column_counts,
                        total_items,
                    })
                    .into_response()
                }
                Err(e) => {
                    warn!("Failed to get board work items: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
                }
            }
        }
        Ok(None) => (StatusCode::NOT_FOUND, "Project not found").into_response(),
        Err(e) => {
            warn!("Failed to get project: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
struct MoveCardRequest {
    /// Work item ID to move
    item_id: String,
    /// Target column (backlog, todo, in_progress, in_review, testing, done)
    to_column: String,
    /// Target position in the column (0-based)
    to_position: i64,
}

#[derive(Debug, Serialize)]
struct MoveCardResponse {
    success: bool,
    item_id: String,
    from_column: String,
    to_column: String,
    to_position: i64,
}

/// POST /api/projects/:id/board/move - Move a card to a new column and position.
async fn move_card(
    State(state): State<ProjectState>,
    Path(project_id): Path<String>,
    Json(req): Json<MoveCardRequest>,
) -> impl IntoResponse {
    // Validate target column
    if !BOARD_COLUMNS.contains(&req.to_column.as_str()) {
        return (
            StatusCode::BAD_REQUEST,
            format!(
                "Invalid column '{}'. Valid columns: {}",
                req.to_column,
                BOARD_COLUMNS.join(", ")
            ),
        )
            .into_response();
    }

    let store = state.store.write().await;

    // Verify project exists
    match store.get_project(&project_id).await {
        Ok(Some(_project)) => {
            // Get the work item to move
            match store.get_work_item(&req.item_id).await {
                Ok(Some(item)) => {
                    // Verify work item belongs to this project
                    if item.project_id != project_id {
                        return (StatusCode::BAD_REQUEST, "Work item does not belong to this project")
                            .into_response();
                    }

                    let from_column = item.board_column.clone();

                    // Shift existing cards in target column to make room
                    if let Err(e) = store
                        .shift_positions(&project_id, &req.to_column, req.to_position, 1)
                        .await
                    {
                        warn!("Failed to shift positions: {}", e);
                        return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
                    }

                    // Update the work item's position
                    match store
                        .update_work_item_position(&req.item_id, &req.to_column, req.to_position)
                        .await
                    {
                        Ok(true) => {
                            let item_id = req.item_id.clone();
                            drop(store);

                            // Emit card moved event with custom data
                            state.event_bus.publish(
                                ProjectEvent::new(ProjectEventType::IssueUpdated, &project_id)
                                    .with_resource(&item_id)
                                    .with_data(serde_json::json!({
                                        "action": "card_moved",
                                        "from_column": from_column,
                                        "to_column": req.to_column,
                                        "to_position": req.to_position,
                                    })),
                            );

                            Json(MoveCardResponse {
                                success: true,
                                item_id,
                                from_column,
                                to_column: req.to_column,
                                to_position: req.to_position,
                            })
                            .into_response()
                        }
                        Ok(false) => {
                            (StatusCode::NOT_FOUND, "Work item not found or update failed").into_response()
                        }
                        Err(e) => {
                            warn!("Failed to move card: {}", e);
                            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
                        }
                    }
                }
                Ok(None) => (StatusCode::NOT_FOUND, "Work item not found").into_response(),
                Err(e) => {
                    warn!("Failed to get work item: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
                }
            }
        }
        Ok(None) => (StatusCode::NOT_FOUND, "Project not found").into_response(),
        Err(e) => {
            warn!("Failed to get project: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

/// GET /api/projects/:id/board/columns - Get work item counts by column.
async fn get_column_counts(
    State(state): State<ProjectState>,
    Path(project_id): Path<String>,
) -> impl IntoResponse {
    let store = state.store.read().await;

    match store.get_project(&project_id).await {
        Ok(Some(project)) => {
            match store.count_work_items_by_column(&project.id).await {
                Ok(counts) => {
                    let counts_map: std::collections::HashMap<String, i64> =
                        counts.into_iter().collect();

                    // Ensure all columns are present
                    let mut result: std::collections::HashMap<String, i64> =
                        std::collections::HashMap::new();
                    for col in BOARD_COLUMNS.iter() {
                        result.insert(col.to_string(), *counts_map.get(*col).unwrap_or(&0));
                    }

                    Json(serde_json::json!({
                        "columns": result,
                        "total": result.values().sum::<i64>(),
                    }))
                    .into_response()
                }
                Err(e) => {
                    warn!("Failed to get column counts: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
                }
            }
        }
        Ok(None) => (StatusCode::NOT_FOUND, "Project not found").into_response(),
        Err(e) => {
            warn!("Failed to get project: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

/// GET /api/projects/:id/work-items/:item_id/children - Get child work items for a parent.
async fn get_child_work_items(
    State(state): State<ProjectState>,
    Path((project_id, item_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let store = state.store.read().await;

    // Verify project exists
    match store.get_project(&project_id).await {
        Ok(Some(_)) => {
            // Get parent work item to verify it exists and belongs to project
            let parent_result = if let Ok(num) = item_id.parse::<u32>() {
                store.get_work_item_by_number(&project_id, num).await
            } else {
                store.get_work_item(&item_id).await
            };

            match parent_result {
                Ok(Some(parent)) => {
                    if parent.project_id != project_id {
                        return (StatusCode::BAD_REQUEST, "Work item does not belong to this project")
                            .into_response();
                    }

                    match store.get_child_work_items(&parent.id).await {
                        Ok(children) => {
                            let child_responses: Vec<WorkItemResponse> =
                                children.iter().map(WorkItemResponse::from).collect();

                            Json(serde_json::json!({
                                "parent_id": parent.id,
                                "parent_type": parent.item_type,
                                "children": child_responses,
                                "total": child_responses.len(),
                            }))
                            .into_response()
                        }
                        Err(e) => {
                            warn!("Failed to get child work items: {}", e);
                            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
                        }
                    }
                }
                Ok(None) => (StatusCode::NOT_FOUND, "Parent work item not found").into_response(),
                Err(e) => {
                    warn!("Failed to get parent work item: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
                }
            }
        }
        Ok(None) => (StatusCode::NOT_FOUND, "Project not found").into_response(),
        Err(e) => {
            warn!("Failed to get project: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

// =============================================================================
// KNOWLEDGE LINKING ENDPOINTS
// =============================================================================

/// Request body for linking an entry.
#[derive(Debug, Deserialize)]
struct LinkEntryRequest {
    relevance: Option<f64>,
    notes: Option<String>,
}

/// POST /api/projects/:id/entries/:entry_id - Link a knowledge entry to a project.
async fn link_entry(
    State(state): State<ProjectState>,
    Path((project_id, entry_id)): Path<(String, String)>,
    Json(body): Json<LinkEntryRequest>,
) -> impl IntoResponse {
    let result = kix_services::link_entry(
        &state.store,
        Some(&state.event_bus),
        &project_id,
        &entry_id,
        body.relevance,
        body.notes.as_deref(),
    )
    .await;

    match result {
        Ok(link_result) => {
            Json(serde_json::json!({
                "success": true,
                "link_id": link_result.link_id,
                "project_id": link_result.project_id,
                "entry_id": link_result.entry_id,
            }))
            .into_response()
        }
        Err(e) => {
            warn!("Failed to link entry: {}", e);
            let status = match &e {
                kix_services::ServiceError::NotFound { .. } => StatusCode::NOT_FOUND,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            (status, e.to_string()).into_response()
        }
    }
}

/// DELETE /api/projects/:id/entries/:entry_id - Unlink a knowledge entry from a project.
async fn unlink_entry(
    State(state): State<ProjectState>,
    Path((project_id, entry_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let result = kix_services::unlink_entry(
        &state.store,
        Some(&state.event_bus),
        &project_id,
        &entry_id,
    )
    .await;

    match result {
        Ok(()) => {
            Json(serde_json::json!({
                "success": true,
                "project_id": project_id,
                "entry_id": entry_id,
            }))
            .into_response()
        }
        Err(e) => {
            warn!("Failed to unlink entry: {}", e);
            let status = match &e {
                kix_services::ServiceError::NotFound { .. } => StatusCode::NOT_FOUND,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            (status, e.to_string()).into_response()
        }
    }
}

/// Query parameters for project search.
#[derive(Debug, Deserialize)]
struct SearchProjectQuery {
    query: String,
    #[serde(rename = "type")]
    search_type: Option<String>,
    include_closed: Option<bool>,
    limit: Option<usize>,
}

/// GET /api/projects/:id/search - Search within a project's scope.
async fn search_project(
    State(state): State<ProjectState>,
    Path(project_id): Path<String>,
    Query(params): Query<SearchProjectQuery>,
) -> impl IntoResponse {
    let options = kix_services::ProjectSearchOptions {
        search_type: params.search_type,
        include_closed: params.include_closed.unwrap_or(false),
        limit: params.limit.unwrap_or(20),
    };

    let result = kix_services::search_project(&state.store, &project_id, &params.query, options).await;

    match result {
        Ok(search_result) => {
            Json(serde_json::json!({
                "query": params.query,
                "project_id": project_id,
                "work_items": search_result.work_items,
                "entries": search_result.entries,
                "total": search_result.work_items.len() + search_result.entries.len(),
            }))
            .into_response()
        }
        Err(e) => {
            warn!("Failed to search project: {}", e);
            let status = match &e {
                kix_services::ServiceError::NotFound { .. } => StatusCode::NOT_FOUND,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            (status, e.to_string()).into_response()
        }
    }
}

