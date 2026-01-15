//! Project management MCP tool parameters and response types.
//!
//! This module defines all the types for the project management tools:
//! - Project CRUD (5 tools)
//! - Work Item CRUD (5 tools)
//! - Board Operations (3 tools): get_board, move_card, get_child_work_items
//! - AI Planning (4 tools)
//! - Knowledge linking (3 tools)
//! - Search (1 tool)

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// =============================================================================
// PROJECT CRUD PARAMETERS
// =============================================================================

/// Parameters for creating a new project.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateProjectParams {
    /// Project name (required)
    #[schemars(description = "Name of the project")]
    pub name: String,

    /// Project description (optional)
    #[schemars(description = "Project description")]
    pub description: Option<String>,

    /// Project color (hex format, optional)
    #[schemars(description = "Project color in hex format (e.g., '#3B82F6')")]
    pub color: Option<String>,
}

/// Response from creating a project.
#[derive(Debug, Serialize)]
pub struct CreateProjectResponse {
    pub success: bool,
    pub project_id: String,
    pub name: String,
    pub slug: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Parameters for listing projects.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListProjectsParams {
    /// Include archived projects
    #[schemars(description = "Include archived projects (default: false)")]
    pub include_archived: Option<bool>,

    /// Maximum number of results
    #[schemars(description = "Maximum number of results (default: 50)")]
    pub limit: Option<usize>,

    /// Offset for pagination
    #[schemars(description = "Offset for pagination (default: 0)")]
    pub offset: Option<usize>,
}

/// A project summary for listing.
#[derive(Debug, Serialize)]
pub struct ProjectSummary {
    pub id: String,
    pub name: String,
    pub slug: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    pub archived: bool,
    pub open_items: usize,
    pub closed_items: usize,
    pub created_at: String,
}

/// Response from listing projects.
#[derive(Debug, Serialize)]
pub struct ListProjectsResponse {
    pub projects: Vec<ProjectSummary>,
    pub total: usize,
    pub has_more: bool,
}

/// Parameters for getting a project.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetProjectParams {
    /// Project ID or slug
    #[schemars(description = "Project ID or slug")]
    pub project: String,

    /// Include work item counts
    #[schemars(description = "Include work item counts (default: true)")]
    pub include_stats: Option<bool>,
}

/// Detailed project response.
#[derive(Debug, Serialize)]
pub struct ProjectDetail {
    pub id: String,
    pub name: String,
    pub slug: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    pub archived: bool,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats: Option<ProjectStats>,
}

/// Project statistics.
#[derive(Debug, Serialize)]
pub struct ProjectStats {
    pub open_items: usize,
    pub closed_items: usize,
    pub total_items: usize,
    pub linked_entries: usize,
}

/// Parameters for updating a project.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateProjectParams {
    /// Project ID or slug
    #[schemars(description = "Project ID or slug")]
    pub project: String,

    /// New name
    #[schemars(description = "New project name")]
    pub name: Option<String>,

    /// New description
    #[schemars(description = "New project description")]
    pub description: Option<String>,

    /// New color
    #[schemars(description = "New project color in hex format")]
    pub color: Option<String>,

    /// Archive/unarchive
    #[schemars(description = "Set archived status")]
    pub archived: Option<bool>,
}

/// Response from updating a project.
#[derive(Debug, Serialize)]
pub struct UpdateProjectResponse {
    pub success: bool,
    pub project_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Parameters for deleting a project.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteProjectParams {
    /// Project ID or slug
    #[schemars(description = "Project ID or slug")]
    pub project: String,

    /// Delete associated work items from Kix
    #[schemars(description = "Also delete local work item copies (default: true)")]
    pub delete_items: Option<bool>,
}

/// Response from deleting a project.
#[derive(Debug, Serialize)]
pub struct DeleteProjectResponse {
    pub success: bool,
    pub items_deleted: usize,
    pub entries_unlinked: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// =============================================================================
// WORK ITEM CRUD PARAMETERS
// =============================================================================

/// Parameters for creating a work item.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateWorkItemParams {
    /// Project ID or slug
    #[schemars(description = "Project ID or slug")]
    pub project: String,

