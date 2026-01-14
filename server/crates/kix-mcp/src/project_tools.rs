//! Project management MCP tool parameters and response types.
//!
//! This module defines all the types for the project management tools:
//! - Project CRUD (5 tools)
//! - Issue CRUD (5 tools)
//! - GitHub Projects V2 (5 tools)
//! - AI Planning (4 tools)
//! - Token management (2 tools)
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

    /// GitHub repository owner (required)
    #[schemars(description = "GitHub repository owner (user or organization)")]
    pub github_owner: String,

    /// GitHub repository name (required)
    #[schemars(description = "GitHub repository name")]
    pub github_repo: String,

    /// GitHub Project V2 template (required)
    #[schemars(description = "Project template: 'kanban', 'bug_tracking', 'sprint_planning', or 'feature_roadmap'")]
    pub template: String,

    /// Project description (optional)
    #[schemars(description = "Project description")]
    pub description: Option<String>,

    /// Project color (hex format, optional)
    #[schemars(description = "Project color in hex format (e.g., '#3B82F6')")]
    pub color: Option<String>,

    /// Enable automatic sync with GitHub
    #[schemars(description = "Enable automatic sync with GitHub (default: false)")]
    pub auto_sync: Option<bool>,

    /// Sync direction
    #[schemars(description = "Sync direction: 'pull', 'push', or 'bidirectional' (default)")]
    pub sync_direction: Option<String>,
}

/// Response from creating a project.
#[derive(Debug, Serialize)]
pub struct CreateProjectResponse {
    pub success: bool,
    pub project_id: String,
    pub name: String,
    pub slug: String,
    pub github_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github_project_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
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
    pub github_owner: String,
    pub github_repo: String,
    pub archived: bool,
    pub open_issues: usize,
    pub closed_issues: usize,
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

    /// Include issue counts
    #[schemars(description = "Include issue counts (default: true)")]
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
    pub github_owner: String,
    pub github_repo: String,
    pub github_url: String,
    pub archived: bool,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats: Option<ProjectStats>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github_project: Option<GitHubProjectInfo>,
}

/// Project statistics.
#[derive(Debug, Serialize)]
pub struct ProjectStats {
    pub open_issues: usize,
    pub closed_issues: usize,
    pub total_issues: usize,
    pub linked_entries: usize,
}

/// GitHub Project V2 info.
#[derive(Debug, Serialize)]
pub struct GitHubProjectInfo {
    pub node_id: String,
    pub number: u32,
    pub title: String,
    pub url: String,
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

    /// Delete associated issues from Kix (not GitHub)
    #[schemars(description = "Also delete local issue copies (default: true)")]
    pub delete_issues: Option<bool>,
}

/// Response from deleting a project.
#[derive(Debug, Serialize)]
pub struct DeleteProjectResponse {
    pub success: bool,
    pub issues_deleted: usize,
    pub entries_unlinked: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// =============================================================================
// ISSUE CRUD PARAMETERS
// =============================================================================

/// Parameters for creating an issue.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateIssueParams {
    /// Project ID or slug
    #[schemars(description = "Project ID or slug")]
    pub project: String,

    /// Issue title (required)
    #[schemars(description = "Issue title")]
    pub title: String,

    /// Issue body/description
    #[schemars(description = "Issue body in markdown")]
    pub body: Option<String>,

    /// Labels to apply
    #[schemars(description = "Labels to apply to the issue")]
    pub labels: Option<Vec<String>>,

    /// Assignees (GitHub usernames)
    #[schemars(description = "GitHub usernames to assign")]
    pub assignees: Option<Vec<String>>,

    /// Push to GitHub immediately
    #[schemars(description = "Create issue on GitHub immediately (default: true)")]
    pub push_to_github: Option<bool>,
}

