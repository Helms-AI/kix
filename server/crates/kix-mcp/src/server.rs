//! MCP server implementation for EIP Knowledge System.

use reqwest::Client as HttpClient;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Content, ServerCapabilities, ServerInfo};
use rmcp::schemars;
use rmcp::tool;
use rmcp::tool_router;
use rmcp::ErrorData as McpError;
use rmcp::ServerHandler;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};

use kix_embeddings::{DocumentChunker, EmbeddingGenerator};
use kix_parser::{Entry, EntryType, PdfParser, SourceType};
use kix_crawler::ContentExtractor;
use kix_store::search::{EntrySummary, SearchFilters, SearchResult};
use kix_store::KixStore;

/// Search patterns request.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchPatternsParams {
    /// Search query text
    #[schemars(description = "Natural language search query for finding patterns")]
    pub query: String,
    /// Maximum number of results (default: 5)
    #[schemars(description = "Maximum number of results to return")]
    pub limit: Option<usize>,
    /// Filter by pattern type (messaging, conversation)
    #[schemars(description = "Filter by pattern type: 'messaging' or 'conversation'")]
    pub pattern_type: Option<String>,
}

/// Get pattern request.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetPatternParams {
    /// Pattern name to retrieve
    #[schemars(description = "Name of the pattern to retrieve")]
    pub name: String,
}

/// List patterns request.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListPatternsParams {
    /// Category to filter by
    #[schemars(description = "Category to filter patterns by (e.g., 'Message Routing', 'Message Channel')")]
    pub category: Option<String>,
    /// Pattern type to filter by
    #[schemars(description = "Pattern type filter: 'messaging' or 'conversation'")]
    pub pattern_type: Option<String>,
}

/// Find related patterns request.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct FindRelatedParams {
    /// Pattern name to find related patterns for
    #[schemars(description = "Name of the pattern to find related patterns for")]
    pub pattern_name: String,
}

/// Search by problem request.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchByProblemParams {
    /// Problem description to find patterns for
    #[schemars(description = "Description of the problem you're trying to solve")]
    pub problem_description: String,
    /// Maximum number of results
    #[schemars(description = "Maximum number of results to return")]
    pub limit: Option<usize>,
}

/// Explain pattern request.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExplainPatternParams {
    /// Pattern name to explain
    #[schemars(description = "Name of the pattern to explain")]
    pub pattern_name: String,
    /// Focus area for explanation
    #[schemars(description = "Focus area: 'usage', 'implementation', or 'tradeoffs'")]
    pub focus: Option<String>,
}

/// Get category overview request.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetCategoryOverviewParams {
    /// Category name
    #[schemars(description = "Name of the category to get an overview of")]
    pub category: String,
}

/// Compare patterns request.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ComparePatternsParams {
    /// First pattern to compare
    #[schemars(description = "Name of the first pattern to compare")]
    pub pattern_a: String,
    /// Second pattern to compare
    #[schemars(description = "Name of the second pattern to compare")]
    pub pattern_b: String,
    /// Aspects to compare
    #[schemars(description = "Aspects to compare: use_cases, trade_offs, complexity")]
    pub aspects: Option<Vec<String>>,
}

/// Suggest architecture request.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SuggestArchitectureParams {
    /// System description
    #[schemars(description = "Description of the system you're building")]
    pub system_description: String,
    /// Constraints to consider
    #[schemars(description = "Constraints like 'high_throughput', 'fault_tolerant', 'simple'")]
    pub constraints: Option<Vec<String>>,
    /// Maximum number of pattern suggestions
    #[schemars(description = "Maximum number of patterns to suggest")]
    pub limit: Option<usize>,
}

/// Pattern sequence request.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PatternSequenceParams {
    /// Starting pattern
    #[schemars(description = "Starting pattern in the sequence")]
    pub start_pattern: String,
    /// Ending pattern
    #[schemars(description = "Ending pattern in the sequence")]
    pub end_pattern: Option<String>,
    /// Include alternatives
    #[schemars(description = "Whether to include alternative sequences")]
    pub include_alternatives: Option<bool>,
}

/// Search by technology request.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchByTechnologyParams {
    /// Technology to search for
    #[schemars(description = "Technology name like 'Apache Camel', 'Spring Integration', 'MuleSoft'")]
    pub technology: String,
    /// Pattern type filter
    #[schemars(description = "Pattern type filter: 'messaging' or 'conversation'")]
    pub pattern_type: Option<String>,
    /// Maximum results
    #[schemars(description = "Maximum number of results")]
    pub limit: Option<usize>,
}

// =============================================================================
// Indexing Tool Parameters
// =============================================================================

/// Content source for indexing - specifies where to get the content from.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ContentSource {
    /// Raw text content to index directly
    #[schemars(description = "Raw text or markdown content to index")]
    RawText(String),
    /// Local file path (HTML or PDF)
    #[schemars(description = "Absolute path to an HTML or PDF file")]
    FilePath(String),
    /// URL to fetch and index
    #[schemars(description = "HTTP/HTTPS URL to fetch and index")]
    Url(String),
}

