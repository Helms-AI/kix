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
        }
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
        // Data management endpoints
        .route("/api/indexing/data", delete(delete_all_data))
        // Start indexing endpoints
        .route("/api/indexing/url", post(start_url_indexing))
        .route("/api/indexing/files", post(start_file_indexing))
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
    /// Whether to render JavaScript (requires browser)
    #[serde(default)]
    pub render_js: bool,
    /// Optional job priority (1-10, default 5)
    #[serde(default = "default_priority")]
    pub priority: u8,
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
            render_js: request.render_js,
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
    let chunk_count = store.chunk_count().await.unwrap_or(0);
    drop(store); // Release read lock before write operation

    // Acquire write lock and clear tables
    let mut store = state.app_state.store().write().await;
    store.clear_tables().await.map_err(|e| {
        error!(error = %e, "Failed to clear tables");
        ApiIndexingError::bad_request("clear_failed", format!("Failed to delete data: {}", e))
    })?;
    drop(store);

    // Invalidate all caches since data has changed
    state.app_state.invalidate_caches();

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