/// Response from creating an issue.
#[derive(Debug, Serialize)]
pub struct CreateIssueResponse {
    pub success: bool,
    pub issue_id: String,
    pub number: u32,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Parameters for listing issues.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListIssuesParams {
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
    #[schemars(description = "Filter by assignee GitHub username")]
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

/// Issue summary for listing.
#[derive(Debug, Serialize)]
pub struct IssueSummary {
    pub id: String,
    pub number: u32,
    pub title: String,
    pub state: String,
    pub labels: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignees: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github_url: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Response from listing issues.
#[derive(Debug, Serialize)]
pub struct ListIssuesResponse {
    pub issues: Vec<IssueSummary>,
    pub total: usize,
    pub has_more: bool,
}

/// Parameters for getting an issue.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetIssueParams {
    /// Project ID or slug
    #[schemars(description = "Project ID or slug")]
    pub project: String,

    /// Issue number or ID
    #[schemars(description = "Issue number (e.g., 42) or full ID")]
    pub issue: String,
}

/// Detailed issue response.
#[derive(Debug, Serialize)]
pub struct IssueDetail {
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github_number: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github_url: Option<String>,
    pub source: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Parameters for updating an issue.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateIssueParams {
    /// Project ID or slug
    #[schemars(description = "Project ID or slug")]
    pub project: String,

    /// Issue number or ID
    #[schemars(description = "Issue number or ID")]
    pub issue: String,

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

    /// Push changes to GitHub
    #[schemars(description = "Push changes to GitHub (default: true)")]
    pub push_to_github: Option<bool>,
}

/// Response from updating an issue.
#[derive(Debug, Serialize)]
pub struct UpdateIssueResponse {
    pub success: bool,
    pub issue_id: String,
    pub synced_to_github: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Parameters for deleting an issue.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteIssueParams {
    /// Project ID or slug
    #[schemars(description = "Project ID or slug")]
    pub project: String,

    /// Issue number or ID
    #[schemars(description = "Issue number or ID")]
    pub issue: String,

    /// Also close on GitHub (not delete)
    #[schemars(description = "Close the issue on GitHub (default: false)")]
    pub close_on_github: Option<bool>,
}

/// Response from deleting an issue.
#[derive(Debug, Serialize)]
pub struct DeleteIssueResponse {
    pub success: bool,
    pub closed_on_github: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// =============================================================================
// GITHUB PROJECTS V2 PARAMETERS
// =============================================================================

/// Parameters for creating a GitHub Project V2.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateGitHubProjectParams {
    /// Kix project ID or slug
    #[schemars(description = "Kix project ID or slug")]
    pub project: String,

    /// Project title on GitHub
    #[schemars(description = "Title for the GitHub Project (defaults to Kix project name)")]
    pub title: Option<String>,

    /// Template to use
    #[schemars(
        description = "Template: 'kanban', 'bug_tracking', 'sprint_planning', or 'feature_roadmap'"
    )]
    pub template: Option<String>,
}

/// Response from creating a GitHub Project.
#[derive(Debug, Serialize)]
pub struct CreateGitHubProjectResponse {
    pub success: bool,
    pub project_node_id: String,
    pub project_number: u32,
    pub project_url: String,
    pub template_applied: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Parameters for getting a GitHub Project.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetGitHubProjectParams {
    /// Kix project ID or slug
    #[schemars(description = "Kix project ID or slug")]
    pub project: String,

    /// Include items on the board
    #[schemars(description = "Include items on the board (default: true)")]
    pub include_items: Option<bool>,
}

/// GitHub Project field info.
#[derive(Debug, Serialize)]
pub struct GitHubFieldInfo {
    pub id: String,
    pub name: String,
    pub field_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<GitHubFieldOptionInfo>>,
}

/// GitHub field option.
#[derive(Debug, Serialize)]
pub struct GitHubFieldOptionInfo {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

/// GitHub Project item.
#[derive(Debug, Serialize)]
pub struct GitHubProjectItemInfo {
    pub item_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issue_number: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub field_values: Vec<GitHubFieldValueInfo>,
}

/// GitHub field value.
#[derive(Debug, Serialize)]
pub struct GitHubFieldValueInfo {
    pub field_name: String,
    pub value: String,
}

/// Response from getting a GitHub Project.
#[derive(Debug, Serialize)]
pub struct GetGitHubProjectResponse {
    pub node_id: String,
    pub number: u32,
    pub title: String,
    pub url: String,
    pub fields: Vec<GitHubFieldInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<GitHubProjectItemInfo>>,
}

/// Parameters for adding an issue to a GitHub Project.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddIssueToProjectParams {
    /// Kix project ID or slug
    #[schemars(description = "Kix project ID or slug")]
    pub project: String,

    /// Issue number or ID to add
    #[schemars(description = "Issue number or ID to add to the project board")]
    pub issue: String,

    /// Initial status (column)
    #[schemars(description = "Initial status (e.g., 'Todo', 'In Progress')")]
    pub status: Option<String>,
}

/// Response from adding an issue to a project.
#[derive(Debug, Serialize)]
pub struct AddIssueToProjectResponse {
    pub success: bool,
    pub item_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Parameters for updating a project item.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateProjectItemParams {
    /// Kix project ID or slug
    #[schemars(description = "Kix project ID or slug")]
    pub project: String,

    /// Issue number or project item ID
    #[schemars(description = "Issue number or project item ID")]
    pub item: String,

    /// New status
    #[schemars(description = "New status value")]
    pub status: Option<String>,

    /// Priority (for templates that support it)
    #[schemars(description = "Priority value (e.g., 'High', 'Medium', 'Low')")]
    pub priority: Option<String>,

    /// Custom field updates
    #[schemars(description = "Custom field updates as field_name: value pairs")]
    pub custom_fields: Option<std::collections::HashMap<String, String>>,
}

/// Response from updating a project item.
#[derive(Debug, Serialize)]
pub struct UpdateProjectItemResponse {
    pub success: bool,
    pub fields_updated: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Parameters for syncing a GitHub Project.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SyncGitHubProjectParams {
    /// Kix project ID or slug
    #[schemars(description = "Kix project ID or slug")]
    pub project: String,

    /// Sync direction
    #[schemars(description = "Sync direction: 'pull', 'push', or 'bidirectional' (default)")]
    pub direction: Option<String>,

    /// Include closed issues
    #[schemars(description = "Include closed issues in sync (default: true)")]
    pub include_closed: Option<bool>,
}

/// Response from syncing a GitHub Project.
#[derive(Debug, Serialize)]
pub struct SyncGitHubProjectResponse {
    pub success: bool,
    pub issues_pulled: usize,
    pub issues_pushed: usize,
    pub issues_updated: usize,
    pub errors: Vec<String>,
    pub synced_at: String,
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
// TOKEN MANAGEMENT PARAMETERS
// =============================================================================

/// Parameters for setting a GitHub token.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetGitHubTokenParams {
    /// The GitHub Personal Access Token
    #[schemars(description = "GitHub Personal Access Token (PAT)")]
    pub token: String,

    /// Scope: global or project-specific
    #[schemars(description = "Scope: 'global' or a project ID/slug for project-specific token")]
    pub scope: Option<String>,
}

/// Response from setting a token.
#[derive(Debug, Serialize)]
pub struct SetGitHubTokenResponse {
    pub success: bool,
    pub scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Parameters for syncing GitHub issues.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SyncGitHubIssuesParams {
    /// Kix project ID or slug
    #[schemars(description = "Kix project ID or slug")]
    pub project: String,

    /// Sync direction
    #[schemars(description = "Sync direction: 'pull', 'push', or 'bidirectional'")]
    pub direction: Option<String>,

    /// Include closed issues
    #[schemars(description = "Include closed issues (default: true)")]
    pub include_closed: Option<bool>,

    /// Labels filter
    #[schemars(description = "Only sync issues with these labels")]
    pub labels: Option<Vec<String>>,

    /// Maximum issues
    #[schemars(description = "Maximum issues to sync (default: 100)")]
    pub max_issues: Option<usize>,
}

/// Response from syncing issues.
#[derive(Debug, Serialize)]
pub struct SyncGitHubIssuesResponse {
    pub success: bool,
    pub issues_pulled: usize,
    pub issues_pushed: usize,
    pub issues_updated: usize,
    pub issues_failed: usize,
    pub errors: Vec<String>,
    pub synced_at: String,
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
    #[schemars(description = "Search type: 'all', 'issues', or 'knowledge' (default: 'all')")]
    pub search_type: Option<String>,

    /// Maximum results
    #[schemars(description = "Maximum results (default: 20)")]
    pub limit: Option<usize>,

    /// Include closed issues
    #[schemars(description = "Include closed issues (default: false)")]
    pub include_closed: Option<bool>,
}

/// Issue search result.
#[derive(Debug, Serialize)]
pub struct IssueSearchResultItem {
    pub id: String,
    pub number: u32,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub excerpt: Option<String>,
    pub state: String,
    pub labels: Vec<String>,
    pub score: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github_url: Option<String>,
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
    pub issues: Vec<IssueSearchResultItem>,
    pub knowledge: Vec<KnowledgeSearchResultItem>,
}
