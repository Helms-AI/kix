//! Retrieval services for search, document access, and RAG context.
//!
//! These services provide the shared business logic for:
//! - Knowledge base search (hybrid, vector, text)
//! - Document retrieval with optional chunks
//! - Page context for RAG synthesis
//! - Related entries via semantic similarity

use kix_embeddings::OllamaEmbedder;
use kix_store::{KixStore, SearchFilters, SearchResult as StoreSearchResult};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use crate::error::{ServiceError, ServiceResult};

// =============================================================================
// TYPES
// =============================================================================

/// Search mode for knowledge base queries.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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

/// Pagination parameters for list operations.
#[derive(Debug, Clone, Default)]
pub struct Pagination {
    pub limit: usize,
    pub offset: usize,
}

impl Pagination {
    pub fn new(limit: Option<usize>, offset: Option<usize>) -> Self {
        Self {
            limit: limit.unwrap_or(10).min(100),
            offset: offset.unwrap_or(0),
        }
    }
}

/// Filters for search queries (re-export from store with builder pattern).
#[derive(Debug, Clone, Default)]
pub struct QueryFilters {
    pub entry_type: Option<String>,
    pub chunk_type: Option<String>,
    pub tag: Option<String>,
    pub source_domain: Option<String>,
}

impl From<QueryFilters> for SearchFilters {
    fn from(f: QueryFilters) -> Self {
        SearchFilters {
            entry_type: f.entry_type,
            chunk_type: f.chunk_type,
            tag: f.tag,
            source_domain: f.source_domain,
        }
    }
}

/// A single search result item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResultItem {
    pub chunk_id: String,
    pub entry_id: String,
    pub page_id: Option<String>,
    pub text: String,
    pub score: f32,
    pub entry_title: String,
    pub entry_type: String,
    pub source_url: Option<String>,
    pub tags: Vec<String>,
}

impl From<StoreSearchResult> for SearchResultItem {
    fn from(r: StoreSearchResult) -> Self {
        Self {
            chunk_id: r.chunk_id,
            entry_id: r.entry_id,
            page_id: r.page_id,
            text: r.text,
            score: r.score,
            entry_title: r.entry_title,
            entry_type: r.entry_type,
            source_url: r.source_domain.map(|d| format!("https://{}", d)),
            tags: r.tags,
        }
    }
}

/// Search results with pagination info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResults {
    pub results: Vec<SearchResultItem>,
    pub total_count: usize,
    pub has_more: bool,
    pub query: String,
}

/// Full page content for RAG context enrichment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageContext {
    pub page_id: String,
    pub url: String,
    pub title: Option<String>,
    pub full_content: String,
    pub content_length: usize,
    pub code_block_count: usize,
}

/// Chunk information for a document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkInfo {
    pub chunk_id: String,
    pub chunk_index: i32,
    pub chunk_type: Option<String>,
    pub text: String,
}

/// Document metadata with optional chunks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    pub title: String,
    pub description: String,
    pub entry_type: String,
    pub source_url: Option<String>,
    pub source_domain: Option<String>,
    pub tags: Vec<String>,
    pub created_at: Option<String>,
    pub chunks: Option<Vec<ChunkInfo>>,
}

/// A related entry found via semantic similarity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelatedEntry {
    pub id: String,
    pub title: String,
    pub entry_type: String,
    pub description: String,
    pub score: f32,
    pub tags: Vec<String>,
    pub source_domain: Option<String>,
}

// =============================================================================
// SERVICE FUNCTIONS
// =============================================================================

