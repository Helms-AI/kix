//! Indexing API routes for URL crawling and file upload with SSE progress updates.

use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    extract::{ConnectInfo, Path, Query, State},
    http::StatusCode,
    response::{
        sse::{Event, Sse},
        IntoResponse,
    },
    routing::{delete, get, post},
    Json, Router,
};
use tower_http::cors::{Any, CorsLayer};
use futures_util::stream::Stream;
use base64::Engine;
use serde::{Deserialize, Serialize};
use tracing::{error, info};
use uuid::Uuid;

use kix_crawler::file_handler::{FileHandler, UploadedFile};
use kix_jobs::{
    job::{Job, JobConfig, JobState, JobType},
    queue::JobQueue,
};
use kix_sse::manager::ConnectionManager;
use kix_store::JobStore;

use crate::routes::AppState;

/// Extended application state with indexing components
#[derive(Clone)]
pub struct IndexingState {
    /// Base application state
    pub app_state: AppState,
    /// Job queue for indexing operations
    pub job_queue: Arc<JobQueue>,
    /// SSE connection manager
    pub sse_manager: Arc<ConnectionManager>,
    /// File handler for uploads
    pub file_handler: Arc<FileHandler>,
    /// Job history store (optional - for persisted job history)
    pub job_store: Option<Arc<JobStore>>,
}

impl IndexingState {
    pub fn new(
        app_state: AppState,
        job_queue: Arc<JobQueue>,
        sse_manager: Arc<ConnectionManager>,
        file_handler: Arc<FileHandler>,
    ) -> Self {
        Self {
            app_state,
            job_queue,
            sse_manager,
            file_handler,
            job_store: None,
        }
    }

    /// Set the job store for history persistence
    pub fn with_job_store(mut self, job_store: Arc<JobStore>) -> Self {
        self.job_store = Some(job_store);
        self
    }
}

/// Create the indexing router
pub fn create_indexing_router(state: IndexingState) -> Router {
    // CORS configuration - allow all origins for development
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        // Job management endpoints
        .route("/api/indexing/jobs", get(list_jobs))
        .route("/api/indexing/jobs/:id", get(get_job))
        .route("/api/indexing/jobs/:id", delete(cancel_job))
        // Job history endpoints
        .route("/api/indexing/history", get(list_job_history))
        .route("/api/indexing/history/:job_id", get(get_job_detail))
        .route("/api/indexing/history/:job_id/items", get(get_job_items))
        // Data management endpoints
        .route("/api/indexing/data", delete(delete_all_data))
        // Start indexing endpoints
        .route("/api/indexing/url", post(start_url_indexing))
        .route("/api/indexing/files", post(start_file_indexing))
        // Re-index endpoints
        .route("/api/indexing/reindex/:id", post(reindex_entry))
        .route("/api/indexing/reindex-domain", post(reindex_by_domain))
        // SSE endpoints
        .route("/api/indexing/sse/:job_id", get(sse_job_stream))
        .route("/api/indexing/sse", get(sse_global_stream))
        .layer(cors)
        .with_state(state)
}

// === Request/Response Types ===

/// Request to start URL indexing
#[derive(Debug, Deserialize)]
pub struct StartUrlIndexingRequest {
    /// URL to start crawling from
    pub url: String,
    /// Depth levels to crawl (0 = just the URL, 1 = follow links one level deep, etc.)
    #[serde(default = "default_depth")]
    pub levels: usize,
    /// Whether to respect robots.txt
    #[serde(default = "default_true")]
    pub respect_robots: bool,
    /// Skip JavaScript rendering (faster, for static pages only)
    /// Note: By default, JS rendering is ENABLED for better content extraction
    #[serde(default)]
    pub skip_render: bool,
    /// Timeout for browser rendering in seconds (default: 30)
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
    /// Optional job priority (1-10, default 5)
    #[serde(default = "default_priority")]
    pub priority: u8,
    /// Maximum pages to process (default: 0 = unlimited/discovery mode)
    #[serde(default = "default_max_pages")]
    pub max_pages: usize,
}

fn default_depth() -> usize {
    1
}

