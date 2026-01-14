//! KIX Unified Store
//!
//! This module provides the `KixStore` which combines:
//! - **SQLite** (via kix-sqlite) for structured data (entries, pages, projects, issues, tokens, jobs)
//! - **SQLite + sqlite-vec** (via kix-vectors) for vector embeddings (chunks with vectors)
//!
//! ## Directory Structure
//!
//! ```text
//! data/
//! └── sqlite/
//!     ├── kix.db          # Structured data (entries, pages, projects, etc.)
//!     └── vectors.db      # Vector embeddings (chunks with vectors)
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! let store = KixStore::new(Path::new("data")).await?;
//! store.init().await?;
//!
//! // Store entries in SQLite
//! store.insert_entry(&entry).await?;
//!
//! // Store chunks with vectors in vectors.db
//! store.insert_chunks(&chunks, &embeddings).await?;
//!
//! // Hybrid search combines FTS (SQLite) + vector search (sqlite-vec)
//! let results = store.hybrid_search("query", &embedding, 10, &filters).await?;
//! ```

use crate::error::StoreError;
use crate::search::{PatternSummary, SearchFilters, SearchResult};
use kix_parser::{Entry, EntryChunk};
use kix_sqlite::{
    EntryRecord, IssueRecord, JobRecord, PageRecord, ProjectEntryRecord, ProjectRecord,
    SqliteStore, TokenRecord,
};
use kix_vectors::{SearchFilter, VectorSearchResult, VectorStore};
use std::collections::HashMap;
use std::path::Path;
use tracing::info;

/// Default embedding dimensions (768 for bge-base-en-v1.5).
pub const DEFAULT_EMBEDDING_DIM: usize = 768;

/// Unified KIX store using SQLite for all storage.
///
/// - Structured data (entries, pages, projects, etc.) stored in kix.db via kix-sqlite
/// - Vector embeddings stored in vectors.db via kix-vectors (sqlite-vec)
pub struct KixStore {
    /// SQLite store for structured data (kix.db)
    pub sqlite: SqliteStore,
    /// Vector store for embeddings (vectors.db)
    pub vectors: VectorStore,
    /// Embedding dimensions
    embedding_dim: usize,
}

impl KixStore {
    /// Create a new KIX store at the given data directory.
    ///
    /// Creates the directory structure:
    /// - `data_dir/sqlite/kix.db` - Structured data
    /// - `data_dir/sqlite/vectors.db` - Vector embeddings
    pub async fn new(data_dir: &Path) -> Result<Self, StoreError> {
        Self::new_with_dim(data_dir, get_embedding_dim()).await
    }

    /// Create a new KIX store with specified embedding dimensions.
    pub async fn new_with_dim(data_dir: &Path, embedding_dim: usize) -> Result<Self, StoreError> {
        // Create directory
        let sqlite_dir = data_dir.join("sqlite");

        std::fs::create_dir_all(&sqlite_dir)
            .map_err(|e| StoreError::Database(format!("Failed to create sqlite dir: {}", e)))?;

        let sqlite_path = sqlite_dir.join("kix.db");
        let vectors_path = sqlite_dir.join("vectors.db");

        info!(
            "Creating unified store: SQLite={}, Vectors={} (embedding_dim={})",
            sqlite_path.display(),
            vectors_path.display(),
            embedding_dim
        );

        // Initialize SQLite (async)
        let sqlite = SqliteStore::new(&sqlite_path).await.map_err(|e| {
            StoreError::Database(format!("Failed to create SQLite store: {}", e))
        })?;

        // Initialize VectorStore (sync, but fast)
        let vectors = VectorStore::new(&vectors_path, embedding_dim).map_err(|e| {
            StoreError::Database(format!("Failed to create Vector store: {}", e))
        })?;

        Ok(Self {
            sqlite,
            vectors,
            embedding_dim,
        })
    }