/// Search the knowledge base using natural language.
///
/// Supports three search modes:
/// - `Hybrid`: Combined vector and full-text search (default, recommended)
/// - `Vector`: Pure semantic vector search
/// - `Text`: Pure keyword/full-text search
///
/// # Arguments
/// * `store` - The KixStore instance
/// * `embedder` - The Ollama embedding generator for vector search
/// * `query` - Natural language search query
/// * `mode` - Search mode (hybrid, vector, text)
/// * `filters` - Optional filters for entry_type, tag, etc.
/// * `pagination` - Limit and offset for results
///
/// # Returns
/// Search results with pagination info
pub async fn search_knowledge(
    store: &Arc<RwLock<KixStore>>,
    embedder: &Arc<OllamaEmbedder>,
    query: &str,
    mode: SearchMode,
    filters: QueryFilters,
    pagination: Pagination,
) -> ServiceResult<SearchResults> {
    info!("Search: {} (mode: {:?})", query, mode);

    let search_filters: SearchFilters = filters.into();
    let fetch_limit = pagination.limit + pagination.offset;

    // Generate embedding for vector/hybrid search (async)
    let embedding = match mode {
        SearchMode::Text => vec![], // Not needed for text-only
        _ => {
            embedder.embed_one(query).await.map_err(|e| {
                ServiceError::External(format!("Failed to generate embedding: {}", e))
            })?
        }
    };

    // Perform search based on mode
    let store_guard = store.read().await;
    let results = match mode {
        SearchMode::Hybrid | SearchMode::Text => {
            // Both hybrid and text modes use hybrid_search (includes FTS)
            store_guard
                .hybrid_search(query, &embedding, fetch_limit, &search_filters)
                .await
                .map_err(ServiceError::Store)?
        }
        SearchMode::Vector => {
            // Pure vector search
            store_guard
                .vector_search(&embedding, fetch_limit, &search_filters)
                .await
                .map_err(ServiceError::Store)?
        }
    };

    // Apply pagination
    let total_count = results.len();
    let paginated: Vec<SearchResultItem> = results
        .into_iter()
        .skip(pagination.offset)
        .take(pagination.limit)
        .map(SearchResultItem::from)
        .collect();
    let has_more = total_count > pagination.offset + pagination.limit;

    Ok(SearchResults {
        results: paginated,
        total_count,
        has_more,
        query: query.to_string(),
    })
}