fn default_true() -> bool {
    true
}

fn default_priority() -> u8 {
    5
}

fn default_timeout_secs() -> u64 {
    30
}

fn default_max_pages() -> usize {
    0  // Default to unlimited (discovery mode) - crawler will discover actual count
}

/// Response when starting an indexing job
#[derive(Debug, Serialize)]
pub struct StartIndexingResponse {
    /// Job ID
    pub job_id: Uuid,
    /// SSE endpoint for progress updates
    pub sse_url: String,
    /// Job status
    pub status: String,
}

/// Response for delete all data operation
#[derive(Debug, Serialize)]
pub struct DeleteAllDataResponse {
    pub status: String,
    pub message: String,
    pub documents_deleted: usize,
    pub chunks_deleted: usize,
}

/// Job list query parameters
#[derive(Debug, Deserialize)]
pub struct ListJobsQuery {
    /// Filter by status
    pub status: Option<String>,
    /// Limit results
    pub limit: Option<usize>,
    /// Offset for pagination
    pub offset: Option<usize>,
}

/// Job list response
#[derive(Debug, Serialize)]
pub struct JobListResponse {
    pub jobs: Vec<JobResponse>,
    pub total: usize,
}

/// Individual job response
#[derive(Debug, Serialize)]
pub struct JobResponse {
    pub id: Uuid,
    pub job_type: String,
    pub status: String,
    pub created_at: String,
    pub progress: Option<ProgressResponse>,
}

/// Progress response
#[derive(Debug, Serialize)]
pub struct ProgressResponse {
    pub processed: usize,
    pub total: usize,
    pub percentage: f32,
    pub current_item: Option<String>,
    pub rate: f64,
}

/// Error response
#[derive(Debug, Clone, Serialize)]
pub struct IndexingErrorResponse {
    pub error: String,
    pub message: String,
}

/// API error type
pub struct ApiIndexingError {
    status: StatusCode,
    body: IndexingErrorResponse,
}

impl ApiIndexingError {
    pub fn bad_request(error: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            body: IndexingErrorResponse {
                error: error.into(),
                message: message.into(),
            },
        }
    }
}

impl IntoResponse for ApiIndexingError {
    fn into_response(self) -> axum::response::Response {
        (self.status, Json(self.body)).into_response()
    }
}

/// Request to re-index a single entry
#[derive(Debug, Deserialize)]
pub struct ReindexEntryRequest {
    /// Whether to skip JavaScript rendering (default: false, meaning render by default)
    #[serde(default)]
    pub skip_render: bool,
    /// Timeout for browser rendering in seconds (default: 30)
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
    /// Optional job priority (1-10, default 5)
    #[serde(default = "default_priority")]
    pub priority: u8,
}

/// Request to re-index all entries from a domain
#[derive(Debug, Deserialize)]
pub struct ReindexByDomainRequest {
    /// Domain to re-index (e.g., "claude.com")
    pub domain: String,
    /// Whether to skip JavaScript rendering (default: false)
    #[serde(default)]
    pub skip_render: bool,
    /// Timeout for browser rendering in seconds (default: 30)
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
    /// Optional job priority (1-10, default 5)
    #[serde(default = "default_priority")]
    pub priority: u8,
}

/// Response when re-indexing is started
#[derive(Debug, Serialize)]
pub struct ReindexResponse {
    /// Number of entries queued for re-indexing
    pub entries_queued: usize,
    /// Job IDs created for re-indexing
    pub job_ids: Vec<Uuid>,
    /// Status message
    pub status: String,
}

impl From<IndexingErrorResponse> for ApiIndexingError {
    fn from(err: IndexingErrorResponse) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            body: err,
        }
    }
}

// === Route Handlers ===

