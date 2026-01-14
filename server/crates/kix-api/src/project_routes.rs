//! Project API routes with SSE for real-time events.
//!
//! Provides REST endpoints for project management and a Server-Sent Events
//! endpoint for real-time updates from MCP operations.

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
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
use tracing::{debug, info, warn};

use kix_projects::{ProjectTemplate, ProjectTokenType, SharedEventBus, TokenService};
use kix_projects::github::{GitHubRestClient, GitHubSyncService, ProjectV2Service, SyncConfig, SyncDirection};
use kix_store::{IssueRecord, ProjectRecord, ProjectStore, TokenStore};

/// State for project routes.
#[derive(Clone)]
pub struct ProjectState {
    /// Project store
    store: Arc<RwLock<ProjectStore>>,
    /// Event bus for real-time updates
    event_bus: SharedEventBus,
    /// Token service for GitHub token management
    token_service: Arc<TokenService>,
}

impl ProjectState {
    /// Create a new project state.
    pub fn new(
        store: Arc<RwLock<ProjectStore>>,
        event_bus: SharedEventBus,
        token_store: Arc<RwLock<TokenStore>>,
    ) -> Self {
        // Create token service with optional encryption
        let token_service = match TokenService::with_encryption(token_store.clone()) {
            Ok(service) => {
                info!("GitHub token encryption enabled");
                Arc::new(service)
            }
            Err(e) => {
                warn!(
                    "GitHub token encryption not available: {}. Tokens will be stored in plaintext.",
                    e
                );
                Arc::new(TokenService::new(token_store, None))
            }
        };

        Self {
            store,
            event_bus,
            token_service,
        }
    }

    /// Get a reference to the project store.
    pub fn store(&self) -> &Arc<RwLock<ProjectStore>> {
        &self.store
    }

    /// Get a reference to the event bus.
    pub fn event_bus(&self) -> &SharedEventBus {
        &self.event_bus
    }