/// Pattern-specific schema with structured fields.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct PatternSchema {
    /// Pattern title (required)
    #[schemars(description = "Pattern name, e.g., 'Aggregator', 'Content-Based Router'")]
    pub title: String,
    /// Problem statement this pattern addresses
    #[schemars(description = "The problem this pattern solves")]
    pub problem: Option<String>,
    /// Solution description
    #[schemars(description = "How the pattern solves the problem")]
    pub solution: Option<String>,
    /// Categories for organization
    #[schemars(description = "Categories like 'Message Routing', 'Message Channel'")]
    pub categories: Option<Vec<String>>,
    /// Pattern type
    #[schemars(description = "Pattern type: messaging, conversation, integration_style, article, case_study, pdf, other")]
    pub pattern_type: Option<String>,
    /// Related pattern names
    #[schemars(description = "Names of related patterns")]
    pub related_patterns: Option<Vec<String>>,
    /// Keywords for search
    #[schemars(description = "Keywords for improving searchability")]
    pub keywords: Option<Vec<String>>,
}

/// Generic document schema for arbitrary content.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct GenericSchema {
    /// Document title (required)
    #[schemars(description = "Title of the document")]
    pub title: String,
    /// Tags for categorization
    #[schemars(description = "Tags for categorization")]
    pub tags: Option<Vec<String>>,
    /// Source attribution
    #[schemars(description = "Source URL or reference")]
    pub source: Option<String>,
    /// Description override
    #[schemars(description = "Short description (auto-extracted if not provided)")]
    pub description: Option<String>,
}

/// Document schema type - pattern or generic.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DocumentSchema {
    /// Pattern schema with problem/solution structure
    Pattern(PatternSchema),
    /// Generic document schema
    Generic(GenericSchema),
}

/// Index a single document request.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct IndexDocumentParams {
    /// Content source - raw text, file path, or URL
    #[schemars(description = "Source of content: {\"raw_text\": \"...\"}, {\"file_path\": \"/path/to/file\"}, or {\"url\": \"https://...\"}")]
    pub source: ContentSource,
    /// Document schema with metadata
    #[schemars(description = "Document schema: {\"pattern\": {...}} or {\"generic\": {...}}")]
    pub schema: DocumentSchema,
    /// Custom document ID (auto-generated if not provided)
    #[schemars(description = "Custom document ID (auto-generated from title if omitted)")]
    pub id: Option<String>,
    /// Replace existing document with same ID
    #[schemars(description = "Replace existing document with same ID (default: false)")]
    pub replace_existing: Option<bool>,
}

/// Single document in a batch.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct BatchDocument {
    /// Content source
    #[schemars(description = "Source of content")]
    pub source: ContentSource,
    /// Document schema
    #[schemars(description = "Document schema with metadata")]
    pub schema: DocumentSchema,
    /// Optional custom ID
    #[schemars(description = "Custom document ID")]
    pub id: Option<String>,
}

/// Index multiple documents request.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct IndexBatchParams {
    /// List of documents to index (max 50)
    #[schemars(description = "Array of documents to index (max 50)")]
    pub documents: Vec<BatchDocument>,
    /// Continue on individual failures
    #[schemars(description = "Continue processing if individual documents fail (default: true)")]
    pub continue_on_error: Option<bool>,
    /// Create indexes after batch complete
    #[schemars(description = "Rebuild search indexes after batch (default: true)")]
    pub create_indexes: Option<bool>,
}

/// Delete document request.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteDocumentParams {
    /// Document ID to delete
    #[schemars(description = "ID of the document to delete")]
    pub id: String,
    /// Also delete associated chunks
    #[schemars(description = "Delete associated chunks (default: true)")]
    pub delete_chunks: Option<bool>,
}

/// Get index status request.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetIndexStatusParams {
    /// Include detailed breakdown
    #[schemars(description = "Include breakdown by document type")]
    pub detailed: Option<bool>,
}

/// Result of indexing a single document.
#[derive(Debug, Serialize)]
pub struct IndexResult {
    pub success: bool,
    pub document_id: String,
    pub chunks_created: usize,
    pub message: String,
}

/// Result for a single document in a batch.
#[derive(Debug, Serialize)]
pub struct BatchDocumentResult {
    pub index: usize,
    pub success: bool,
    pub document_id: Option<String>,
    pub chunks_created: Option<usize>,
    pub error: Option<String>,
}

/// Result of batch indexing.
#[derive(Debug, Serialize)]
pub struct BatchIndexResult {
    pub total: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub results: Vec<BatchDocumentResult>,
    pub indexes_created: bool,
}

