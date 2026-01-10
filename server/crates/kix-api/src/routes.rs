//! API route definitions for the EIP Knowledge System dashboard.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use moka::future::Cache;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};
use tracing::{debug, info, error};

use kix_embeddings::EmbeddingGenerator;
use kix_store::search::{PatternSummary, SearchFilters, SearchResult};
use kix_store::KixStore;

/// Cache configuration constants
const SEARCH_CACHE_MAX_CAPACITY: u64 = 1000;  // Max cached search results
const SEARCH_CACHE_TTL_SECS: u64 = 300;       // 5 minute TTL
const EMBEDDING_CACHE_MAX_CAPACITY: u64 = 500; // Max cached embeddings
const EMBEDDING_CACHE_TTL_SECS: u64 = 600;    // 10 minute TTL for embeddings

/// Cached search result (must be cloneable)
#[derive(Clone)]
struct CachedSearchResult {
    results: Vec<SearchResult>,
}

/// Cached embedding vector
#[derive(Clone)]
struct CachedEmbedding {
    vector: Vec<f32>,
}

/// Application state shared across all routes.
/// Uses RwLock for high-concurrency read access - most API operations are reads.
/// Includes query caches for improved performance.
#[derive(Clone)]
pub struct AppState {
    store: Arc<RwLock<KixStore>>,
    embedder: Arc<RwLock<EmbeddingGenerator>>,
    /// Cache for search results (key: hash of query+filters)
    search_cache: Cache<String, CachedSearchResult>,
    /// Cache for query embeddings (key: query text hash)
    embedding_cache: Cache<String, CachedEmbedding>,
}

impl AppState {
    /// Create a new application state with caching enabled.
    pub fn new(store: KixStore, embedder: EmbeddingGenerator) -> Self {
        // Build search cache with TTL and max capacity
        let search_cache: Cache<String, CachedSearchResult> = Cache::builder()
            .max_capacity(SEARCH_CACHE_MAX_CAPACITY)
            .time_to_live(Duration::from_secs(SEARCH_CACHE_TTL_SECS))
            .build();

        // Build embedding cache with TTL and max capacity
        let embedding_cache: Cache<String, CachedEmbedding> = Cache::builder()
            .max_capacity(EMBEDDING_CACHE_MAX_CAPACITY)
            .time_to_live(Duration::from_secs(EMBEDDING_CACHE_TTL_SECS))
            .build();

        info!(
            "Initialized API caches: search_cache(max={}, ttl={}s), embedding_cache(max={}, ttl={}s)",
            SEARCH_CACHE_MAX_CAPACITY, SEARCH_CACHE_TTL_SECS,
            EMBEDDING_CACHE_MAX_CAPACITY, EMBEDDING_CACHE_TTL_SECS
        );

        Self {
            store: Arc::new(RwLock::new(store)),
            embedder: Arc::new(RwLock::new(embedder)),
            search_cache,
            embedding_cache,
        }
    }

    /// Get a reference to the store (for read operations use .read().await).
    pub fn store(&self) -> &Arc<RwLock<KixStore>> {
        &self.store
    }

    /// Invalidate all caches (call after data changes).
    pub fn invalidate_caches(&self) {
        self.search_cache.invalidate_all();
        self.embedding_cache.invalidate_all();
        info!("All caches invalidated");
    }
}

/// Generate a cache key from query parameters.
fn cache_key(query: &str, filters: &SearchFilters, limit: usize) -> String {
    let mut hasher = Sha256::new();
    hasher.update(query.as_bytes());
    hasher.update(filters.entry_type.as_deref().unwrap_or("").as_bytes());
    hasher.update(filters.chunk_type.as_deref().unwrap_or("").as_bytes());
    hasher.update(filters.tag.as_deref().unwrap_or("").as_bytes());
    hasher.update(limit.to_le_bytes());
    format!("{:x}", hasher.finalize())
}

/// Generate an embedding cache key from query text.
fn embedding_cache_key(query: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(query.as_bytes());
    format!("emb_{:x}", hasher.finalize())
}

/// Create the API router with all routes.
pub fn create_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/api/stats", get(get_stats))
        .route("/api/patterns", get(list_patterns))
        .route("/api/patterns-related/*id", get(get_related_patterns))
        .route("/api/patterns/*id", get(get_pattern))
        .route("/api/categories", get(list_categories))
        .route("/api/search", get(search_patterns))
        .route("/api/graph", get(get_pattern_graph))
        .route("/api/health", get(health_check))
        .layer(cors)
        .with_state(state)
}

// === Response Types ===