/// POST /api/indexing/url - Start URL indexing job
async fn start_url_indexing(
    State(state): State<IndexingState>,
    Json(request): Json<StartUrlIndexingRequest>,
) -> Result<Json<StartIndexingResponse>, ApiIndexingError> {
    info!(url = %request.url, levels = request.levels, "Starting URL indexing");

    // Validate URL
    let _parsed_url = url::Url::parse(&request.url).map_err(|e| {
        ApiIndexingError::bad_request("invalid_url", format!("Invalid URL: {}", e))
    })?;

    // Create job
    let mut config = JobConfig::default();
    config.priority = request.priority;

    let job = Job::new(
        JobType::Url {
            url: request.url.clone(),
            depth: request.levels,
            respect_robots: request.respect_robots,
            render_js: !request.skip_render,  // Invert: skip_render=false means render_js=true
            timeout_secs: request.timeout_secs,
            max_pages: request.max_pages,
        },
        config,
    );

    let job_id = job.id;

    // Submit to queue
    state
        .job_queue
        .submit(job)
        .await
        .map_err(|e| ApiIndexingError::bad_request(
            "queue_error",
            format!("Failed to submit job: {}", e),
        ))?;

    Ok(Json(StartIndexingResponse {
        job_id,
        sse_url: format!("/api/indexing/sse/{}", job_id),
        status: "pending".to_string(),
    }))
}

/// Request to upload files (base64 encoded for JSON compatibility)
#[derive(Debug, Deserialize)]
pub struct UploadFilesRequest {
    /// Files to upload (filename + base64-encoded content)
    pub files: Vec<FileUpload>,
    /// Whether to extract archives (ZIP files)
    #[serde(default = "default_true")]
    pub extract_archives: bool,
    /// Optional job priority
    #[serde(default = "default_priority")]
    pub priority: u8,
}

#[derive(Debug, Deserialize)]
pub struct FileUpload {
    /// Original filename
    pub name: String,
    /// Base64-encoded file content
    pub content: String,
}

/// POST /api/indexing/files - Start file upload indexing job
async fn start_file_indexing(
    State(state): State<IndexingState>,
    Json(request): Json<UploadFilesRequest>,
) -> Result<Json<StartIndexingResponse>, ApiIndexingError> {
    info!(file_count = request.files.len(), "Starting file upload indexing");

    if request.files.is_empty() {
        return Err(ApiIndexingError::bad_request(
            "no_files",
            "No files were provided",
        ));
    }

    let mut uploaded_files: Vec<UploadedFile> = Vec::new();

    // Process each file
    for file in &request.files {
        // Decode base64 content
        let data = base64::engine::general_purpose::STANDARD
            .decode(&file.content)
            .map_err(|e| ApiIndexingError::bad_request(
                "decode_error",
                format!("Failed to decode base64 for file '{}': {}", file.name, e),
            ))?;

        info!(filename = %file.name, size = data.len(), "Processing uploaded file");

        // Process the upload (handles ZIP extraction automatically)
        let processed = state
            .file_handler
            .process_upload(&file.name, &data)
            .await
            .map_err(|e| ApiIndexingError::bad_request(
                "processing_error",
                format!("Failed to process file '{}': {}", file.name, e),
            ))?;

        uploaded_files.extend(processed);
    }

    if uploaded_files.is_empty() {
        return Err(ApiIndexingError::bad_request(
            "no_valid_files",
            "No valid files could be processed",
        ));
    }

    // Create job with file paths
    let file_paths: Vec<_> = uploaded_files.iter().map(|f| f.path.clone()).collect();
    let file_names: Vec<_> = uploaded_files.iter().map(|f| f.original_name.clone()).collect();

    let mut config = JobConfig::default();
    config.priority = request.priority;

    let job = Job::new(
        JobType::FileUpload {
            file_paths,
            file_names,
            extract_archives: request.extract_archives,
        },
        config,
    );
    let job_id = job.id;

    // Submit to queue
    state
        .job_queue
        .submit(job)
        .await
        .map_err(|e| ApiIndexingError::bad_request(
            "queue_error",
            format!("Failed to submit job: {}", e),
        ))?;

    Ok(Json(StartIndexingResponse {
        job_id,
        sse_url: format!("/api/indexing/sse/{}", job_id),
        status: "pending".to_string(),
    }))
}

