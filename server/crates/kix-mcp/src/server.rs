//! MCP server implementation for RAG (Retrieval Augmented Generation) system.
//!
//! This module provides domain-agnostic tools for AI agents to interact with
//! the knowledge base, plus project management tools for AI-assisted planning.
//!
//! **Retrieval (3 tools):**
//! - `search` - Unified semantic + keyword search
//! - `get_context` - Full page content for RAG synthesis
//! - `get_document` - Document metadata and chunks
//!
//! **Indexing (4 tools):**
//! - `index` - Synchronous single document indexing
//! - `index_async` - Async crawl/batch indexing with job tracking
//! - `job_status` - Check async job progress
//! - `delete` - Remove documents by ID or filter
//!
//! **Status (1 tool):**
//! - `status` - Index health and statistics
//!
//! **Project Management (25+ tools):**
//! - Project CRUD: `create_project`, `list_projects`, `get_project`, `update_project`, `delete_project`
//! - Issue CRUD: `create_issue`, `list_issues`, `get_issue`, `update_issue`, `delete_issue`
//! - GitHub Projects V2: `create_github_project`, `get_github_project`, `add_issue_to_project`, `update_project_item`, `sync_github_project`
//! - AI Planning: `plan_project`, `suggest_tasks`, `get_project_context`, `breakdown_task`
//! - Token management: `set_github_token`, `sync_github_issues`
//! - Knowledge linking: `link_entry_to_project`, `unlink_entry_from_project`, `list_project_entries`
//! - Search: `search_project`

use reqwest::Client as HttpClient;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::tool::ToolCallContext;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolRequestParam, CallToolResult, Content, ListToolsResult,
    PaginatedRequestParam, ServerCapabilities, ServerInfo,
};
use rmcp::schemars;
use rmcp::service::RequestContext;
use rmcp::tool;
use rmcp::tool_router;
use rmcp::ErrorData as McpError;
use rmcp::{RoleServer, ServerHandler};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};
use uuid::Uuid;

use kix_embeddings::{DocumentChunker, EmbeddingGenerator};
use kix_jobs::{Job, JobConfig, JobQueue, JobState, JobType};
use kix_parser::{Entry, EntryType, PdfParser, SourceType};
use kix_crawler::ContentExtractor;
use kix_services::{self, Pagination};
use kix_store::{KixStore, ProjectRecord, IssueRecord, ProjectEntryRecord};
use kix_store::projects::ProjectStore;
use kix_projects::{
    GitHubTokenManager, InMemoryTokenStorage, TokenStorage, TokenScope, TokenService,
    GitHubSyncService, IssueInfo, IssueState, IssueSource,
    SyncDirection, SyncConfig, calculate_text_score, generate_excerpt,
    SharedEventBus, ProjectV2Service, ProjectTemplate,
};

use crate::project_tools::*;

// =============================================================================
// RETRIEVAL TOOL PARAMETERS
// =============================================================================

/// Search mode for queries.
#[derive(Debug, Clone, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "lowercase")]
pub enum SearchMode {
    /// Combined vector and full-text search (default, recommended)
    #[default]
    Hybrid,
    /// Pure semantic vector search
    Vector,
    /// Pure keyword/full-text search
    Text,
}

/// Filters for search queries.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
pub struct QueryFilters {
    /// Filter by document type (e.g., "document", "pdf", "article")
    #[schemars(description = "Filter by document type: 'document', 'pdf', 'article', 'code'")]
    pub entry_type: Option<String>,
    /// Filter by chunk type (e.g., "content", "code", "header")
    #[schemars(description = "Filter by chunk type: 'content', 'code', 'header', 'summary'")]
    pub chunk_type: Option<String>,
    /// Filter by tag
    #[schemars(description = "Filter by tag")]
    pub tag: Option<String>,
    /// Filter by source domain
    #[schemars(description = "Filter by source domain (e.g., 'docs.example.com')")]
    pub source_domain: Option<String>,
}

/// Parameters for the `search` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchParams {
    /// Natural language search query
    #[schemars(description = "Natural language search query")]
    pub query: String,
    /// Maximum number of results (default: 10, max: 100)
    #[schemars(description = "Maximum number of results to return (default: 10, max: 100)")]
    pub limit: Option<usize>,
    /// Pagination offset (default: 0)
    #[schemars(description = "Pagination offset (default: 0)")]
    pub offset: Option<usize>,
    /// Search mode: hybrid, vector, or text
    #[schemars(description = "Search mode: 'hybrid' (default), 'vector', or 'text'")]
    pub mode: Option<SearchMode>,
    /// Optional filters
    #[schemars(description = "Optional filters for entry_type, chunk_type, tag, source_domain")]
    pub filters: Option<QueryFilters>,
}

/// A single search result.
#[derive(Debug, Serialize)]
pub struct SearchResultItem {
    pub chunk_id: String,
    pub entry_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_id: Option<String>,
    pub text: String,
    pub score: f32,
    pub entry_title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
}

/// Response from the `search` tool.
#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResultItem>,
    pub total_count: usize,
    pub has_more: bool,
}

/// Parameters for the `get_context` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetContextParams {
    /// Page ID for direct lookup
    #[schemars(description = "Page ID from search result for direct lookup")]
    pub page_id: Option<String>,
    /// Chunk ID to lookup page via chunk's page_id
    #[schemars(description = "Chunk ID to lookup the associated page")]
    pub chunk_id: Option<String>,
}

/// Full page context for RAG.
#[derive(Debug, Serialize)]
pub struct PageContext {
    pub page_id: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub full_content: String,
    pub content_length: usize,
    pub code_block_count: usize,
}

/// Parameters for the `get_document` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetDocumentParams {
    /// Document ID
    #[schemars(description = "Document ID to retrieve")]
    pub id: String,
    /// Include all chunks for this document
    #[schemars(description = "Include all chunks for this document (default: false)")]
    pub include_chunks: Option<bool>,
}

/// A chunk within a document.
#[derive(Debug, Serialize)]
pub struct ChunkInfo {
    pub chunk_id: String,
    pub chunk_index: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunk_type: Option<String>,
    pub text: String,
}

/// Full document with metadata.
#[derive(Debug, Serialize)]
pub struct Document {
    pub id: String,
    pub title: String,
    pub description: String,
    pub entry_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_domain: Option<String>,
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunks: Option<Vec<ChunkInfo>>,
}

// =============================================================================
// INDEXING TOOL PARAMETERS
// =============================================================================

/// Content source for indexing.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ContentSource {
    /// Raw text or markdown content
    #[schemars(description = "Raw text or markdown content to index")]
    pub text: Option<String>,
    /// Local file path (HTML, PDF, DOCX, etc.)
    #[schemars(description = "Absolute path to a file (HTML, PDF, DOCX, etc.)")]
    pub file: Option<String>,
    /// URL to fetch and index (single page, no crawling)
    #[schemars(description = "URL to fetch and index (single page)")]
    pub url: Option<String>,
}

/// Parameters for the `index` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct IndexParams {
    /// Content source - provide text, file, or url
    #[schemars(description = "Content to index: provide 'text', 'file', or 'url'")]
    pub content: ContentSource,
    /// Document title (auto-extracted if omitted)
    #[schemars(description = "Document title (auto-extracted from content if not provided)")]
    pub title: Option<String>,
    /// Custom document ID (auto-generated if omitted)
    #[schemars(description = "Custom document ID (auto-generated if not provided)")]
    pub id: Option<String>,
    /// Tags for categorization
    #[schemars(description = "Tags for categorization and filtering")]
    pub tags: Option<Vec<String>>,
    /// Replace existing document with same ID
    #[schemars(description = "Replace existing document with same ID (default: false)")]
    pub replace: Option<bool>,
}

/// Result of indexing a document.
#[derive(Debug, Serialize)]
pub struct IndexResult {
    pub success: bool,
    pub document_id: String,
    pub title: String,
    pub chunks_created: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// URL source with crawl settings for async indexing.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct UrlSource {
    /// URL to crawl
    #[schemars(description = "URL to crawl")]
    pub url: String,
    /// Crawl depth (0 = single page, 1 = follow links one level, default: 1)
    #[schemars(description = "Crawl depth: 0 = single page, 1+ = follow links (default: 1)")]
    pub depth: Option<usize>,
    /// Maximum pages to index (0 = unlimited/discovery mode)
    #[schemars(description = "Maximum pages to index (default: 0 = unlimited/discovery mode)")]
    pub max_pages: Option<usize>,
    /// Whether to respect robots.txt (default: true)
    #[schemars(description = "Whether to respect robots.txt (default: true)")]
    pub respect_robots: Option<bool>,
    /// Whether to render JavaScript (default: true)
    #[schemars(description = "Whether to render JavaScript for dynamic content (default: true)")]
    pub render_js: Option<bool>,
    /// Timeout for browser rendering in seconds (default: 30)
    #[schemars(description = "Timeout for browser rendering in seconds (default: 30)")]
    pub timeout_secs: Option<u64>,
    /// Job priority (1-10, higher = more urgent, default: 5)
    #[schemars(description = "Job priority 1-10, higher = more urgent (default: 5)")]
    pub priority: Option<u8>,
}

/// Async source for batch/crawl operations.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct AsyncSource {
    /// URL with optional crawl settings
    #[schemars(description = "URL to crawl with optional depth and max_pages")]
    pub url: Option<UrlSource>,
    /// Multiple file paths
    #[schemars(description = "Array of file paths to index")]
    pub files: Option<Vec<String>>,
}

/// Parameters for the `index_async` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct IndexAsyncParams {
    /// Source to index - URL with crawl settings or file list
    #[schemars(description = "Source to index: 'url' with crawl settings or 'files' array")]
    pub source: AsyncSource,
    /// Tags to apply to all indexed documents
    #[schemars(description = "Tags to apply to all indexed documents")]
    pub tags: Option<Vec<String>>,
}

/// Result of starting an async job.
#[derive(Debug, Serialize)]
pub struct JobCreated {
    pub job_id: String,
    pub status: String,
    pub source_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_items: Option<usize>,
}

/// Parameters for the `job_status` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct JobStatusParams {
    /// Job ID from index_async
    #[schemars(description = "Job ID from index_async")]
    pub job_id: String,
}

/// Progress information for a running job.
#[derive(Debug, Serialize)]
pub struct JobProgress {
    pub processed: usize,
    pub total: usize,
    pub percentage: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_item: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eta_seconds: Option<usize>,
}

/// Result of a completed job.
#[derive(Debug, Serialize)]
pub struct JobResult {
    pub documents_created: usize,
    pub chunks_created: usize,
    pub errors: Vec<String>,
}