    /// Initialize all tables (called automatically by new()).
    pub async fn init(&mut self) -> Result<(), StoreError> {
        // Both stores auto-initialize tables on creation
        info!("KIX store initialized (unified SQLite architecture)");
        Ok(())
    }

    /// Compatibility alias for init() - kept for backward compatibility.
    pub async fn init_tables(&mut self) -> Result<(), StoreError> {
        self.init().await
    }

    /// Returns the embedding dimensions for this store.
    pub fn embedding_dim(&self) -> usize {
        self.embedding_dim
    }

    /// Get reference to SQLite store.
    pub fn sqlite(&self) -> &SqliteStore {
        &self.sqlite
    }

    /// Get reference to Vector store.
    pub fn vectors(&self) -> &VectorStore {
        &self.vectors
    }

    /// Get reference to page store (for backward compatibility, returns self since pages are in SQLite).
    pub fn page_store(&self) -> &Self {
        self
    }

    // =========================================================================
    // Entry Operations (SQLite)
    // =========================================================================

    /// Insert an entry into SQLite.
    pub async fn insert_entry(&self, entry: &EntryRecord) -> Result<(), StoreError> {
        self.sqlite
            .insert_entry(entry)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))
    }

    /// Get an entry by ID from SQLite.
    pub async fn get_entry(&self, id: &str) -> Result<Option<EntryRecord>, StoreError> {
        self.sqlite
            .get_entry(id)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))
    }

    /// Delete an entry from SQLite (also deletes associated chunks).
    pub async fn delete_entry(&self, id: &str) -> Result<bool, StoreError> {
        // Delete chunks from vector store first
        let _ = self.vectors.delete_chunks_by_entry(id);

        self.sqlite
            .delete_entry(id)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))
    }

    /// List entries from SQLite.
    pub async fn list_entries(
        &self,
        entry_type: Option<&str>,
        source_domain: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<EntryRecord>, StoreError> {
        self.sqlite
            .list_entries(entry_type, source_domain, limit, offset)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))
    }

    /// Get entry count from SQLite.
    pub async fn entry_count(&self) -> Result<usize, StoreError> {
        self.sqlite
            .entry_count()
            .await
            .map_err(|e| StoreError::Database(e.to_string()))
    }

    // =========================================================================
    // Document Operations (Backward Compatibility Aliases)
    // =========================================================================

    /// Check if a document/entry exists by ID.
    pub async fn document_exists(&self, id: &str) -> Result<bool, StoreError> {
        Ok(self.get_entry(id).await?.is_some())
    }

    /// Insert multiple documents/entries.
    pub async fn insert_documents(&self, entries: &[EntryRecord]) -> Result<(), StoreError> {
        for entry in entries {
            self.insert_entry(entry).await?;
        }
        Ok(())
    }

    /// Delete a document/entry by ID (alias for delete_entry).
    pub async fn delete_document(&self, id: &str) -> Result<bool, StoreError> {
        self.delete_entry(id).await
    }

    /// Delete chunks by document ID (alias for delete_chunks_by_entry).
    pub fn delete_chunks_by_document(&self, entry_id: &str) -> Result<usize, StoreError> {
        self.delete_chunks_by_entry(entry_id)
    }

    /// Insert documents from kix_parser Entry type (with automatic conversion).
    pub async fn insert_documents_from_entries(&self, entries: &[Entry]) -> Result<(), StoreError> {
        for entry in entries {
            let record = entry_to_record(entry);
            self.insert_entry(&record).await?;
        }
        Ok(())
    }

    // =========================================================================
    // Page Operations (SQLite)
    // =========================================================================

    /// Insert a page into SQLite.
    pub async fn insert_page(&self, page: &PageRecord) -> Result<(), StoreError> {
        self.sqlite
            .insert_page(page)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))
    }

    /// Get a page by ID from SQLite.
    pub async fn get_page(&self, page_id: &str) -> Result<Option<PageRecord>, StoreError> {
        self.sqlite
            .get_page(page_id)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))
    }

    /// Get page count from SQLite.
    pub async fn page_count(&self) -> Result<usize, StoreError> {
        self.sqlite
            .page_count()
            .await
            .map_err(|e| StoreError::Database(e.to_string()))
    }

    /// Delete pages by source entry ID.
    pub async fn delete_pages_by_source(&self, source_id: &str) -> Result<usize, StoreError> {
        self.sqlite
            .delete_pages_by_source(source_id)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))
    }

    // =========================================================================
    // Chunk Operations (VectorStore)
    // =========================================================================

    /// Insert chunks with embeddings into the vector store.
    pub fn insert_chunks(
        &self,
        chunks: &[EntryChunk],
        embeddings: &[Vec<f32>],
    ) -> Result<(), StoreError> {
        self.vectors
            .insert_chunks(chunks, embeddings)
            .map_err(|e| StoreError::Database(e.to_string()))
    }

    /// Delete chunks by entry ID from vector store.
    pub fn delete_chunks_by_entry(&self, entry_id: &str) -> Result<usize, StoreError> {
        self.vectors
            .delete_chunks_by_entry(entry_id)
            .map_err(|e| StoreError::Database(e.to_string()))
    }

    /// Get chunk count from vector store.
    pub fn chunk_count(&self) -> Result<usize, StoreError> {
        self.vectors
            .chunk_count()
            .map_err(|e| StoreError::Database(e.to_string()))
    }

    // =========================================================================
    // Two-Layer Storage
    // =========================================================================

    /// Store a page (SQLite) and its chunks (VectorStore) together.
    pub async fn store_page_with_chunks(
        &self,
        page: &PageRecord,
        chunks: &[EntryChunk],
        embeddings: &[Vec<f32>],
    ) -> Result<(), StoreError> {
        // Store page in SQLite
        self.insert_page(page).await?;

        // Store chunks in VectorStore
        self.insert_chunks(chunks, embeddings)?;

        info!(
            "Stored page {} (SQLite) with {} chunks (VectorStore)",
            page.page_id,
            chunks.len()
        );
        Ok(())
    }

    /// Get page for a chunk (RAG context retrieval).
    pub async fn get_page_for_chunk(&self, page_id: &str) -> Result<Option<PageRecord>, StoreError> {
        self.get_page(page_id).await
    }

    // =========================================================================
    // Hybrid Search
    // =========================================================================

    /// Perform hybrid search combining vector search and FTS.
    ///
    /// Uses Reciprocal Rank Fusion to combine results from both sources.
    pub async fn hybrid_search(
        &self,
        query_text: &str,
        query_embedding: &[f32],
        limit: usize,
        filters: &SearchFilters,
    ) -> Result<Vec<SearchResult>, StoreError> {
        // 1. Vector search in VectorStore
        let vec_filter = filters_to_vec_filter(filters);
        let vector_results = self
            .vectors
            .vector_search(query_embedding, limit * 2, vec_filter.as_ref())
            .map_err(|e| StoreError::Database(e.to_string()))?;

        // 2. FTS search in SQLite
        let fts_results = self
            .sqlite
            .search_entries(query_text, limit * 2)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))?;

        // 3. Combine using Reciprocal Rank Fusion
        let combined = reciprocal_rank_fusion(&vector_results, &fts_results, limit);

        Ok(combined)
    }

    /// Perform vector-only search.
    pub fn vector_search(
        &self,
        query_embedding: &[f32],
        limit: usize,
        filters: &SearchFilters,
    ) -> Result<Vec<SearchResult>, StoreError> {
        let vec_filter = filters_to_vec_filter(filters);
        let results = self
            .vectors
            .vector_search(query_embedding, limit, vec_filter.as_ref())
            .map_err(|e| StoreError::Database(e.to_string()))?;

        Ok(vec_results_to_search_results(&results))
    }

    // =========================================================================
    // Project Operations (SQLite)
    // =========================================================================

    /// Insert a project.
    pub async fn insert_project(&self, project: &ProjectRecord) -> Result<(), StoreError> {
        self.sqlite
            .insert_project(project)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))
    }

    /// Get a project by ID.
    pub async fn get_project(&self, id: &str) -> Result<Option<ProjectRecord>, StoreError> {
        self.sqlite
            .get_project(id)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))
    }

    /// List all projects.
    pub async fn list_projects(&self, include_archived: bool) -> Result<Vec<ProjectRecord>, StoreError> {
        self.sqlite
            .list_projects(include_archived)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))
    }

    /// Update a project.
    pub async fn update_project(&self, project: &ProjectRecord) -> Result<bool, StoreError> {
        self.sqlite
            .update_project(project)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))
    }

    /// Delete a project.
    pub async fn delete_project(&self, id: &str) -> Result<bool, StoreError> {
        self.sqlite
            .delete_project(id)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))
    }

    /// Get a project by slug.
    pub async fn get_project_by_slug(&self, slug: &str) -> Result<Option<ProjectRecord>, StoreError> {
        self.sqlite
            .get_project_by_slug(slug)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))
    }

    // =========================================================================
    // Issue Operations (SQLite)
    // =========================================================================

    /// Insert an issue.
    pub async fn insert_issue(&self, issue: &IssueRecord) -> Result<(), StoreError> {
        self.sqlite
            .insert_issue(issue)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))
    }

    /// Get an issue by ID.
    pub async fn get_issue(&self, id: &str) -> Result<Option<IssueRecord>, StoreError> {
        self.sqlite
            .get_issue(id)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))
    }

    /// List issues for a project.
    pub async fn list_issues(
        &self,
        project_id: &str,
        state: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<IssueRecord>, StoreError> {
        self.sqlite
            .list_issues(project_id, state, limit, offset)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))
    }

    /// Update an issue.
    pub async fn update_issue(&self, issue: &IssueRecord) -> Result<bool, StoreError> {
        self.sqlite
            .update_issue(issue)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))
    }

    /// Delete an issue.
    pub async fn delete_issue(&self, id: &str) -> Result<bool, StoreError> {
        self.sqlite
            .delete_issue(id)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))
    }

    /// Get an issue by number within a project.
    pub async fn get_issue_by_number(
        &self,
        project_id: &str,
        number: u32,
    ) -> Result<Option<IssueRecord>, StoreError> {
        self.sqlite
            .get_issue_by_number(project_id, number)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))
    }

    /// Get the next issue number for a project.
    pub async fn next_issue_number(&self, project_id: &str) -> Result<u32, StoreError> {
        self.sqlite
            .next_issue_number(project_id)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))
    }

    /// Count issues for a project.
    pub async fn issue_count(&self, project_id: &str) -> Result<usize, StoreError> {
        self.sqlite
            .issue_count(project_id)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))
    }

    // =========================================================================
    // Token Operations (SQLite)
    // =========================================================================

    /// Store a token.
    pub async fn store_token(&self, token: &TokenRecord) -> Result<(), StoreError> {
        self.sqlite
            .store_token(token)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))
    }

    /// Get a token by scope.
    pub async fn get_token(&self, scope: &str) -> Result<Option<TokenRecord>, StoreError> {
        self.sqlite
            .get_token(scope)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))
    }

    // =========================================================================
    // Job Operations (SQLite)
    // =========================================================================

    /// Insert a job record.
    pub async fn insert_job(&self, job: &JobRecord) -> Result<(), StoreError> {
        self.sqlite
            .insert_job(job)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))
    }

    /// Get a job by ID.
    pub async fn get_job(&self, job_id: &str) -> Result<Option<JobRecord>, StoreError> {
        self.sqlite
            .get_job(job_id)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))
    }

    /// List jobs.
    pub async fn list_jobs(
        &self,
        status: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<JobRecord>, StoreError> {
        self.sqlite
            .list_jobs(status, limit, offset)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))
    }

    // =========================================================================
    // Project Entry Links (SQLite)
    // =========================================================================

    /// Link an entry to a project.
    pub async fn link_entry_to_project(
        &self,
        project_id: &str,
        entry_id: &str,
        relevance: Option<f64>,
        notes: Option<&str>,
    ) -> Result<(), StoreError> {
        let link = ProjectEntryRecord::new(project_id, entry_id)
            .with_relevance(relevance.unwrap_or(1.0))
            .with_notes(notes.unwrap_or(""));
        self.sqlite
            .link_entry(&link)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))
    }

    /// Unlink an entry from a project.
    pub async fn unlink_entry_from_project(
        &self,
        project_id: &str,
        entry_id: &str,
    ) -> Result<bool, StoreError> {
        self.sqlite
            .unlink_entry(project_id, entry_id)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))
    }

    /// List entries linked to a project.
    pub async fn list_project_entries(
        &self,
        project_id: &str,
    ) -> Result<Vec<ProjectEntryRecord>, StoreError> {
        self.sqlite
            .list_project_entries(project_id)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))
    }

    // =========================================================================
    // Utility Methods
    // =========================================================================

    /// Alias for entry_count (backward compatibility).
    pub async fn document_count(&self) -> Result<usize, StoreError> {
        self.entry_count().await
    }

    /// Clear all data from the store (use with caution!).
    pub async fn clear_all(&self) -> Result<(), StoreError> {
        // Clear SQLite tables
        self.sqlite
            .clear_all()
            .await
            .map_err(|e| StoreError::Database(e.to_string()))?;

        // Clear vector store
        self.vectors
            .clear_all()
            .map_err(|e| StoreError::Database(e.to_string()))?;

        info!("Cleared all data from KIX store");
        Ok(())
    }

    // =========================================================================
    // Backward Compatibility Aliases
    // =========================================================================

    /// Get entry by ID (alias for get_entry).
    pub async fn get_entry_by_id(&self, id: &str) -> Result<Option<EntryRecord>, StoreError> {
        self.get_entry(id).await
    }

    /// Get entry by ID (alias for get_entry, for "pattern" naming convention).
    pub async fn get_pattern_by_id(&self, id: &str) -> Result<Option<PatternSummary>, StoreError> {
        self.get_entry(id).await.map(|opt| opt.map(PatternSummary::from))
    }

    /// Check if an entry exists (alias for document_exists).
    pub async fn entry_exists(&self, id: &str) -> Result<bool, StoreError> {
        self.document_exists(id).await
    }

    /// List all entries (alias for list_entries with large limit).
    pub async fn list_all_entries(&self) -> Result<Vec<EntryRecord>, StoreError> {
        self.list_entries(None, None, 100000, 0).await
    }

    /// List all patterns (alias for list_entries with large limit).
    pub async fn list_all_patterns(&self) -> Result<Vec<PatternSummary>, StoreError> {
        let entries = self.list_entries(None, None, 100000, 0).await?;
        Ok(entries.into_iter().map(PatternSummary::from).collect())
    }

    /// List patterns by category (uses entry_type as category proxy).
    pub async fn list_by_category(&self, category: &str) -> Result<Vec<PatternSummary>, StoreError> {
        // Category maps to entry_type for filtering
        let entries = self.list_entries(Some(category), None, 100000, 0).await?;
        Ok(entries.into_iter().map(PatternSummary::from).collect())
    }

    /// List patterns by entry type.
    pub async fn list_by_pattern_type(&self, entry_type: &str) -> Result<Vec<PatternSummary>, StoreError> {
        let entries = self.list_entries(Some(entry_type), None, 100000, 0).await?;
        Ok(entries.into_iter().map(PatternSummary::from).collect())
    }

    /// Get chunks by entry ID from vector store (returns chunk metadata).
    pub fn get_chunks_by_entry_id(&self, entry_id: &str) -> Result<Vec<VectorSearchResult>, StoreError> {
        self.vectors
            .get_chunks_by_entry(entry_id)
            .map_err(|e| StoreError::Database(e.to_string()))
    }
}

