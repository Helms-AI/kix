//! API route definitions for the EIP Knowledge System dashboard.
//!
//! NOTE: Caching has been intentionally removed to ensure real-time data availability.
//! LanceDB table handles are refreshed before each search operation to see the latest data.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};
use tracing::{info, error};

use kix_embeddings::EmbeddingGenerator;
use kix_store::search::{PatternSummary, SearchFilters, SearchResult, EntryChunk};
use kix_store::KixStore;

/// Application state shared across all routes.
/// Uses RwLock for high-concurrency read access - most API operations are reads.
#[derive(Clone)]
pub struct AppState {
    store: Arc<RwLock<KixStore>>,
    embedder: Arc<RwLock<EmbeddingGenerator>>,
}

impl AppState {
    /// Create a new application state with an owned store.
    ///
    /// This wraps the store in Arc<RwLock> internally.
    /// For sharing a store with other components (e.g., JobExecutor),
    /// use `new_with_store()` instead.
    pub fn new(store: KixStore, embedder: EmbeddingGenerator) -> Self {
        Self::new_with_store(Arc::new(RwLock::new(store)), embedder)
    }

    /// Create a new application state with a pre-wrapped shared store.
    ///
    /// This is the preferred constructor when the store needs to be shared
    /// with other components like the JobExecutor's ContentProcessor.
    pub fn new_with_store(store: Arc<RwLock<KixStore>>, embedder: EmbeddingGenerator) -> Self {
        Self::with_shared(store, Arc::new(RwLock::new(embedder)))
    }

    /// Create a new application state with both shared store and shared embedder.
    ///
    /// This is the preferred constructor for unified server mode where both
    /// API and MCP servers share the same resources.
    pub fn with_shared(
        store: Arc<RwLock<KixStore>>,
        embedder: Arc<RwLock<EmbeddingGenerator>>,
    ) -> Self {
        info!("Initialized API state with shared store and embedder (real-time data mode)");

        Self { store, embedder }
    }

    /// Get a reference to the store (for read operations use .read().await).
    pub fn store(&self) -> &Arc<RwLock<KixStore>> {
        &self.store
    }
}

/// Create the API router with all routes.
pub fn create_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/api/stats", get(get_stats))
        .route("/api/entries", get(list_entries))
        .route("/api/entries-related/*id", get(get_related_entries))
        .route("/api/entries-chunks/*id", get(get_entry_chunks))
        .route("/api/entries/*id", get(get_entry))
        .route("/api/categories", get(list_categories))
        .route("/api/search", get(search_entries))
        .route("/api/graph", get(get_entry_graph))
        .route("/api/health", get(health_check))
        // Admin routes are now nested under /api/admin
        .nest("/api/admin", crate::admin::admin_routes())
        .layer(cors)
        .with_state(state)
}

// === Response Types ===

/// Dashboard statistics response.
#[derive(Serialize)]
pub struct StatsResponse {
    pub total_entries: usize,
    pub document_entries: usize,
    pub article_entries: usize,
    pub pdf_entries: usize,
    pub code_entries: usize,
    pub total_chunks: usize,
    pub total_documents: usize,
    pub categories: Vec<CategoryCount>,
}

/// Category count for stats.
#[derive(Serialize)]
pub struct CategoryCount {
    pub name: String,
    pub count: usize,
}

/// Entry list response.
#[derive(Serialize)]
pub struct EntryListResponse {
    pub entries: Vec<EntryResponse>,
    pub total: usize,
}

/// Individual entry response.
#[derive(Serialize)]
pub struct EntryResponse {
    pub id: String,
    pub title: String,
    pub entry_type: String,
    pub tags: Vec<String>,
    pub description: String,
    pub source_type: Option<String>,
    pub source_domain: Option<String>,
    pub source_path: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

impl From<PatternSummary> for EntryResponse {
    fn from(p: PatternSummary) -> Self {
        Self {
            id: p.id,
            title: p.title,
            entry_type: p.entry_type,
            tags: p.tags,
            description: p.description,
            source_type: p.source_type,
            source_domain: p.source_domain,
            source_path: p.source_path,
            created_at: p.created_at,
            updated_at: p.updated_at,
        }
    }
}

/// Search results response.
#[derive(Serialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResultResponse>,
    pub total: usize,
    pub query: String,
}

/// Individual search result response.
#[derive(Serialize)]
pub struct SearchResultResponse {
    pub id: String,
    pub entry_id: String,
    pub entry_title: String,
    pub entry_type: String,
    pub tags: Vec<String>,
    pub chunk_type: Option<String>,
    pub source_domain: Option<String>,
    pub text: String,
    pub score: f32,
}