/// GET /api/indexing/jobs - List all jobs
async fn list_jobs(
    State(state): State<IndexingState>,
    Query(query): Query<ListJobsQuery>,
) -> Json<JobListResponse> {
    let jobs = state.job_queue.list().await;
    let total = jobs.len();

    let jobs: Vec<JobResponse> = jobs
        .into_iter()
        .skip(query.offset.unwrap_or(0))
        .take(query.limit.unwrap_or(100))
        .map(|j| JobResponse {
            id: j.id,
            job_type: j.job_type,
            status: j.state,
            created_at: j.created_at.to_rfc3339(),
            progress: None,
        })
        .collect();

    Json(JobListResponse { jobs, total })
}

/// GET /api/indexing/jobs/:id - Get job details
async fn get_job(
    State(state): State<IndexingState>,
    Path(job_id): Path<Uuid>,
) -> Result<Json<JobResponse>, StatusCode> {
    let job = state.job_queue.get(job_id).ok_or(StatusCode::NOT_FOUND)?;

    let state_str = match job.get_state().await {
        JobState::Pending { .. } => "pending",
        JobState::Queued { .. } => "queued",
        JobState::Running { .. } => "running",
        JobState::Completed { .. } => "completed",
        JobState::Failed { .. } => "failed",
        JobState::Cancelled { .. } => "cancelled",
    };

    let job_type = match &job.job_type {
        JobType::Url { .. } => "url",
        JobType::FileUpload { .. } => "file_upload",
        JobType::Reindex { .. } => "reindex",
    };

    Ok(Json(JobResponse {
        id: job.id,
        job_type: job_type.to_string(),
        status: state_str.to_string(),
        created_at: job.created_at.to_rfc3339(),
        progress: None,
    }))
}