/// The main Knowledge Indexer MCP server.
#[derive(Clone)]
pub struct KixMcpServer {
    store: Arc<Mutex<KixStore>>,
    embedder: Arc<Mutex<EmbeddingGenerator>>,
    http_client: HttpClient,
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl KixMcpServer {
    /// Creates a new MCP server with the given store and embedder.
    pub fn new(store: KixStore, embedder: EmbeddingGenerator) -> Self {
        Self {
            store: Arc::new(Mutex::new(store)),
            embedder: Arc::new(Mutex::new(embedder)),
            http_client: HttpClient::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
            tool_router: Self::tool_router(),
        }
    }

    /// Semantic search across all patterns.
    #[tool(description = "Search for patterns using natural language. Returns relevant patterns based on semantic similarity.")]
    async fn search_patterns(
        &self,
        params: Parameters<SearchPatternsParams>,
    ) -> Result<CallToolResult, McpError> {
        info!("Searching patterns for: {}", params.0.query);

        let limit = params.0.limit.unwrap_or(5);
        let filters = SearchFilters {
            entry_type: params.0.pattern_type.clone(),
            chunk_type: None,
            tag: None,
            source_domain: None,
        };

        // Generate embedding for the query
        let embedding = {
            let mut embedder = self.embedder.lock().await;
            embedder
                .embed_query(&params.0.query)
                .map_err(|e| McpError::internal_error(e.to_string(), None))?
        };

        // Perform hybrid search
        let store = self.store.lock().await;
        let results = store
            .hybrid_search(&params.0.query, &embedding, limit, &filters)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let response = format_search_results(&results);
        Ok(CallToolResult::success(vec![Content::text(response)]))
    }

    /// Retrieve a specific pattern by name.
    #[tool(description = "Get detailed information about a specific pattern by its name.")]
    async fn get_pattern(
        &self,
        params: Parameters<GetPatternParams>,
    ) -> Result<CallToolResult, McpError> {
        info!("Getting pattern: {}", params.0.name);

        let store = self.store.lock().await;
        let pattern = store
            .get_pattern_by_name(&params.0.name)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        match pattern {
            Some(p) => {
                let response = format_pattern_summary(&p);
                Ok(CallToolResult::success(vec![Content::text(response)]))
            }
            None => {
                let msg = format!("Pattern '{}' not found", params.0.name);
                Ok(CallToolResult::success(vec![Content::text(msg)]))
            }
        }
    }

    /// List patterns by category or type.
    #[tool(description = "List patterns filtered by category or pattern type.")]
    async fn list_patterns(
        &self,
        params: Parameters<ListPatternsParams>,
    ) -> Result<CallToolResult, McpError> {
        info!(
            "Listing patterns - category: {:?}, type: {:?}",
            params.0.category, params.0.pattern_type
        );

        let store = self.store.lock().await;

        let patterns = if let Some(ref category) = params.0.category {
            store
                .list_by_category(category)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?
        } else if let Some(ref pattern_type) = params.0.pattern_type {
            store
                .list_by_pattern_type(pattern_type)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?
        } else {
            store
                .list_all_patterns()
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?
        };

        let response = format_pattern_list(&patterns);
        Ok(CallToolResult::success(vec![Content::text(response)]))
    }

    /// Find patterns related to a given pattern.
    #[tool(description = "Find patterns that are related to or commonly used with a given pattern.")]
    async fn find_related(
        &self,
        params: Parameters<FindRelatedParams>,
    ) -> Result<CallToolResult, McpError> {
        info!("Finding related patterns for: {}", params.0.pattern_name);

        // Search for the pattern and its related patterns
        let store = self.store.lock().await;
        let pattern = store
            .get_pattern_by_name(&params.0.pattern_name)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        match pattern {
            Some(p) => {
                // Use semantic search to find related patterns
                let query = format!("patterns related to {} {}", p.title, p.description);
                drop(store); // Release store lock

                let embedding = {
                    let mut embedder = self.embedder.lock().await;
                    embedder
                        .embed_query(&query)
                        .map_err(|e| McpError::internal_error(e.to_string(), None))?
                };

                let store = self.store.lock().await;
                let filters = SearchFilters {
                    entry_type: Some(p.entry_type.clone()),
                    ..Default::default()
                };
                let results = store
                    .vector_search(&embedding, 6, &filters)
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                // Filter out the original pattern
                let related: Vec<_> = results
                    .into_iter()
                    .filter(|r| r.entry_title != params.0.pattern_name)
                    .take(5)
                    .collect();

                let response = format!(
                    "## Related Patterns for '{}'\n\n{}",
                    params.0.pattern_name,
                    format_search_results(&related)
                );
                Ok(CallToolResult::success(vec![Content::text(response)]))
            }
            None => {
                let msg = format!("Pattern '{}' not found", params.0.pattern_name);
                Ok(CallToolResult::success(vec![Content::text(msg)]))
            }
        }
    }

    /// Find patterns that solve a specific problem.
    #[tool(description = "Find patterns that address a specific integration problem or challenge.")]
    async fn search_by_problem(
        &self,
        params: Parameters<SearchByProblemParams>,
    ) -> Result<CallToolResult, McpError> {
        info!(
            "Searching for patterns to solve: {}",
            params.0.problem_description
        );

        let limit = params.0.limit.unwrap_or(5);

        // Enhance query for problem-focused search
        let query = format!(
            "problem: {} solution pattern",
            params.0.problem_description
        );

        let embedding = {
            let mut embedder = self.embedder.lock().await;
            embedder
                .embed_query(&query)
                .map_err(|e| McpError::internal_error(e.to_string(), None))?
        };

        let store = self.store.lock().await;
        let filters = SearchFilters {
            chunk_type: Some("problem".to_string()),
            ..Default::default()
        };
        let results = store
            .hybrid_search(&query, &embedding, limit, &filters)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let response = format!(
            "## Patterns for Problem: {}\n\n{}",
            params.0.problem_description,
            format_search_results(&results)
        );
        Ok(CallToolResult::success(vec![Content::text(response)]))
    }

    /// Get detailed explanation of a pattern.
    #[tool(description = "Get a detailed explanation of a pattern with optional focus on specific aspects.")]
    async fn explain_pattern(
        &self,
        params: Parameters<ExplainPatternParams>,
    ) -> Result<CallToolResult, McpError> {
        info!(
            "Explaining pattern: {} (focus: {:?})",
            params.0.pattern_name, params.0.focus
        );

        let store = self.store.lock().await;
        let pattern = store
            .get_pattern_by_name(&params.0.pattern_name)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        match pattern {
            Some(p) => {
                // Build explanation based on focus
                let focus = params.0.focus.clone().unwrap_or_else(|| "usage".to_string());

                let response = format!(
                    "## {} Pattern\n\n\
                    **Type:** {}\n\
                    **Tags:** {}\n\n\
                    **Description:**\n{}\n\n\
                    **Focus: {}**\n\n\
                    This pattern is used in the context of enterprise integration \
                    to address messaging and communication challenges.\n",
                    p.title,
                    p.entry_type,
                    p.tags.join(", "),
                    p.description,
                    focus
                );
                Ok(CallToolResult::success(vec![Content::text(response)]))
            }
            None => {
                let msg = format!("Pattern '{}' not found", params.0.pattern_name);
                Ok(CallToolResult::success(vec![Content::text(msg)]))
            }
        }
    }

    /// Get overview of a pattern category.
    #[tool(description = "Get an overview of a pattern category including all patterns within it.")]
    async fn get_category_overview(
        &self,
        params: Parameters<GetCategoryOverviewParams>,
    ) -> Result<CallToolResult, McpError> {
        info!("Getting category overview: {}", params.0.category);

        let store = self.store.lock().await;
        let patterns = store
            .list_by_category(&params.0.category)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        if patterns.is_empty() {
            return Ok(CallToolResult::success(vec![Content::text(format!(
                "No patterns found in category '{}'",
                params.0.category
            ))]));
        }

        let pattern_list: Vec<String> = patterns
            .iter()
            .map(|p| format!("- **{}**: {}", p.title, p.description))
            .collect();

        let response = format!(
            "## Category: {}\n\n\
            **Pattern Count:** {}\n\n\
            **Patterns:**\n{}",
            params.0.category,
            patterns.len(),
            pattern_list.join("\n")
        );
        Ok(CallToolResult::success(vec![Content::text(response)]))
    }

    /// Compare two patterns side-by-side.
    #[tool(description = "Compare two patterns side-by-side, showing differences in use cases, trade-offs, and when to choose each.")]
    async fn compare_patterns(
        &self,
        params: Parameters<ComparePatternsParams>,
    ) -> Result<CallToolResult, McpError> {
        info!(
            "Comparing patterns: {} vs {}",
            params.0.pattern_a, params.0.pattern_b
        );

        let store = self.store.lock().await;
        let pattern_a = store
            .get_pattern_by_name(&params.0.pattern_a)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let pattern_b = store
            .get_pattern_by_name(&params.0.pattern_b)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        match (pattern_a, pattern_b) {
            (Some(a), Some(b)) => {
                let aspects = params.0
                    .aspects
                    .clone()
                    .unwrap_or_else(|| vec!["use_cases".to_string(), "trade_offs".to_string()]);

                let mut comparison = format!(
                    "## Pattern Comparison: {} vs {}\n\n",
                    a.title, b.title
                );

                comparison.push_str(&format!(
                    "### {}\n\
                    **Type:** {}\n\
                    **Tags:** {}\n\
                    **Description:** {}\n\n",
                    a.title,
                    a.entry_type,
                    a.tags.join(", "),
                    a.description
                ));

                comparison.push_str(&format!(
                    "### {}\n\
                    **Type:** {}\n\
                    **Tags:** {}\n\
                    **Description:** {}\n\n",
                    b.title,
                    b.entry_type,
                    b.tags.join(", "),
                    b.description
                ));

                comparison.push_str("### Comparison Aspects\n");
                for aspect in aspects {
                    comparison.push_str(&format!("- **{}**: Both patterns serve different purposes in the integration architecture.\n", aspect));
                }

                Ok(CallToolResult::success(vec![Content::text(comparison)]))
            }
            (None, _) => Ok(CallToolResult::success(vec![Content::text(format!(
                "Pattern '{}' not found",
                params.0.pattern_a
            ))])),
            (_, None) => Ok(CallToolResult::success(vec![Content::text(format!(
                "Pattern '{}' not found",
                params.0.pattern_b
            ))])),
        }
    }

    /// Suggest patterns for a system architecture.
    #[tool(description = "Given a system description, suggest relevant patterns and how they might work together.")]
    async fn suggest_architecture(
        &self,
        params: Parameters<SuggestArchitectureParams>,
    ) -> Result<CallToolResult, McpError> {
        info!(
            "Suggesting architecture for: {}",
            params.0.system_description
        );

        let limit = params.0.limit.unwrap_or(10);

        // Build search query from system description
        let mut query = params.0.system_description.clone();
        if let Some(ref constraints) = params.0.constraints {
            query.push_str(" ");
            query.push_str(&constraints.join(" "));
        }

        let embedding = {
            let mut embedder = self.embedder.lock().await;
            embedder
                .embed_query(&query)
                .map_err(|e| McpError::internal_error(e.to_string(), None))?
        };

        let store = self.store.lock().await;
        let results = store
            .hybrid_search(&query, &embedding, limit, &SearchFilters::default())
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let mut response = format!(
            "## Suggested Architecture for:\n> {}\n\n",
            params.0.system_description
        );

        if let Some(ref constraints) = params.0.constraints {
            response.push_str(&format!("**Constraints:** {}\n\n", constraints.join(", ")));
        }

        response.push_str("### Recommended Patterns\n\n");
        for (i, result) in results.iter().enumerate() {
            response.push_str(&format!(
                "{}. **{}** (Score: {:.2})\n   {}\n\n",
                i + 1,
                result.entry_title,
                result.score,
                truncate_text(&result.text, 150)
            ));
        }

        response.push_str("\n### Suggested Integration Flow\n");
        response.push_str("Consider combining these patterns to create a robust integration architecture.\n");

        Ok(CallToolResult::success(vec![Content::text(response)]))
    }

    /// Show typical pattern sequences.
    #[tool(description = "Show typical sequence/flow of patterns in a pipeline, useful for understanding how patterns compose.")]
    async fn pattern_sequence(
        &self,
        params: Parameters<PatternSequenceParams>,
    ) -> Result<CallToolResult, McpError> {
        info!(
            "Finding pattern sequence from: {} to {:?}",
            params.0.start_pattern, params.0.end_pattern
        );

        let store = self.store.lock().await;
        let start = store
            .get_pattern_by_name(&params.0.start_pattern)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        match start {
            Some(start_pattern) => {
                let mut response = format!(
                    "## Pattern Sequence Starting from '{}'\n\n",
                    start_pattern.title
                );

                // Find patterns that commonly follow
                let query = format!(
                    "patterns that follow {} in a pipeline sequence",
                    start_pattern.title
                );
                drop(store); // Release lock

                let embedding = {
                    let mut embedder = self.embedder.lock().await;
                    embedder
                        .embed_query(&query)
                        .map_err(|e| McpError::internal_error(e.to_string(), None))?
                };

                let store = self.store.lock().await;
                let results = store
                    .vector_search(&embedding, 5, &SearchFilters::default())
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                response.push_str(&format!("**Starting Pattern:** {}\n", start_pattern.title));
                response.push_str(&format!(
                    "**Description:** {}\n\n",
                    start_pattern.description
                ));

                response.push_str("### Common Follow-up Patterns\n\n");
                for (i, result) in results.iter().enumerate() {
                    if result.entry_title != params.0.start_pattern {
                        response.push_str(&format!(
                            "{}. {} → **{}**\n",
                            i + 1,
                            start_pattern.title,
                            result.entry_title
                        ));
                    }
                }

                if params.0.include_alternatives.unwrap_or(false) {
                    response.push_str("\n### Alternative Sequences\n");
                    response.push_str(
                        "Alternative patterns may be used depending on specific requirements.\n",
                    );
                }

                Ok(CallToolResult::success(vec![Content::text(response)]))
            }
            None => Ok(CallToolResult::success(vec![Content::text(format!(
                "Pattern '{}' not found",
                params.0.start_pattern
            ))])),
        }
    }

    /// Search for patterns by technology.
    #[tool(description = "Find patterns with examples in specific technologies (Apache Camel, Spring Integration, MuleSoft, etc.).")]
    async fn search_by_technology(
        &self,
        params: Parameters<SearchByTechnologyParams>,
    ) -> Result<CallToolResult, McpError> {
        info!("Searching patterns for technology: {}", params.0.technology);

        let limit = params.0.limit.unwrap_or(10);
        let query = format!("{} implementation example", params.0.technology);

        let embedding = {
            let mut embedder = self.embedder.lock().await;
            embedder
                .embed_query(&query)
                .map_err(|e| McpError::internal_error(e.to_string(), None))?
        };

        let filters = SearchFilters {
            entry_type: params.0.pattern_type.clone(),
            ..Default::default()
        };

        let store = self.store.lock().await;
        let results = store
            .hybrid_search(&query, &embedding, limit, &filters)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let response = format!(
            "## Patterns for Technology: {}\n\n{}",
            params.0.technology,
            format_search_results(&results)
        );
        Ok(CallToolResult::success(vec![Content::text(response)]))
    }

    // =========================================================================
    // Indexing Tools
    // =========================================================================

    /// Index a single document from raw text, file path, or URL.
    #[tool(description = "Index a document from raw text, file path, or URL. Supports pattern schema (title, problem, solution) or generic document schema.")]
    async fn index_document(
        &self,
        params: Parameters<IndexDocumentParams>,
    ) -> Result<CallToolResult, McpError> {
        info!("Indexing document from source");

        // Process content source into an Entry
        let mut entry = self
            .process_content_source(&params.0.source, &params.0.schema)
            .await?;

        // Apply custom ID if provided
        if let Some(ref custom_id) = params.0.id {
            entry.id = custom_id.clone();
        }

        // Check for existing entry
        let exists = {
            let store = self.store.lock().await;
            store
                .entry_exists(&entry.id)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?
        };

        if exists && !params.0.replace_existing.unwrap_or(false) {
            return Ok(CallToolResult::success(vec![Content::text(format!(
                "## Document Already Exists\n\n\
                Document with ID '{}' already exists. Set `replace_existing: true` to overwrite.",
                entry.id
            ))]));
        }

        // If replacing, delete existing first
        if exists {
            let store = self.store.lock().await;
            store
                .delete_chunks_by_entry(&entry.id)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
            store
                .delete_entry(&entry.id)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
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
            document_id: entry.id.clone(),
            chunks_created: chunks.len(),
            message: format!("Document '{}' indexed successfully", entry.title),
        };

        Ok(CallToolResult::success(vec![Content::text(
            format_index_result(&result),
        )]))
    }

    /// Index multiple documents in a single operation.
    #[tool(description = "Index multiple documents in a single operation. Returns results for each document. Max 50 documents per batch.")]
    async fn index_batch(
        &self,
        params: Parameters<IndexBatchParams>,
    ) -> Result<CallToolResult, McpError> {
        info!("Batch indexing {} documents", params.0.documents.len());

        // Validate batch size
        if params.0.documents.len() > 50 {
            return Ok(CallToolResult::success(vec![Content::text(
                "## Batch Size Exceeded\n\nMaximum 50 documents per batch.",
            )]));
        }

        if params.0.documents.is_empty() {
            return Ok(CallToolResult::success(vec![Content::text(
                "## Empty Batch\n\nNo documents provided.",
            )]));
        }

        let continue_on_error = params.0.continue_on_error.unwrap_or(true);
        let create_indexes = params.0.create_indexes.unwrap_or(true);

        let mut results = Vec::new();
        let mut succeeded = 0;
        let mut failed = 0;

        for (index, batch_doc) in params.0.documents.iter().enumerate() {
            let result = self
                .process_and_index_document(&batch_doc.source, &batch_doc.schema, &batch_doc.id)
                .await;

            match result {
                Ok((doc_id, chunks_count)) => {
                    succeeded += 1;
                    results.push(BatchDocumentResult {
                        index,
                        success: true,
                        document_id: Some(doc_id),
                        chunks_created: Some(chunks_count),
                        error: None,
                    });
                }
                Err(e) => {
                    failed += 1;
                    results.push(BatchDocumentResult {
                        index,
                        success: false,
                        document_id: None,
                        chunks_created: None,
                        error: Some(e.to_string()),
                    });

                    if !continue_on_error {
                        break;
                    }
                }
            }
        }

        // Create indexes if requested and we had successes
        let indexes_created = if create_indexes && succeeded > 0 {
            let store = self.store.lock().await;
            match store.create_indexes().await {
                Ok(_) => true,
                Err(e) => {
                    warn!("Index creation failed: {}", e);
                    false
                }
            }
        } else {
            false
        };

        let batch_result = BatchIndexResult {
            total: params.0.documents.len(),
            succeeded,
            failed,
            results,
            indexes_created,
        };

        Ok(CallToolResult::success(vec![Content::text(
            format_batch_result(&batch_result),
        )]))
    }

    /// Delete a document and its chunks from the index.
    #[tool(description = "Delete a document and its chunks from the index by document ID.")]
    async fn delete_document(
        &self,
        params: Parameters<DeleteDocumentParams>,
    ) -> Result<CallToolResult, McpError> {
        info!("Deleting document: {}", params.0.id);

        let delete_chunks = params.0.delete_chunks.unwrap_or(true);

        // Check if entry exists
        let exists = {
            let store = self.store.lock().await;
            store
                .entry_exists(&params.0.id)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?
        };

        if !exists {
            return Ok(CallToolResult::success(vec![Content::text(format!(
                "## Document Not Found\n\nNo document with ID '{}' exists.",
                params.0.id
            ))]));
        }

        // Delete chunks first if requested
        if delete_chunks {
            let store = self.store.lock().await;
            store
                .delete_chunks_by_entry(&params.0.id)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        }

        // Delete entry
        {
            let store = self.store.lock().await;
            store
                .delete_entry(&params.0.id)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        }

        Ok(CallToolResult::success(vec![Content::text(format!(
            "## Document Deleted\n\n\
            **Document ID:** {}\n\
            **Chunks Deleted:** {}",
            params.0.id,
            if delete_chunks { "Yes" } else { "No" }
        ))]))
    }

    /// Get current indexing statistics.
    #[tool(description = "Get current indexing statistics including document count and chunk count.")]
    async fn get_index_status(
        &self,
        params: Parameters<GetIndexStatusParams>,
    ) -> Result<CallToolResult, McpError> {
        info!("Getting index status");

        let store = self.store.lock().await;

        let entry_count = store
            .entry_count()
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let chunk_count = store
            .chunk_count()
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let health = if entry_count == 0 {
            "empty"
        } else if chunk_count == 0 {
            "needs_reindex"
        } else {
            "healthy"
        };

        let mut response = format!(
            "## Index Status\n\n\
            **Documents:** {}\n\
            **Chunks:** {}\n\
            **Health:** {}\n",
            entry_count, chunk_count, health
        );

        if params.0.detailed.unwrap_or(false) {
            response.push_str("\n### Details\n");
            response.push_str(&format!(
                "- Average chunks per document: {:.1}\n",
                if entry_count > 0 {
                    chunk_count as f64 / entry_count as f64
                } else {
                    0.0
                }
            ));
        }

        Ok(CallToolResult::success(vec![Content::text(response)]))
    }

    // =========================================================================
    // Helper Methods for Indexing
    // =========================================================================

    /// Process content source into an Entry.
    async fn process_content_source(
        &self,
        source: &ContentSource,
        schema: &DocumentSchema,
    ) -> Result<Entry, McpError> {
        match source {
            ContentSource::RawText(text) => self.process_raw_text(text, schema),
            ContentSource::FilePath(path) => self.process_file_path(path, schema).await,
            ContentSource::Url(url) => self.process_url(url, schema).await,
        }
    }

    /// Process raw text content.
    fn process_raw_text(
        &self,
        text: &str,
        schema: &DocumentSchema,
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

        let mut entry = self.create_entry_from_schema(schema, text);

        // Compute hash
        let mut hasher = Sha256::new();
        hasher.update(text.as_bytes());
        entry.source_hash = format!("{:x}", hasher.finalize());
        entry.source_type = SourceType::Html;
        entry.source_path = format!("raw://{}", entry.id);

        Ok(entry)
    }

    /// Process file path content.
    async fn process_file_path(
        &self,
        path: &str,
        schema: &DocumentSchema,
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

                // Use ContentExtractor for consistent HTML processing
                let extractor = ContentExtractor::default();
                let url = url::Url::parse(&format!("file://{}", path))
                    .unwrap_or_else(|_| url::Url::parse("file:///unknown").unwrap());
                let extracted = extractor.extract(&content, &url);
                let mut entry = self.create_entry_from_extracted(&extracted, path);

                self.apply_schema_overrides(&mut entry, schema);
                Ok(entry)
            }
            "pdf" => {
                let parser = PdfParser::new();
                let mut entry = parser
                    .parse(path)
                    .map_err(|e| McpError::internal_error(format!("PDF parse error: {}", e), None))?;

                self.apply_schema_overrides(&mut entry, schema);
                Ok(entry)
            }
            _ => Err(McpError::invalid_params(
                format!("Unsupported file type: {}", extension),
                None,
            )),
        }
    }

    /// Process URL content.
    async fn process_url(&self, url_str: &str, schema: &DocumentSchema) -> Result<Entry, McpError> {
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

        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "text/html".to_string());

        let content = response
            .text()
            .await
            .map_err(|e| McpError::internal_error(format!("Failed to read response: {}", e), None))?;

        if content_type.contains("application/pdf") {
            // For PDFs from URLs, we'd need to save to temp file
            // For now, treat as HTML
            warn!("PDF URLs not fully supported, treating as HTML");
        }

        // Parse as HTML using ContentExtractor
        let extractor = ContentExtractor::default();
        let extracted = extractor.extract(&content, &parsed_url);
        let mut entry = self.create_entry_from_extracted(&extracted, url_str);

        self.apply_schema_overrides(&mut entry, schema);
        Ok(entry)
    }

    /// Create an Entry from ContentExtractor output.
    ///
    /// This helper function converts extracted content to an Entry
    /// for consistent indexing.
    fn create_entry_from_extracted(
        &self,
        extracted: &kix_crawler::ExtractedContent,
        source_path: &str,
    ) -> Entry {
        // Generate slug/ID from path
        let slug = source_path.to_string();
        let id = Entry::generate_id_from_path(source_path);

        // Use extracted description or derive from markdown
        let description = extracted
            .description
            .clone()
            .unwrap_or_else(|| extracted.markdown.chars().take(300).collect());

        // Determine entry type from path
        let entry_type = if source_path.contains("/blog/") || source_path.contains("/article/") {
            EntryType::Article
        } else if source_path.contains("/docs/") || source_path.contains("/documentation/") {
            EntryType::Document
        } else {
            EntryType::Document
        };

        Entry::with_id(
            id,
            extracted.title.clone(),
            source_path.to_string(),
            extracted.content_hash.clone(),
        )
        .with_description(description)
        .with_content(extracted.markdown.clone())
        .with_tags(vec![])
        .with_entry_type(entry_type)
        .with_source_type(SourceType::Html)
        .with_slug(slug)
    }

    /// Create an Entry from schema metadata.
    fn create_entry_from_schema(&self, schema: &DocumentSchema, content: &str) -> Entry {
        match schema {
            DocumentSchema::Pattern(p) => {
                let slug = slugify(&p.title);
                let entry_type = p
                    .pattern_type
                    .as_ref()
                    .map(|s| EntryType::from_str(s))
                    .unwrap_or(EntryType::Document);

                // Combine categories and keywords into tags
                let mut tags = p.categories.clone().unwrap_or_default();
                if let Some(ref keywords) = p.keywords {
                    tags.extend(keywords.clone());
                }
                tags.sort();
                tags.dedup();

                // Use problem as description if available
                let description = p.problem.clone().unwrap_or_default();

                Entry::with_id(slug.clone(), p.title.clone(), String::new(), String::new())
                    .with_description(description)
                    .with_content(content.to_string())
                    .with_tags(tags)
                    .with_entry_type(entry_type)
                    .with_source_type(SourceType::Html)
                    .with_slug(slug)
            }
            DocumentSchema::Generic(g) => {
                let slug = slugify(&g.title);
                let tags = g.tags.clone().unwrap_or_default();
                let description = g.description.clone().unwrap_or_default();
                let source_path = g.source.clone().unwrap_or_default();

                Entry::with_id(slug.clone(), g.title.clone(), source_path, String::new())
                    .with_description(description)
                    .with_content(content.to_string())
                    .with_tags(tags)
                    .with_entry_type(EntryType::Document)
                    .with_source_type(SourceType::Html)
                    .with_slug(slug)
            }
        }
    }

    /// Apply schema overrides to a parsed entry.
    fn apply_schema_overrides(&self, entry: &mut Entry, schema: &DocumentSchema) {
        match schema {
            DocumentSchema::Pattern(p) => {
                entry.title = p.title.clone();
                if let Some(ref problem) = p.problem {
                    entry.description = problem.clone();
                }
                // Combine categories and keywords into tags
                if let Some(ref categories) = p.categories {
                    entry.tags.extend(categories.clone());
                }
                if let Some(ref keywords) = p.keywords {
                    entry.tags.extend(keywords.clone());
                }
                entry.tags.sort();
                entry.tags.dedup();
                if let Some(ref pt) = p.pattern_type {
                    entry.entry_type = EntryType::from_str(pt);
                }

                // Regenerate slug
                entry.slug = slugify(&entry.title);
            }
            DocumentSchema::Generic(g) => {
                entry.title = g.title.clone();
                if let Some(ref desc) = g.description {
                    entry.description = desc.clone();
                }
                if let Some(ref tags) = g.tags {
                    entry.tags = tags.clone();
                }
                if let Some(ref source) = g.source {
                    entry.source_path = source.clone();
                }
                entry.entry_type = EntryType::Document;

                // Regenerate slug
                entry.slug = slugify(&entry.title);
            }
        }
    }

    /// Process and index a single entry (helper for batch).
    async fn process_and_index_document(
        &self,
        source: &ContentSource,
        schema: &DocumentSchema,
        custom_id: &Option<String>,
    ) -> Result<(String, usize), McpError> {
        let mut entry = self.process_content_source(source, schema).await?;

        if let Some(ref id) = custom_id {
            entry.id = id.clone();
        }

        // Check and handle existing
        let exists = {
            let store = self.store.lock().await;
            store
                .entry_exists(&entry.id)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?
        };

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

        // Store
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

        Ok((entry.id, chunks.len()))
    }
}