/// Full job status response.
#[derive(Debug, Serialize)]
pub struct JobStatusResponse {
    pub job_id: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<JobProgress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<JobResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Filter for delete operations.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct DeleteFilter {
    /// Delete all documents with this tag
    #[schemars(description = "Delete all documents with this tag")]
    pub tag: Option<String>,
    /// Delete all documents from this domain
    #[schemars(description = "Delete all documents from this source domain")]
    pub source_domain: Option<String>,
}

/// Parameters for the `delete` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteParams {
    /// Single document ID to delete
    #[schemars(description = "Single document ID to delete")]
    pub id: Option<String>,
    /// Multiple document IDs to delete
    #[schemars(description = "Multiple document IDs to delete")]
    pub ids: Option<Vec<String>>,
    /// Filter to delete matching documents
    #[schemars(description = "Filter to delete documents by tag or source_domain")]
    pub filter: Option<DeleteFilter>,
    /// Preview deletion without executing
    #[schemars(description = "Preview what would be deleted without actually deleting (default: false)")]
    pub dry_run: Option<bool>,
}

/// Result of delete operation.
#[derive(Debug, Serialize)]
pub struct DeleteResult {
    pub success: bool,
    pub documents_deleted: usize,
    pub chunks_deleted: usize,
    pub deleted_ids: Vec<String>,
    pub dry_run: bool,
}

// =============================================================================
// STATUS TOOL PARAMETERS
// =============================================================================

/// Parameters for the `status` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct StatusParams {
    /// Include breakdown by type and domain
    #[schemars(description = "Include breakdown by type and domain (default: false)")]
    pub detailed: Option<bool>,
}

/// Breakdown statistics.
#[derive(Debug, Serialize)]
pub struct StatusBreakdown {
    pub by_type: HashMap<String, usize>,
    pub by_domain: HashMap<String, usize>,
}

/// Index status response.
#[derive(Debug, Serialize)]
pub struct IndexStatus {
    pub health: String,
    pub document_count: usize,
    pub chunk_count: usize,
    pub page_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub breakdown: Option<StatusBreakdown>,
}

// =============================================================================
// MCP SERVER IMPLEMENTATION
// =============================================================================

