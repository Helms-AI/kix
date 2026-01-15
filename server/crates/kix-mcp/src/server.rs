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
//! **Project Management (20+ tools):**
//! - Project CRUD: `create_project`, `list_projects`, `get_project`, `update_project`, `delete_project`
//! - Work Item CRUD: `create_work_item`, `list_work_items`, `get_work_item`, `update_work_item`, `delete_work_item`
//! - Board: `get_board`, `move_card`, `get_child_work_items`
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
use tracing::info;
use uuid::Uuid;

use kix_embeddings::{DocumentChunker, OllamaEmbedder};
use kix_jobs::{Job, JobConfig, JobQueue, JobState, JobType};
use kix_parser::{Entry, EntryType, PdfParser, SourceType};
use kix_crawler::ContentExtractor;
use kix_services::{self, Pagination};
use kix_store::{KixStore, ProjectEntryRecord};
use kix_store::projects::ProjectStore;
use kix_projects::{
    calculate_text_score, generate_excerpt,
    SharedEventBus,
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
    embedder: Arc<OllamaEmbedder>,
    http_client: HttpClient,
    /// Job queue for async indexing operations
    job_queue: Arc<JobQueue>,
    /// Project store for project management
    project_store: Option<Arc<RwLock<ProjectStore>>>,
    /// Event bus for real-time events
    event_bus: Option<SharedEventBus>,
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl KixMcpServer {
    /// Creates a new MCP server with the given store, embedder, and job queue.
    pub fn new(store: KixStore, embedder: OllamaEmbedder, job_queue: Arc<JobQueue>) -> Self {
        Self::with_shared(
            Arc::new(RwLock::new(store)),
            Arc::new(embedder),
            job_queue,
        )
    }

    /// Creates a new MCP server with a shared store.
    pub fn with_shared_store(
        store: Arc<RwLock<KixStore>>,
        embedder: OllamaEmbedder,
        job_queue: Arc<JobQueue>,
    ) -> Self {
        Self::with_shared(store, Arc::new(embedder), job_queue)
    }

    /// Creates a new MCP server with both shared store and shared embedder.
    pub fn with_shared(
        store: Arc<RwLock<KixStore>>,
        embedder: Arc<OllamaEmbedder>,
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

        let texts: Vec<String> = chunks.iter().map(|c| c.text.clone()).collect();
        let embeddings = self.embedder
            .embed_batch(&texts)
            .await
            .map_err(|e| McpError::internal_error(format!("Embedding failed: {}", e), None))?;

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

    /// Create a new project.
    #[tool(description = "Create a new project for work item tracking.")]
    async fn create_project(
        &self,
        params: Parameters<CreateProjectParams>,
    ) -> Result<CallToolResult, McpError> {
        info!("Creating project: {}", params.0.name);
        let project_store = self.require_project_store()?;

        // Use shared service layer
        let result = kix_services::create_project(
            project_store,
            self.event_bus.as_ref(),
            kix_services::CreateProjectData {
                name: params.0.name.clone(),
                description: params.0.description.clone(),
                color: params.0.color.clone(),
            },
        )
        .await
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let response = CreateProjectResponse {
            success: true,
            project_id: result.project_id,
            name: result.name,
            slug: result.slug,
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

        // Use shared service layer
        let result = kix_services::list_projects(
            project_store,
            kix_services::ProjectFilters {
                include_archived: params.0.include_archived.unwrap_or(false),
            },
            Pagination {
                limit: params.0.limit.unwrap_or(50),
                offset: params.0.offset.unwrap_or(0),
            },
        )
        .await
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        // Convert service type to MCP response type
        let summaries: Vec<ProjectSummary> = result.projects.into_iter().map(|p| {
            ProjectSummary {
                id: p.id,
                name: p.name,
                slug: p.slug,
                description: p.description,
                color: p.color,
                archived: p.archived,
                open_items: 0,
                closed_items: 0,
                created_at: p.created_at,
            }
        }).collect();

        let response = ListProjectsResponse {
            projects: summaries,
            total: result.total,
            has_more: result.has_more,
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

        // Use shared service layer
        let result = kix_services::get_project(
            project_store,
            &params.0.project,
            params.0.include_stats.unwrap_or(true),
        )
        .await;

        match result {
            Ok(p) => {
                // Convert service type to MCP response type
                let stats = p.stats.map(|s| ProjectStats {
                    open_items: s.open_items,
                    closed_items: s.closed_items,
                    total_items: s.total_items,
                    linked_entries: s.linked_entries,
                });

                let response = ProjectDetail {
                    id: p.id,
                    name: p.name,
                    slug: p.slug,
                    description: p.description,
                    color: p.color,
                    archived: p.archived,
                    created_at: p.created_at,
                    updated_at: p.updated_at,
                    stats,
                };

                let json = serde_json::to_string_pretty(&response)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            Err(e) if e.to_string().contains("not found") => {
                let error = serde_json::json!({
                    "error": "not_found",
                    "message": format!("Project '{}' not found", params.0.project)
                });
                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&error).unwrap(),
                )]))
            }
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
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

        // Use shared service layer
        let result = kix_services::update_project(
            project_store,
            self.event_bus.as_ref(),
            &params.0.project,
            kix_services::ProjectUpdates {
                name: params.0.name.clone(),
                description: params.0.description.clone(),
                color: params.0.color.clone(),
                archived: params.0.archived,
            },
        )
        .await;

        match result {
            Ok(p) => {
                let response = UpdateProjectResponse {
                    success: true,
                    project_id: p.id,
                    error: None,
                };

                let json = serde_json::to_string_pretty(&response)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            Err(e) if e.to_string().contains("not found") => {
                let response = UpdateProjectResponse {
                    success: false,
                    project_id: String::new(),
                    error: Some(format!("Project '{}' not found", params.0.project)),
                };

                let json = serde_json::to_string_pretty(&response)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    /// Delete a project.
    #[tool(description = "Delete a project and its local work items/entries.")]
    async fn delete_project(
        &self,
        params: Parameters<DeleteProjectParams>,
    ) -> Result<CallToolResult, McpError> {
        info!("Deleting project: {}", params.0.project);
        let project_store = self.require_project_store()?;

        // Use shared service layer
        let result = kix_services::delete_project(
            project_store,
            self.event_bus.as_ref(),
            &params.0.project,
            kix_services::DeleteProjectOptions {
                delete_items: params.0.delete_items.unwrap_or(true),
            },
        )
        .await;

        match result {
            Ok(r) => {
                let response = DeleteProjectResponse {
                    success: true,
                    items_deleted: r.items_deleted,
                    entries_unlinked: r.entries_unlinked,
                    error: None,
                };

                let json = serde_json::to_string_pretty(&response)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            Err(e) if e.to_string().contains("not found") => {
                let response = DeleteProjectResponse {
                    success: false,
                    items_deleted: 0,
                    entries_unlinked: 0,
                    error: Some(format!("Project '{}' not found", params.0.project)),
                };

                let json = serde_json::to_string_pretty(&response)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    // =========================================================================
    // WORK ITEM CRUD TOOLS
    // =========================================================================

    /// Create a new work item in a project.
    #[tool(description = "Create a new work item in a project.")]
    async fn create_work_item(
        &self,
        params: Parameters<CreateWorkItemParams>,
    ) -> Result<CallToolResult, McpError> {
        info!("Creating work item in project: {}", params.0.project);
        let project_store = self.require_project_store()?;

        // Use shared service layer
        let result = kix_services::create_work_item(
            project_store,
            self.event_bus.as_ref(),
            &params.0.project,
            kix_services::CreateWorkItemData {
                title: params.0.title.clone(),
                body: params.0.body.clone(),
                labels: params.0.labels.clone(),
                assignees: params.0.assignees.clone(),
                item_type: params.0.item_type.clone(),
                parent_id: params.0.parent_id.clone(),
                board_column: params.0.board_column.clone(),
                story_points: params.0.story_points,
                epic_color: params.0.epic_color.clone(),
            },
        )
        .await
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let response = CreateWorkItemResponse {
            success: true,
            item_id: result.item_id,
            number: result.number,
            title: result.title,
            error: None,
        };

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// List work items in a project.
    #[tool(description = "List work items in a project with optional filters for state, labels, assignee, and search.")]
    async fn list_work_items(
        &self,
        params: Parameters<ListWorkItemsParams>,
    ) -> Result<CallToolResult, McpError> {
        info!("Listing work items for project: {}", params.0.project);
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

        // list_work_items takes (project_id, state, limit, offset) - fetch more for filtering
        let items = store.list_work_items(&project.id, state_filter, 10000, 0).await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        // Apply additional filters in memory (labels, assignee, search)
        let filtered: Vec<_> = items.into_iter().filter(|i| {
            // Label filter (labels is Option<String> JSON)
            if let Some(labels) = &params.0.labels {
                let item_labels = i.labels_vec();
                if !labels.iter().any(|l| item_labels.contains(l)) {
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

        let summaries: Vec<WorkItemSummary> = paginated.into_iter().map(|i| {
            let labels_list = i.labels_vec();
            let assignees_list = i.assignees_vec();

            WorkItemSummary {
                id: i.id,
                number: i.number as u32,
                title: i.title,
                state: i.state,
                labels: labels_list,
                assignees: if assignees_list.is_empty() { None } else { Some(assignees_list) },
                created_at: i.created_at,
                updated_at: i.updated_at,
                // Board fields
                item_type: i.item_type,
                parent_id: i.parent_id,
                board_column: i.board_column,
                position: i.position,
                story_points: i.story_points,
                epic_color: i.epic_color,
            }
        }).collect();

        let response = ListWorkItemsResponse {
            items: summaries,
            total,
            has_more,
        };

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// Get a specific work item.
    #[tool(description = "Get detailed information about a specific work item by number or ID.")]
    async fn get_work_item(
        &self,
        params: Parameters<GetWorkItemParams>,
    ) -> Result<CallToolResult, McpError> {
        info!("Getting work item {} in project {}", params.0.item, params.0.project);
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
        let item = if let Ok(num) = params.0.item.parse::<u32>() {
            store.get_work_item_by_number(&project.id, num).await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?
        } else {
            store.get_work_item(&params.0.item).await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?
        };

        match item {
            Some(i) => {
                let labels_list = i.labels_vec();
                let assignees_list = i.assignees_vec();
                let response = WorkItemDetail {
                    id: i.id,
                    project_id: i.project_id,
                    number: i.number as u32,
                    title: i.title,
                    body: i.body,
                    state: i.state,
                    labels: labels_list,
                    assignees: if assignees_list.is_empty() { None } else { Some(assignees_list) },
                    created_at: i.created_at,
                    updated_at: i.updated_at,
                    // Board fields
                    item_type: i.item_type,
                    parent_id: i.parent_id,
                    board_column: i.board_column,
                    position: i.position,
                    story_points: i.story_points,
                    epic_color: i.epic_color,
                };

                let json = serde_json::to_string_pretty(&response)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            None => {
                let error = serde_json::json!({
                    "error": "not_found",
                    "message": format!("Work item '{}' not found in project '{}'", params.0.item, params.0.project)
                });
                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&error).unwrap(),
                )]))
            }
        }
    }

    /// Update a work item.
    #[tool(description = "Update a work item's title, body, state, labels, or assignees.")]
    async fn update_work_item(
        &self,
        params: Parameters<UpdateWorkItemParams>,
    ) -> Result<CallToolResult, McpError> {
        info!("Updating work item {} in project {}", params.0.item, params.0.project);
        let project_store = self.require_project_store()?;

        let store = project_store.write().await;

        // Get project
        let project = store.get_project(&params.0.project).await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .ok_or_else(|| McpError::invalid_params(
                format!("Project '{}' not found", params.0.project),
                None,
            ))?;

        // Get work item
        let item = if let Ok(num) = params.0.item.parse::<u32>() {
            store.get_work_item_by_number(&project.id, num).await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?
        } else {
            store.get_work_item(&params.0.item).await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?
        };

        match item {
            Some(mut i) => {
                let was_open = i.state != "closed";

                // Apply updates
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
                if let Some(item_type) = &params.0.item_type {
                    i.item_type = item_type.clone();
                }
                if let Some(parent_id) = &params.0.parent_id {
                    i.parent_id = Some(parent_id.clone());
                }
                if let Some(board_column) = &params.0.board_column {
                    i.board_column = board_column.clone();
                }
                if let Some(story_points) = params.0.story_points {
                    i.story_points = Some(story_points);
                }
                if let Some(epic_color) = &params.0.epic_color {
                    i.epic_color = Some(epic_color.clone());
                }

                i.updated_at = chrono::Utc::now().to_rfc3339();
                let is_now_closed = i.state == "closed";
                let item_id = i.id.clone();
                let project_id = project.id.clone();

                store.update_work_item(&i).await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                drop(store); // Release lock before emitting event

                // Emit appropriate event
                if let Some(bus) = &self.event_bus {
                    if was_open && is_now_closed {
                        bus.issue_closed(&project_id, &item_id);
                    } else if !was_open && !is_now_closed {
                        bus.issue_reopened(&project_id, &item_id);
                    } else {
                        bus.issue_updated(&project_id, &item_id);
                    }
                }

                let response = UpdateWorkItemResponse {
                    success: true,
                    item_id,
                    error: None,
                };

                let json = serde_json::to_string_pretty(&response)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            None => {
                let response = UpdateWorkItemResponse {
                    success: false,
                    item_id: String::new(),
                    error: Some(format!("Work item '{}' not found", params.0.item)),
                };

                let json = serde_json::to_string_pretty(&response)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
        }
    }

    /// Delete a work item from Kix.
    #[tool(description = "Delete a work item from Kix.")]
    async fn delete_work_item(
        &self,
        params: Parameters<DeleteWorkItemParams>,
    ) -> Result<CallToolResult, McpError> {
        info!("Deleting work item {} from project {}", params.0.item, params.0.project);
        let project_store = self.require_project_store()?;

        let store = project_store.write().await;

        // Get project
        let project = store.get_project(&params.0.project).await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .ok_or_else(|| McpError::invalid_params(
                format!("Project '{}' not found", params.0.project),
                None,
            ))?;

        // Get work item
        let item = if let Ok(num) = params.0.item.parse::<u32>() {
            store.get_work_item_by_number(&project.id, num).await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?
        } else {
            store.get_work_item(&params.0.item).await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?
        };

        match item {
            Some(i) => {
                let item_id = i.id.clone();
                let project_id = project.id.clone();

                store.delete_work_item(&i.id).await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                drop(store); // Release lock before emitting event

                // Emit event
                if let Some(bus) = &self.event_bus {
                    bus.issue_deleted(&project_id, &item_id);
                }

                let response = DeleteWorkItemResponse {
                    success: true,
                    error: None,
                };

                let json = serde_json::to_string_pretty(&response)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            None => {
                let response = DeleteWorkItemResponse {
                    success: false,
                    error: Some(format!("Work item '{}' not found", params.0.item)),
                };

                let json = serde_json::to_string_pretty(&response)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
        }
    }

    // =========================================================================
    // BOARD OPERATIONS
    // =========================================================================

    /// Get the Kanban board view for a project with work items organized by swimlanes and columns.
    #[tool(description = "Get the Kanban board view for a project. Returns work items organized by swimlanes (item type) and columns (workflow status). Columns: backlog, todo, in_progress, in_review, testing, done. Swimlanes: epic, story, task, subtask, bug.")]
    async fn get_board(
        &self,
        params: Parameters<GetBoardParams>,
    ) -> Result<CallToolResult, McpError> {
        info!("Getting board view for project: {}", params.0.project);
        let project_store = self.require_project_store()?;

        let store = project_store.read().await;

        // Get project
        let project = store.get_project(&params.0.project).await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .ok_or_else(|| McpError::invalid_params(
                format!("Project '{}' not found", params.0.project),
                None,
            ))?;

        // Get all work items for board
        let items = store.list_work_items_for_board(&project.id, params.0.item_type.as_deref()).await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        // Organize by swimlane and column
        let mut swimlane_data: std::collections::HashMap<String, std::collections::HashMap<String, Vec<WorkItemSummary>>> = std::collections::HashMap::new();
        let mut column_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

        // Initialize swimlanes
        for item_type in &["epic", "story", "task", "subtask", "bug"] {
            swimlane_data.insert(item_type.to_string(), std::collections::HashMap::new());
        }

        // Populate work items into swimlanes
        for item in &items {
            let swimlane = item.item_type.clone();
            let column = item.board_column.clone();

            // Update column count
            *column_counts.entry(column.clone()).or_insert(0) += 1;

            // Add to swimlane
            if let Some(swimlane_map) = swimlane_data.get_mut(&swimlane) {
                swimlane_map.entry(column).or_insert_with(Vec::new).push(WorkItemSummary {
                    id: item.id.clone(),
                    number: item.number as u32,
                    title: item.title.clone(),
                    state: item.state.clone(),
                    labels: item.labels_vec(),
                    assignees: Some(item.assignees_vec()),
                    created_at: item.created_at.clone(),
                    updated_at: item.updated_at.clone(),
                    item_type: item.item_type.clone(),
                    parent_id: item.parent_id.clone(),
                    board_column: item.board_column.clone(),
                    position: item.position,
                    story_points: item.story_points,
                    epic_color: item.epic_color.clone(),
                });
            }
        }

        // Build swimlanes response
        let swimlanes: Vec<BoardSwimlane> = ["epic", "story", "task", "subtask", "bug"]
            .iter()
            .map(|&item_type| {
                let columns = swimlane_data.get(item_type).cloned().unwrap_or_default();
                let total_items = columns.values().map(|v| v.len()).sum();
                BoardSwimlane {
                    item_type: item_type.to_string(),
                    label: match item_type {
                        "epic" => "Epics".to_string(),
                        "story" => "Stories".to_string(),
                        "task" => "Tasks".to_string(),
                        "subtask" => "Subtasks".to_string(),
                        "bug" => "Bugs".to_string(),
                        _ => item_type.to_string(),
                    },
                    columns,
                    total_items,
                }
            })
            .collect();

        // Build column info
        let columns = vec![
            BoardColumnInfo { id: "backlog".into(), name: "backlog".into(), display_name: "Backlog".into() },
            BoardColumnInfo { id: "todo".into(), name: "todo".into(), display_name: "To Do".into() },
            BoardColumnInfo { id: "in_progress".into(), name: "in_progress".into(), display_name: "In Progress".into() },
            BoardColumnInfo { id: "in_review".into(), name: "in_review".into(), display_name: "In Review".into() },
            BoardColumnInfo { id: "testing".into(), name: "testing".into(), display_name: "Testing".into() },
            BoardColumnInfo { id: "done".into(), name: "done".into(), display_name: "Done".into() },
        ];

        let response = GetBoardResponse {
            columns,
            swimlanes,
            column_counts,
            total_items: items.len(),
        };

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// Get column counts for a project's board.
    #[tool(description = "Get the count of work items in each board column for a project. Returns a lightweight summary useful for dashboards and quick status checks.")]
    async fn get_column_counts(
        &self,
        params: Parameters<GetColumnCountsParams>,
    ) -> Result<CallToolResult, McpError> {
        info!("Getting column counts for project: {}", params.0.project);
        let project_store = self.require_project_store()?;

        let store = project_store.read().await;

        // Get project
        let project = store.get_project(&params.0.project).await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .ok_or_else(|| McpError::invalid_params(
                format!("Project '{}' not found", params.0.project),
                None,
            ))?;

        // Get column counts
        let counts_vec = store.count_work_items_by_column(&project.id).await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let counts: std::collections::HashMap<String, usize> = counts_vec
            .into_iter()
            .map(|(k, v)| (k, v as usize))
            .collect();

        let total: usize = counts.values().sum();

        let response = GetColumnCountsResponse {
            counts,
            total,
        };

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// Move a work item card to a different board column and/or position.
    #[tool(description = "Move a work item card to a different board column. Use this to update workflow status (e.g., move from 'backlog' to 'in_progress').")]
    async fn move_card(
        &self,
        params: Parameters<MoveCardParams>,
    ) -> Result<CallToolResult, McpError> {
        info!("Moving card {} to column {} in project {}", params.0.item, params.0.to_column, params.0.project);
        let project_store = self.require_project_store()?;

        let store = project_store.write().await;

        // Validate column
        let valid_columns = ["backlog", "todo", "in_progress", "in_review", "testing", "done"];
        if !valid_columns.contains(&params.0.to_column.as_str()) {
            return Err(McpError::invalid_params(
                format!("Invalid column '{}'. Must be one of: {:?}", params.0.to_column, valid_columns),
                None,
            ));
        }

        // Get project
        let project = store.get_project(&params.0.project).await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .ok_or_else(|| McpError::invalid_params(
                format!("Project '{}' not found", params.0.project),
                None,
            ))?;

        // Get work item
        let item = if let Ok(num) = params.0.item.parse::<u32>() {
            store.get_work_item_by_number(&project.id, num).await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?
        } else {
            store.get_work_item(&params.0.item).await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?
        };

        match item {
            Some(i) => {
                let from_column = i.board_column.clone();
                let to_position = params.0.to_position.unwrap_or(0);

                // Update position
                store.update_work_item_position(&i.id, &params.0.to_column, to_position).await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                let item_id = i.id.clone();
                let project_id = project.id.clone();
                let to_column = params.0.to_column.clone();

                drop(store); // Release lock before emitting event

                // Emit event
                if let Some(bus) = &self.event_bus {
                    bus.issue_updated(&project_id, &item_id);
                }

                let response = MoveCardResponse {
                    success: true,
                    item_id,
                    from_column,
                    to_column,
                    to_position,
                    error: None,
                };

                let json = serde_json::to_string_pretty(&response)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            None => {
                let response = MoveCardResponse {
                    success: false,
                    item_id: params.0.item.clone(),
                    from_column: String::new(),
                    to_column: params.0.to_column.clone(),
                    to_position: params.0.to_position.unwrap_or(0),
                    error: Some(format!("Work item '{}' not found", params.0.item)),
                };

                let json = serde_json::to_string_pretty(&response)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
        }
    }

    /// Get child work items for a parent work item (e.g., subtasks under a story).
    #[tool(description = "Get child work items for a parent work item. Useful for viewing subtasks under a story, or tasks under an epic.")]
    async fn get_child_work_items(
        &self,
        params: Parameters<GetChildWorkItemsParams>,
    ) -> Result<CallToolResult, McpError> {
        info!("Getting child work items for parent: {}", params.0.parent_id);
        let project_store = self.require_project_store()?;

        let store = project_store.read().await;

        // Get project
        let _project = store.get_project(&params.0.project).await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .ok_or_else(|| McpError::invalid_params(
                format!("Project '{}' not found", params.0.project),
                None,
            ))?;

        // Get child work items
        let children = store.get_child_work_items(&params.0.parent_id).await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let child_summaries: Vec<WorkItemSummary> = children
            .iter()
            .map(|item| WorkItemSummary {
                id: item.id.clone(),
                number: item.number as u32,
                title: item.title.clone(),
                state: item.state.clone(),
                labels: item.labels_vec(),
                assignees: Some(item.assignees_vec()),
                created_at: item.created_at.clone(),
                updated_at: item.updated_at.clone(),
                item_type: item.item_type.clone(),
                parent_id: item.parent_id.clone(),
                board_column: item.board_column.clone(),
                position: item.position,
                story_points: item.story_points,
                epic_color: item.epic_color.clone(),
            })
            .collect();

        let response = GetChildWorkItemsResponse {
            parent_id: params.0.parent_id.clone(),
            children: child_summaries,
            total: children.len(),
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

    /// Search within a project's work items and linked knowledge.
    #[tool(description = "Search within a project's scope across work items and linked knowledge entries.")]
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

        let mut item_results = Vec::new();
        let mut knowledge_results = Vec::new();

        // Search work items (list_work_items takes project_id, state, limit, offset)
        if search_type == "all" || search_type == "work_items" {
            let state_filter = if include_closed { None } else { Some("open") };
            let items = store.list_work_items(&project.id, state_filter, 10000, 0).await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;

            for item in items {
                let score = calculate_text_score(&params.0.query, &item.title, item.body.as_deref());
                if score > 0.0 {
                    let excerpt = item.body.as_ref().and_then(|b| generate_excerpt(b, &params.0.query, 50));
                    let labels_list = item.labels_vec();

                    item_results.push(WorkItemSearchResultItem {
                        id: item.id,
                        number: item.number as u32,
                        title: item.title,
                        excerpt,
                        state: item.state,
                        labels: labels_list,
                        score,
                    });
                }
            }
            item_results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
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
        item_results.truncate(limit);
        knowledge_results.truncate(limit);

        let total = item_results.len() + knowledge_results.len();

        let response = SearchProjectResponse {
            total,
            work_items: item_results,
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