    /// Work item title (required)
    #[schemars(description = "Work item title")]
    pub title: String,

    /// Work item body/description
    #[schemars(description = "Work item body in markdown")]
    pub body: Option<String>,

    /// Labels to apply
    #[schemars(description = "Labels to apply to the work item")]
    pub labels: Option<Vec<String>>,

    /// Assignees (usernames)
    #[schemars(description = "Usernames to assign")]
    pub assignees: Option<Vec<String>>,

    // Board fields
    /// Item type for board swimlane placement
    #[schemars(description = "Item type: 'epic', 'story', 'task', 'subtask', or 'bug' (default: 'task')")]
    pub item_type: Option<String>,

    /// Parent work item ID for hierarchy
    #[schemars(description = "Parent work item ID for creating sub-items (e.g., a subtask under a story)")]
    pub parent_id: Option<String>,

    /// Board column for initial placement
    #[schemars(description = "Board column: 'backlog', 'todo', 'in_progress', 'in_review', 'testing', or 'done' (default: 'backlog')")]
    pub board_column: Option<String>,

    /// Story points estimate
    #[schemars(description = "Story points estimate (for agile planning)")]
    pub story_points: Option<i64>,

    /// Epic color (hex, for epic items only)
    #[schemars(description = "Epic color in hex format (e.g., 'A855F7'), only used for epic item type")]
    pub epic_color: Option<String>,
}

/// Response from creating a work item.
#[derive(Debug, Serialize)]
pub struct CreateWorkItemResponse {
    pub success: bool,
    pub item_id: String,
    pub number: u32,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Parameters for listing work items.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListWorkItemsParams {
    /// Project ID or slug
    #[schemars(description = "Project ID or slug")]
    pub project: String,

    /// Filter by state
    #[schemars(description = "Filter by state: 'open', 'closed', or 'all' (default: 'open')")]
    pub state: Option<String>,

    /// Filter by labels
    #[schemars(description = "Filter by labels (all must match)")]
    pub labels: Option<Vec<String>>,

    /// Filter by assignee
    #[schemars(description = "Filter by assignee username")]
    pub assignee: Option<String>,

    /// Search in title/body
    #[schemars(description = "Search term for title and body")]
    pub search: Option<String>,

    /// Maximum results
    #[schemars(description = "Maximum results (default: 50)")]
    pub limit: Option<usize>,

    /// Offset for pagination
    #[schemars(description = "Offset for pagination (default: 0)")]
    pub offset: Option<usize>,
}

/// Work item summary for listing.
#[derive(Debug, Clone, Serialize)]
pub struct WorkItemSummary {
    pub id: String,
    pub number: u32,
    pub title: String,
    pub state: String,
    pub labels: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignees: Option<Vec<String>>,
    pub created_at: String,
    pub updated_at: String,
    // Board fields
    pub item_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    pub board_column: String,
    pub position: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub story_points: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub epic_color: Option<String>,
}

/// Response from listing work items.
#[derive(Debug, Serialize)]
pub struct ListWorkItemsResponse {
    pub items: Vec<WorkItemSummary>,
    pub total: usize,
    pub has_more: bool,
}

/// Parameters for getting a work item.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetWorkItemParams {
    /// Project ID or slug
    #[schemars(description = "Project ID or slug")]
    pub project: String,

    /// Work item number or ID
    #[schemars(description = "Work item number (e.g., 42) or full ID")]
    pub item: String,
}

/// Detailed work item response.
#[derive(Debug, Serialize)]
pub struct WorkItemDetail {
    pub id: String,
    pub project_id: String,
    pub number: u32,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    pub state: String,
    pub labels: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignees: Option<Vec<String>>,
    pub created_at: String,
    pub updated_at: String,
    // Board fields
    pub item_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    pub board_column: String,
    pub position: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub story_points: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub epic_color: Option<String>,
}

/// Parameters for updating a work item.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateWorkItemParams {
    /// Project ID or slug
    #[schemars(description = "Project ID or slug")]
    pub project: String,