/// DELETE /api/indexing/jobs/:id - Cancel a job
async fn cancel_job(
    State(state): State<IndexingState>,
    Path(job_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    state
        .job_queue
        .cancel(job_id, "Cancelled by user".to_string())
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok(Json(serde_json::json!({
        "status": "cancelled",
        "job_id": job_id
    })))
}

/// DELETE /api/indexing/data - Delete all indexed data
async fn delete_all_data(
    State(state): State<IndexingState>,
) -> Result<Json<DeleteAllDataResponse>, ApiIndexingError> {
    info!("Deleting all indexed data");

    // Get counts before deletion for reporting
    let store = state.app_state.store().read().await;
    let doc_count = store.document_count().await.unwrap_or(0);
    let chunk_count = store.chunk_count().unwrap_or(0);  // sync operation
    drop(store); // Release read lock before write operation

    // Acquire write lock and clear all data
    let store = state.app_state.store().write().await;
    store.clear_all().await.map_err(|e| {
        error!(error = %e, "Failed to clear tables");
        ApiIndexingError::bad_request("clear_failed", format!("Failed to delete data: {}", e))
    })?;
    drop(store);

    // Note: No cache invalidation needed - tables are refreshed before each search

    info!(
        documents = doc_count,
        chunks = chunk_count,
        "Successfully deleted all indexed data"
    );

    Ok(Json(DeleteAllDataResponse {
        status: "success".to_string(),
        message: "All indexed data has been deleted".to_string(),
        documents_deleted: doc_count,
        chunks_deleted: chunk_count,
    }))
}

/// GET /api/indexing/sse/:job_id - SSE stream for specific job
async fn sse_job_stream(
    State(state): State<IndexingState>,
    Path(job_id): Path<Uuid>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, StatusCode> {
    info!(job_id = %job_id, client = %addr, "SSE connection for job");

    let connection = state
        .sse_manager
        .connect_to_job(job_id, addr.ip())
        .map_err(|e| {
            error!(error = %e, "Failed to create SSE connection");
            StatusCode::SERVICE_UNAVAILABLE
        })?;

    Ok(connection.into_sse())
}

/// GET /api/indexing/sse - Global SSE stream for all jobs
async fn sse_global_stream(
    State(state): State<IndexingState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, StatusCode> {
    info!(client = %addr, "Global SSE connection");

    let connection = state.sse_manager.connect_global(addr.ip()).map_err(|e| {
        error!(error = %e, "Failed to create SSE connection");
        StatusCode::SERVICE_UNAVAILABLE
    })?;

    Ok(connection.into_sse())
}

/// POST /api/indexing/reindex/:id - Re-index a single entry
async fn reindex_entry(
    State(state): State<IndexingState>,
    Path(id): Path<String>,
    Json(request): Json<ReindexEntryRequest>,
) -> Result<Json<StartIndexingResponse>, ApiIndexingError> {
    info!(entry_id = %id, "Re-indexing entry");

    // Get the entry to find its source URL
    let store = state.app_state.store().read().await;
    let entry = store
        .get_entry(&id)
        .await
        .map_err(|e| ApiIndexingError::bad_request("store_error", format!("Failed to get entry: {}", e)))?
        .ok_or_else(|| ApiIndexingError::bad_request("not_found", format!("Entry not found: {}", id)))?;

    // Get the source URL from source_path
    let source_url = entry.source_path.clone();
    if source_url.is_empty() {
        return Err(ApiIndexingError::bad_request("no_source", "Entry has no source URL and cannot be re-indexed"));
    }

    // Validate it's a URL
    let _parsed_url = url::Url::parse(&source_url).map_err(|e| {
        ApiIndexingError::bad_request("invalid_url", format!("Entry source is not a valid URL: {}", e))
    })?;

    drop(store);

    // Delete existing entry and chunks
    {
        let store = state.app_state.store().write().await;
        if let Err(e) = store.delete_chunks_by_entry(&id) {  // sync operation
            error!(error = %e, "Failed to delete chunks for entry");
        }
        if let Err(e) = store.delete_entry(&id).await {
            error!(error = %e, "Failed to delete entry");
        }
    }

    // Note: No cache invalidation needed - tables are refreshed before each search

    // Create a new URL indexing job with browser rendering
    let mut config = JobConfig::default();
    config.priority = request.priority;

    let job = Job::new(
        JobType::Url {
            url: source_url.clone(),
            depth: 0, // Only re-index this specific URL, no following links
            respect_robots: true,
            render_js: !request.skip_render,
            timeout_secs: request.timeout_secs,
            max_pages: 1,  // Only this one page
        },
        config,
    );

    let job_id = job.id;

    // Submit to queue
    state
        .job_queue
        .submit(job)
        .await
        .map_err(|e| ApiIndexingError::bad_request(
            "queue_error",
            format!("Failed to submit re-index job: {}", e),
        ))?;

    info!(job_id = %job_id, url = %source_url, "Re-index job submitted");

    Ok(Json(StartIndexingResponse {
        job_id,
        sse_url: format!("/api/indexing/sse/{}", job_id),
        status: "pending".to_string(),
    }))
}

/// POST /api/indexing/reindex-domain - Re-index all entries from a domain
async fn reindex_by_domain(
    State(state): State<IndexingState>,
    Json(request): Json<ReindexByDomainRequest>,
) -> Result<Json<ReindexResponse>, ApiIndexingError> {
    info!(domain = %request.domain, "Re-indexing all entries from domain");

    // Get entries filtered by domain (using source_domain filter)
    let store = state.app_state.store().read().await;
    let domain_entries = store
        .list_entries(None, Some(&request.domain), 10000, 0)  // Large limit to get all
        .await
        .map_err(|e| ApiIndexingError::bad_request("store_error", format!("Failed to list entries: {}", e)))?;

    if domain_entries.is_empty() {
        return Err(ApiIndexingError::bad_request(
            "no_entries",
            format!("No entries found for domain: {}", request.domain),
        ));
    }

    let entry_count = domain_entries.len();
    drop(store);

    // Delete all entries and their chunks, then create jobs
    let mut job_ids = Vec::new();

    for entry in domain_entries {
        let entry_id = entry.id.clone();
        let source_url = entry.source_path.clone();
        if source_url.is_empty() {
            continue;  // Skip entries without source path
        }

        // Delete existing entry and chunks
        {
            let store = state.app_state.store().write().await;
            if let Err(e) = store.delete_chunks_by_entry(&entry_id) {  // sync operation
                error!(error = %e, entry_id = %entry_id, "Failed to delete chunks");
            }
            if let Err(e) = store.delete_entry(&entry_id).await {
                error!(error = %e, entry_id = %entry_id, "Failed to delete entry");
            }
        }

        // Create job for this URL
        let mut config = JobConfig::default();
        config.priority = request.priority;

        let job = Job::new(
            JobType::Url {
                url: source_url,
                depth: 0,
                respect_robots: true,
                render_js: !request.skip_render,
                timeout_secs: request.timeout_secs,
                max_pages: 1,  // Only this one page
            },
            config,
        );

        let job_id = job.id;
        job_ids.push(job_id);

        // Submit to queue
        if let Err(e) = state.job_queue.submit(job).await {
            error!(error = %e, "Failed to submit re-index job");
        }
    }

    // Note: No cache invalidation needed - tables are refreshed before each search

    info!(
        domain = %request.domain,
        entries_queued = job_ids.len(),
        "Domain re-index jobs submitted"
    );

    Ok(Json(ReindexResponse {
        entries_queued: job_ids.len(),
        job_ids,
        status: format!("Queued {} entries from domain '{}' for re-indexing", entry_count, request.domain),
    }))
}

// ============================================================================
// Job History Endpoints
// ============================================================================

/// Query parameters for job history list
#[derive(Debug, Deserialize)]
pub struct JobHistoryQuery {
    /// Maximum jobs to return (default: 50)
    #[serde(default = "default_history_limit")]
    pub limit: usize,
    /// Offset for pagination (default: 0)
    #[serde(default)]
    pub offset: usize,
}

fn default_history_limit() -> usize {
    50
}

/// Response for job history list
#[derive(Debug, Serialize)]
pub struct JobHistoryResponse {
    pub jobs: Vec<JobHistoryItem>,
    pub total: usize,
}

/// A job in the history list
#[derive(Debug, Serialize)]
pub struct JobHistoryItem {
    pub job_id: String,
    pub job_type: String,
    pub status: String,
    pub source_url: Option<String>,
    pub source_domain: Option<String>,
    pub created_at: String,
    pub completed_at: String,
    pub stats: JobHistoryStats,
}

/// Job statistics
#[derive(Debug, Serialize)]
pub struct JobHistoryStats {
    pub items_processed: u32,
    pub items_discovered: u32,
    pub chunks_created: u32,
    pub embeddings_generated: u32,
    pub error_count: u32,
    pub duration_ms: u64,
    pub rate: f64,
}

/// Detailed job response with items
#[derive(Debug, Serialize)]
pub struct JobDetailResponse {
    pub job: JobHistoryItem,
    pub items: Vec<JobHistoryItemDetail>,
    pub errors: Vec<String>,
}

/// Individual item (page/file) in job history
#[derive(Debug, Serialize)]
pub struct JobHistoryItemDetail {
    pub url: String,
    pub status: String,
    pub parent_url: Option<String>,
    pub depth: u32,
    pub discovered_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub chunks_created: u32,
    pub embeddings_generated: u32,
    pub duration_ms: u64,
    pub error_message: Option<String>,
}

/// GET /api/indexing/history - List persisted job history
async fn list_job_history(
    State(state): State<IndexingState>,
    Query(query): Query<JobHistoryQuery>,
) -> Result<Json<JobHistoryResponse>, (StatusCode, String)> {
    let store = state.job_store.as_ref().ok_or_else(|| {
        (StatusCode::SERVICE_UNAVAILABLE, "Job history not available".to_string())
    })?;

    let jobs = store
        .list_jobs(None, query.limit, query.offset)  // (status, limit, offset)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to list job history");
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to list jobs: {}", e))
        })?;

    // Get total count from aggregate stats
    let aggregate = store.get_aggregate_stats().await.unwrap_or_default();
    let total = aggregate.total_jobs;

    let items: Vec<JobHistoryItem> = jobs
        .into_iter()
        .map(|j| JobHistoryItem {
            job_id: j.job_id,
            job_type: j.job_type,
            status: j.status,
            source_url: j.source_url,
            source_domain: j.source_domain,
            created_at: j.created_at.clone(),  // Already a String
            completed_at: j.completed_at.clone(),  // Already a String
            stats: JobHistoryStats {
                items_processed: j.items_processed as u32,  // inline field
                items_discovered: j.items_discovered as u32,
                chunks_created: j.chunks_created as u32,
                embeddings_generated: j.embeddings_generated as u32,
                error_count: j.error_count as u32,
                duration_ms: j.duration_ms as u64,
                rate: j.processing_rate,
            },
        })
        .collect();

    Ok(Json(JobHistoryResponse { jobs: items, total }))
}

/// GET /api/indexing/history/:job_id - Get detailed job history with items
async fn get_job_detail(
    State(state): State<IndexingState>,
    Path(job_id): Path<String>,
) -> Result<Json<JobDetailResponse>, (StatusCode, String)> {
    let store = state.job_store.as_ref().ok_or_else(|| {
        (StatusCode::SERVICE_UNAVAILABLE, "Job history not available".to_string())
    })?;

    let job = store
        .get_job(&job_id)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to get job");
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to get job: {}", e))
        })?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Job not found".to_string()))?;

    let items = store
        .get_job_items(&job_id)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to get job items");
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to get job items: {}", e))
        })?;

    let job_item = JobHistoryItem {
        job_id: job.job_id,
        job_type: job.job_type,
        status: job.status,
        source_url: job.source_url,
        source_domain: job.source_domain,
        created_at: job.created_at.clone(),  // Already a String
        completed_at: job.completed_at.clone(),  // Already a String
        stats: JobHistoryStats {
            items_processed: job.items_processed as u32,  // inline field
            items_discovered: job.items_discovered as u32,
            chunks_created: job.chunks_created as u32,
            embeddings_generated: job.embeddings_generated as u32,
            error_count: job.error_count as u32,
            duration_ms: job.duration_ms as u64,
            rate: job.processing_rate,
        },
    };

    let item_details: Vec<JobHistoryItemDetail> = items
        .into_iter()
        .map(|i| JobHistoryItemDetail {
            url: i.item_path,
            status: i.status,
            parent_url: i.parent_url,
            depth: i.depth as u32,  // i64 to u32
            discovered_at: i.discovered_at.clone(),  // Already a String
            started_at: i.started_at.clone(),  // Already Option<String>
            completed_at: i.completed_at.clone(),  // Already Option<String>
            chunks_created: i.chunks_created as u32,  // i64 to u32
            embeddings_generated: i.embeddings_generated as u32,  // i64 to u32
            duration_ms: i.duration_ms as u64,  // i64 to u64
            error_message: i.error_message,
        })
        .collect();

    // Parse errors from JSON string to Vec<String>
    let errors: Vec<String> = job.errors
        .as_ref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();

    Ok(Json(JobDetailResponse {
        job: job_item,
        items: item_details,
        errors,
    }))
}