/// Dashboard statistics response.
#[derive(Serialize)]
pub struct StatsResponse {
    pub total_patterns: usize,
    pub messaging_patterns: usize,
    pub conversation_patterns: usize,
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

/// Pattern list response.
#[derive(Serialize)]
pub struct PatternListResponse {
    pub patterns: Vec<PatternResponse>,
    pub total: usize,
}

/// Individual pattern response.
#[derive(Serialize)]
pub struct PatternResponse {
    pub id: String,
    pub title: String,
    pub pattern_type: String,
    pub categories: Vec<String>,
    pub description: String,
}

impl From<PatternSummary> for PatternResponse {
    fn from(p: PatternSummary) -> Self {
        Self {
            id: p.id,
            title: p.title,
            pattern_type: p.entry_type,
            categories: p.tags,
            description: p.description,
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
    pub pattern_name: String,
    pub pattern_type: String,
    pub categories: Vec<String>,
    pub text: String,
    pub score: f32,
}

impl From<SearchResult> for SearchResultResponse {
    fn from(r: SearchResult) -> Self {
        Self {
            id: r.entry_id,
            pattern_name: r.entry_title,
            pattern_type: r.entry_type,
            categories: r.tags,
            text: r.text,
            score: r.score,
        }
    }
}

/// Categories list response.
#[derive(Serialize)]
pub struct CategoriesResponse {
    pub categories: Vec<CategoryInfo>,
}

/// Category info with patterns.
#[derive(Serialize)]
pub struct CategoryInfo {
    pub name: String,
    pub pattern_count: usize,
    pub pattern_type: String,
}

/// Pattern graph response for visualization.
#[derive(Serialize)]
pub struct PatternGraphResponse {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

/// Graph node representing a pattern.
#[derive(Serialize)]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub pattern_type: String,
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

/// Query parameters for listing patterns.
#[derive(Deserialize)]
pub struct ListPatternsQuery {
    pub category: Option<String>,
    pub pattern_type: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// Query parameters for search.
#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: String,
    pub pattern_type: Option<String>,
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

    // Get all patterns to compute stats
    let all_patterns = store
        .list_all_patterns()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let messaging_patterns = all_patterns
        .iter()
        .filter(|p| p.entry_type == "messaging")
        .count();
    let conversation_patterns = all_patterns
        .iter()
        .filter(|p| p.entry_type == "conversation")
        .count();

    // Count by category
    let mut category_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for pattern in &all_patterns {
        for category in &pattern.tags {
            *category_counts.entry(category.clone()).or_insert(0) += 1;
        }
    }

    let categories: Vec<CategoryCount> = category_counts
        .into_iter()
        .map(|(name, count)| CategoryCount { name, count })
        .collect();

    // Get total chunks (estimate from pattern count * avg chunks per pattern)
    let total_chunks = all_patterns.len() * 3; // Rough estimate
    let total_documents = all_patterns.len();

    Ok(Json(StatsResponse {
        total_patterns: all_patterns.len(),
        messaging_patterns,
        conversation_patterns,
        total_chunks,
        total_documents,
        categories,
    }))
}

/// GET /api/patterns - List all patterns.
async fn list_patterns(
    State(state): State<AppState>,
    Query(query): Query<ListPatternsQuery>,
) -> Result<Json<PatternListResponse>, StatusCode> {
    info!(
        "Listing patterns - category: {:?}, type: {:?}",
        query.category, query.pattern_type
    );

    let store = state.store.read().await;

    let patterns = if let Some(ref category) = query.category {
        store
            .list_by_category(category)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    } else if let Some(ref pattern_type) = query.pattern_type {
        store
            .list_by_pattern_type(pattern_type)
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

    let patterns: Vec<PatternResponse> = patterns
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(PatternResponse::from)
        .collect();

    Ok(Json(PatternListResponse { patterns, total }))
}

/// GET /api/patterns/:id - Get a specific pattern.
async fn get_pattern(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<PatternResponse>, StatusCode> {
    info!("Getting pattern: {}", id);

    let store = state.store.read().await;
    let pattern = store
        .get_pattern_by_id(&id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    match pattern {
        Some(p) => Ok(Json(PatternResponse::from(p))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// GET /api/patterns-related/:id - Get related patterns.
async fn get_related_patterns(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<PatternListResponse>, StatusCode> {
    info!("Getting related patterns for: {}", id);

    let store = state.store.read().await;
    let pattern = store
        .get_pattern_by_id(&id)
        .await
        .map_err(|e| {
            error!("Failed to get pattern by id '{}': {}", id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    match pattern {
        Some(p) => {
            // Use semantic search to find related patterns
            let query = format!("patterns related to {} {}", p.title, p.description);
            drop(store); // Release read lock before embedding

            let embedding = {
                let mut embedder = state.embedder.write().await;
                embedder
                    .embed_query(&query)
                    .map_err(|e| {
                        error!("Failed to generate embedding for related patterns: {}", e);
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
                    error!("Vector search for related patterns failed: {}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;

            // Filter out the original pattern and convert to summaries
            let mut related: Vec<PatternResponse> = Vec::new();
            for result in results {
                if result.entry_id != id {
                    // Create a simple response from search result
                    related.push(PatternResponse {
                        id: result.entry_id,
                        title: result.entry_title,
                        pattern_type: result.entry_type,
                        categories: result.tags,
                        description: result.text,
                    });
                }
            }
            let total = related.len();

            Ok(Json(PatternListResponse {
                patterns: related.into_iter().take(5).collect(),
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
        .map(|(name, (pattern_count, pattern_type))| CategoryInfo {
            name,
            pattern_count,
            pattern_type,
        })
        .collect();

    Ok(Json(CategoriesResponse { categories }))
}

/// GET /api/search - Search patterns with caching.
async fn search_patterns(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<SearchResponse>, StatusCode> {
    let limit = query.limit.unwrap_or(10);
    let filters = SearchFilters {
        entry_type: query.pattern_type.clone(),
        tag: query.category.clone(),
        chunk_type: None,
        source_domain: query.source_domain.clone(),
    };

    // Generate cache key for the full search
    let search_key = cache_key(&query.q, &filters, limit);

    // Check search cache first
    if let Some(cached) = state.search_cache.get(&search_key).await {
        debug!("Search cache hit for query: {}", query.q);
        let total = cached.results.len();
        let results: Vec<SearchResultResponse> = cached
            .results
            .into_iter()
            .map(SearchResultResponse::from)
            .collect();
        return Ok(Json(SearchResponse {
            results,
            total,
            query: query.q,
        }));
    }

    info!("Search cache miss, executing search for: {}", query.q);

    // Check embedding cache or generate new embedding
    let emb_key = embedding_cache_key(&query.q);
    let embedding = if let Some(cached_emb) = state.embedding_cache.get(&emb_key).await {
        debug!("Embedding cache hit for query: {}", query.q);
        cached_emb.vector
    } else {
        // Generate embedding and cache it
        let mut embedder = state.embedder.write().await;
        let vec = embedder.embed_query(&query.q).map_err(|e| {
            error!("Failed to generate embedding for search query: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        drop(embedder); // Release lock before caching

        // Cache the embedding
        state
            .embedding_cache
            .insert(emb_key, CachedEmbedding { vector: vec.clone() })
            .await;
        vec
    };

    // Perform hybrid search
    let store = state.store.read().await;
    let results = store
        .hybrid_search(&query.q, &embedding, limit, &filters)
        .await
        .map_err(|e| {
            error!("Hybrid search failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    drop(store); // Release lock before caching

    // Cache the search results
    state
        .search_cache
        .insert(search_key, CachedSearchResult { results: results.clone() })
        .await;

    let total = results.len();
    let results: Vec<SearchResultResponse> =
        results.into_iter().map(SearchResultResponse::from).collect();

    Ok(Json(SearchResponse {
        results,
        total,
        query: query.q,
    }))
}

/// GET /api/graph - Get pattern graph for visualization.
async fn get_pattern_graph(
    State(state): State<AppState>,
) -> Result<Json<PatternGraphResponse>, StatusCode> {
    info!("Getting pattern graph");

    let store = state.store.read().await;
    let all_patterns = store
        .list_all_patterns()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Create nodes from patterns
    let nodes: Vec<GraphNode> = all_patterns
        .iter()
        .map(|p| GraphNode {
            id: p.id.clone(),
            label: p.title.clone(),
            pattern_type: p.entry_type.clone(),
            category: p.tags.first().cloned().unwrap_or_default(),
        })
        .collect();

    // Create edges based on category relationships
    // Patterns in the same category are connected
    let mut edges: Vec<GraphEdge> = Vec::new();
    let mut seen_edges: std::collections::HashSet<(String, String)> =
        std::collections::HashSet::new();

    for (i, p1) in all_patterns.iter().enumerate() {
        for p2 in all_patterns.iter().skip(i + 1) {
            // Check if patterns share a category
            let shared_categories: Vec<_> = p1
                .tags
                .iter()
                .filter(|c| p2.tags.contains(c))
                .collect();

            if !shared_categories.is_empty() {
                let edge_key = if p1.id < p2.id {
                    (p1.id.clone(), p2.id.clone())
                } else {
                    (p2.id.clone(), p1.id.clone())
                };

                if !seen_edges.contains(&edge_key) {
                    seen_edges.insert(edge_key);
                    edges.push(GraphEdge {
                        source: p1.id.clone(),
                        target: p2.id.clone(),
                        weight: shared_categories.len() as f32,
                    });
                }
            }
        }
    }

    Ok(Json(PatternGraphResponse { nodes, edges }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_response_from() {
        let summary = PatternSummary {
            id: "test-id".to_string(),
            title: "Test Pattern".to_string(),
            entry_type: "messaging".to_string(),
            tags: vec!["Category1".to_string()],
            description: "Test description".to_string(),
        };

        let response = PatternResponse::from(summary);
        assert_eq!(response.id, "test-id");
        assert_eq!(response.title, "Test Pattern");
    }
}