    /// Work item number or ID
    #[schemars(description = "Work item number or ID")]
    pub item: String,

    /// New title
    #[schemars(description = "New title")]
    pub title: Option<String>,

    /// New body
    #[schemars(description = "New body in markdown")]
    pub body: Option<String>,

    /// New state
    #[schemars(description = "New state: 'open' or 'closed'")]
    pub state: Option<String>,

    /// New labels (replaces existing)
    #[schemars(description = "New labels (replaces existing)")]
    pub labels: Option<Vec<String>>,

    /// New assignees (replaces existing)
    #[schemars(description = "New assignees (replaces existing)")]
    pub assignees: Option<Vec<String>>,

    // Board fields
    /// Change item type (may affect swimlane)
    #[schemars(description = "Change item type: 'epic', 'story', 'task', 'subtask', or 'bug'")]
    pub item_type: Option<String>,

    /// Change parent work item
    #[schemars(description = "New parent work item ID (null to make top-level)")]
    pub parent_id: Option<String>,

    /// Move to board column
    #[schemars(description = "Move to board column: 'backlog', 'todo', 'in_progress', 'in_review', 'testing', or 'done'")]
    pub board_column: Option<String>,

    /// Update story points
    #[schemars(description = "Update story points estimate")]
    pub story_points: Option<i64>,

    /// Update epic color (epics only)
    #[schemars(description = "Update epic color in hex format")]
    pub epic_color: Option<String>,
}

/// Response from updating a work item.
#[derive(Debug, Serialize)]
pub struct UpdateWorkItemResponse {
    pub success: bool,
    pub item_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Parameters for deleting a work item.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteWorkItemParams {
    /// Project ID or slug
    #[schemars(description = "Project ID or slug")]
    pub project: String,

    /// Work item number or ID
    #[schemars(description = "Work item number or ID")]
    pub item: String,
}

/// Response from deleting a work item.
#[derive(Debug, Serialize)]
pub struct DeleteWorkItemResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// =============================================================================
// AI PLANNING PARAMETERS
// =============================================================================

/// Parameters for AI project planning.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PlanProjectParams {
    /// Kix project ID or slug
    #[schemars(description = "Kix project ID or slug")]
    pub project: String,

    /// Project goal/description for planning
    #[schemars(description = "Project goal or description to plan for")]
    pub goal: String,

    /// Limit knowledge context
    #[schemars(description = "Maximum knowledge entries to include (default: 10)")]
    pub max_context: Option<usize>,

    /// Template style
    #[schemars(description = "Planning template: 'agile', 'waterfall', 'kanban' (default: 'agile')")]
    pub template: Option<String>,
}

/// Planned task from AI.
#[derive(Debug, Serialize)]
pub struct PlannedTask {
    pub title: String,
    pub description: String,
    pub labels: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depends_on: Option<Vec<usize>>,
    pub estimated_effort: String,
    pub priority: String,
}

/// Response from project planning.
#[derive(Debug, Serialize)]
pub struct PlanProjectResponse {
    pub success: bool,
    pub tasks: Vec<PlannedTask>,
    pub knowledge_used: Vec<String>,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Parameters for task suggestions.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SuggestTasksParams {
    /// Kix project ID or slug
    #[schemars(description = "Kix project ID or slug")]
    pub project: String,

    /// Context for suggestions
    #[schemars(description = "Context or area for task suggestions")]
    pub context: Option<String>,

    /// Number of suggestions
    #[schemars(description = "Number of suggestions to return (default: 5)")]
    pub count: Option<usize>,
}

/// Task suggestion.
#[derive(Debug, Serialize)]
pub struct TaskSuggestion {
    pub title: String,
    pub description: String,
    pub reason: String,
    pub related_knowledge: Vec<String>,
}

/// Response from task suggestions.
#[derive(Debug, Serialize)]
pub struct SuggestTasksResponse {
    pub suggestions: Vec<TaskSuggestion>,
}

/// Parameters for getting project context.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetProjectContextParams {
    /// Kix project ID or slug
    #[schemars(description = "Kix project ID or slug")]
    pub project: String,