    /// Get a reference to the token service.
    pub fn token_service(&self) -> &Arc<TokenService> {
        &self.token_service
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
            "/api/projects/:id/issues",
            get(list_issues).post(create_issue),
        )
        .route(
            "/api/projects/:id/issues/:issue_id",
            get(get_issue).put(update_issue).delete(delete_issue),
        )
        .route("/api/projects/:id/entries", get(list_project_entries))
        // GitHub token management endpoints (global)
        .route("/api/github/token/status", get(github_token_status))
        .route("/api/github/token/global", post(set_global_github_token))
        .route("/api/github/user", get(github_user))
        .route("/api/github/orgs", get(github_orgs))
        .route("/api/github/repos", get(github_repos))
        .route("/api/github/verify-access", post(github_verify_access))
        // Project-specific GitHub integration endpoints
        .route(
            "/api/projects/:id/github/token-status",
            get(project_token_status),
        )
        .route("/api/projects/:id/github/token", post(set_github_token))
        .route("/api/projects/:id/github/sync", post(sync_github))
        .route("/api/projects/:id/github/link-project", post(link_github_project))
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
    github_owner: Option<String>,
    github_repo: Option<String>,
    has_github: bool,
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
                    let has_github = p.has_github();
                    let github_owner = p.github_owner().map(String::from);
                    let github_repo = p.github_repo().map(String::from);
                    let is_archived = p.is_archived();
                    ProjectSummary {
                        id: p.id,
                        name: p.name,
                        slug: p.slug,
                        description: p.description,
                        color: p.color,
                        github_owner,
                        github_repo,
                        has_github,
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

    // GitHub config (all optional for local-only projects)
    github_owner: Option<String>,
    github_repo: Option<String>,

    // GitHub Project V2 template (required for GitHub projects)
    // Valid values: "kanban", "bug_tracking", "sprint_planning", "feature_roadmap"
    template: Option<String>,

    // Token config
    #[serde(default)]
    use_global_token: bool, // Default: true if no token provided
    token: Option<String>,  // Project-specific token
}

async fn create_project(
    State(state): State<ProjectState>,
    Json(req): Json<CreateProjectRequest>,
) -> impl IntoResponse {
    // Determine if this is a GitHub-integrated project or local-only
    let has_github =
        req.github_owner.is_some() && req.github_repo.is_some();

    // Validate: GitHub projects require a template
    if has_github && req.template.is_none() {
        return (
            StatusCode::BAD_REQUEST,
            "Template is required for GitHub projects. Choose: kanban, bug_tracking, sprint_planning, feature_roadmap"
        ).into_response();
    }

    // Parse template if provided
    let template = if let Some(ref template_str) = req.template {
        match template_str.as_str() {
            "kanban" => Some(ProjectTemplate::Kanban),
            "bug_tracking" => Some(ProjectTemplate::BugTracking),
            "sprint_planning" => Some(ProjectTemplate::SprintPlanning),
            "feature_roadmap" => Some(ProjectTemplate::FeatureRoadmap),
            _ => {
                return (
                    StatusCode::BAD_REQUEST,
                    format!("Invalid template '{}'. Choose: kanban, bug_tracking, sprint_planning, feature_roadmap", template_str)
                ).into_response();
            }
        }
    } else {
        None
    };

    // Create project record based on whether GitHub is configured
    let mut project = if has_github {
        ProjectRecord::new(
            req.name.clone(),
            req.github_owner.clone().unwrap(),
            req.github_repo.clone().unwrap(),
        )
    } else {
        ProjectRecord::new_local(req.name.clone())
    };

    if let Some(desc) = req.description.clone() {
        project = project.with_description(desc);
    }
    if let Some(color) = req.color.clone() {
        project = project.with_color(color);
    }

    // Store the project first
    let store = state.store.write().await;
    match store.create_project(&project).await {
        Ok(_) => {
            drop(store);

            // Track GitHub token and Project V2 creation results
            let mut github_project_url: Option<String> = None;
            let mut project_v2_warning: Option<String> = None;

            // If a token was provided, store it as project-specific
            if let Some(ref token) = req.token {
                let verify_repo = if has_github {
                    Some((
                        req.github_owner.as_ref().unwrap().as_str(),
                        req.github_repo.as_ref().unwrap().as_str(),
                    ))
                } else {
                    None
                };

                match state
                    .token_service
                    .set_project_token(&project.id, token, verify_repo)
                    .await
                {
                    Ok(result) => {
                        debug!(
                            "Set project token for {}: verified_user={:?}",
                            project.id, result.verified_user
                        );
                    }
                    Err(e) => {
                        warn!(
                            "Failed to set project token for {}: {}. Project created but token not saved.",
                            project.id, e
                        );
                        // Note: We don't fail the request here, project is already created
                    }
                }
            }

            // Create GitHub Project V2 if this is a GitHub project with a template
            if has_github && template.is_some() {
                let owner = req.github_owner.as_ref().unwrap();
                let repo = req.github_repo.as_ref().unwrap();
                let template_type = template.unwrap();

                // Get token for GitHub API (prefer project-specific, fallback to global)
                let github_token = if let Some(ref token) = req.token {
                    Some(token.clone())
                } else if req.use_global_token {
                    match state.token_service.get_global_token_decrypted().await {
                        Ok(token) => Some(token),
                        _ => None,
                    }
                } else {
                    None
                };

                if let Some(token) = github_token {
                    match ProjectV2Service::new(&token) {
                        Ok(v2_service) => {
                            info!(
                                "Creating GitHub Project V2 '{}' with {} template for {}/{}",
                                project.name, template_type, owner, repo
                            );

                            match v2_service
                                .create_project_with_template(owner, repo, &project.name, template_type)
                                .await
                            {
                                Ok(v2_config) => {
                                    info!(
                                        "GitHub Project V2 created: {} at {}",
                                        v2_config.project_number, v2_config.url
                                    );
                                    github_project_url = Some(v2_config.url.clone());

                                    // Update project record with Project V2 config
                                    let v2_json = serde_json::json!({
                                        "project_id": v2_config.project_id,
                                        "project_number": v2_config.project_number,
                                        "url": v2_config.url,
                                        "status_field_id": v2_config.status_field_id,
                                        "status_options": v2_config.status_options.iter().map(|o| {
                                            serde_json::json!({
                                                "name": o.name,
                                                "option_id": o.option_id
                                            })
                                        }).collect::<Vec<_>>(),
                                        "custom_fields": v2_config.custom_fields.iter().map(|f| {
                                            serde_json::json!({
                                                "name": f.name,
                                                "field_id": f.field_id,
                                                "field_type": format!("{:?}", f.field_type),
                                                "options": f.options.iter().map(|o| {
                                                    serde_json::json!({
                                                        "name": o.name,
                                                        "option_id": o.option_id
                                                    })
                                                }).collect::<Vec<_>>()
                                            })
                                        }).collect::<Vec<_>>()
                                    });

                                    let updated_project = project.clone().with_github_project_v2(v2_json);

                                    let store = state.store.write().await;
                                    if let Err(e) = store.update_project(&updated_project).await {
                                        warn!(
                                            "Failed to save Project V2 config for {}: {}",
                                            project.id, e
                                        );
                                        project_v2_warning = Some(
                                            "Project V2 created but config not saved".to_string()
                                        );
                                    }
                                }
                                Err(e) => {
                                    warn!("Failed to create GitHub Project V2: {}", e);
                                    project_v2_warning = Some(format!(
                                        "Failed to create GitHub Project V2: {}",
                                        e
                                    ));
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Failed to create ProjectV2Service: {}", e);
                            project_v2_warning = Some(format!(
                                "Failed to initialize GitHub GraphQL client: {}",
                                e
                            ));
                        }
                    }
                } else {
                    project_v2_warning = Some(
                        "No GitHub token available. Project V2 not created.".to_string()
                    );
                }
            }

            // Emit event
            state.event_bus.project_created(&project.id, &project.name);

            // Build response
            let mut response = serde_json::json!({
                "success": true,
                "project_id": project.id,
                "name": project.name,
                "slug": project.slug,
                "has_github": has_github,
            });

            if let Some(url) = github_project_url {
                response["github_project_url"] = serde_json::json!(url);
            }
            if let Some(warning) = project_v2_warning {
                response["warning"] = serde_json::json!(warning);
            }

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
    github_owner: Option<String>,
    github_repo: Option<String>,
    has_github: bool,
    has_token: bool,
    /// URL to the linked GitHub Project V2 board (if any)
    github_project_v2_url: Option<String>,
    archived: bool,
    created_at: String,
    updated_at: String,
}

/// Issue response with properly parsed labels and assignees arrays.
#[derive(Debug, Serialize)]
struct IssueResponse {
    id: String,
    project_id: String,
    number: u32,
    title: String,
    body: Option<String>,
    state: String,
    labels: Vec<String>,
    assignees: Vec<String>,
    priority: Option<String>,
    github_number: Option<u32>,
    github_url: Option<String>,
    source: String,
    created_at: String,
    updated_at: String,
}

impl From<&IssueRecord> for IssueResponse {
    fn from(issue: &IssueRecord) -> Self {
        let priority = issue.priority.map(|p| match p {
            1 => "critical".to_string(),
            2 => "high".to_string(),
            3 => "medium".to_string(),
            _ => "low".to_string(),
        });

        IssueResponse {
            id: issue.id.clone(),
            project_id: issue.project_id.clone(),
            number: issue.number as u32,
            title: issue.title.clone(),
            body: issue.body.clone(),
            state: issue.state.clone(),
            labels: issue.labels_vec(),
            assignees: issue.assignees_vec(),
            priority,
            github_number: issue.github_number.map(|n| n as u32),
            github_url: issue.github_url.clone(),
            source: issue.source.clone(),
            created_at: issue.created_at.clone(),
            updated_at: issue.updated_at.clone(),
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
            // Check if token is configured for this project
            let has_token = match state.token_service.get_project_token_type(&project.id).await {
                Ok(ProjectTokenType::NotConfigured) => false,
                Ok(_) => true,
                Err(_) => false,
            };

            let has_github = project.has_github();
            let github_project_v2_url = project.github_project_v2_url().map(String::from);
            let response = ProjectResponse {
                id: project.id.clone(),
                name: project.name.clone(),
                slug: project.slug.clone(),
                description: project.description.clone(),
                color: project.color.clone(),
                github_owner: project.github_owner().map(String::from),
                github_repo: project.github_repo().map(String::from),
                has_github,
                has_token,
                github_project_v2_url,
                archived: project.is_archived(),
                created_at: project.created_at.clone(),
                updated_at: project.updated_at.clone(),
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
// ISSUE REST ENDPOINTS
// =============================================================================

#[derive(Debug, Deserialize)]
struct ListIssuesQuery {
    state: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
}

async fn list_issues(
    State(state): State<ProjectState>,
    Path(project_id): Path<String>,
    Query(query): Query<ListIssuesQuery>,
) -> impl IntoResponse {
    let store = state.store.read().await;

    // Verify project exists
    match store.get_project(&project_id).await {
        Ok(Some(project)) => {
            let state_filter = query.state.as_deref();
            let limit = query.limit.unwrap_or(50);
            let offset = query.offset.unwrap_or(0);
            match store.list_issues(&project.id, state_filter, limit, offset).await {
                Ok(issues) => {
                    let total = issues.len();
                    let has_more = issues.len() >= limit;

                    // Convert to IssueResponse to properly parse labels/assignees
                    let issue_responses: Vec<IssueResponse> = issues.iter()
                        .map(IssueResponse::from)
                        .collect();

                    Json(serde_json::json!({
                        "issues": issue_responses,
                        "total": total,
                        "has_more": has_more,
                    }))
                    .into_response()
                }
                Err(e) => {
                    warn!("Failed to list issues: {}", e);
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
struct CreateIssueRequest {
    title: String,
    body: Option<String>,
    labels: Option<Vec<String>>,
    assignees: Option<Vec<String>>,
}

async fn create_issue(
    State(state): State<ProjectState>,
    Path(project_id): Path<String>,
    Json(req): Json<CreateIssueRequest>,
) -> impl IntoResponse {
    let store = state.store.write().await;

    // Verify project exists
    match store.get_project(&project_id).await {
        Ok(Some(project)) => {
            let github_owner = project.github_owner().unwrap_or_default();
            let github_repo = project.github_repo().unwrap_or_default();
            let has_github_config = !github_owner.is_empty() && !github_repo.is_empty();

            // Check if token is configured
            let has_token = match state.token_service.get_project_token_type(&project_id).await {
                Ok(ProjectTokenType::NotConfigured) => false,
                Ok(_) => true,
                Err(_) => false,
            };

            // If GitHub is configured, create there FIRST - fail if it fails
            if has_github_config && has_token {
                // Get the token and create GitHub client
                let token = match state.token_service.get_token_for_project(&project_id).await {
                    Ok(t) => t,
                    Err(e) => {
                        return (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            format!("Failed to get GitHub token: {}", e),
                        ).into_response();
                    }
                };

                let client = match GitHubRestClient::new(&token) {
                    Ok(c) => c,
                    Err(e) => {
                        return (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            format!("Failed to create GitHub client: {}", e),
                        ).into_response();
                    }
                };

                // Build GitHub create request
                let github_req = kix_projects::github::models::CreateIssueRequest {
                    title: req.title.clone(),
                    body: req.body.clone(),
                    labels: req.labels.clone().unwrap_or_default(),
                    assignees: req.assignees.clone().unwrap_or_default(),
                };

                // Create on GitHub FIRST
                let gh_issue = match client.create_issue(&github_owner, &github_repo, &github_req).await {
                    Ok(issue) => {
                        info!("Created GitHub issue #{} for project {}", issue.number, project_id);
                        issue
                    }
                    Err(e) => {
                        warn!("Failed to create issue on GitHub: {}", e);
                        return (
                            StatusCode::BAD_GATEWAY,
                            format!("Failed to create issue on GitHub: {}", e),
                        ).into_response();
                    }
                };

                // GitHub succeeded - now create locally with GitHub's number
                let mut issue = IssueRecord::new(project.id.clone(), gh_issue.number, req.title.clone());
                if let Some(body) = req.body {
                    issue = issue.with_body(body);
                }
                if let Some(labels) = req.labels {
                    issue = issue.with_labels(labels);
                }
                if let Some(assignees) = req.assignees {
                    issue.set_assignees(assignees);
                }
                issue.github_number = Some(gh_issue.number as i64);
                issue.github_node_id = Some(gh_issue.node_id.clone());
                issue.github_url = Some(gh_issue.html_url.clone());
                issue.source = "github".to_string();

                let issue_id = issue.id.clone();
                let issue_title = issue.title.clone();
                let project_id_clone = project.id.clone();
                let number = gh_issue.number;

                match store.create_issue(&issue).await {
                    Ok(_) => {
                        drop(store);
                        state.event_bus.issue_created(&project_id_clone, &issue_id, &issue_title);
                        Json(serde_json::json!({
                            "success": true,
                            "issue_id": issue_id,
                            "number": number,
                            "github_number": number,
                            "github_url": issue.github_url,
                            "title": issue_title,
                        }))
                        .into_response()
                    }
                    Err(e) => {
                        warn!("Failed to save issue locally (GitHub issue #{} was created): {}", number, e);
                        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
                    }
                }
            } else {
                // No GitHub config - create locally only
                let number = match store.next_issue_number(&project.id).await {
                    Ok(n) => n,
                    Err(e) => {
                        warn!("Failed to get next issue number: {}", e);
                        return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
                    }
                };

                let mut issue = IssueRecord::new(project.id.clone(), number, req.title.clone());
                if let Some(body) = req.body {
                    issue = issue.with_body(body);
                }
                if let Some(labels) = req.labels {
                    issue = issue.with_labels(labels);
                }
                if let Some(assignees) = req.assignees {
                    issue.set_assignees(assignees);
                }

                let issue_id = issue.id.clone();
                let issue_title = issue.title.clone();
                let project_id_clone = project.id.clone();

                match store.create_issue(&issue).await {
                    Ok(_) => {
                        drop(store);
                        state.event_bus.issue_created(&project_id_clone, &issue_id, &issue_title);
                        Json(serde_json::json!({
                            "success": true,
                            "issue_id": issue_id,
                            "number": number,
                            "title": issue_title,
                        }))
                        .into_response()
                    }
                    Err(e) => {
                        warn!("Failed to create issue: {}", e);
                        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
                    }
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

async fn get_issue(
    State(state): State<ProjectState>,
    Path((project_id, issue_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let store = state.store.read().await;

    // Try to parse as issue number
    let issue = if let Ok(num) = issue_id.parse::<u32>() {
        store.get_issue_by_number(&project_id, num).await
    } else {
        store.get_issue(&issue_id).await
    };

    match issue {
        Ok(Some(issue)) => {
            // Convert to IssueResponse to properly parse labels/assignees
            let response = IssueResponse::from(&issue);
            Json(response).into_response()
        }
        Ok(None) => (StatusCode::NOT_FOUND, "Issue not found").into_response(),
        Err(e) => {
            warn!("Failed to get issue: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
struct UpdateIssueRequest {
    title: Option<String>,
    body: Option<String>,
    state: Option<String>,
    labels: Option<Vec<String>>,
    assignees: Option<Vec<String>>,
}

async fn update_issue(
    State(state): State<ProjectState>,
    Path((project_id, issue_id)): Path<(String, String)>,
    Json(req): Json<UpdateIssueRequest>,
) -> impl IntoResponse {
    let store = state.store.write().await;

    // Get project to check GitHub config
    let project = match store.get_project(&project_id).await {
        Ok(Some(p)) => p,
        Ok(None) => return (StatusCode::NOT_FOUND, "Project not found").into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    // Get issue
    let issue_result = if let Ok(num) = issue_id.parse::<u32>() {
        store.get_issue_by_number(&project_id, num).await
    } else {
        store.get_issue(&issue_id).await
    };

    match issue_result {
        Ok(Some(mut issue)) => {
            let was_open = issue.state != "closed";
            let github_owner = project.github_owner().unwrap_or_default();
            let github_repo = project.github_repo().unwrap_or_default();
            let has_github_config = !github_owner.is_empty() && !github_repo.is_empty();

            // Check if issue has a GitHub number (was synced to GitHub)
            let github_number = issue.github_number.map(|n| n as u32);

            // If GitHub is configured and issue exists on GitHub, update there FIRST
            if has_github_config && github_number.is_some() {
                let gh_num = github_number.unwrap();

                // Check if token is configured
                let has_token = match state.token_service.get_project_token_type(&project_id).await {
                    Ok(ProjectTokenType::NotConfigured) => false,
                    Ok(_) => true,
                    Err(_) => false,
                };

                if has_token {
                    let token = match state.token_service.get_token_for_project(&project_id).await {
                        Ok(t) => t,
                        Err(e) => {
                            return (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                format!("Failed to get GitHub token: {}", e),
                            ).into_response();
                        }
                    };

                    let client = match GitHubRestClient::new(&token) {
                        Ok(c) => c,
                        Err(e) => {
                            return (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                format!("Failed to create GitHub client: {}", e),
                            ).into_response();
                        }
                    };

                    // Build GitHub update request
                    let github_req = kix_projects::github::models::UpdateIssueRequest {
                        title: req.title.clone(),
                        body: req.body.clone(),
                        state: req.state.clone(),
                        labels: req.labels.clone(),
                        assignees: req.assignees.clone(),
                    };

                    // Update on GitHub FIRST
                    match client.update_issue(&github_owner, &github_repo, gh_num, &github_req).await {
                        Ok(gh_issue) => {
                            info!("Updated GitHub issue #{} for project {}", gh_num, project_id);
                            // Update local issue with data from GitHub response
                            issue.title = gh_issue.title;
                            issue.body = gh_issue.body;
                            issue.state = gh_issue.state.clone();
                            issue.set_labels(gh_issue.labels.iter().map(|l| l.name.clone()).collect());
                            issue.set_assignees(gh_issue.assignees.iter().map(|u| u.login.clone()).collect());
                        }
                        Err(e) => {
                            warn!("Failed to update issue on GitHub: {}", e);
                            return (
                                StatusCode::BAD_GATEWAY,
                                format!("Failed to update issue on GitHub: {}", e),
                            ).into_response();
                        }
                    }
                } else {
                    // No token but has GitHub config - this shouldn't happen for synced issues
                    warn!("Issue has GitHub number but no token configured");
                }
            } else {
                // No GitHub config or issue not on GitHub - update locally only
                if let Some(title) = req.title {
                    issue.title = title;
                }
                if let Some(body) = req.body {
                    issue.body = Some(body);
                }
                if let Some(state_str) = req.state {
                    issue.state = state_str;
                }
                if let Some(labels) = req.labels {
                    issue.set_labels(labels);
                }
                if let Some(assignees) = req.assignees {
                    issue.set_assignees(assignees);
                }
            }

            issue.updated_at = chrono::Utc::now().to_rfc3339();
            let is_now_closed = issue.state == "closed";
            let issue_id_str = issue.id.clone();

            match store.update_issue(&issue).await {
                Ok(_) => {
                    drop(store);
                    // Emit appropriate event
                    if was_open && is_now_closed {
                        state.event_bus.issue_closed(&project_id, &issue_id_str);
                    } else if !was_open && !is_now_closed {
                        state.event_bus.issue_reopened(&project_id, &issue_id_str);
                    } else {
                        state.event_bus.issue_updated(&project_id, &issue_id_str);
                    }
                    Json(serde_json::json!({
                        "success": true,
                        "issue_id": issue_id_str,
                    }))
                    .into_response()
                }
                Err(e) => {
                    warn!("Failed to update issue: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
                }
            }
        }
        Ok(None) => (StatusCode::NOT_FOUND, "Issue not found").into_response(),
        Err(e) => {
            warn!("Failed to get issue: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

async fn delete_issue(
    State(state): State<ProjectState>,
    Path((project_id, issue_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let store = state.store.write().await;

    // Get project to check GitHub config
    let project = match store.get_project(&project_id).await {
        Ok(Some(p)) => p,
        Ok(None) => return (StatusCode::NOT_FOUND, "Project not found").into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    // Get issue
    let issue_result = if let Ok(num) = issue_id.parse::<u32>() {
        store.get_issue_by_number(&project_id, num).await
    } else {
        store.get_issue(&issue_id).await
    };

    match issue_result {
        Ok(Some(issue)) => {
            let github_owner = project.github_owner().unwrap_or_default();
            let github_repo = project.github_repo().unwrap_or_default();
            let has_github_config = !github_owner.is_empty() && !github_repo.is_empty();

            // Check if issue has a GitHub number (was synced to GitHub)
            let github_number = issue.github_number.map(|n| n as u32);

            // If GitHub is configured and issue exists on GitHub, close there FIRST
            // Note: GitHub API doesn't allow deleting issues, only closing them
            if has_github_config && github_number.is_some() {
                let gh_num = github_number.unwrap();

                // Check if token is configured
                let has_token = match state.token_service.get_project_token_type(&project_id).await {
                    Ok(ProjectTokenType::NotConfigured) => false,
                    Ok(_) => true,
                    Err(_) => false,
                };

                if has_token {
                    let token = match state.token_service.get_token_for_project(&project_id).await {
                        Ok(t) => t,
                        Err(e) => {
                            return (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                format!("Failed to get GitHub token: {}", e),
                            ).into_response();
                        }
                    };

                    let client = match GitHubRestClient::new(&token) {
                        Ok(c) => c,
                        Err(e) => {
                            return (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                format!("Failed to create GitHub client: {}", e),
                            ).into_response();
                        }
                    };

                    // Close on GitHub FIRST (GitHub doesn't allow deleting issues)
                    match client.close_issue(&github_owner, &github_repo, gh_num).await {
                        Ok(_) => {
                            info!("Closed GitHub issue #{} for project {}", gh_num, project_id);
                        }
                        Err(e) => {
                            warn!("Failed to close issue on GitHub: {}", e);
                            return (
                                StatusCode::BAD_GATEWAY,
                                format!("Failed to close issue on GitHub: {}", e),
                            ).into_response();
                        }
                    }
                } else {
                    warn!("Issue has GitHub number but no token configured");
                }
            }

            // GitHub succeeded (or no GitHub) - now delete locally
            let issue_id_str = issue.id.clone();
            match store.delete_issue(&issue_id_str).await {
                Ok(_) => {
                    drop(store);
                    state.event_bus.issue_deleted(&project_id, &issue_id_str);
                    Json(serde_json::json!({
                        "success": true,
                        "github_closed": github_number.is_some(),
                    }))
                    .into_response()
                }
                Err(e) => {
                    warn!("Failed to delete issue: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
                }
            }
        }
        Ok(None) => (StatusCode::NOT_FOUND, "Issue not found").into_response(),
        Err(e) => {
            warn!("Failed to get issue: {}", e);
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
// GITHUB INTEGRATION ENDPOINTS
// =============================================================================

#[derive(Debug, Deserialize)]
struct SetGitHubTokenRequest {
    token: String,
}

async fn set_github_token(
    State(state): State<ProjectState>,
    Path(project_id): Path<String>,
    Json(req): Json<SetGitHubTokenRequest>,
) -> impl IntoResponse {
    // Verify project exists
    let store = state.store.read().await;
    match store.get_project(&project_id).await {
        Ok(Some(project)) => {
            drop(store);

            // Get owner/repo for verification
            let owner = project.github_owner().unwrap_or_default();
            let repo = project.github_repo().unwrap_or_default();

            // Set project token with optional verification
            let verify_repo = if !owner.is_empty() && !repo.is_empty() {
                Some((owner.as_str(), repo.as_str()))
            } else {
                None
            };

            match state
                .token_service
                .set_project_token(&project_id, &req.token, verify_repo)
                .await
            {
                Ok(result) => {
                    debug!("Stored GitHub token for project {}", project_id);
                    Json(serde_json::json!({
                        "success": true,
                        "message": "GitHub token saved successfully",
                        "verified_user": result.verified_user,
                    }))
                    .into_response()
                }
                Err(e) => {
                    warn!("Failed to store token: {}", e);
                    (StatusCode::BAD_REQUEST, e.to_string()).into_response()
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

async fn sync_github(
    State(state): State<ProjectState>,
    Path(project_id): Path<String>,
) -> impl IntoResponse {
    // Verify project exists and has token
    let store = state.store.read().await;
    match store.get_project(&project_id).await {
        Ok(Some(_project)) => {
            drop(store);

            // Check if token is configured
            let has_token = match state.token_service.get_project_token_type(&project_id).await {
                Ok(ProjectTokenType::NotConfigured) => false,
                Ok(_) => true,
                Err(_) => false,
            };

            if !has_token {
                return (
                    StatusCode::BAD_REQUEST,
                    "GitHub token not configured for this project. Set a token first.",
                )
                    .into_response();
            }

            // Emit sync started event
            state.event_bus.github_sync_started(&project_id);

            // Get the decrypted token and create sync service
            let token = match state.token_service.get_token_for_project(&project_id).await {
                Ok(t) => t,
                Err(e) => {
                    state.event_bus.github_sync_failed(&project_id, &e.to_string());
                    return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to get token: {}", e))
                        .into_response();
                }
            };

            let sync_service = match GitHubSyncService::new(&token) {
                Ok(s) => s,
                Err(e) => {
                    state.event_bus.github_sync_failed(&project_id, &e.to_string());
                    return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to create sync service: {}", e))
                        .into_response();
                }
            };

            // Get project details for GitHub owner/repo
            let store = state.store.read().await;
            let project = match store.get_project(&project_id).await {
                Ok(Some(p)) => p,
                Ok(None) => {
                    state.event_bus.github_sync_failed(&project_id, "Project not found");
                    return (StatusCode::NOT_FOUND, "Project not found").into_response();
                }
                Err(e) => {
                    state.event_bus.github_sync_failed(&project_id, &e.to_string());
                    return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
                }
            };

            let github_owner = project.github_owner().unwrap_or_default();
            let github_repo = project.github_repo().unwrap_or_default();

            // Get local issues for bidirectional sync
            let local_issues_records = match store.list_issues(&project_id, None, 1000, 0).await {
                Ok(issues) => issues,
                Err(e) => {
                    drop(store);
                    state.event_bus.github_sync_failed(&project_id, &e.to_string());
                    return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to get local issues: {}", e))
                        .into_response();
                }
            };
            drop(store);

            // Configure sync for BIDIRECTIONAL (always bidirectional now)
            let config = SyncConfig {
                direction: SyncDirection::Bidirectional,
                include_closed: true,
                labels_filter: vec![],
                max_issues: 500, // Increased limit for full sync
            };

            // Pull issues from GitHub
            let github_issues = match sync_service.pull_issues(&github_owner, &github_repo, &config).await {
                Ok(issues) => issues,
                Err(e) => {
                    state.event_bus.github_sync_failed(&project_id, &e.to_string());
                    return Json(serde_json::json!({
                        "success": false,
                        "message": format!("Failed to pull issues from GitHub: {}", e),
                        "pulled": 0,
                        "pushed": 0,
                        "merged": 0,
                        "conflicts_resolved": 0,
                        "errors": [e.to_string()],
                    }))
                    .into_response();
                }
            };

            // Build maps for efficient lookup
            let mut local_by_github_num: std::collections::HashMap<u32, IssueRecord> = std::collections::HashMap::new();
            for issue in &local_issues_records {
                if let Some(gh_num) = issue.github_number {
                    local_by_github_num.insert(gh_num as u32, issue.clone());
                }
            }

            let store = state.store.write().await;
            let mut pulled = 0u32;
            let mut pushed = 0u32;
            let mut merged = 0u32;
            let mut conflicts_resolved = 0u32;
            let mut errors: Vec<String> = vec![];
            let mut change_details: Vec<serde_json::Value> = vec![];

            // Process GitHub issues (pull direction)
            for gh_issue in &github_issues {
                match local_by_github_num.get(&gh_issue.number) {
                    Some(existing) => {
                        // Issue exists locally - check for changes
                        let gh_title = &gh_issue.title;
                        let gh_body = gh_issue.body.as_deref().unwrap_or("");
                        let gh_state = &gh_issue.state;
                        let gh_labels: Vec<String> = gh_issue.labels.iter().map(|l| l.name.clone()).collect();
                        let gh_assignees: Vec<String> = gh_issue.assignees.iter().map(|a| a.login.clone()).collect();

                        let local_body = existing.body.as_deref().unwrap_or("");
                        let local_labels = existing.labels_vec();
                        let local_assignees = existing.assignees_vec();

                        // Check if GitHub has changes we need to pull
                        let gh_changed = existing.title != *gh_title
                            || local_body != gh_body
                            || existing.state != *gh_state
                            || local_labels != gh_labels
                            || local_assignees != gh_assignees;

                        if gh_changed {
                            // Smart merge: combine changes
                            let mut issue = existing.clone();

                            // Merge labels (union)
                            let mut merged_labels: std::collections::BTreeSet<String> = local_labels.into_iter().collect();
                            for label in gh_labels {
                                merged_labels.insert(label);
                            }
                            issue.set_labels(merged_labels.into_iter().collect());

                            // Merge assignees (union)
                            let mut merged_assignees: std::collections::BTreeSet<String> = local_assignees.into_iter().collect();
                            for assignee in gh_assignees {
                                merged_assignees.insert(assignee);
                            }
                            issue.set_assignees(merged_assignees.into_iter().collect());

                            // State: closed wins
                            if gh_state == "closed" || existing.state == "closed" {
                                issue.state = "closed".to_string();
                            }

                            // For title and body: prefer local (user preference)
                            // But if local hasn't changed from last sync, take GitHub's version
                            // For now, keeping local as per user requirement

                            issue.updated_at = chrono::Utc::now().to_rfc3339();
                            issue.synced_at = Some(chrono::Utc::now().to_rfc3339());

                            if store.update_issue(&issue).await.is_ok() {
                                merged += 1;
                                conflicts_resolved += 1;
                                change_details.push(serde_json::json!({
                                    "issue_number": gh_issue.number,
                                    "title": issue.title,
                                    "action": "merged",
                                    "fields_affected": ["labels", "assignees", "state"]
                                }));
                            }
                        }
                    }
                    None => {
                        // New issue from GitHub - create locally
                        let issue_number = match store.next_issue_number(&project_id).await {
                            Ok(n) => n,
                            Err(e) => {
                                errors.push(format!("Failed to get next issue number: {}", e));
                                continue;
                            }
                        };

                        let mut issue = IssueRecord::new(project_id.clone(), issue_number, gh_issue.title.clone());
                        if let Some(body) = &gh_issue.body {
                            issue = issue.with_body(body.clone());
                        }
                        issue.state = if gh_issue.state == "open" { "open".to_string() } else { "closed".to_string() };
                        issue = issue.with_labels(gh_issue.labels.iter().map(|l| l.name.clone()).collect());
                        issue.set_assignees(gh_issue.assignees.iter().map(|a| a.login.clone()).collect());
                        issue.github_number = Some(gh_issue.number as i64);
                        issue.github_node_id = Some(gh_issue.node_id.clone());
                        issue.github_url = Some(gh_issue.html_url.clone());
                        issue.source = "github".to_string();
                        issue.synced_at = Some(chrono::Utc::now().to_rfc3339());

                        // Add to GitHub Project V2 if configured
                        if let Some(project_v2_id) = project.github_project_v2_id() {
                            if let Ok(v2_service) = ProjectV2Service::new(&token) {
                                if let Ok(item_id) = v2_service.add_issue_to_project(&project_v2_id, &gh_issue.node_id).await {
                                    issue.github_project_item_id = Some(item_id);
                                }
                            }
                        }

                        if store.create_issue(&issue).await.is_ok() {
                            pulled += 1;
                            change_details.push(serde_json::json!({
                                "issue_number": gh_issue.number,
                                "title": issue.title,
                                "action": "pulled",
                                "fields_affected": []
                            }));
                        }
                    }
                }
            }

            // Build set of GitHub issue numbers for push check
            let github_issue_nums: std::collections::HashSet<u32> = github_issues.iter().map(|i| i.number).collect();

            // Process local-only issues (push direction)
            for local_issue in &local_issues_records {
                // Skip if already on GitHub
                if let Some(gh_num) = local_issue.github_number {
                    if github_issue_nums.contains(&(gh_num as u32)) {
                        continue;
                    }
                }

                // Skip if it came from GitHub (shouldn't push back)
                if local_issue.source == "github" {
                    continue;
                }

                // Push local issue to GitHub
                let create_req = kix_projects::github::models::CreateIssueRequest {
                    title: local_issue.title.clone(),
                    body: local_issue.body.clone(),
                    labels: local_issue.labels_vec(),
                    assignees: local_issue.assignees_vec(),
                };

                match sync_service.rest().create_issue(&github_owner, &github_repo, &create_req).await {
                    Ok(created_issue) => {
                        // Update local issue with GitHub info
                        let mut updated_issue = local_issue.clone();
                        updated_issue.github_number = Some(created_issue.number as i64);
                        updated_issue.github_node_id = Some(created_issue.node_id.clone());
                        updated_issue.github_url = Some(created_issue.html_url.clone());
                        updated_issue.synced_at = Some(chrono::Utc::now().to_rfc3339());

                        // Add to GitHub Project V2 if configured
                        if let Some(project_v2_id) = project.github_project_v2_id() {
                            if let Ok(v2_service) = ProjectV2Service::new(&token) {
                                if let Ok(item_id) = v2_service.add_issue_to_project(&project_v2_id, &created_issue.node_id).await {
                                    updated_issue.github_project_item_id = Some(item_id);
                                }
                            }
                        }

                        if store.update_issue(&updated_issue).await.is_ok() {
                            pushed += 1;
                            change_details.push(serde_json::json!({
                                "issue_number": created_issue.number,
                                "title": local_issue.title,
                                "action": "pushed",
                                "fields_affected": []
                            }));
                        }
                    }
                    Err(e) => {
                        errors.push(format!("Failed to push issue '{}': {}", local_issue.title, e));
                    }
                }
            }

            drop(store);

            // Emit completion event
            state.event_bus.github_sync_completed(&project_id, pulled as usize, (pushed + merged) as usize);

            Json(serde_json::json!({
                "success": true,
                "message": format!(
                    "Bidirectional sync completed: {} pulled, {} pushed, {} merged, {} conflicts auto-resolved",
                    pulled, pushed, merged, conflicts_resolved
                ),
                "pulled": pulled,
                "pushed": pushed,
                "merged": merged,
                "conflicts_resolved": conflicts_resolved,
                "change_details": change_details,
                "errors": errors,
            }))
            .into_response()
        }
        Ok(None) => (StatusCode::NOT_FOUND, "Project not found").into_response(),
        Err(e) => {
            warn!("Failed to get project: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
struct LinkGitHubProjectRequest {
    /// Template for the GitHub Project V2 board
    template: String,
}

/// POST /api/projects/:id/github/link-project - Create and link a GitHub Project V2 board.
async fn link_github_project(
    State(state): State<ProjectState>,
    Path(project_id): Path<String>,
    Json(req): Json<LinkGitHubProjectRequest>,
) -> impl IntoResponse {
    // Parse template
    let template = match req.template.as_str() {
        "kanban" => ProjectTemplate::Kanban,
        "bug_tracking" => ProjectTemplate::BugTracking,
        "sprint_planning" => ProjectTemplate::SprintPlanning,
        "feature_roadmap" => ProjectTemplate::FeatureRoadmap,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                format!("Invalid template '{}'. Choose: kanban, bug_tracking, sprint_planning, feature_roadmap", req.template)
            ).into_response();
        }
    };

    // Get project
    let store = state.store.read().await;
    let project = match store.get_project(&project_id).await {
        Ok(Some(p)) => p,
        Ok(None) => return (StatusCode::NOT_FOUND, "Project not found").into_response(),
        Err(e) => {
            warn!("Failed to get project: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
        }
    };
    drop(store);

    // Verify this is a GitHub project
    if !project.has_github() {
        return (StatusCode::BAD_REQUEST, "This is not a GitHub project").into_response();
    }

    // Check if already linked
    if project.github_project_v2_id().is_some() {
        return (StatusCode::CONFLICT, "GitHub Project V2 already linked").into_response();
    }

    let owner = project.github_owner().unwrap();
    let repo = project.github_repo().unwrap();

    // Get token (project-specific or falls back to global)
    let token = match state.token_service.get_token_for_project(&project_id).await {
        Ok(t) => t,
        Err(e) => {
            warn!("Failed to get token for project {}: {}", project_id, e);
            return (StatusCode::BAD_REQUEST, format!("No GitHub token available: {}", e)).into_response();
        }
    };

    // Create GitHub Project V2
    let v2_service = match ProjectV2Service::new(&token) {
        Ok(s) => s,
        Err(e) => {
            warn!("Failed to create ProjectV2Service: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to initialize GitHub GraphQL client: {}", e)).into_response();
        }
    };

    info!("Creating GitHub Project V2 '{}' with {} template for {}/{}", project.name, template, owner, repo);

    match v2_service.create_project_with_template(&owner, &repo, &project.name, template).await {
        Ok(v2_config) => {
            info!("GitHub Project V2 created: {} at {}", v2_config.project_number, v2_config.url);

            // Update project record with Project V2 config
            let v2_json = serde_json::json!({
                "project_id": v2_config.project_id,
                "project_number": v2_config.project_number,
                "url": v2_config.url,
                "status_field_id": v2_config.status_field_id,
                "status_options": v2_config.status_options.iter().map(|o| {
                    serde_json::json!({
                        "name": o.name,
                        "option_id": o.option_id
                    })
                }).collect::<Vec<_>>(),
                "custom_fields": v2_config.custom_fields.iter().map(|f| {
                    serde_json::json!({
                        "name": f.name,
                        "field_id": f.field_id,
                        "field_type": format!("{:?}", f.field_type),
                        "options": f.options.iter().map(|o| {
                            serde_json::json!({
                                "name": o.name,
                                "option_id": o.option_id
                            })
                        }).collect::<Vec<_>>()
                    })
                }).collect::<Vec<_>>()
            });

            let updated_project = project.clone().with_github_project_v2(v2_json);

            let store = state.store.write().await;
            if let Err(e) = store.update_project(&updated_project).await {
                warn!("Failed to save Project V2 config: {}", e);
                // Still return success since Project V2 was created
                return Json(serde_json::json!({
                    "success": true,
                    "github_project_v2_url": v2_config.url,
                    "warning": "Project V2 created but config not saved to database"
                })).into_response();
            }
            drop(store);

            // Emit event
            state.event_bus.project_updated(&project_id);

            Json(serde_json::json!({
                "success": true,
                "github_project_v2_url": v2_config.url
            })).into_response()
        }
        Err(e) => {
            warn!("Failed to create GitHub Project V2: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to create GitHub Project V2: {}", e)).into_response()
        }
    }
}

// =============================================================================
// GLOBAL GITHUB API ENDPOINTS
// =============================================================================

/// GET /api/github/token/status - Check if global token exists.
async fn github_token_status(State(state): State<ProjectState>) -> impl IntoResponse {
    match state.token_service.has_global_token().await {
        Ok(has_token) => {
            // Get token info if exists
            let token_info = if has_token {
                state
                    .token_service
                    .get_global_token_info()
                    .await
                    .ok()
                    .flatten()
            } else {
                None
            };

            Json(serde_json::json!({
                "has_global_token": has_token,
                "token_suffix": token_info.as_ref().and_then(|t| t.token_suffix.clone()),
                "verified_user": token_info.as_ref().and_then(|t| t.verified_user.clone()),
            }))
            .into_response()
        }
        Err(e) => {
            warn!("Failed to check global token status: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
struct SetGlobalTokenRequest {
    token: String,
}

/// POST /api/github/token/global - Set global token.
async fn set_global_github_token(
    State(state): State<ProjectState>,
    Json(req): Json<SetGlobalTokenRequest>,
) -> impl IntoResponse {
    match state.token_service.set_global_token(&req.token, true).await {
        Ok(result) => {
            info!("Global GitHub token set successfully");
            Json(serde_json::json!({
                "success": true,
                "username": result.verified_user,
            }))
            .into_response()
        }
        Err(e) => {
            warn!("Failed to set global token: {}", e);
            (StatusCode::BAD_REQUEST, e.to_string()).into_response()
        }
    }
}

/// GET /api/github/user - Get authenticated user info.
async fn github_user(State(state): State<ProjectState>) -> impl IntoResponse {
    // Get the global token
    let token = match state.token_service.get_global_token_decrypted().await {
        Ok(token) => token,
        Err(e) => {
            return (StatusCode::UNAUTHORIZED, e.to_string()).into_response();
        }
    };

    // Create client and get user info
    let client = match GitHubRestClient::new(&token) {
        Ok(client) => client,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
        }
    };

    match client.get_authenticated_user().await {
        Ok(user) => Json(user).into_response(),
        Err(e) => {
            warn!("Failed to get GitHub user: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

/// Response type for /api/github/orgs
#[derive(Debug, Serialize)]
struct GitHubOrgsResponse {
    orgs: Vec<OrgItem>,
}

#[derive(Debug, Serialize)]
struct OrgItem {
    login: String,
    avatar_url: String,
    #[serde(rename = "type")]
    org_type: String,
    description: Option<String>,
}

/// GET /api/github/orgs - List user's organizations (includes user as first item).
/// Accepts optional Authorization header with Bearer token.
async fn github_orgs(
    State(state): State<ProjectState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    // Try to get token from Authorization header first, then fall back to global
    let token = if let Some(auth_header) = headers.get("authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if auth_str.starts_with("Bearer ") {
                auth_str.trim_start_matches("Bearer ").to_string()
            } else {
                return (StatusCode::BAD_REQUEST, "Invalid Authorization header format").into_response();
            }
        } else {
            return (StatusCode::BAD_REQUEST, "Invalid Authorization header encoding").into_response();
        }
    } else {
        // Fall back to global token
        match state.token_service.get_global_token_decrypted().await {
            Ok(token) => token,
            Err(e) => {
                return (StatusCode::UNAUTHORIZED, e.to_string()).into_response();
            }
        }
    };

    // Create client
    let client = match GitHubRestClient::new(&token) {
        Ok(client) => client,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
        }
    };

    // Get user info first (to include as first item)
    let user = match client.get_authenticated_user().await {
        Ok(user) => user,
        Err(e) => {
            warn!("Failed to get authenticated user: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
        }
    };

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
            warn!("Failed to list organizations: {}", e);
            // Continue with just the user
        }
    }

    Json(GitHubOrgsResponse { orgs }).into_response()
}

#[derive(Debug, Deserialize)]
struct ListReposQuery {
    owner: String,
    search: Option<String>,
    page: Option<u32>,
    per_page: Option<u32>,
}

#[derive(Debug, Serialize)]
struct GitHubReposResponse {
    repos: Vec<RepoItem>,
    has_more: bool,
}

#[derive(Debug, Serialize)]
struct RepoItem {
    name: String,
    full_name: String,
    description: Option<String>,
    private: bool,
}

/// GET /api/github/repos - List/search repositories for owner.
/// Accepts optional Authorization header with Bearer token.
async fn github_repos(
    State(state): State<ProjectState>,
    headers: HeaderMap,
    Query(query): Query<ListReposQuery>,
) -> impl IntoResponse {
    // Try to get token from Authorization header first, then fall back to global
    let token = if let Some(auth_header) = headers.get("authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if auth_str.starts_with("Bearer ") {
                auth_str.trim_start_matches("Bearer ").to_string()
            } else {
                return (StatusCode::BAD_REQUEST, "Invalid Authorization header format").into_response();
            }
        } else {
            return (StatusCode::BAD_REQUEST, "Invalid Authorization header encoding").into_response();
        }
    } else {
        // Fall back to global token
        match state.token_service.get_global_token_decrypted().await {
            Ok(token) => token,
            Err(e) => {
                return (StatusCode::UNAUTHORIZED, e.to_string()).into_response();
            }
        }
    };

    // Create client
    let client = match GitHubRestClient::new(&token) {
        Ok(client) => client,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
        }
    };

    let per_page = query.per_page.unwrap_or(50);
    let page = query.page.unwrap_or(1);

    // Use search if query provided
    match client
        .search_repos(&query.owner, query.search.as_deref(), Some(per_page), Some(page))
        .await
    {
        Ok(result) => {
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

            Json(GitHubReposResponse { repos, has_more }).into_response()
        }
        Err(e) => {
            warn!("Failed to search repos: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
struct VerifyAccessRequest {
    owner: String,
    repo: String,
    token: Option<String>,
}

#[derive(Debug, Serialize)]
struct VerifyAccessResponse {
    has_access: bool,
    permissions: Option<RepoPermissions>,
}

#[derive(Debug, Serialize)]
struct RepoPermissions {
    admin: bool,
    push: bool,
    pull: bool,
}

/// POST /api/github/verify-access - Verify token access to owner/repo.
async fn github_verify_access(
    State(state): State<ProjectState>,
    Json(req): Json<VerifyAccessRequest>,
) -> impl IntoResponse {
    // Use provided token or fall back to global token
    let token = if let Some(ref provided_token) = req.token {
        provided_token.clone()
    } else {
        match state.token_service.get_global_token_decrypted().await {
            Ok(token) => token,
            Err(e) => {
                return (StatusCode::UNAUTHORIZED, e.to_string()).into_response();
            }
        }
    };

    // Create client
    let client = match GitHubRestClient::new(&token) {
        Ok(client) => client,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
        }
    };

    // Verify access
    match client.verify_access(&req.owner, &req.repo).await {
        Ok(has_access) => Json(VerifyAccessResponse {
            has_access,
            permissions: if has_access {
                Some(RepoPermissions {
                    admin: false, // Would need separate API call
                    push: true,
                    pull: true,
                })
            } else {
                None
            },
        })
        .into_response(),
        Err(e) => {
            warn!("Failed to verify access: {}", e);
            Json(VerifyAccessResponse {
                has_access: false,
                permissions: None,
            })
            .into_response()
        }
    }
}

/// GET /api/projects/:id/github/token-status - Get project's token type.
async fn project_token_status(
    State(state): State<ProjectState>,
    Path(project_id): Path<String>,
) -> impl IntoResponse {
    // Verify project exists
    let store = state.store.read().await;
    match store.get_project(&project_id).await {
        Ok(Some(_)) => {
            drop(store);

            match state.token_service.get_project_token_type(&project_id).await {
                Ok(token_type) => Json(token_type).into_response(),
                Err(e) => {
                    warn!("Failed to get project token type: {}", e);
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