/// Get document metadata by ID with optional chunks.
///
/// # Arguments
/// * `store` - The KixStore instance
/// * `id` - Document ID
/// * `include_chunks` - Whether to include all chunks
///
/// # Returns
/// Document with metadata and optionally chunks
pub async fn get_document(
    store: &Arc<RwLock<KixStore>>,
    id: &str,
    include_chunks: bool,
) -> ServiceResult<Document> {
    info!("Get document: {} (include_chunks: {})", id, include_chunks);

    let store_guard = store.read().await;

    // Get entry by ID
    let entry = store_guard
        .get_entry_by_id(id)
        .await
        .map_err(ServiceError::Store)?
        .ok_or_else(|| ServiceError::not_found("Document", id))?;

    // Optionally get chunks
    let chunks = if include_chunks {
        let chunk_list = store_guard
            .get_chunks_by_entry_id(id)
            .await
            .map_err(ServiceError::Store)?;

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

    // Parse tags from JSON
    let tags: Vec<String> = entry
        .tags
        .as_ref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();

    Ok(Document {
        id: entry.id,
        title: entry.title,
        description: entry.description.unwrap_or_default(),
        entry_type: entry.entry_type,
        source_url: Some(entry.source_path),
        source_domain: entry.source_domain,
        tags,
        created_at: Some(entry.created_at),
        chunks,
    })
}

/// Retrieve full page content for RAG synthesis.
///
/// Use page_id from search results, or chunk_id to find the associated page.
///
/// # Arguments
/// * `store` - The KixStore instance
/// * `page_id` - Direct page ID (preferred)
/// * `chunk_id` - Chunk ID to look up the page (fallback)
///
/// # Returns
/// Full page content with metadata
pub async fn get_page_context(
    store: &Arc<RwLock<KixStore>>,
    page_id: Option<&str>,
    chunk_id: Option<&str>,
) -> ServiceResult<PageContext> {
    // Validate input - need either page_id or chunk_id
    let resolved_page_id = match (page_id, chunk_id) {
        (Some(pid), _) => pid.to_string(),
        (None, Some(cid)) => {
            // Parse chunk_id to get entry_id
            // Chunk IDs are formatted as "{entry_id}#{chunk_index}"
            cid.split('#').next().unwrap_or(cid).to_string()
        }
        (None, None) => {
            return Err(ServiceError::Validation(
                "Either page_id or chunk_id must be provided".to_string(),
            ))
        }
    };

    info!("Get context for page: {}", resolved_page_id);

    let store_guard = store.read().await;
    let page = store_guard
        .get_page_for_chunk(&resolved_page_id)
        .await
        .map_err(ServiceError::Store)?
        .ok_or_else(|| ServiceError::not_found("Page", &resolved_page_id))?;

    Ok(PageContext {
        page_id: page.page_id,
        url: page.url,
        title: page.title,
        full_content: page.full_content,
        content_length: page.content_length as usize,
        code_block_count: page.code_block_count as usize,
    })
}

/// Find related entries via semantic similarity.
///
/// Uses vector search to find entries similar to the given entry.
/// Optimized to use cached embeddings from indexing when available,
/// avoiding expensive Ollama calls.
///
/// # Arguments
/// * `store` - The KixStore instance
/// * `embedder` - The Ollama embedding generator (fallback if no cached embedding)
/// * `entry_id` - The entry to find related entries for
/// * `limit` - Maximum number of related entries to return
///
/// # Returns
/// List of related entries sorted by similarity
pub async fn find_related(
    store: &Arc<RwLock<KixStore>>,
    embedder: &Arc<OllamaEmbedder>,
    entry_id: &str,
    limit: usize,
) -> ServiceResult<Vec<RelatedEntry>> {
    info!("Find related for: {} (limit: {})", entry_id, limit);

    // First, get the entry to verify it exists and get metadata
    let store_guard = store.read().await;
    let entry = store_guard
        .get_pattern_by_id(entry_id)
        .await
        .map_err(ServiceError::Store)?
        .ok_or_else(|| ServiceError::not_found("Entry", entry_id))?;

    let entry_type = entry.entry_type.clone();
    let entry_title = entry.title.clone();
    let entry_description = entry.description.clone();

    // Try to get cached embedding first (from indexing)
    let cached_embedding = store_guard
        .get_entry_embedding(entry_id)
        .await
        .map_err(ServiceError::Store)?;

    drop(store_guard); // Release lock before potential embedding generation

    // Use cached embedding or fall back to generating via Ollama
    let embedding = match cached_embedding {
        Some(emb) => {
            info!(
                entry_id = %entry_id,
                "Using cached embedding for related entries (cache hit)"
            );
            emb
        }
        None => {
            // Fallback: Generate embedding via Ollama
            // This happens for entries without chunks (newly created, failed indexing)
            info!(
                entry_id = %entry_id,
                "No cached embedding found, generating via Ollama (cache miss)"
            );
            let query = format!("entries related to {} {}", entry_title, entry_description);
            embedder.embed_one(&query).await.map_err(|e| {
                ServiceError::External(format!("Failed to generate embedding: {}", e))
            })?
        }
    };

    // Search for similar entries (get extra to filter out self)
    let store_guard = store.read().await;
    let filters = SearchFilters {
        entry_type: Some(entry_type),
        ..Default::default()
    };
    let results = store_guard
        .vector_search(&embedding, limit + 1, &filters)
        .await
        .map_err(ServiceError::Store)?;

    // Filter out the original entry and convert to RelatedEntry
    let related: Vec<RelatedEntry> = results
        .into_iter()
        .filter(|r| r.entry_id != entry_id)
        .take(limit)
        .map(|r| RelatedEntry {
            id: r.entry_id,
            title: r.entry_title,
            entry_type: r.entry_type,
            description: r.text,
            score: r.score,
            tags: r.tags,
            source_domain: r.source_domain,
        })
        .collect();

    Ok(related)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pagination_defaults() {
        let p = Pagination::new(None, None);
        assert_eq!(p.limit, 10);
        assert_eq!(p.offset, 0);
    }

    #[test]
    fn test_pagination_max_limit() {
        let p = Pagination::new(Some(500), None);
        assert_eq!(p.limit, 100); // Capped at 100
    }

    #[test]
    fn test_query_filters_conversion() {
        let qf = QueryFilters {
            entry_type: Some("document".to_string()),
            tag: Some("rust".to_string()),
            ..Default::default()
        };
        let sf: SearchFilters = qf.into();
        assert_eq!(sf.entry_type, Some("document".to_string()));
        assert_eq!(sf.tag, Some("rust".to_string()));
    }

    #[test]
    fn test_search_mode_default() {
        let mode = SearchMode::default();
        assert!(matches!(mode, SearchMode::Hybrid));
    }
}