/// Convert SearchFilters to VectorStore SearchFilter.
fn filters_to_vec_filter(filters: &SearchFilters) -> Option<SearchFilter> {
    if filters.entry_type.is_none()
        && filters.chunk_type.is_none()
        && filters.source_domain.is_none()
        && filters.tag.is_none()
    {
        return None;
    }

    let mut filter = SearchFilter::new();
    if let Some(ref et) = filters.entry_type {
        filter = filter.with_entry_type(et.clone());
    }
    if let Some(ref ct) = filters.chunk_type {
        filter = filter.with_chunk_type(ct.clone());
    }
    if let Some(ref sd) = filters.source_domain {
        filter = filter.with_source_domain(sd.clone());
    }
    if let Some(ref t) = filters.tag {
        filter = filter.with_tag(t.clone());
    }
    Some(filter)
}

/// Convert VectorSearchResults to SearchResults.
fn vec_results_to_search_results(vec_results: &[VectorSearchResult]) -> Vec<SearchResult> {
    vec_results
        .iter()
        .map(|r| SearchResult {
            chunk_id: r.chunk_id.clone(),
            entry_id: r.entry_id.clone(),
            page_id: r.page_id.clone(),
            entry_title: r.entry_title.clone(),
            text: r.text.clone(),
            score: r.score,
            entry_type: r.entry_type.clone(),
            tags: r.tags.clone(),
            chunk_type: Some(r.chunk_type.clone()),
            source_domain: r.source_domain.clone(),
        })
        .collect()
}

