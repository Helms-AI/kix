//! MCP server implementation for RAG (Retrieval Augmented Generation) system.
//!
//! This module provides 8 domain-agnostic tools for AI agents to interact with
//! the knowledge base:
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
use tokio::sync::Mutex;
use tracing::info;
use uuid::Uuid;

use kix_embeddings::{DocumentChunker, EmbeddingGenerator};
use kix_jobs::{Job, JobConfig, JobQueue, JobState, JobType};
use kix_parser::{Entry, EntryType, PdfParser, SourceType};
use kix_crawler::ContentExtractor;
use kix_store::search::SearchFilters;
use kix_store::KixStore;

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
    store: Arc<Mutex<KixStore>>,
    embedder: Arc<Mutex<EmbeddingGenerator>>,
    http_client: HttpClient,
    /// Job queue for async indexing operations
    job_queue: Arc<JobQueue>,
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl KixMcpServer {
    /// Creates a new MCP server with the given store, embedder, and job queue.
    pub fn new(store: KixStore, embedder: EmbeddingGenerator, job_queue: Arc<JobQueue>) -> Self {
        Self {
            store: Arc::new(Mutex::new(store)),
            embedder: Arc::new(Mutex::new(embedder)),
            http_client: HttpClient::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
            job_queue,
            tool_router: Self::tool_router(),
        }
    }

    // =========================================================================
    // RETRIEVAL TOOLS
    // =========================================================================

    /// Unified search across all indexed content.
    #[tool(description = "Search the knowledge base using natural language. Returns relevant chunks with scores. Use 'get_context' with page_id to retrieve full page content for RAG synthesis.")]
    async fn search(
        &self,
        params: Parameters<SearchParams>,
    ) -> Result<CallToolResult, McpError> {
        let query = &params.0.query;
        info!("Search: {}", query);

        let limit = params.0.limit.unwrap_or(10).min(100);
        let offset = params.0.offset.unwrap_or(0);
        let mode = params.0.mode.clone().unwrap_or_default();

        // Convert QueryFilters to SearchFilters
        let filters = params.0.filters.as_ref().map(|f| SearchFilters {
            entry_type: f.entry_type.clone(),
            chunk_type: f.chunk_type.clone(),
            tag: f.tag.clone(),
            source_domain: f.source_domain.clone(),
        }).unwrap_or_default();

        // Generate embedding for vector/hybrid search
        let embedding = match mode {
            SearchMode::Text => vec![], // Not needed for text-only
            _ => {
                let mut embedder = self.embedder.lock().await;
                embedder
                    .embed_query(query)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?
            }
        };

        // Perform search based on mode
        let store = self.store.lock().await;
        let results = match mode {
            SearchMode::Hybrid => {
                store
                    .hybrid_search(query, &embedding, limit + offset, &filters)
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?
            }
            SearchMode::Vector => {
                store
                    .vector_search(&embedding, limit + offset, &filters)
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?
            }
            SearchMode::Text => {
                store
                    .text_search(query, limit + offset, &filters)
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?
            }
        };

        // Apply pagination
        let total_count = results.len();
        let paginated: Vec<_> = results.into_iter().skip(offset).take(limit).collect();
        let has_more = total_count > offset + limit;

        // Convert to response format
        let items: Vec<SearchResultItem> = paginated
            .into_iter()
            .map(|r| SearchResultItem {
                chunk_id: r.chunk_id,
                entry_id: r.entry_id,
                page_id: r.page_id,
                text: r.text,
                score: r.score,
                entry_title: r.entry_title,
                source_url: r.source_domain.map(|d| format!("https://{}", d)),
            })
            .collect();

        let response = SearchResponse {
            results: items,
            total_count,
            has_more,
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

        let store = self.store.lock().await;

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

        let store = self.store.lock().await;

        // Get entry by ID
        let entry = store
            .get_entry_by_id(&params.0.id)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        match entry {
            Some(e) => {
                // Optionally get chunks
                let chunks = if params.0.include_chunks.unwrap_or(false) {
                    let chunk_list = store
                        .get_chunks_by_entry_id(&params.0.id)
                        .await
                        .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                    Some(
                        chunk_list
                            .into_iter()
                            .map(|c| ChunkInfo {
                                chunk_id: c.chunk_id,
                                chunk_index: c.chunk_index.unwrap_or(0),
                                chunk_type: c.chunk_type,
                                text: c.text,
                            })
                            .collect(),
                    )
                } else {
                    None
                };

                let doc = Document {
                    id: e.id,
                    title: e.title,
                    description: e.description,
                    entry_type: e.entry_type,
                    source_url: e.source_path,
                    source_domain: e.source_domain,
                    tags: e.tags,
                    created_at: e.created_at,
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
            let store = self.store.lock().await;
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
            let store = self.store.lock().await;
            store.delete_chunks_by_entry(&entry.id).await.ok();
            store.delete_entry(&entry.id).await.ok();
        }

        // Chunk and embed
        let chunker = DocumentChunker::with_defaults();
        let chunks = chunker.chunk(&entry);

        let embeddings = {
            let mut embedder = self.embedder.lock().await;
            let texts: Vec<&str> = chunks.iter().map(|c| c.text.as_str()).collect();
            embedder
                .embed_texts(&texts)
                .map_err(|e| McpError::internal_error(format!("Embedding failed: {}", e), None))?
        };

        // Store entry and chunks
        {
            let store = self.store.lock().await;
            store
                .insert_entries(&[entry.clone()])
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
            store
                .insert_chunks(&chunks, &embeddings)
                .await
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
            let store = self.store.lock().await;

            if let Some(ref tag) = filter.tag {
                let entries = store
                    .list_by_tag(tag)
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                ids_to_delete.extend(entries.into_iter().map(|e| e.id));
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
            let store = self.store.lock().await;
            for id in &ids_to_delete {
                if store.entry_exists(id).await.unwrap_or(false) {
                    let chunks = store.get_chunks_by_entry_id(id).await.unwrap_or_default();
                    chunks_deleted += chunks.len();
                    actually_deleted.push(id.clone());
                }
            }
        } else {
            // Actually delete
            let store = self.store.lock().await;
            for id in &ids_to_delete {
                if store.entry_exists(id).await.unwrap_or(false) {
                    let chunks = store.get_chunks_by_entry_id(id).await.unwrap_or_default();
                    chunks_deleted += chunks.len();

                    store.delete_chunks_by_entry(id).await.ok();
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

        let store = self.store.lock().await;

        let document_count = store
            .entry_count()
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let chunk_count = store
            .chunk_count()
            .await
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