// Implement ServerHandler trait for the MCP server
impl ServerHandler for KixMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some("Knowledge Indexer System - Search and explore indexed content using natural language.".into()),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

/// Backward compatibility alias for KixMcpServer.
pub type EipMcpServer = KixMcpServer;

/// Format search results for display.
fn format_search_results(results: &[SearchResult]) -> String {
    if results.is_empty() {
        return "No results found.".to_string();
    }

    let mut output = String::new();
    for (i, result) in results.iter().enumerate() {
        output.push_str(&format!(
            "### {}. {} (Score: {:.2})\n\
            **Type:** {} | **Tags:** {}\n\
            {}\n\n",
            i + 1,
            result.entry_title,
            result.score,
            result.entry_type,
            result.tags.join(", "),
            truncate_text(&result.text, 200)
        ));
    }
    output
}

/// Format entry summary for display.
fn format_pattern_summary(entry: &EntrySummary) -> String {
    format!(
        "## {}\n\n\
        **ID:** {}\n\
        **Type:** {}\n\
        **Tags:** {}\n\n\
        **Description:**\n{}\n",
        entry.title,
        entry.id,
        entry.entry_type,
        entry.tags.join(", "),
        entry.description
    )
}

/// Format entry list for display.
fn format_pattern_list(entries: &[EntrySummary]) -> String {
    if entries.is_empty() {
        return "No entries found.".to_string();
    }

    let mut output = format!("## Found {} Entries\n\n", entries.len());
    for entry in entries {
        output.push_str(&format!(
            "- **{}** ({}): {}\n",
            entry.title,
            entry.entry_type,
            truncate_text(&entry.description, 100)
        ));
    }
    output
}