/// Reciprocal Rank Fusion to combine vector and FTS results.
fn reciprocal_rank_fusion(
    vector_results: &[VectorSearchResult],
    fts_results: &[kix_sqlite::search::FtsResult],
    limit: usize,
) -> Vec<SearchResult> {
    const K: f32 = 60.0; // RRF constant

    let mut scores: HashMap<String, f32> = HashMap::new();
    let mut result_data: HashMap<String, SearchResult> = HashMap::new();

    // Add vector search scores
    for (rank, result) in vector_results.iter().enumerate() {
        let rrf_score = 1.0 / (K + rank as f32);
        *scores.entry(result.chunk_id.clone()).or_insert(0.0) += rrf_score;
        result_data.entry(result.chunk_id.clone()).or_insert_with(|| {
            SearchResult {
                chunk_id: result.chunk_id.clone(),
                entry_id: result.entry_id.clone(),
                page_id: result.page_id.clone(),
                entry_title: result.entry_title.clone(),
                text: result.text.clone(),
                score: result.score,
                entry_type: result.entry_type.clone(),
                tags: result.tags.clone(),
                chunk_type: Some(result.chunk_type.clone()),
                source_domain: result.source_domain.clone(),
            }
        });
    }

    // Add FTS scores (FTS results are entry-level, we boost chunks from those entries)
    for (rank, fts_result) in fts_results.iter().enumerate() {
        let rrf_score = 1.0 / (K + rank as f32);
        // Find chunks belonging to this entry and boost them
        for result in vector_results.iter() {
            if result.entry_id == fts_result.id {
                *scores.entry(result.chunk_id.clone()).or_insert(0.0) += rrf_score;
            }
        }
    }

    // Sort by combined score
    let mut scored: Vec<_> = scores.into_iter().collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Return top results with updated scores
    scored
        .into_iter()
        .take(limit)
        .filter_map(|(id, score)| {
            result_data.remove(&id).map(|mut r| {
                r.score = score;
                r
            })
        })
        .collect()
}