/// The main Knowledge Indexer MCP server for RAG systems.
#[derive(Clone)]
pub struct KixMcpServer {
    store: Arc<RwLock<KixStore>>,
    embedder: Arc<RwLock<EmbeddingGenerator>>,
    http_client: HttpClient,
    /// Job queue for async indexing operations
    job_queue: Arc<JobQueue>,
    /// Project store for project management
    project_store: Option<Arc<RwLock<ProjectStore>>>,
    /// GitHub token storage (legacy)
    token_storage: Option<Arc<dyn TokenStorage>>,
    /// GitHub token manager for encryption (legacy)
    token_manager: Option<Arc<GitHubTokenManager>>,
    /// Token service for GitHub integration (preferred)
    token_service: Option<Arc<TokenService>>,
    /// Event bus for real-time events
    event_bus: Option<SharedEventBus>,
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl KixMcpServer {
    /// Creates a new MCP server with the given store, embedder, and job queue.
    /// This creates owned store and embedder wrapped in Arc<RwLock<>>.
    pub fn new(store: KixStore, embedder: EmbeddingGenerator, job_queue: Arc<JobQueue>) -> Self {
        Self::with_shared(
            Arc::new(RwLock::new(store)),
            Arc::new(RwLock::new(embedder)),
            job_queue,
        )
    }

    /// Creates a new MCP server with a shared store.
    /// Use this when running MCP and API servers in the same process to share the store.
    pub fn with_shared_store(
        store: Arc<RwLock<KixStore>>,
        embedder: EmbeddingGenerator,
        job_queue: Arc<JobQueue>,
    ) -> Self {
        Self::with_shared(store, Arc::new(RwLock::new(embedder)), job_queue)
    }

    /// Creates a new MCP server with both shared store and shared embedder.
    /// Use this in unified server mode where API and MCP share all resources.
    pub fn with_shared(
        store: Arc<RwLock<KixStore>>,
        embedder: Arc<RwLock<EmbeddingGenerator>>,
        job_queue: Arc<JobQueue>,
    ) -> Self {
        Self {
            store,
            embedder,
            http_client: HttpClient::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
            job_queue,
            project_store: None,
            token_storage: None,
            token_manager: None,
            token_service: None,
            event_bus: None,
            tool_router: Self::tool_router(),
        }
    }

    /// Enable project management features with the given project store.
    pub fn with_project_store(mut self, project_store: Arc<RwLock<ProjectStore>>) -> Self {
        self.project_store = Some(project_store);
        self
    }

    /// Enable event bus for real-time events.
    pub fn with_event_bus(mut self, event_bus: SharedEventBus) -> Self {
        self.event_bus = Some(event_bus);
        self
    }

    /// Get the event bus if configured.
    pub fn event_bus(&self) -> Option<&SharedEventBus> {
        self.event_bus.as_ref()
    }

    /// Enable GitHub token storage with the given storage backend.
    pub fn with_token_storage(mut self, storage: Arc<dyn TokenStorage>, manager: Arc<GitHubTokenManager>) -> Self {
        self.token_storage = Some(storage);
        self.token_manager = Some(manager);
        self
    }

    /// Create with in-memory token storage (for testing or simple setups).
    pub fn with_in_memory_tokens(mut self, manager: Arc<GitHubTokenManager>) -> Self {
        self.token_storage = Some(Arc::new(InMemoryTokenStorage::default()));
        self.token_manager = Some(manager);
        self
    }

    /// Enable GitHub token service (preferred over raw storage).
    /// Uses the same token service as the REST API.
    pub fn with_token_service(mut self, service: Arc<TokenService>) -> Self {
        self.token_service = Some(service);
        self
    }

    // =========================================================================
    // RETRIEVAL TOOLS
    // =========================================================================

    /// Unified search across all indexed content.
    /// Uses shared service layer for consistency with REST API.
    #[tool(description = "Search the knowledge base using natural language. Returns relevant chunks with scores. Use 'get_context' with page_id to retrieve full page content for RAG synthesis.")]
    async fn search(
        &self,
        params: Parameters<SearchParams>,
    ) -> Result<CallToolResult, McpError> {
        let query = &params.0.query;
        info!("Search: {}", query);

        // Convert local types to shared service types
        let mode = match params.0.mode.clone().unwrap_or_default() {
            SearchMode::Hybrid => kix_services::SearchMode::Hybrid,
            SearchMode::Vector => kix_services::SearchMode::Vector,
            SearchMode::Text => kix_services::SearchMode::Text,
        };

        let filters = params.0.filters.as_ref().map(|f| kix_services::QueryFilters {
            entry_type: f.entry_type.clone(),
            chunk_type: f.chunk_type.clone(),
            tag: f.tag.clone(),
            source_domain: f.source_domain.clone(),
        }).unwrap_or_default();

        let pagination = Pagination::new(params.0.limit, params.0.offset);

        // Use shared service for search
        let results = kix_services::search_knowledge(
            &self.store,
            &self.embedder,
            query,
            mode,
            filters,
            pagination,
        )
        .await
        .map_err(|e| McpError::from(e))?;

        // Convert to MCP response format
        let items: Vec<SearchResultItem> = results
            .results
            .into_iter()
            .map(|r| SearchResultItem {
                chunk_id: r.chunk_id,
                entry_id: r.entry_id,
                page_id: r.page_id,
                text: r.text,
                score: r.score,
                entry_title: r.entry_title,
                source_url: r.source_url,
            })
            .collect();

        let response = SearchResponse {
            results: items,
            total_count: results.total_count,
            has_more: results.has_more,
        };

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// Retrieve full page content for RAG context enrichment.
    #[tool(description = "Retrieve full page content for RAG synthesis. Use page_id from search results, or chunk_id to find the associated page.")]
    async fn get_context(
        &self,
        params: Parameters<GetContextParams>,
    ) -> Result<CallToolResult, McpError> {
        // Validate input - need either page_id or chunk_id
        if params.0.page_id.is_none() && params.0.chunk_id.is_none() {
            return Err(McpError::invalid_params(
                "Either page_id or chunk_id must be provided",
                None,
            ));
        }

        let store = self.store.read().await;

        // Get page_id from chunk if needed
        let page_id = if let Some(ref pid) = params.0.page_id {
            pid.clone()
        } else if let Some(ref chunk_id) = params.0.chunk_id {
            // Parse chunk_id to get entry_id, then look up page
            // Chunk IDs are formatted as "{entry_id}#{chunk_index}"
            let entry_id = chunk_id.split('#').next().unwrap_or(chunk_id);

            // Try to get the page for this entry
            // For now, use the entry_id as page_id (they're often the same)
            entry_id.to_string()
        } else {
            return Err(McpError::invalid_params(
                "Either page_id or chunk_id must be provided",
                None,
            ));
        };

        info!("Get context for page: {}", page_id);

        // Retrieve the page
        let page = store
            .get_page_for_chunk(&page_id)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        match page {
            Some(p) => {
                let context = PageContext {
                    page_id: p.page_id,
                    url: p.url,
                    title: p.title,
                    full_content: p.full_content,
                    content_length: p.content_length as usize,
                    code_block_count: p.code_block_count as usize,
                };

                let json = serde_json::to_string_pretty(&context)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            None => {
                let error = serde_json::json!({
                    "error": "not_found",
                    "message": format!("No page found for ID '{}'", page_id)
                });
                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&error).unwrap(),
                )]))
            }
        }
    }

    /// Get document metadata and optionally all chunks.
    #[tool(description = "Get document metadata by ID. Set include_chunks=true to also retrieve all chunks for the document.")]
    async fn get_document(
        &self,
        params: Parameters<GetDocumentParams>,
    ) -> Result<CallToolResult, McpError> {
        info!("Get document: {}", params.0.id);

        let store = self.store.read().await;

        // Get entry by ID
        let entry = store
            .get_entry_by_id(&params.0.id)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        match entry {
            Some(e) => {
                // Optionally get chunks
                let chunks = if params.0.include_chunks.unwrap_or(false) {
                    // get_chunks_by_entry_id is now async (uses spawn_blocking internally)
                    let chunk_list = store
                        .get_chunks_by_entry_id(&params.0.id)
                        .await
                        .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                    Some(
                        chunk_list
                            .into_iter()
                            .map(|c| ChunkInfo {
                                chunk_id: c.chunk_id,
                                chunk_index: c.chunk_index as i32,
                                chunk_type: Some(c.chunk_type),
                                text: c.text,
                            })
                            .collect(),
                    )
                } else {
                    None
                };

                // Convert EntryRecord to Document
                let tags: Vec<String> = e.tags.as_ref()
                    .and_then(|s| serde_json::from_str(s).ok())
                    .unwrap_or_default();

                let doc = Document {
                    id: e.id,
                    title: e.title,
                    description: e.description.unwrap_or_default(),
                    entry_type: e.entry_type,
                    source_url: Some(e.source_path),
                    source_domain: e.source_domain,
                    tags,
                    created_at: Some(e.created_at),
                    chunks,
                };

                let json = serde_json::to_string_pretty(&doc)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            None => {
                let error = serde_json::json!({
                    "error": "not_found",
                    "message": format!("Document '{}' not found", params.0.id)
                });
                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&error).unwrap(),
                )]))
            }
        }
    }

    // =========================================================================
    // INDEXING TOOLS
    // =========================================================================

    /// Index a single document synchronously.
    #[tool(description = "Index a document from text, file path, or URL. Returns immediately with indexing result.")]
    async fn index(
        &self,
        params: Parameters<IndexParams>,
    ) -> Result<CallToolResult, McpError> {
        info!("Indexing document");

        // Validate content source
        let content = &params.0.content;
        let source_count = [&content.text, &content.file, &content.url]
            .iter()
            .filter(|x| x.is_some())
            .count();

        if source_count == 0 {
            return Err(McpError::invalid_params(
                "Must provide one of: text, file, or url",
                None,
            ));
        }
        if source_count > 1 {
            return Err(McpError::invalid_params(
                "Provide only one of: text, file, or url",
                None,
            ));
        }

        // Process content into Entry
        let mut entry = self
            .process_content(&params.0.content, params.0.title.as_deref())
            .await?;

        // Apply custom ID if provided
        if let Some(ref custom_id) = params.0.id {
            entry.id = custom_id.clone();
        }

        // Apply tags if provided
        if let Some(ref tags) = params.0.tags {
            entry.tags.extend(tags.clone());
            entry.tags.sort();
            entry.tags.dedup();
        }

        // Check for existing entry
        let exists = {
            let store = self.store.read().await;
            store
                .entry_exists(&entry.id)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?
        };

        if exists && !params.0.replace.unwrap_or(false) {
            let result = IndexResult {
                success: false,
                document_id: entry.id.clone(),
                title: entry.title.clone(),
                chunks_created: 0,
                error: Some(format!(
                    "Document '{}' already exists. Set replace=true to overwrite.",
                    entry.id
                )),
            };
            let json = serde_json::to_string_pretty(&result)
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
            return Ok(CallToolResult::success(vec![Content::text(json)]));
        }

        // If replacing, delete existing first
        if exists {
            let store = self.store.write().await;
            // delete_entry handles chunk deletion internally
            store.delete_entry(&entry.id).await.ok();
        }

        // Chunk and embed
        let chunker = DocumentChunker::with_defaults();
        let chunks = chunker.chunk(&entry);

        let embeddings = {
            let mut embedder = self.embedder.write().await;
            let texts: Vec<&str> = chunks.iter().map(|c| c.text.as_str()).collect();
            embedder
                .embed_texts(&texts)
                .map_err(|e| McpError::internal_error(format!("Embedding failed: {}", e), None))?
        };

        // Store entry and chunks
        {
            let store = self.store.write().await;
            // Insert entry using the Entry type converter
            store
                .insert_documents_from_entries(&[entry.clone()])
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
            // insert_chunks is sync (sqlite-vec)
            store
                .insert_chunks(&chunks, &embeddings)
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        }

        let result = IndexResult {
            success: true,
            document_id: entry.id,
            title: entry.title,
            chunks_created: chunks.len(),
            error: None,
        };

        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// Start an async indexing job for crawling or batch operations.
    #[tool(description = "Start an async indexing job for URL crawling or batch file indexing. Returns job_id to check progress with job_status.")]
    async fn index_async(
        &self,
        params: Parameters<IndexAsyncParams>,
    ) -> Result<CallToolResult, McpError> {
        info!("Starting async indexing job");

        // Validate source
        let source = &params.0.source;
        if source.url.is_none() && source.files.is_none() {
            return Err(McpError::invalid_params(
                "Must provide either url or files",
                None,
            ));
        }

        // Determine source type and create job
        let (source_type, estimated_items, job) = if let Some(ref url_source) = source.url {
            // URL crawling job
            let depth = url_source.depth.unwrap_or(1); // Default to 1 level
            let max_pages = url_source.max_pages.unwrap_or(0); // 0 = unlimited/discovery mode
            let respect_robots = url_source.respect_robots.unwrap_or(true);
            let render_js = url_source.render_js.unwrap_or(true);
            let timeout_secs = url_source.timeout_secs.unwrap_or(30);
            let priority = url_source.priority.unwrap_or(5);

            let mut config = JobConfig::default();
            config.priority = priority;

            let job = Job::new(
                JobType::Url {
                    url: url_source.url.clone(),
                    depth,
                    respect_robots,
                    render_js,
                    timeout_secs,
                    max_pages,
                },
                config,
            );

            // estimated_items is None when max_pages=0 (unlimited)
            let estimated = if max_pages > 0 { Some(max_pages) } else { None };
            ("url".to_string(), estimated, job)
        } else if let Some(ref files) = source.files {
            // File indexing job
            let file_paths: Vec<std::path::PathBuf> = files.iter().map(std::path::PathBuf::from).collect();
            let file_names: Vec<String> = files.iter().map(|p| {
                std::path::Path::new(p)
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| p.clone())
            }).collect();

            let job = Job::new(
                JobType::FileUpload {
                    file_paths,
                    file_names: file_names.clone(),
                    extract_archives: true,
                },
                JobConfig::default(),
            );

            ("files".to_string(), Some(files.len()), job)
        } else {
            return Err(McpError::invalid_params(
                "Must provide either url or files",
                None,
            ));
        };

        let job_id = job.id;

        // Submit job to queue
        self.job_queue
            .submit(job)
            .await
            .map_err(|e| McpError::internal_error(format!("Failed to queue job: {}", e), None))?;

        info!(job_id = %job_id, source_type = %source_type, "Job submitted to queue");

        let response = JobCreated {
            job_id: job_id.to_string(),
            status: "queued".to_string(),
            source_type,
            estimated_items,
        };

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// Check the status of an async indexing job.
    #[tool(description = "Check the progress of an async indexing job started with index_async.")]
    async fn job_status(
        &self,
        params: Parameters<JobStatusParams>,
    ) -> Result<CallToolResult, McpError> {
        info!("Checking job status: {}", params.0.job_id);

        // Parse job ID
        let job_id = Uuid::parse_str(&params.0.job_id)
            .map_err(|_| McpError::invalid_params("Invalid job ID format", None))?;

        // Look up job from queue
        let response = if let Some(job) = self.job_queue.get(job_id) {
            let state = job.get_state().await;

            let (status, progress, result, error) = match state {
                JobState::Pending { .. } => {
                    ("pending".to_string(), None, None, None)
                }
                JobState::Queued { .. } => {
                    ("queued".to_string(), None, None, None)
                }
                JobState::Running { .. } => {
                    // For running jobs, we could add progress tracking if available
                    ("running".to_string(), None, None, None)
                }
                JobState::Completed { result: job_result, .. } => {
                    let result = JobResult {
                        documents_created: job_result.items_processed,
                        chunks_created: job_result.chunks_created,
                        errors: job_result.errors,
                    };
                    ("completed".to_string(), None, Some(result), None)
                }
                JobState::Failed { error, .. } => {
                    ("failed".to_string(), None, None, Some(error))
                }
                JobState::Cancelled { reason, .. } => {
                    ("cancelled".to_string(), None, None, Some(reason))
                }
            };

            JobStatusResponse {
                job_id: params.0.job_id.clone(),
                status,
                progress,
                result,
                error,
            }
        } else {
            // Job not found in active queue
            JobStatusResponse {
                job_id: params.0.job_id.clone(),
                status: "not_found".to_string(),
                progress: None,
                result: None,
                error: Some("Job not found in active queue. It may have expired or never existed.".to_string()),
            }
        };

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// Delete documents by ID or filter.
    #[tool(description = "Delete documents by ID, multiple IDs, or filter (tag/source_domain). Use dry_run=true to preview.")]
    async fn delete(
        &self,
        params: Parameters<DeleteParams>,
    ) -> Result<CallToolResult, McpError> {
        info!("Delete documents");

        let dry_run = params.0.dry_run.unwrap_or(false);

        // Collect IDs to delete
        let mut ids_to_delete: Vec<String> = Vec::new();

        // Single ID
        if let Some(ref id) = params.0.id {
            ids_to_delete.push(id.clone());
        }

        // Multiple IDs
        if let Some(ref ids) = params.0.ids {
            ids_to_delete.extend(ids.clone());
        }

        // Filter-based deletion
        if let Some(ref filter) = params.0.filter {
            let store = self.store.read().await;

            if let Some(ref tag) = filter.tag {
                // List all entries and filter by tag (tags stored as JSON array)
                let entries = store
                    .list_all_entries()
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                ids_to_delete.extend(
                    entries
                        .into_iter()
                        .filter(|e| {
                            e.tags.as_ref()
                                .and_then(|t| serde_json::from_str::<Vec<String>>(t).ok())
                                .map(|tags| tags.contains(tag))
                                .unwrap_or(false)
                        })
                        .map(|e| e.id),
                );
            }

            if let Some(ref domain) = filter.source_domain {
                // List all entries and filter by domain
                let entries = store
                    .list_all_entries()
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                ids_to_delete.extend(
                    entries
                        .into_iter()
                        .filter(|e| e.source_domain.as_deref() == Some(domain.as_str()))
                        .map(|e| e.id),
                );
            }
        }

        // Deduplicate
        ids_to_delete.sort();
        ids_to_delete.dedup();

        if ids_to_delete.is_empty() {
            let result = DeleteResult {
                success: true,
                documents_deleted: 0,
                chunks_deleted: 0,
                deleted_ids: vec![],
                dry_run,
            };
            let json = serde_json::to_string_pretty(&result)
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
            return Ok(CallToolResult::success(vec![Content::text(json)]));
        }

        // Count chunks that would be deleted (for dry run info)
        let mut chunks_deleted = 0;
        let mut actually_deleted: Vec<String> = Vec::new();

        if dry_run {
            // Just count what would be deleted
            let store = self.store.read().await;
            for id in &ids_to_delete {
                if store.entry_exists(id).await.unwrap_or(false) {
                    // get_chunks_by_entry_id is now async (uses spawn_blocking internally)
                    let chunks = store.get_chunks_by_entry_id(id).await.unwrap_or_default();
                    chunks_deleted += chunks.len();
                    actually_deleted.push(id.clone());
                }
            }
        } else {
            // Actually delete
            let store = self.store.write().await;
            for id in &ids_to_delete {
                if store.entry_exists(id).await.unwrap_or(false) {
                    // get_chunks_by_entry_id is now async (uses spawn_blocking internally)
                    let chunks = store.get_chunks_by_entry_id(id).await.unwrap_or_default();
                    chunks_deleted += chunks.len();

                    // delete_entry handles chunk deletion internally
                    store.delete_entry(id).await.ok();
                    actually_deleted.push(id.clone());
                }
            }
        }

        let result = DeleteResult {
            success: true,
            documents_deleted: actually_deleted.len(),
            chunks_deleted,
            deleted_ids: actually_deleted,
            dry_run,
        };

        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    // =========================================================================
    // STATUS TOOL
    // =========================================================================

    /// Get index health and statistics.
    #[tool(description = "Get index health and statistics. Set detailed=true for breakdown by type and domain.")]
    async fn status(
        &self,
        params: Parameters<StatusParams>,
    ) -> Result<CallToolResult, McpError> {
        info!("Getting index status");

        let store = self.store.read().await;

        let document_count = store
            .entry_count()
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        // chunk_count is sync (sqlite-vec)
        let chunk_count = store
            .chunk_count()
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let page_count = store
            .page_count()
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let health = if document_count == 0 {
            "empty"
        } else if chunk_count == 0 {
            "degraded"
        } else {
            "healthy"
        }
        .to_string();

        // Build breakdown if detailed
        let breakdown = if params.0.detailed.unwrap_or(false) {
            let entries = store
                .list_all_entries()
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;

            let mut by_type: HashMap<String, usize> = HashMap::new();
            let mut by_domain: HashMap<String, usize> = HashMap::new();

            for entry in entries {
                *by_type.entry(entry.entry_type).or_insert(0) += 1;
                if let Some(domain) = entry.source_domain {
                    *by_domain.entry(domain).or_insert(0) += 1;
                }
            }

            Some(StatusBreakdown { by_type, by_domain })
        } else {
            None
        };

        let status = IndexStatus {
            health,
            document_count,
            chunk_count,
            page_count,
            breakdown,
        };

        let json = serde_json::to_string_pretty(&status)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    // =========================================================================
    // PROJECT MANAGEMENT TOOLS
    // =========================================================================

    /// Helper to get project store or return error.
    fn require_project_store(&self) -> Result<&Arc<RwLock<ProjectStore>>, McpError> {
        self.project_store.as_ref().ok_or_else(|| {
            McpError::internal_error("Project management not enabled. Initialize server with project store.".to_string(), None)
        })
    }

    /// Helper to get token storage and manager or return error.
    fn require_token_storage(&self) -> Result<(&Arc<dyn TokenStorage>, &Arc<GitHubTokenManager>), McpError> {
        let storage = self.token_storage.as_ref().ok_or_else(|| {
            McpError::internal_error("Token storage not configured.".to_string(), None)
        })?;
        let manager = self.token_manager.as_ref().ok_or_else(|| {
            McpError::internal_error("Token manager not configured.".to_string(), None)
        })?;
        Ok((storage, manager))
    }

    /// Helper to get a GitHub sync service for a project.
    /// Prefers token_service if configured, falls back to legacy token_storage.
    async fn get_sync_service(&self, project_id: Option<&str>) -> Result<GitHubSyncService, McpError> {
        // Prefer token_service if configured (same as REST API)
        if let Some(token_service) = &self.token_service {
            let token = if let Some(pid) = project_id {
                token_service.get_token_for_project(pid).await
            } else {
                token_service.get_global_token_decrypted().await
            };

            let token = token.map_err(|e| McpError::internal_error(
                format!("Failed to get GitHub token: {}", e), None
            ))?;

            return GitHubSyncService::new(&token)
                .map_err(|e| McpError::internal_error(e.to_string(), None));
        }

        // Fall back to legacy token storage
        let (storage, manager) = self.require_token_storage()?;
        let token = kix_projects::get_token_with_fallback(storage, manager.as_ref(), project_id)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        GitHubSyncService::new(token)
            .map_err(|e| McpError::internal_error(e.to_string(), None))
    }

    /// Create a new project with required GitHub repository connection.
    #[tool(description = "Create a new project. Projects must be connected to a GitHub repository for issue tracking.")]
    async fn create_project(
        &self,
        params: Parameters<CreateProjectParams>,
    ) -> Result<CallToolResult, McpError> {
        info!("Creating project: {}", params.0.name);
        let project_store = self.require_project_store()?;

        // Parse template
        let template = match params.0.template.as_str() {
            "kanban" => ProjectTemplate::Kanban,
            "bug_tracking" => ProjectTemplate::BugTracking,
            "sprint_planning" => ProjectTemplate::SprintPlanning,
            "feature_roadmap" => ProjectTemplate::FeatureRoadmap,
            _ => {
                return Ok(CallToolResult::success(vec![Content::text(format!(
                    "Invalid template '{}'. Choose: kanban, bug_tracking, sprint_planning, feature_roadmap",
                    params.0.template
                ))]));
            }
        };

        // Create project record with GitHub config
        let mut project = ProjectRecord::new(
            params.0.name.clone(),
            params.0.github_owner.clone(),
            params.0.github_repo.clone(),
        );

        if let Some(desc) = &params.0.description {
            project = project.with_description(desc.clone());
        }
        if let Some(color) = &params.0.color {
            project = project.with_color(color.clone());
        }

        // Configure sync in github_config JSON
        if params.0.auto_sync.is_some() || params.0.sync_direction.is_some() {
            if let Ok(mut config) = serde_json::from_str::<serde_json::Value>(&project.github_config) {
                if let Some(sync) = config.get_mut("sync") {
                    if let Some(auto_sync) = params.0.auto_sync {
                        sync["enabled"] = serde_json::json!(auto_sync);
                    }
                    if let Some(direction) = &params.0.sync_direction {
                        sync["direction"] = serde_json::json!(direction);
                    }
                }
                project.github_config = config.to_string();
            }
        }

        // Store project
        {
            let store = project_store.write().await;
            store.create_project(&project).await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        }

        let github_url = format!(
            "https://github.com/{}/{}",
            params.0.github_owner, params.0.github_repo
        );

        // Create GitHub Project V2
        let mut github_project_url: Option<String> = None;
        let mut warning: Option<String> = None;

        // Get token for GitHub API using fallback pattern
        let github_token = self.require_token_storage()
            .and_then(|(storage, manager)| {
                kix_projects::get_token_with_fallback(storage, manager.as_ref(), Some(&project.id))
                    .map_err(|e| McpError::internal_error(e.to_string(), None))
            })
            .ok();

        if let Some(token) = github_token {
            match ProjectV2Service::new(&token) {
                Ok(v2_service) => {
                    info!(
                        "Creating GitHub Project V2 '{}' with {} template for {}/{}",
                        project.name, template, params.0.github_owner, params.0.github_repo
                    );

                    match v2_service
                        .create_project_with_template(
                            &params.0.github_owner,
                            &params.0.github_repo,
                            &project.name,
                            template,
                        )
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

                            let store = project_store.write().await;
                            if let Err(e) = store.update_project(&updated_project).await {
                                warning = Some(format!("Project V2 created but config not saved: {}", e));
                            }
                        }
                        Err(e) => {
                            warning = Some(format!("Failed to create GitHub Project V2: {}", e));
                        }
                    }
                }
                Err(e) => {
                    warning = Some(format!("Failed to initialize GitHub GraphQL client: {}", e));
                }
            }
        } else {
            warning = Some("No GitHub token available. Project V2 not created.".to_string());
        }

        // Emit event
        if let Some(bus) = &self.event_bus {
            bus.project_created(&project.id, &project.name);
        }

        let response = CreateProjectResponse {
            success: true,
            project_id: project.id.clone(),
            name: project.name.clone(),
            slug: project.slug.clone(),
            github_url,
            github_project_url,
            warning,
            error: None,
        };

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// List all projects.
    #[tool(description = "List all projects. Use include_archived=true to include archived projects.")]
    async fn list_projects(
        &self,
        params: Parameters<ListProjectsParams>,
    ) -> Result<CallToolResult, McpError> {
        info!("Listing projects");
        let project_store = self.require_project_store()?;

        let include_archived = params.0.include_archived.unwrap_or(false);
        let limit = params.0.limit.unwrap_or(50);
        let offset = params.0.offset.unwrap_or(0);

        let projects = {
            let store = project_store.read().await;
            store.list_projects(include_archived).await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?
        };

        let total = projects.len();
        let paginated: Vec<_> = projects.into_iter().skip(offset).take(limit).collect();
        let has_more = total > offset + limit;

        // Convert to summaries (we'd need issue counts from a join or separate query)
        let summaries: Vec<ProjectSummary> = paginated.into_iter().map(|p| {
            let github_owner = p.github_owner().unwrap_or_default().to_string();
            let github_repo = p.github_repo().unwrap_or_default().to_string();
            let is_archived = p.is_archived();
            ProjectSummary {
                id: p.id,
                name: p.name,
                slug: p.slug,
                description: p.description,
                color: p.color,
                github_owner,
                github_repo,
                archived: is_archived,
                open_issues: 0, // Would need separate query
                closed_issues: 0,
                created_at: p.created_at,
            }
        }).collect();

        let response = ListProjectsResponse {
            projects: summaries,
            total,
            has_more,
        };

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// Get a project by ID or slug.
    #[tool(description = "Get detailed information about a project by ID or slug.")]
    async fn get_project(
        &self,
        params: Parameters<GetProjectParams>,
    ) -> Result<CallToolResult, McpError> {
        info!("Getting project: {}", params.0.project);
        let project_store = self.require_project_store()?;

        let store = project_store.read().await;
        let project = store.get_project(&params.0.project).await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        match project {
            Some(p) => {
                // Get stats if requested
                let stats = if params.0.include_stats.unwrap_or(true) {
                    // list_issues takes 4 args: project_id, state_filter, limit, offset
                    let issues = store.list_issues(&p.id, None, 10000, 0).await.unwrap_or_default();
                    let open_count = issues.iter().filter(|i| i.state == "open").count();
                    let closed_count = issues.len() - open_count;
                    let entries = store.list_project_entries(&p.id).await.unwrap_or_default();
                    Some(ProjectStats {
                        open_issues: open_count,
                        closed_issues: closed_count,
                        total_issues: issues.len(),
                        linked_entries: entries.len(),
                    })
                } else {
                    None
                };

                // Get GitHub Project info if linked (github_config is a String, parse it)
                let github_project = serde_json::from_str::<serde_json::Value>(&p.github_config)
                    .ok()
                    .and_then(|config| {
                        let pv2 = config.get("project_v2")?;
                        let node_id = pv2.get("project_id")?.as_str()?.to_string();
                        let number = pv2.get("project_number")?.as_u64()? as u32;
                        let title = pv2.get("title").and_then(|t| t.as_str()).unwrap_or("").to_string();
                        let owner = p.github_owner().unwrap_or_default();
                        Some(GitHubProjectInfo {
                            node_id,
                            number,
                            title,
                            url: format!("https://github.com/orgs/{}/projects/{}", owner, number),
                        })
                    });

                let github_owner = p.github_owner().unwrap_or_default().to_string();
                let github_repo = p.github_repo().unwrap_or_default().to_string();
                let is_archived = p.is_archived();

                let response = ProjectDetail {
                    id: p.id,
                    name: p.name,
                    slug: p.slug,
                    description: p.description,
                    color: p.color,
                    github_owner: github_owner.clone(),
                    github_repo: github_repo.clone(),
                    github_url: format!("https://github.com/{}/{}", github_owner, github_repo),
                    archived: is_archived,
                    created_at: p.created_at.clone(),
                    updated_at: p.updated_at.clone(),
                    stats,
                    github_project,
                };

                let json = serde_json::to_string_pretty(&response)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            None => {
                let error = serde_json::json!({
                    "error": "not_found",
                    "message": format!("Project '{}' not found", params.0.project)
                });
                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&error).unwrap(),
                )]))
            }
        }
    }

    /// Update a project.
    #[tool(description = "Update project properties like name, description, color, or archived status.")]
    async fn update_project(
        &self,
        params: Parameters<UpdateProjectParams>,
    ) -> Result<CallToolResult, McpError> {
        info!("Updating project: {}", params.0.project);
        let project_store = self.require_project_store()?;

        let store = project_store.write().await;

        // Get existing project
        let existing = store.get_project(&params.0.project).await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        match existing {
            Some(mut project) => {
                // Apply updates (archived is i64: 0 = false, non-0 = true)
                let was_archived = project.is_archived();
                if let Some(name) = &params.0.name {
                    project.name = name.clone();
                    // Create simple slug
                    project.slug = name.to_lowercase()
                        .chars()
                        .map(|c| if c.is_alphanumeric() { c } else { '-' })
                        .collect();
                }
                if let Some(desc) = &params.0.description {
                    project.description = Some(desc.clone());
                }
                if let Some(color) = &params.0.color {
                    project.color = Some(color.clone());
                }
                if let Some(archived) = params.0.archived {
                    project.archived = if archived { 1 } else { 0 };
                }
                project.updated_at = chrono::Utc::now().to_rfc3339();

                let project_id = project.id.clone();
                let now_archived = project.is_archived();

                store.update_project(&project).await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                drop(store); // Release lock before emitting event

                // Emit appropriate event
                if let Some(bus) = &self.event_bus {
                    if !was_archived && now_archived {
                        bus.project_archived(&project_id);
                    } else if was_archived && !now_archived {
                        bus.project_unarchived(&project_id);
                    } else {
                        bus.project_updated(&project_id);
                    }
                }

                let response = UpdateProjectResponse {
                    success: true,
                    project_id,
                    error: None,
                };

                let json = serde_json::to_string_pretty(&response)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            None => {
                let response = UpdateProjectResponse {
                    success: false,
                    project_id: String::new(),
                    error: Some(format!("Project '{}' not found", params.0.project)),
                };

                let json = serde_json::to_string_pretty(&response)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
        }
    }

    /// Delete a project.
    #[tool(description = "Delete a project and its local issues/entries. Does not delete GitHub issues.")]
    async fn delete_project(
        &self,
        params: Parameters<DeleteProjectParams>,
    ) -> Result<CallToolResult, McpError> {
        info!("Deleting project: {}", params.0.project);
        let project_store = self.require_project_store()?;

        let store = project_store.write().await;

        // Get project first to count what will be deleted
        let project = store.get_project(&params.0.project).await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        match project {
            Some(p) => {
                // Count issues and entries before deletion
                let project_id = p.id.clone();
                let issues_deleted = store.list_issues(&p.id, None, 10000, 0).await
                    .map(|i| i.len())
                    .unwrap_or(0);
                let entries_unlinked = store.list_project_entries(&p.id).await
                    .map(|e| e.len())
                    .unwrap_or(0);

                // Delete project (cascade deletes issues and entries)
                store.delete_project(&p.id).await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                drop(store); // Release lock before emitting event

                // Emit event
                if let Some(bus) = &self.event_bus {
                    bus.project_deleted(&project_id);
                }

                let response = DeleteProjectResponse {
                    success: true,
                    issues_deleted,
                    entries_unlinked,
                    error: None,
                };

                let json = serde_json::to_string_pretty(&response)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            None => {
                let response = DeleteProjectResponse {
                    success: false,
                    issues_deleted: 0,
                    entries_unlinked: 0,
                    error: Some(format!("Project '{}' not found", params.0.project)),
                };

                let json = serde_json::to_string_pretty(&response)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
        }
    }

    // =========================================================================
    // ISSUE CRUD TOOLS
    // =========================================================================

    /// Create a new issue in a project.
    #[tool(description = "Create a new issue in a project. When GitHub integration is configured, the issue is ALWAYS created on GitHub first. If GitHub fails, the operation fails.")]
    async fn create_issue(
        &self,
        params: Parameters<CreateIssueParams>,
    ) -> Result<CallToolResult, McpError> {
        info!("Creating issue in project: {}", params.0.project);
        let project_store = self.require_project_store()?;

        let store = project_store.write().await;

        // Get project
        let project = store.get_project(&params.0.project).await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .ok_or_else(|| McpError::invalid_params(
                format!("Project '{}' not found", params.0.project),
                None,
            ))?;

        let owner = project.github_owner().unwrap_or_default();
        let repo = project.github_repo().unwrap_or_default();
        let has_github_config = !owner.is_empty() && !repo.is_empty();

        // If GitHub is configured, ALWAYS create there FIRST - fail if it fails
        if has_github_config {
            // Get sync service - fail if not available
            let sync_service = match self.get_sync_service(Some(&project.id)).await {
                Ok(service) => service,
                Err(e) => {
                    let response = CreateIssueResponse {
                        success: false,
                        issue_id: String::new(),
                        number: 0,
                        title: params.0.title.clone(),
                        github_url: None,
                        error: Some(format!("GitHub sync service unavailable: {}", e)),
                    };
                    let json = serde_json::to_string_pretty(&response)
                        .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                    return Ok(CallToolResult::success(vec![Content::text(json)]));
                }
            };

            let issue_info = IssueInfo {
                id: uuid::Uuid::new_v4().to_string(),
                title: params.0.title.clone(),
                body: params.0.body.clone(),
                state: IssueState::Open,
                labels: params.0.labels.clone().unwrap_or_default(),
                assignees: params.0.assignees.clone().unwrap_or_default(),
                github_number: None,
                github_node_id: None,
                source: IssueSource::Mcp,
            };

            // Create on GitHub FIRST - fail if it fails
            let gh_issue = match sync_service.push_issue(&owner, &repo, &issue_info).await {
                Ok(issue) => {
                    info!("Created GitHub issue #{} for project {}", issue.number, project.id);
                    issue
                }
                Err(e) => {
                    warn!("Failed to create issue on GitHub: {}", e);
                    let response = CreateIssueResponse {
                        success: false,
                        issue_id: String::new(),
                        number: 0,
                        title: params.0.title.clone(),
                        github_url: None,
                        error: Some(format!("Failed to create on GitHub: {}", e)),
                    };
                    let json = serde_json::to_string_pretty(&response)
                        .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                    return Ok(CallToolResult::success(vec![Content::text(json)]));
                }
            };

            // GitHub succeeded - now create locally with GitHub's number
            let mut issue = IssueRecord::new(project.id.clone(), gh_issue.number, params.0.title.clone());
            if let Some(body) = &params.0.body {
                issue = issue.with_body(body.clone());
            }
            if let Some(labels) = &params.0.labels {
                issue = issue.with_labels(labels.clone());
            }
            if let Some(assignees) = &params.0.assignees {
                issue = issue.with_assignees(assignees.clone());
            }
            issue.github_number = Some(gh_issue.number as i64);
            issue.github_node_id = Some(gh_issue.node_id.clone());
            issue.github_url = Some(gh_issue.html_url.clone());
            issue.source = "github".to_string();

            // Add issue to GitHub Project V2 if configured (non-blocking)
            if let Some(project_v2_id) = project.github_project_v2_id() {
                if let Some(token_service) = &self.token_service {
                    if let Ok(token) = token_service.get_token_for_project(&project.id).await {
                        if let Ok(v2_service) = ProjectV2Service::new(&token) {
                            match v2_service.add_issue_to_project(&project_v2_id, &gh_issue.node_id).await {
                                Ok(item_id) => {
                                    info!("Added issue #{} to GitHub Project V2: item_id={}", gh_issue.number, item_id);
                                    issue.github_project_item_id = Some(item_id);
                                }
                                Err(e) => {
                                    info!("Failed to add issue to Project V2: {}", e);
                                }
                            }
                        }
                    }
                }
            }

            let issue_id = issue.id.clone();
            let issue_title = issue.title.clone();
            let project_id = project.id.clone();
            let issue_number = gh_issue.number;
            let github_url = Some(gh_issue.html_url);

            store.create_issue(&issue).await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;

            drop(store);

            if let Some(bus) = &self.event_bus {
                bus.issue_created(&project_id, &issue_id, &issue_title);
            }

            let response = CreateIssueResponse {
                success: true,
                issue_id,
                number: issue_number as u32,
                title: issue_title,
                github_url,
                error: None,
            };

            let json = serde_json::to_string_pretty(&response)
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;

            return Ok(CallToolResult::success(vec![Content::text(json)]));
        }

        // No GitHub config - create locally only
        let issue_number = store.next_issue_number(&project.id).await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let mut issue = IssueRecord::new(project.id.clone(), issue_number, params.0.title.clone());
        if let Some(body) = &params.0.body {
            issue = issue.with_body(body.clone());
        }
        if let Some(labels) = &params.0.labels {
            issue = issue.with_labels(labels.clone());
        }
        if let Some(assignees) = &params.0.assignees {
            issue = issue.with_assignees(assignees.clone());
        }
        issue.source = "mcp".to_string();

        let issue_id = issue.id.clone();
        let issue_title = issue.title.clone();
        let project_id = project.id.clone();

        store.create_issue(&issue).await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        drop(store);

        if let Some(bus) = &self.event_bus {
            bus.issue_created(&project_id, &issue_id, &issue_title);
        }

        let response = CreateIssueResponse {
            success: true,
            issue_id,
            number: issue_number as u32,
            title: issue_title,
            github_url: None,
            error: None,
        };

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// List issues in a project.
    #[tool(description = "List issues in a project with optional filters for state, labels, assignee, and search.")]
    async fn list_issues(
        &self,
        params: Parameters<ListIssuesParams>,
    ) -> Result<CallToolResult, McpError> {
        info!("Listing issues for project: {}", params.0.project);
        let project_store = self.require_project_store()?;

        let store = project_store.read().await;

        // Get project
        let project = store.get_project(&params.0.project).await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .ok_or_else(|| McpError::invalid_params(
                format!("Project '{}' not found", params.0.project),
                None,
            ))?;

        // State filter (pass as Option<&str>)
        let state_filter = params.0.state.as_deref();

        let limit = params.0.limit.unwrap_or(50);
        let offset = params.0.offset.unwrap_or(0);

        // list_issues takes (project_id, state, limit, offset) - fetch more for filtering
        let issues = store.list_issues(&project.id, state_filter, 10000, 0).await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        // Apply additional filters in memory (labels, assignee, search)
        let filtered: Vec<_> = issues.into_iter().filter(|i| {
            // Label filter (labels is Option<String> JSON)
            if let Some(labels) = &params.0.labels {
                let issue_labels = i.labels_vec();
                if !labels.iter().any(|l| issue_labels.contains(l)) {
                    return false;
                }
            }
            // Assignee filter (assignees is Option<String> JSON)
            if let Some(assignee) = &params.0.assignee {
                if !i.assignees_vec().contains(assignee) {
                    return false;
                }
            }
            // Search filter
            if let Some(search) = &params.0.search {
                let search_lower = search.to_lowercase();
                if !i.title.to_lowercase().contains(&search_lower) &&
                   !i.body.as_ref().map(|b| b.to_lowercase().contains(&search_lower)).unwrap_or(false) {
                    return false;
                }
            }
            true
        }).collect();

        let total = filtered.len();
        let paginated: Vec<_> = filtered.into_iter().skip(offset).take(limit).collect();
        let has_more = total > offset + limit;

        let github_owner = project.github_owner().unwrap_or_default();
        let github_repo = project.github_repo().unwrap_or_default();

        let summaries: Vec<IssueSummary> = paginated.into_iter().map(|i| {
            let github_url = i.github_number.map(|n| {
                format!("https://github.com/{}/{}/issues/{}", github_owner, github_repo, n)
            });
            let labels_list = i.labels_vec();
            let assignees_list = i.assignees_vec();

            IssueSummary {
                id: i.id,
                number: i.number as u32,
                title: i.title,
                state: i.state,
                labels: labels_list,
                assignees: if assignees_list.is_empty() { None } else { Some(assignees_list) },
                github_url,
                created_at: i.created_at,
                updated_at: i.updated_at,
            }
        }).collect();

        let response = ListIssuesResponse {
            issues: summaries,
            total,
            has_more,
        };

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// Get a specific issue.
    #[tool(description = "Get detailed information about a specific issue by number or ID.")]
    async fn get_issue(
        &self,
        params: Parameters<GetIssueParams>,
    ) -> Result<CallToolResult, McpError> {
        info!("Getting issue {} in project {}", params.0.issue, params.0.project);
        let project_store = self.require_project_store()?;

        let store = project_store.read().await;

        // Get project
        let project = store.get_project(&params.0.project).await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .ok_or_else(|| McpError::invalid_params(
                format!("Project '{}' not found", params.0.project),
                None,
            ))?;

        // Try to parse as number first, then as ID
        let issue = if let Ok(num) = params.0.issue.parse::<u32>() {
            store.get_issue_by_number(&project.id, num).await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?
        } else {
            store.get_issue(&params.0.issue).await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?
        };

        match issue {
            Some(i) => {
                let github_owner = project.github_owner().unwrap_or_default();
                let github_repo = project.github_repo().unwrap_or_default();
                let github_url = i.github_number.map(|n| {
                    format!("https://github.com/{}/{}/issues/{}", github_owner, github_repo, n)
                });

                let labels_list = i.labels_vec();
                let assignees_list = i.assignees_vec();
                let response = IssueDetail {
                    id: i.id,
                    project_id: i.project_id,
                    number: i.number as u32,
                    title: i.title,
                    body: i.body,
                    state: i.state,
                    labels: labels_list,
                    assignees: if assignees_list.is_empty() { None } else { Some(assignees_list) },
                    github_number: i.github_number.map(|n| n as u32),
                    github_url,
                    source: i.source,
                    created_at: i.created_at,
                    updated_at: i.updated_at,
                };

                let json = serde_json::to_string_pretty(&response)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            None => {
                let error = serde_json::json!({
                    "error": "not_found",
                    "message": format!("Issue '{}' not found in project '{}'", params.0.issue, params.0.project)
                });
                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&error).unwrap(),
                )]))
            }
        }
    }

    /// Update an issue.
    #[tool(description = "Update an issue's title, body, state, labels, or assignees. When GitHub is configured, updates GitHub first. If GitHub fails, the operation fails.")]
    async fn update_issue(
        &self,
        params: Parameters<UpdateIssueParams>,
    ) -> Result<CallToolResult, McpError> {
        info!("Updating issue {} in project {}", params.0.issue, params.0.project);
        let project_store = self.require_project_store()?;

        let store = project_store.write().await;

        // Get project
        let project = store.get_project(&params.0.project).await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .ok_or_else(|| McpError::invalid_params(
                format!("Project '{}' not found", params.0.project),
                None,
            ))?;

        // Get issue
        let issue = if let Ok(num) = params.0.issue.parse::<u32>() {
            store.get_issue_by_number(&project.id, num).await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?
        } else {
            store.get_issue(&params.0.issue).await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?
        };

        match issue {
            Some(mut i) => {
                let was_open = i.state != "closed";
                let owner = project.github_owner().unwrap_or_default();
                let repo = project.github_repo().unwrap_or_default();
                let has_github_config = !owner.is_empty() && !repo.is_empty();

                // If GitHub is configured AND issue exists on GitHub, ALWAYS update there FIRST
                if has_github_config && i.github_number.is_some() {
                    let gh_num = i.github_number.unwrap() as u32;

                    // Get sync service - fail if not available
                    let sync_service = match self.get_sync_service(Some(&project.id)).await {
                        Ok(service) => service,
                        Err(e) => {
                            let response = UpdateIssueResponse {
                                success: false,
                                issue_id: i.id.clone(),
                                synced_to_github: false,
                                error: Some(format!("GitHub sync service unavailable: {}", e)),
                            };
                            let json = serde_json::to_string_pretty(&response)
                                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                            return Ok(CallToolResult::success(vec![Content::text(json)]));
                        }
                    };

                    // Build update request with GitHub models
                    let github_req = kix_projects::github::models::UpdateIssueRequest {
                        title: params.0.title.clone(),
                        body: params.0.body.clone(),
                        state: params.0.state.clone(),
                        labels: params.0.labels.clone(),
                        assignees: params.0.assignees.clone(),
                    };

                    // Update on GitHub FIRST - fail if it fails
                    match sync_service.rest().update_issue(&owner, &repo, gh_num, &github_req).await {
                        Ok(gh_issue) => {
                            info!("Updated GitHub issue #{} for project {}", gh_num, project.id);
                            // Update local issue with data from GitHub response
                            i.title = gh_issue.title;
                            i.body = gh_issue.body;
                            i.state = gh_issue.state.clone();
                            i.set_labels(gh_issue.labels.iter().map(|l| l.name.clone()).collect());
                            i.set_assignees(gh_issue.assignees.iter().map(|u| u.login.clone()).collect());
                        }
                        Err(e) => {
                            warn!("Failed to update issue on GitHub: {}", e);
                            let response = UpdateIssueResponse {
                                success: false,
                                issue_id: i.id.clone(),
                                synced_to_github: false,
                                error: Some(format!("Failed to update on GitHub: {}", e)),
                            };
                            let json = serde_json::to_string_pretty(&response)
                                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                            return Ok(CallToolResult::success(vec![Content::text(json)]));
                        }
                    }
                } else {
                    // No GitHub config or issue not on GitHub - update locally only
                    if let Some(title) = &params.0.title {
                        i.title = title.clone();
                    }
                    if let Some(body) = &params.0.body {
                        i.body = Some(body.clone());
                    }
                    if let Some(state) = &params.0.state {
                        i.state = state.clone();
                    }
                    if let Some(labels) = &params.0.labels {
                        i.set_labels(labels.clone());
                    }
                    if let Some(assignees) = &params.0.assignees {
                        i.set_assignees(assignees.clone());
                    }
                }

                i.updated_at = chrono::Utc::now().to_rfc3339();
                let is_now_closed = i.state == "closed";
                let issue_id = i.id.clone();
                let project_id = project.id.clone();
                let synced = has_github_config && i.github_number.is_some();

                store.update_issue(&i).await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                drop(store); // Release lock before emitting event

                // Emit appropriate event
                if let Some(bus) = &self.event_bus {
                    if was_open && is_now_closed {
                        bus.issue_closed(&project_id, &issue_id);
                    } else if !was_open && !is_now_closed {
                        bus.issue_reopened(&project_id, &issue_id);
                    } else {
                        bus.issue_updated(&project_id, &issue_id);
                    }
                }

                let response = UpdateIssueResponse {
                    success: true,
                    issue_id,
                    synced_to_github: synced,
                    error: None,
                };

                let json = serde_json::to_string_pretty(&response)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            None => {
                let response = UpdateIssueResponse {
                    success: false,
                    issue_id: String::new(),
                    synced_to_github: false,
                    error: Some(format!("Issue '{}' not found", params.0.issue)),
                };

                let json = serde_json::to_string_pretty(&response)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
        }
    }

    /// Delete an issue from Kix. When the issue exists on GitHub, closes it there first.
    #[tool(description = "Delete an issue from Kix. When GitHub is configured and the issue exists there, closes it on GitHub first (GitHub doesn't allow deleting issues). If GitHub close fails, the operation fails.")]
    async fn delete_issue(
        &self,
        params: Parameters<DeleteIssueParams>,
    ) -> Result<CallToolResult, McpError> {
        info!("Deleting issue {} from project {}", params.0.issue, params.0.project);
        let project_store = self.require_project_store()?;

        let store = project_store.write().await;

        // Get project
        let project = store.get_project(&params.0.project).await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .ok_or_else(|| McpError::invalid_params(
                format!("Project '{}' not found", params.0.project),
                None,
            ))?;

        // Get issue
        let issue = if let Ok(num) = params.0.issue.parse::<u32>() {
            store.get_issue_by_number(&project.id, num).await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?
        } else {
            store.get_issue(&params.0.issue).await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?
        };

        match issue {
            Some(i) => {
                let owner = project.github_owner().unwrap_or_default();
                let repo = project.github_repo().unwrap_or_default();
                let has_github_config = !owner.is_empty() && !repo.is_empty();
                let mut closed_on_github = false;

                // If GitHub is configured AND issue exists on GitHub, ALWAYS close there FIRST
                if has_github_config && i.github_number.is_some() {
                    let gh_num = i.github_number.unwrap() as u32;

                    // Get sync service - fail if not available
                    let sync_service = match self.get_sync_service(Some(&project.id)).await {
                        Ok(service) => service,
                        Err(e) => {
                            let response = DeleteIssueResponse {
                                success: false,
                                closed_on_github: false,
                                error: Some(format!("GitHub sync service unavailable: {}", e)),
                            };
                            let json = serde_json::to_string_pretty(&response)
                                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                            return Ok(CallToolResult::success(vec![Content::text(json)]));
                        }
                    };

                    // Close on GitHub FIRST - fail if it fails (unless already deleted)
                    match sync_service.rest().close_issue(&owner, &repo, gh_num).await {
                        Ok(_) => {
                            info!("Closed GitHub issue #{} for project {}", gh_num, project.id);
                            closed_on_github = true;
                        }
                        Err(e) => {
                            let error_str = e.to_string();
                            // Treat 410 Gone as success - issue was already deleted on GitHub
                            if error_str.contains("410") || error_str.contains("Gone") {
                                info!("GitHub issue #{} was already deleted, proceeding with local deletion", gh_num);
                                closed_on_github = true; // Consider it handled
                            } else {
                                warn!("Failed to close issue on GitHub: {}", e);
                                let response = DeleteIssueResponse {
                                    success: false,
                                    closed_on_github: false,
                                    error: Some(format!("Failed to close on GitHub: {}", e)),
                                };
                                let json = serde_json::to_string_pretty(&response)
                                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                                return Ok(CallToolResult::success(vec![Content::text(json)]));
                            }
                        }
                    }
                }

                // GitHub succeeded (or no GitHub) - now delete locally
                let issue_id = i.id.clone();
                let project_id = project.id.clone();

                store.delete_issue(&i.id).await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                drop(store); // Release lock before emitting event

                // Emit event
                if let Some(bus) = &self.event_bus {
                    bus.issue_deleted(&project_id, &issue_id);
                }

                let response = DeleteIssueResponse {
                    success: true,
                    closed_on_github,
                    error: None,
                };

                let json = serde_json::to_string_pretty(&response)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            None => {
                let response = DeleteIssueResponse {
                    success: false,
                    closed_on_github: false,
                    error: Some(format!("Issue '{}' not found", params.0.issue)),
                };

                let json = serde_json::to_string_pretty(&response)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
        }
    }

    // =========================================================================
    // GITHUB TOKEN MANAGEMENT
    // =========================================================================

    /// Set a GitHub Personal Access Token for API access.
    #[tool(description = "Set a GitHub Personal Access Token for API access. Use scope='global' or a project ID for project-specific token.")]
    async fn set_github_token(
        &self,
        params: Parameters<SetGitHubTokenParams>,
    ) -> Result<CallToolResult, McpError> {
        info!("Setting GitHub token");
        let (storage, manager) = self.require_token_storage()?;

        // Validate token format
        if let Err(e) = GitHubTokenManager::validate_token_format(&params.0.token) {
            let response = SetGitHubTokenResponse {
                success: false,
                scope: String::new(),
                error: Some(e.to_string()),
            };
            let json = serde_json::to_string_pretty(&response)
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
            return Ok(CallToolResult::success(vec![Content::text(json)]));
        }

        // Determine scope
        let scope_str = params.0.scope.as_deref().unwrap_or("global");
        let scope = if scope_str == "global" {
            TokenScope::Global
        } else {
            TokenScope::Project(scope_str.to_string())
        };

        // Encrypt and store
        let encrypted = manager.encrypt(&params.0.token)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        storage.store_token(&scope, &encrypted)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let response = SetGitHubTokenResponse {
            success: true,
            scope: scope_str.to_string(),
            error: None,
        };

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// Sync issues with GitHub.
    #[tool(description = "Sync issues between Kix and GitHub. Direction can be 'pull', 'push', or 'bidirectional'.")]
    async fn sync_github_issues(
        &self,
        params: Parameters<SyncGitHubIssuesParams>,
    ) -> Result<CallToolResult, McpError> {
        info!("Syncing GitHub issues for project: {}", params.0.project);
        let project_store = self.require_project_store()?;

        let store = project_store.write().await;

        // Get project
        let project = store.get_project(&params.0.project).await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .ok_or_else(|| McpError::invalid_params(
                format!("Project '{}' not found", params.0.project),
                None,
            ))?;

        // Get sync service
        let sync_service = self.get_sync_service(Some(&project.id)).await?;

        // Build sync config
        let direction = params.0.direction.as_ref().map(|d| match d.as_str() {
            "pull" => SyncDirection::Pull,
            "push" => SyncDirection::Push,
            _ => SyncDirection::Bidirectional,
        }).unwrap_or(SyncDirection::Bidirectional);

        let config = SyncConfig {
            direction,
            include_closed: params.0.include_closed.unwrap_or(true),
            labels_filter: params.0.labels.clone().unwrap_or_default(),
            max_issues: params.0.max_issues.unwrap_or(100) as u32,
        };

        // Get local issues for push (list_issues takes project_id, state, limit, offset)
        let local_issues = store.list_issues(&project.id, None, 10000, 0).await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let local_issue_infos: Vec<_> = local_issues.iter().map(|i| {
            // Convert string state/source to enums for IssueInfo
            let state_enum = match i.state.as_str() {
                "closed" => IssueState::Closed,
                _ => IssueState::Open,
            };
            let source_enum = match i.source.as_str() {
                "github" => IssueSource::GitHub,
                "mcp" => IssueSource::Mcp,
                _ => IssueSource::Local,
            };
            IssueInfo {
                id: i.id.clone(),
                title: i.title.clone(),
                body: i.body.clone(),
                state: state_enum,
                labels: i.labels_vec(),
                assignees: i.assignees_vec(),
                github_number: i.github_number.map(|n| n as u32),
                github_node_id: i.github_node_id.clone(),
                source: source_enum,
            }
        }).collect();

        let github_owner = project.github_owner().unwrap_or_default();
        let github_repo = project.github_repo().unwrap_or_default();
        let project_id = project.id.clone();

        // Emit sync started event
        if let Some(bus) = &self.event_bus {
            bus.github_sync_started(&project_id);
        }

        // Perform sync
        let result = sync_service.sync_issues(
            &github_owner,
            &github_repo,
            &config,
            &local_issue_infos,
        ).await.map_err(|e| {
            // Emit sync failed event
            if let Some(bus) = &self.event_bus {
                bus.github_sync_failed(&project_id, &e.to_string());
            }
            McpError::internal_error(e.to_string(), None)
        })?;

        // Import pulled issues
        let pulled_issues = sync_service.pull_issues(
            &github_owner,
            &github_repo,
            &config,
        ).await.map_err(|e| McpError::internal_error(e.to_string(), None))?;

        for gh_issue in pulled_issues {
            // Check if we already have this issue
            if store.get_issue_by_github_number(&project.id, gh_issue.number).await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?.is_some() {
                continue;
            }

            let mut issue = IssueRecord::new(project.id.clone(), gh_issue.number, gh_issue.title.clone());
            if let Some(body) = gh_issue.body {
                issue = issue.with_body(body);
            }
            issue.state = if gh_issue.state == "open" { "open".to_string() } else { "closed".to_string() };
            issue = issue.with_labels(gh_issue.labels.iter().map(|l| l.name.clone()).collect());
            let assignees: Vec<String> = gh_issue.assignees.iter().map(|a| a.login.clone()).collect();
            issue.set_assignees(assignees);
            issue.github_number = Some(gh_issue.number as i64);
            issue.github_node_id = Some(gh_issue.node_id.clone());
            issue.github_url = Some(gh_issue.html_url);
            issue.source = "github".to_string();

            store.create_issue(&issue).await.ok();
        }

        drop(store); // Release lock before emitting event

        // Emit sync completed event
        if let Some(bus) = &self.event_bus {
            bus.github_sync_completed(
                &project_id,
                result.issues_pulled as usize,
                result.issues_updated as usize,
            );
        }

        let response = SyncGitHubIssuesResponse {
            success: true,
            issues_pulled: result.issues_pulled as usize,
            issues_pushed: result.issues_pushed as usize,
            issues_updated: result.issues_updated as usize,
            issues_failed: result.issues_failed as usize,
            errors: result.errors,
            synced_at: result.synced_at.to_rfc3339(),
        };

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    // =========================================================================
    // KNOWLEDGE LINKING TOOLS
    // =========================================================================

    /// Link a knowledge entry to a project.
    #[tool(description = "Link a knowledge entry to a project for project-scoped search and AI planning context.")]
    async fn link_entry_to_project(
        &self,
        params: Parameters<LinkEntryParams>,
    ) -> Result<CallToolResult, McpError> {
        info!("Linking entry {} to project {}", params.0.entry_id, params.0.project);
        let project_store = self.require_project_store()?;

        let store = project_store.write().await;

        // Get project
        let project = store.get_project(&params.0.project).await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .ok_or_else(|| McpError::invalid_params(
                format!("Project '{}' not found", params.0.project),
                None,
            ))?;

        // Verify entry exists
        let kix_store = self.store.read().await;
        let entry_exists = kix_store.entry_exists(&params.0.entry_id).await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        if !entry_exists {
            let response = LinkEntryResponse {
                success: false,
                link_id: String::new(),
                error: Some(format!("Entry '{}' not found", params.0.entry_id)),
            };
            let json = serde_json::to_string_pretty(&response)
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
            return Ok(CallToolResult::success(vec![Content::text(json)]));
        }

        drop(kix_store);

        // Create link record
        let mut link = ProjectEntryRecord::new(project.id.clone(), params.0.entry_id.clone());
        if let Some(notes) = &params.0.notes {
            link = link.with_notes(notes.clone());
        }
        if let Some(relevance) = params.0.relevance {
            link = link.with_relevance(relevance as f64);
        }

        let link_id = link.id.clone();
        let entry_id = params.0.entry_id.clone();
        let project_id = project.id.clone();

        store.link_entry(&link).await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        drop(store); // Release lock before emitting event

        // Emit event
        if let Some(bus) = &self.event_bus {
            bus.entry_linked(&project_id, &entry_id, &link_id);
        }

        let response = LinkEntryResponse {
            success: true,
            link_id,
            error: None,
        };

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// Unlink a knowledge entry from a project.
    #[tool(description = "Remove a knowledge entry link from a project.")]
    async fn unlink_entry_from_project(
        &self,
        params: Parameters<UnlinkEntryParams>,
    ) -> Result<CallToolResult, McpError> {
        info!("Unlinking entry {} from project {}", params.0.entry_id, params.0.project);
        let project_store = self.require_project_store()?;

        let store = project_store.write().await;

        // Get project
        let project = store.get_project(&params.0.project).await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .ok_or_else(|| McpError::invalid_params(
                format!("Project '{}' not found", params.0.project),
                None,
            ))?;

        // Remove link
        let project_id = project.id.clone();
        let entry_id = params.0.entry_id.clone();

        let removed = store.unlink_entry(&project.id, &params.0.entry_id).await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        drop(store); // Release lock before emitting event

        // Emit event if unlinked successfully
        if removed {
            if let Some(bus) = &self.event_bus {
                bus.entry_unlinked(&project_id, &entry_id);
            }
        }

        let response = UnlinkEntryResponse {
            success: removed,
            error: if !removed {
                Some(format!("Entry '{}' was not linked to project", entry_id))
            } else {
                None
            },
        };

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// List knowledge entries linked to a project.
    #[tool(description = "List all knowledge entries linked to a project.")]
    async fn list_project_entries(
        &self,
        params: Parameters<ListProjectEntriesParams>,
    ) -> Result<CallToolResult, McpError> {
        info!("Listing entries for project: {}", params.0.project);
        let project_store = self.require_project_store()?;

        let store = project_store.read().await;

        // Get project
        let project = store.get_project(&params.0.project).await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .ok_or_else(|| McpError::invalid_params(
                format!("Project '{}' not found", params.0.project),
                None,
            ))?;

        let limit = params.0.limit.unwrap_or(50);

        let entries = store.list_project_entries(&project.id).await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        drop(store); // Release lock before accessing main store

        // Look up entry details from main store
        let main_store = self.store.read().await;
        let mut linked_entries: Vec<LinkedEntry> = Vec::new();

        for link in entries {
            // Try to get entry details from main store
            if let Ok(Some(entry)) = main_store.get_entry_by_id(&link.entry_id).await {
                // Filter by type if specified
                if let Some(filter_type) = &params.0.entry_type {
                    if &entry.entry_type != filter_type {
                        continue;
                    }
                }

                // Construct source URL from domain and path
                let source_url = if let Some(domain) = &entry.source_domain {
                    Some(format!("https://{}{}", domain, entry.source_path))
                } else if !entry.source_path.is_empty() {
                    Some(entry.source_path.clone())
                } else {
                    None
                };

                linked_entries.push(LinkedEntry {
                    entry_id: link.entry_id,
                    title: entry.title,
                    entry_type: entry.entry_type,
                    source_url,
                    relevance: link.relevance.map(|r| r as f32),
                    notes: link.notes,
                    linked_at: link.linked_at,
                });
            }
        }

        let total = linked_entries.len();
        linked_entries.truncate(limit);

        let response = ListProjectEntriesResponse {
            entries: linked_entries,
            total,
        };

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    // =========================================================================
    // PROJECT SEARCH TOOL
    // =========================================================================

    /// Search within a project's issues and linked knowledge.
    #[tool(description = "Search within a project's scope across issues and linked knowledge entries.")]
    async fn search_project(
        &self,
        params: Parameters<SearchProjectParams>,
    ) -> Result<CallToolResult, McpError> {
        info!("Searching project {} for: {}", params.0.project, params.0.query);
        let project_store = self.require_project_store()?;

        let store = project_store.read().await;

        // Get project
        let project = store.get_project(&params.0.project).await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .ok_or_else(|| McpError::invalid_params(
                format!("Project '{}' not found", params.0.project),
                None,
            ))?;

        let limit = params.0.limit.unwrap_or(20);
        let include_closed = params.0.include_closed.unwrap_or(false);
        let search_type = params.0.search_type.as_deref().unwrap_or("all");

        let mut issue_results = Vec::new();
        let mut knowledge_results = Vec::new();

        // Search issues (list_issues takes project_id, state, limit, offset)
        if search_type == "all" || search_type == "issues" {
            let state_filter = if include_closed { None } else { Some("open") };
            let issues = store.list_issues(&project.id, state_filter, 10000, 0).await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;

            let github_owner = project.github_owner().unwrap_or_default();
            let github_repo = project.github_repo().unwrap_or_default();

            for issue in issues {
                let score = calculate_text_score(&params.0.query, &issue.title, issue.body.as_deref());
                if score > 0.0 {
                    let excerpt = issue.body.as_ref().and_then(|b| generate_excerpt(b, &params.0.query, 50));
                    let github_url = issue.github_number.map(|n| {
                        format!("https://github.com/{}/{}/issues/{}", github_owner, github_repo, n)
                    });
                    let labels_list = issue.labels_vec();

                    issue_results.push(IssueSearchResultItem {
                        id: issue.id,
                        number: issue.number as u32,
                        title: issue.title,
                        excerpt,
                        state: issue.state,
                        labels: labels_list,
                        score,
                        github_url,
                    });
                }
            }
            issue_results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        }

        // Search knowledge (ProjectEntryRecord only has entry_id and notes,
        // so we search by notes and look up actual entry details from main store)
        if search_type == "all" || search_type == "knowledge" {
            let linked_entries = store.list_project_entries(&project.id).await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;

            let main_store = self.store.read().await;
            for link in linked_entries {
                // Try to get the actual entry from main store
                if let Ok(Some(entry)) = main_store.get_entry_by_id(&link.entry_id).await {
                    let notes_text = link.notes.as_deref();
                    let score = calculate_text_score(&params.0.query, &entry.title, notes_text);
                    if score > 0.0 {
                        let excerpt = link.notes.as_ref().and_then(|n| generate_excerpt(n, &params.0.query, 50));

                        // Construct source URL from domain and path (source_path is String, source_domain is Option)
                        let source_url = if let Some(domain) = &entry.source_domain {
                            Some(format!("https://{}{}", domain, entry.source_path))
                        } else if !entry.source_path.is_empty() {
                            Some(entry.source_path.clone())
                        } else {
                            None
                        };

                        knowledge_results.push(KnowledgeSearchResultItem {
                            entry_id: link.entry_id,
                            title: entry.title,
                            excerpt,
                            entry_type: entry.entry_type,
                            source_url,
                            score,
                        });
                    }
                }
            }
            knowledge_results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        }

        // Apply limit
        issue_results.truncate(limit);
        knowledge_results.truncate(limit);

        let total = issue_results.len() + knowledge_results.len();

        let response = SearchProjectResponse {
            total,
            issues: issue_results,
            knowledge: knowledge_results,
        };

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    // =========================================================================
    // HELPER METHODS
    // =========================================================================

    /// Process content source into an Entry.
    async fn process_content(
        &self,
        content: &ContentSource,
        title_override: Option<&str>,
    ) -> Result<Entry, McpError> {
        if let Some(ref text) = content.text {
            self.process_raw_text(text, title_override)
        } else if let Some(ref file) = content.file {
            self.process_file_path(file, title_override).await
        } else if let Some(ref url) = content.url {
            self.process_url(url, title_override).await
        } else {
            Err(McpError::invalid_params(
                "Must provide text, file, or url",
                None,
            ))
        }
    }

    /// Process raw text content.
    fn process_raw_text(
        &self,
        text: &str,
        title_override: Option<&str>,
    ) -> Result<Entry, McpError> {
        if text.trim().is_empty() {
            return Err(McpError::invalid_params("Empty content provided", None));
        }

        if text.len() > 10_000_000 {
            return Err(McpError::invalid_params(
                "Content too large (max 10MB)",
                None,
            ));
        }

        // Generate title from first line or use override
        let title = title_override
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                text.lines()
                    .next()
                    .unwrap_or("Untitled")
                    .trim_start_matches('#')
                    .trim()
                    .chars()
                    .take(100)
                    .collect()
            });

        let slug = slugify(&title);
        let description = text.chars().take(300).collect();

        // Compute hash
        let mut hasher = Sha256::new();
        hasher.update(text.as_bytes());
        let source_hash = format!("{:x}", hasher.finalize());

        Ok(Entry::with_id(slug.clone(), title, String::new(), source_hash)
            .with_description(description)
            .with_content(text.to_string())
            .with_tags(vec![])
            .with_entry_type(EntryType::Document)
            .with_source_type(SourceType::Markdown)
            .with_slug(slug))
    }

    /// Process file path content.
    async fn process_file_path(
        &self,
        path: &str,
        title_override: Option<&str>,
    ) -> Result<Entry, McpError> {
        use std::path::Path;

        let file_path = Path::new(path);

        if !file_path.exists() {
            return Err(McpError::invalid_params(
                format!("File not found: {}", path),
                None,
            ));
        }

        let extension = file_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        match extension.as_str() {
            "html" | "htm" => {
                let content = tokio::fs::read_to_string(path)
                    .await
                    .map_err(|e| McpError::internal_error(format!("Failed to read file: {}", e), None))?;

                let extractor = ContentExtractor::default();
                let url = url::Url::parse(&format!("file://{}", path))
                    .unwrap_or_else(|_| url::Url::parse("file:///unknown").unwrap());
                let extracted = extractor.extract(&content, &url);

                let title = title_override
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| extracted.title.clone());

                let slug = slugify(&title);
                let description = extracted
                    .description
                    .clone()
                    .unwrap_or_else(|| extracted.markdown.chars().take(300).collect());

                Ok(Entry::with_id(slug.clone(), title, path.to_string(), extracted.content_hash.clone())
                    .with_description(description)
                    .with_content(extracted.markdown)
                    .with_tags(vec![])
                    .with_entry_type(EntryType::Document)
                    .with_source_type(SourceType::Html)
                    .with_slug(slug))
            }
            "pdf" => {
                let parser = PdfParser::new();
                let mut entry = parser
                    .parse(path)
                    .map_err(|e| McpError::internal_error(format!("PDF parse error: {}", e), None))?;

                if let Some(title) = title_override {
                    entry.title = title.to_string();
                    entry.slug = slugify(title);
                }

                Ok(entry)
            }
            "md" | "markdown" => {
                let content = tokio::fs::read_to_string(path)
                    .await
                    .map_err(|e| McpError::internal_error(format!("Failed to read file: {}", e), None))?;

                self.process_raw_text(&content, title_override)
            }
            "txt" => {
                let content = tokio::fs::read_to_string(path)
                    .await
                    .map_err(|e| McpError::internal_error(format!("Failed to read file: {}", e), None))?;

                self.process_raw_text(&content, title_override)
            }
            _ => Err(McpError::invalid_params(
                format!("Unsupported file type: {}. Supported: html, pdf, md, txt", extension),
                None,
            )),
        }
    }

    /// Process URL content.
    async fn process_url(
        &self,
        url_str: &str,
        title_override: Option<&str>,
    ) -> Result<Entry, McpError> {
        // Validate URL
        let parsed_url = url::Url::parse(url_str)
            .map_err(|_| McpError::invalid_params(format!("Invalid URL: {}", url_str), None))?;

        if !matches!(parsed_url.scheme(), "http" | "https") {
            return Err(McpError::invalid_params(
                format!("Only http/https URLs supported, got: {}", parsed_url.scheme()),
                None,
            ));
        }

        // Fetch content
        let response = self
            .http_client
            .get(url_str)
            .send()
            .await
            .map_err(|e| McpError::internal_error(format!("Failed to fetch URL: {}", e), None))?;

        if !response.status().is_success() {
            return Err(McpError::internal_error(
                format!("HTTP error: {}", response.status()),
                None,
            ));
        }

        let content = response
            .text()
            .await
            .map_err(|e| McpError::internal_error(format!("Failed to read response: {}", e), None))?;

        // Parse as HTML
        let extractor = ContentExtractor::default();
        let extracted = extractor.extract(&content, &parsed_url);

        let title = title_override
            .map(|s| s.to_string())
            .unwrap_or_else(|| extracted.title.clone());

        let slug = slugify(&title);
        let description = extracted
            .description
            .clone()
            .unwrap_or_else(|| extracted.markdown.chars().take(300).collect());

        let entry = Entry::with_id(slug.clone(), title, url_str.to_string(), extracted.content_hash.clone())
            .with_description(description)
            .with_content(extracted.markdown)
            .with_tags(vec![])
            .with_entry_type(EntryType::Document)
            .with_source_type(SourceType::Url)
            .with_slug(slug);

        // Note: source_domain is extracted from source_path when storing in the database

        Ok(entry)
    }
}

// Implement ServerHandler trait for the MCP server
impl ServerHandler for KixMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some("RAG Knowledge System - Search, index, and retrieve documents for AI-powered knowledge retrieval.".into()),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, McpError>> + Send + '_ {
        async move {
            let tools = self.tool_router.list_all();
            Ok(ListToolsResult {
                tools,
                ..Default::default()
            })
        }
    }

    fn call_tool(
        &self,
        request: CallToolRequestParam,
        context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, McpError>> + Send + '_ {
        async move {
            let tool_context = ToolCallContext::new(self, request, context);
            self.tool_router.call(tool_context).await
        }
    }
}

/// Backward compatibility alias for KixMcpServer.
pub type EipMcpServer = KixMcpServer;

/// Convert a title to a URL-safe slug.
fn slugify(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c
            } else if c.is_whitespace() || c == '-' || c == '_' {
                '-'
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