/// Truncate text to a maximum length.
fn truncate_text(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        text.to_string()
    } else {
        format!("{}...", &text[..max_len])
    }
}

/// Format index result for display.
fn format_index_result(result: &IndexResult) -> String {
    if result.success {
        format!(
            "## Document Indexed Successfully\n\n\
            **Document ID:** {}\n\
            **Chunks Created:** {}\n\n\
            {}",
            result.document_id, result.chunks_created, result.message
        )
    } else {
        format!("## Indexing Failed\n\n**Error:** {}", result.message)
    }
}

/// Format batch index result for display.
fn format_batch_result(result: &BatchIndexResult) -> String {
    let mut output = format!(
        "## Batch Indexing Complete\n\n\
        **Total:** {} | **Succeeded:** {} | **Failed:** {}\n\
        **Indexes Created:** {}\n\n",
        result.total,
        result.succeeded,
        result.failed,
        if result.indexes_created { "Yes" } else { "No" }
    );

    output.push_str("### Results\n\n");
    for doc_result in &result.results {
        if doc_result.success {
            output.push_str(&format!(
                "- [{}] {} ({} chunks)\n",
                doc_result.index + 1,
                doc_result.document_id.as_deref().unwrap_or("unknown"),
                doc_result.chunks_created.unwrap_or(0)
            ));
        } else {
            output.push_str(&format!(
                "- [{}] FAILED: {}\n",
                doc_result.index + 1,
                doc_result.error.as_deref().unwrap_or("unknown error")
            ));
        }
    }

    output
}

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