/// Returns the configured embedding dimensions from environment.
pub fn get_embedding_dim() -> usize {
    std::env::var("KIX_EMBEDDING_DIM")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| {
            if let Ok(model) = std::env::var("KIX_EMBEDDING_MODEL") {
                match model.to_lowercase().as_str() {
                    s if s.contains("large") => 1024,
                    s if s.contains("base") => 768,
                    s if s.contains("small") || s.contains("minilm") => 384,
                    _ => DEFAULT_EMBEDDING_DIM,
                }
            } else {
                DEFAULT_EMBEDDING_DIM
            }
        })
}

/// Convert a kix_parser Entry to kix_sqlite EntryRecord.
fn entry_to_record(entry: &Entry) -> EntryRecord {
    let tags_json = if entry.tags.is_empty() {
        None
    } else {
        serde_json::to_string(&entry.tags).ok()
    };

    let collection_ids_json = if entry.collection_ids.is_empty() {
        None
    } else {
        serde_json::to_string(&entry.collection_ids).ok()
    };

    // Extract source_domain from source_path if it's a URL
    let source_domain = if entry.source_path.starts_with("http") {
        url::Url::parse(&entry.source_path)
            .ok()
            .and_then(|u| u.host_str().map(|s| s.to_string()))
    } else {
        None
    };

    EntryRecord {
        id: entry.id.clone(),
        title: entry.title.clone(),
        description: if entry.description.is_empty() { None } else { Some(entry.description.clone()) },
        content: if entry.content.is_empty() { None } else { Some(entry.content.clone()) },
        tags: tags_json,
        collection_ids: collection_ids_json,
        entry_type: format!("{:?}", entry.entry_type).to_lowercase(),
        source_type: format!("{:?}", entry.source_type).to_lowercase(),
        source_path: entry.source_path.clone(),
        source_domain,
        source_hash: entry.source_hash.clone(),
        slug: entry.slug.clone(),
        created_at: entry.created_at.to_rfc3339(),
        updated_at: entry.updated_at.to_rfc3339(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_kix_store_creation() {
        let temp_dir = TempDir::new().unwrap();
        let mut store = KixStore::new(temp_dir.path()).await.unwrap();
        store.init().await.unwrap();

        // Verify directory structure
        assert!(temp_dir.path().join("sqlite/kix.db").exists());
        assert!(temp_dir.path().join("sqlite/vectors.db").exists());
    }

    #[tokio::test]
    async fn test_entry_operations() {
        let temp_dir = TempDir::new().unwrap();
        let mut store = KixStore::new(temp_dir.path()).await.unwrap();
        store.init().await.unwrap();

        // Insert entry
        let entry = EntryRecord::new(
            "test-1",
            "Test Entry",
            "document",
            "url",
            "https://example.com",
            "hash123",
        );
        store.insert_entry(&entry).await.unwrap();

        // Retrieve entry
        let retrieved = store.get_entry("test-1").await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().title, "Test Entry");

        // Count
        assert_eq!(store.entry_count().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_page_operations() {
        let temp_dir = TempDir::new().unwrap();
        let mut store = KixStore::new(temp_dir.path()).await.unwrap();
        store.init().await.unwrap();

        // Create entry first
        let entry = EntryRecord::new(
            "entry-1",
            "Entry",
            "document",
            "url",
            "https://example.com",
            "hash",
        );
        store.insert_entry(&entry).await.unwrap();

        // Insert page
        let page = PageRecord::new("entry-1", "https://example.com/page", "# Content");
        store.insert_page(&page).await.unwrap();

        // Retrieve page
        let retrieved = store.get_page(&page.page_id).await.unwrap();
        assert!(retrieved.is_some());

        // Count
        assert_eq!(store.page_count().await.unwrap(), 1);
    }
}