    /// Query for relevant context
    #[schemars(description = "Query to find relevant knowledge")]
    pub query: Option<String>,

    /// Maximum entries
    #[schemars(description = "Maximum knowledge entries to return (default: 10)")]
    pub limit: Option<usize>,
}

/// Knowledge context entry.
#[derive(Debug, Serialize)]
pub struct ContextEntry {
    pub entry_id: String,
    pub title: String,
    pub entry_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub excerpt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    pub relevance_score: f32,
}

/// Response from getting project context.
#[derive(Debug, Serialize)]
pub struct GetProjectContextResponse {
    pub entries: Vec<ContextEntry>,
    pub total_linked: usize,
}

/// Parameters for task breakdown.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct BreakdownTaskParams {
    /// Kix project ID or slug
    #[schemars(description = "Kix project ID or slug")]
    pub project: String,

    /// Task title or description to break down
    #[schemars(description = "Task title or description to break down")]
    pub task: String,

    /// Desired granularity
    #[schemars(description = "Granularity: 'fine' (many small), 'medium', 'coarse' (few large)")]
    pub granularity: Option<String>,
}

/// Subtask from breakdown.
#[derive(Debug, Serialize)]
pub struct SubTask {
    pub title: String,
    pub description: String,
    pub estimated_hours: f32,
    pub order: usize,
}

/// Response from task breakdown.
#[derive(Debug, Serialize)]
pub struct BreakdownTaskResponse {
    pub subtasks: Vec<SubTask>,
    pub total_estimated_hours: f32,
}

// =============================================================================
// KNOWLEDGE LINKING PARAMETERS
// =============================================================================

/// Parameters for linking an entry to a project.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct LinkEntryParams {
    /// Kix project ID or slug
    #[schemars(description = "Kix project ID or slug")]
    pub project: String,

    /// Entry ID to link
    #[schemars(description = "Knowledge entry ID to link")]
    pub entry_id: String,

    /// Relevance score (0.0 to 1.0)
    #[schemars(description = "Relevance score for this link (0.0 to 1.0)")]
    pub relevance: Option<f32>,

    /// Optional notes
    #[schemars(description = "Additional notes about the link")]
    pub notes: Option<String>,
}

/// Response from linking an entry.
#[derive(Debug, Serialize)]
pub struct LinkEntryResponse {
    pub success: bool,
    pub link_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Parameters for unlinking an entry.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct UnlinkEntryParams {
    /// Kix project ID or slug
    #[schemars(description = "Kix project ID or slug")]
    pub project: String,

    /// Entry ID to unlink
    #[schemars(description = "Knowledge entry ID to unlink")]
    pub entry_id: String,
}

/// Response from unlinking an entry.
#[derive(Debug, Serialize)]
pub struct UnlinkEntryResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Parameters for listing project entries.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListProjectEntriesParams {
    /// Kix project ID or slug
    #[schemars(description = "Kix project ID or slug")]
    pub project: String,

    /// Filter by entry type
    #[schemars(description = "Filter by entry type")]
    pub entry_type: Option<String>,

    /// Maximum results
    #[schemars(description = "Maximum results (default: 50)")]
    pub limit: Option<usize>,
}

/// Linked entry info.
#[derive(Debug, Serialize)]
pub struct LinkedEntry {
    pub entry_id: String,
    pub title: String,
    pub entry_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relevance: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    pub linked_at: String,
}

/// Response from listing project entries.
#[derive(Debug, Serialize)]
pub struct ListProjectEntriesResponse {
    pub entries: Vec<LinkedEntry>,
    pub total: usize,
}

// =============================================================================
// PROJECT SEARCH PARAMETERS
// =============================================================================

/// Parameters for searching within a project.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchProjectParams {
    /// Kix project ID or slug
    #[schemars(description = "Kix project ID or slug")]
    pub project: String,

    /// Search query
    #[schemars(description = "Search query")]
    pub query: String,