impl From<SearchResult> for SearchResultResponse {
    fn from(r: SearchResult) -> Self {
        Self {
            id: r.chunk_id.clone(),
            entry_id: r.entry_id,
            entry_title: r.entry_title,
            entry_type: r.entry_type,
            tags: r.tags,
            chunk_type: r.chunk_type,
            source_domain: r.source_domain,
            text: r.text,
            score: r.score,
        }
    }
}

/// Entry chunks response.
#[derive(Serialize)]
pub struct ChunksResponse {
    pub chunks: Vec<ChunkResponse>,
    pub total: usize,
}

/// Individual chunk response.
#[derive(Serialize)]
pub struct ChunkResponse {
    pub id: String,
    pub entry_id: String,
    pub text: String,
    pub chunk_type: Option<String>,
    pub chunk_index: Option<i32>,
}

impl From<EntryChunk> for ChunkResponse {
    fn from(c: EntryChunk) -> Self {
        Self {
            id: c.chunk_id,
            entry_id: c.entry_id,
            text: c.text,
            chunk_type: c.chunk_type,
            chunk_index: c.chunk_index,
        }
    }
}

/// Categories list response.
#[derive(Serialize)]
pub struct CategoriesResponse {
    pub categories: Vec<CategoryInfo>,
}

/// Category info with entries.
#[derive(Serialize)]
pub struct CategoryInfo {
    pub name: String,
    pub entry_count: usize,
    pub entry_type: String,
}

/// Pattern graph response for visualization.
#[derive(Serialize)]
pub struct PatternGraphResponse {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

/// Graph node representing an entry.
#[derive(Serialize)]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub entry_type: String,
    pub category: String,
}

/// Graph edge representing a relationship.
#[derive(Serialize)]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
    pub weight: f32,
}

/// Health check response.
#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

// === Query Parameters ===

/// Query parameters for listing entries.
#[derive(Deserialize)]
pub struct ListEntriesQuery {
    pub category: Option<String>,
    pub entry_type: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// Query parameters for search.
#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: String,
    pub entry_type: Option<String>,
    pub category: Option<String>,
    pub limit: Option<usize>,
    /// Filter by source domain (e.g., "docs.example.com")
    pub source_domain: Option<String>,
}

// === Error Response ===

/// API error response.
#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
}

impl IntoResponse for ErrorResponse {
    fn into_response(self) -> axum::response::Response {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(self)).into_response()
    }
}

// === Route Handlers ===

/// GET /api/health - Health check endpoint.
async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// GET /api/stats - Get dashboard statistics.
async fn get_stats(State(state): State<AppState>) -> Result<Json<StatsResponse>, StatusCode> {
    info!("Getting dashboard stats");

    let store = state.store.read().await;

    // Get all entries to compute stats
    let all_entries = store
        .list_all_patterns()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Count by actual entry types
    let document_entries = all_entries
        .iter()
        .filter(|p| p.entry_type == "document")
        .count();
    let article_entries = all_entries
        .iter()
        .filter(|p| p.entry_type == "article")
        .count();
    let pdf_entries = all_entries
        .iter()
        .filter(|p| p.entry_type == "pdf")
        .count();
    let code_entries = all_entries
        .iter()
        .filter(|p| p.entry_type == "code")
        .count();

    // Count by category
    let mut category_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for entry in &all_entries {
        for category in &entry.tags {
            *category_counts.entry(category.clone()).or_insert(0) += 1;
        }
    }

    let categories: Vec<CategoryCount> = category_counts
        .into_iter()
        .map(|(name, count)| CategoryCount { name, count })
        .collect();

    // Get total chunks (estimate from entry count * avg chunks per entry)
    let total_chunks = all_entries.len() * 3; // Rough estimate
    let total_documents = all_entries.len();

    Ok(Json(StatsResponse {
        total_entries: all_entries.len(),
        document_entries,
        article_entries,
        pdf_entries,
        code_entries,
        total_chunks,
        total_documents,
        categories,
    }))
}