/// GET /api/indexing/history/:job_id/items - Get just the items for a job
async fn get_job_items(
    State(state): State<IndexingState>,
    Path(job_id): Path<String>,
) -> Result<Json<Vec<JobHistoryItemDetail>>, (StatusCode, String)> {
    let store = state.job_store.as_ref().ok_or_else(|| {
        (StatusCode::SERVICE_UNAVAILABLE, "Job history not available".to_string())
    })?;

    let items = store
        .get_job_items(&job_id)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to get job items");
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to get job items: {}", e))
        })?;

    let item_details: Vec<JobHistoryItemDetail> = items
        .into_iter()
        .map(|i| JobHistoryItemDetail {
            url: i.item_path,
            status: i.status,
            parent_url: i.parent_url,
            depth: i.depth as u32,  // i64 to u32
            discovered_at: i.discovered_at.clone(),  // Already a String
            started_at: i.started_at.clone(),  // Already Option<String>
            completed_at: i.completed_at.clone(),  // Already Option<String>
            chunks_created: i.chunks_created as u32,  // i64 to u32
            embeddings_generated: i.embeddings_generated as u32,  // i64 to u32
            duration_ms: i.duration_ms as u64,  // i64 to u64
            error_message: i.error_message,
        })
        .collect();

    Ok(Json(item_details))
}