    /// Search type filter
    #[schemars(description = "Search type: 'all', 'work_items', or 'knowledge' (default: 'all')")]
    pub search_type: Option<String>,

    /// Maximum results
    #[schemars(description = "Maximum results (default: 20)")]
    pub limit: Option<usize>,

    /// Include closed work items
    #[schemars(description = "Include closed work items (default: false)")]
    pub include_closed: Option<bool>,
}

/// Work item search result.
#[derive(Debug, Serialize)]
pub struct WorkItemSearchResultItem {
    pub id: String,
    pub number: u32,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub excerpt: Option<String>,
    pub state: String,
    pub labels: Vec<String>,
    pub score: f32,
}

/// Knowledge search result.
#[derive(Debug, Serialize)]
pub struct KnowledgeSearchResultItem {
    pub entry_id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub excerpt: Option<String>,
    pub entry_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    pub score: f32,
}

/// Response from project search.
#[derive(Debug, Serialize)]
pub struct SearchProjectResponse {
    pub total: usize,
    pub work_items: Vec<WorkItemSearchResultItem>,
    pub knowledge: Vec<KnowledgeSearchResultItem>,
}

// =============================================================================
// BOARD TOOL PARAMETERS
// =============================================================================

/// Parameters for getting board view.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetBoardParams {
    /// Project ID or slug
    #[schemars(description = "Project ID or slug")]
    pub project: String,

    /// Filter by item type (swimlane)
    #[schemars(description = "Filter by item type: 'epic', 'story', 'task', 'subtask', or 'bug'")]
    pub item_type: Option<String>,
}

/// Board column info.
#[derive(Debug, Serialize)]
pub struct BoardColumnInfo {
    pub id: String,
    pub name: String,
    pub display_name: String,
}

/// Board swimlane with work items.
#[derive(Debug, Serialize)]
pub struct BoardSwimlane {
    pub item_type: String,
    pub label: String,
    pub columns: std::collections::HashMap<String, Vec<WorkItemSummary>>,
    pub total_items: usize,
}

/// Response from get_board.
#[derive(Debug, Serialize)]
pub struct GetBoardResponse {
    pub columns: Vec<BoardColumnInfo>,
    pub swimlanes: Vec<BoardSwimlane>,
    pub column_counts: std::collections::HashMap<String, usize>,
    pub total_items: usize,
}

/// Parameters for getting column counts.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetColumnCountsParams {
    /// Project ID or slug
    #[schemars(description = "Project ID or slug")]
    pub project: String,
}

/// Response from get_column_counts.
#[derive(Debug, Serialize)]
pub struct GetColumnCountsResponse {
    pub counts: std::collections::HashMap<String, usize>,
    pub total: usize,
}

/// Parameters for moving a card.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct MoveCardParams {
    /// Project ID or slug
    #[schemars(description = "Project ID or slug")]
    pub project: String,

    /// Work item ID to move
    #[schemars(description = "Work item ID or number to move")]
    pub item: String,

    /// Target board column
    #[schemars(description = "Target column: 'backlog', 'todo', 'in_progress', 'in_review', 'testing', or 'done'")]
    pub to_column: String,

    /// Target position in column (0 = top)
    #[schemars(description = "Target position in column (0 = top, default: 0)")]
    pub to_position: Option<i64>,
}

/// Response from move_card.
#[derive(Debug, Serialize)]
pub struct MoveCardResponse {
    pub success: bool,
    pub item_id: String,
    pub from_column: String,
    pub to_column: String,
    pub to_position: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Parameters for getting child work items.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetChildWorkItemsParams {
    /// Project ID or slug
    #[schemars(description = "Project ID or slug")]
    pub project: String,

    /// Parent work item ID
    #[schemars(description = "Parent work item ID to get children for")]
    pub parent_id: String,
}

/// Response from get_child_work_items.
#[derive(Debug, Serialize)]
pub struct GetChildWorkItemsResponse {
    pub parent_id: String,
    pub children: Vec<WorkItemSummary>,
    pub total: usize,
}