/// GET /api/entries - List all entries.
async fn list_entries(
    State(state): State<AppState>,
    Query(query): Query<ListEntriesQuery>,
) -> Result<Json<EntryListResponse>, StatusCode> {
    info!(
        "Listing entries - category: {:?}, type: {:?}",
        query.category, query.entry_type
    );

    let store = state.store.read().await;

    let patterns = if let Some(ref category) = query.category {
        store
            .list_by_category(category)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    } else if let Some(ref entry_type) = query.entry_type {
        store
            .list_by_pattern_type(entry_type)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    } else {
        store
            .list_all_patterns()
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    };

    let total = patterns.len();
    let offset = query.offset.unwrap_or(0);
    let limit = query.limit.unwrap_or(100);

    let entries: Vec<EntryResponse> = patterns
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(EntryResponse::from)
        .collect();

    Ok(Json(EntryListResponse { entries, total }))
}

/// GET /api/entries/:id - Get a specific entry.
async fn get_entry(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<EntryResponse>, StatusCode> {
    info!("Getting entry: {}", id);

    let store = state.store.read().await;
    let pattern = store
        .get_pattern_by_id(&id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    match pattern {
        Some(p) => Ok(Json(EntryResponse::from(p))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// GET /api/entries-chunks/:id - Get chunks for an entry.
async fn get_entry_chunks(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ChunksResponse>, StatusCode> {
    info!("Getting chunks for entry: {}", id);

    let store = state.store.read().await;
    let chunks = store
        .get_chunks_by_entry_id(&id)
        .await
        .map_err(|e| {
            error!("Failed to get chunks for entry '{}': {}", id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let total = chunks.len();
    let chunks: Vec<ChunkResponse> = chunks.into_iter().map(ChunkResponse::from).collect();

    Ok(Json(ChunksResponse { chunks, total }))
}

/// GET /api/entries-related/:id - Get related entries.
async fn get_related_entries(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<EntryListResponse>, StatusCode> {
    info!("Getting related entries for: {}", id);

    let store = state.store.read().await;
    let pattern = store
        .get_pattern_by_id(&id)
        .await
        .map_err(|e| {
            error!("Failed to get entry by id '{}': {}", id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    match pattern {
        Some(p) => {
            // Use semantic search to find related entries
            let query = format!("entries related to {} {}", p.title, p.description);
            drop(store); // Release read lock before embedding

            let embedding = {
                let mut embedder = state.embedder.write().await;
                embedder
                    .embed_query(&query)
                    .map_err(|e| {
                        error!("Failed to generate embedding for related entries: {}", e);
                        StatusCode::INTERNAL_SERVER_ERROR
                    })?
            };

            let store = state.store.read().await;
            let filters = SearchFilters {
                entry_type: Some(p.entry_type.clone()),
                ..Default::default()
            };
            let results = store
                .vector_search(&embedding, 6, &filters)
                .await
                .map_err(|e| {
                    error!("Vector search for related entries failed: {}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;

            // Filter out the original entry and convert to responses
            let mut related: Vec<EntryResponse> = Vec::new();
            for result in results {
                if result.entry_id != id {
                    // Create a simple response from search result
                    related.push(EntryResponse {
                        id: result.entry_id,
                        title: result.entry_title,
                        entry_type: result.entry_type,
                        tags: result.tags,
                        description: result.text,
                        source_type: None,
                        source_domain: result.source_domain,
                        source_path: None,
                        created_at: None,
                        updated_at: None,
                    });
                }
            }
            let total = related.len();

            Ok(Json(EntryListResponse {
                entries: related.into_iter().take(5).collect(),
                total,
            }))
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// GET /api/categories - List all categories.
async fn list_categories(
    State(state): State<AppState>,
) -> Result<Json<CategoriesResponse>, StatusCode> {
    info!("Listing categories");

    let store = state.store.read().await;
    let all_patterns = store
        .list_all_patterns()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Group by category and count
    let mut category_info: std::collections::HashMap<String, (usize, String)> =
        std::collections::HashMap::new();

    for pattern in &all_patterns {
        for category in &pattern.tags {
            let entry = category_info
                .entry(category.clone())
                .or_insert((0, pattern.entry_type.clone()));
            entry.0 += 1;
        }
    }

    let categories: Vec<CategoryInfo> = category_info
        .into_iter()
        .map(|(name, (entry_count, entry_type))| CategoryInfo {
            name,
            entry_count,
            entry_type,
        })
        .collect();

    Ok(Json(CategoriesResponse { categories }))
}

/// GET /api/search - Search entries (real-time, no caching).
async fn search_entries(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<SearchResponse>, StatusCode> {
    let limit = query.limit.unwrap_or(10);
    let filters = SearchFilters {
        entry_type: query.entry_type.clone(),
        tag: query.category.clone(),
        chunk_type: None,
        source_domain: query.source_domain.clone(),
    };

    info!("Executing real-time search for: {}", query.q);

    // Generate embedding for the query
    let mut embedder = state.embedder.write().await;
    let embedding = embedder.embed_query(&query.q).map_err(|e| {
        error!("Failed to generate embedding for search query: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    drop(embedder);

    // Perform hybrid search (tables are refreshed internally to see latest data)
    let store = state.store.read().await;
    let results = store
        .hybrid_search(&query.q, &embedding, limit, &filters)
        .await
        .map_err(|e| {
            error!("Hybrid search failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let total = results.len();
    let results: Vec<SearchResultResponse> =
        results.into_iter().map(SearchResultResponse::from).collect();

    Ok(Json(SearchResponse {
        results,
        total,
        query: query.q,
    }))
}

/// GET /api/graph - Get entry graph for visualization.
/// Creates edges based on semantic similarity using vector search.
async fn get_entry_graph(
    State(state): State<AppState>,
) -> Result<Json<PatternGraphResponse>, StatusCode> {
    info!("Getting pattern graph with semantic relationships");

    let store = state.store.read().await;
    let all_patterns = store
        .list_all_patterns()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Create nodes from entries
    let nodes: Vec<GraphNode> = all_patterns
        .iter()
        .map(|p| GraphNode {
            id: p.id.clone(),
            label: p.title.clone(),
            entry_type: p.entry_type.clone(),
            category: p.tags.first().cloned().unwrap_or_default(),
        })
        .collect();

    // Build a set of all entry IDs for quick lookup
    let entry_ids: std::collections::HashSet<String> =
        all_patterns.iter().map(|p| p.id.clone()).collect();

    drop(store); // Release read lock before embedding

    // Create edges based on semantic similarity
    // For each entry, find related entries via vector search
    let mut edges: Vec<GraphEdge> = Vec::new();
    let mut seen_edges: std::collections::HashSet<(String, String)> =
        std::collections::HashSet::new();

    // Limit to avoid excessive API calls for large datasets
    let max_entries_to_process = 100.min(all_patterns.len());
    let neighbors_per_entry = 5;

    for pattern in all_patterns.iter().take(max_entries_to_process) {
        // Create a query from the entry's title and description
        let query = format!("{} {}", pattern.title, pattern.description);

        // Generate embedding for this entry
        let embedding = {
            let mut embedder = state.embedder.write().await;
            match embedder.embed_query(&query) {
                Ok(emb) => emb,
                Err(e) => {
                    error!("Failed to embed entry '{}': {}", pattern.id, e);
                    continue;
                }
            }
        };

        // Find related entries via vector search
        let store = state.store.read().await;
        let filters = kix_store::search::SearchFilters::default();
        let results = match store
            .vector_search(&embedding, neighbors_per_entry + 1, &filters)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                error!("Vector search failed for '{}': {}", pattern.id, e);
                continue;
            }
        };
        drop(store);

        // Create edges to related entries
        for result in results {
            // Skip self-connections and entries not in our node set
            if result.entry_id == pattern.id || !entry_ids.contains(&result.entry_id) {
                continue;
            }

            // Create unique edge key (smaller ID first)
            let edge_key = if pattern.id < result.entry_id {
                (pattern.id.clone(), result.entry_id.clone())
            } else {
                (result.entry_id.clone(), pattern.id.clone())
            };

            // Only add edge if not already seen
            if !seen_edges.contains(&edge_key) {
                seen_edges.insert(edge_key);
                edges.push(GraphEdge {
                    source: pattern.id.clone(),
                    target: result.entry_id,
                    // Use similarity score as weight (higher = more related)
                    weight: result.score,
                });
            }
        }
    }

    info!(
        "Graph generated: {} nodes, {} semantic edges",
        nodes.len(),
        edges.len()
    );

    Ok(Json(PatternGraphResponse { nodes, edges }))
}

// Admin endpoints are now in the admin module

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entry_response_from() {
        let summary = PatternSummary {
            id: "test-id".to_string(),
            title: "Test Entry".to_string(),
            entry_type: "document".to_string(),
            tags: vec!["Category1".to_string()],
            description: "Test description".to_string(),
            source_type: Some("url".to_string()),
            source_domain: Some("example.com".to_string()),
            source_path: Some("/docs".to_string()),
            created_at: None,
            updated_at: None,
        };

        let response = EntryResponse::from(summary);
        assert_eq!(response.id, "test-id");
        assert_eq!(response.title, "Test Entry");
        assert_eq!(response.entry_type, "document");
        assert_eq!(response.source_domain, Some("example.com".to_string()));
    }
}
