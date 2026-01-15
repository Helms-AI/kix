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
    EntryRecord, WorkItemRecord, JobRecord, PageRecord, ProjectEntryRecord, ProjectRecord,
    SqliteStore,
};
use kix_search::{
    EntryDocument, EntrySearchFilters, IssueDocument, PageDocument, SearchEngine,
};
use kix_vectors::{SearchFilter, VectorSearchResult, VectorStore};
use std::collections::HashMap;
use std::path::Path;
use tracing::info;

/// Default embedding dimensions (768 for bge-base-en-v1.5).
pub const DEFAULT_EMBEDDING_DIM: usize = 768;

/// Statistics from a full reindex operation.
#[derive(Debug, Clone, Default)]
pub struct SearchReindexStats {
    /// Number of entries indexed.
    pub entries_indexed: usize,
    /// Number of pages indexed.
    pub pages_indexed: usize,
    /// Number of issues indexed.
    pub issues_indexed: usize,
    /// Errors encountered during reindex.
    pub errors: Vec<String>,
}

impl SearchReindexStats {
    /// Total documents indexed.
    pub fn total(&self) -> usize {
        self.entries_indexed + self.pages_indexed + self.issues_indexed
    }

    /// Whether the reindex had any errors.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

/// Unified KIX store using SQLite for all storage.
///
/// - Structured data (entries, pages, projects, etc.) stored in kix.db via kix-sqlite
/// - Vector embeddings stored in vectors.db via kix-vectors (sqlite-vec)
/// - Full-text search via Tantivy (kix-search)
pub struct KixStore {
    /// SQLite store for structured data (kix.db)
    pub sqlite: SqliteStore,
    /// Vector store for embeddings (vectors.db)
    pub vectors: VectorStore,
    /// Embedding dimensions
    embedding_dim: usize,
    /// Tantivy search engine for full-text search
    search: SearchEngine,
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

        // Initialize Tantivy search engine
        let search_path = data_dir.join("search");
        info!("Creating Tantivy search index at {}", search_path.display());
        let search = SearchEngine::new(&search_path).map_err(|e| {
            StoreError::Database(format!("Failed to create search engine: {}", e))
        })?;

        Ok(Self {
            sqlite,
            vectors,
            embedding_dim,
            search,
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

    /// Get reference to Tantivy search engine.
    pub fn search(&self) -> &SearchEngine {
        &self.search
    }

    /// Get reference to page store (for backward compatibility, returns self since pages are in SQLite).
    pub fn page_store(&self) -> &Self {
        self
    }

    // =========================================================================
    // Entry Operations (SQLite)
    // =========================================================================

    /// Insert an entry into SQLite and Tantivy search index.
    pub async fn insert_entry(&self, entry: &EntryRecord) -> Result<(), StoreError> {
        self.sqlite
            .insert_entry(entry)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))?;

        // Sync to Tantivy search index
        let doc = entry_record_to_document(entry);
        self.search
            .index_entry(&doc)
            .map_err(|e| StoreError::Database(format!("Search sync failed: {}", e)))?;

        Ok(())
    }

    /// Get an entry by ID from SQLite.
    pub async fn get_entry(&self, id: &str) -> Result<Option<EntryRecord>, StoreError> {
        self.sqlite
            .get_entry(id)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))
    }

    /// Delete an entry from SQLite and Tantivy (also deletes associated chunks).
    pub async fn delete_entry(&self, id: &str) -> Result<bool, StoreError> {
        // Delete chunks from vector store first (via spawn_blocking)
        let vectors = self.vectors.clone();
        let id_owned = id.to_string();
        let _ = tokio::task::spawn_blocking(move || {
            vectors.delete_chunks_by_entry(&id_owned)
        })
        .await;

        // Delete from Tantivy search index
        self.search
            .delete_entry(id)
            .map_err(|e| StoreError::Database(format!("Search delete failed: {}", e)))?;

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
    pub async fn delete_chunks_by_document(&self, entry_id: &str) -> Result<usize, StoreError> {
        self.delete_chunks_by_entry(entry_id).await
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

    /// Insert a page into SQLite and Tantivy search index.
    pub async fn insert_page(&self, page: &PageRecord) -> Result<(), StoreError> {
        self.sqlite
            .insert_page(page)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))?;

        // Sync to Tantivy search index
        let doc = page_record_to_document(page);
        self.search
            .index_page(&doc)
            .map_err(|e| StoreError::Database(format!("Search sync failed: {}", e)))?;

        Ok(())
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
    ///
    /// Uses spawn_blocking to avoid blocking the async runtime.
    pub async fn delete_chunks_by_entry(&self, entry_id: &str) -> Result<usize, StoreError> {
        let vectors = self.vectors.clone();
        let entry_id = entry_id.to_string();

        tokio::task::spawn_blocking(move || {
            vectors.delete_chunks_by_entry(&entry_id)
        })
        .await
        .map_err(|e| StoreError::Internal(format!("spawn_blocking failed: {}", e)))?
        .map_err(|e| StoreError::Database(e.to_string()))
    }

    /// Get chunk count from vector store.
    pub fn chunk_count(&self) -> Result<usize, StoreError> {
        self.vectors
            .chunk_count()
            .map_err(|e| StoreError::Database(e.to_string()))
    }

    /// Get chunk counts grouped by chunk_type.
    ///
    /// Returns a map of chunk_type -> count (e.g., {"code": 150, "content": 500, ...})
    pub fn chunk_counts_by_type(&self) -> Result<std::collections::HashMap<String, usize>, StoreError> {
        self.vectors
            .chunk_counts_by_type()
            .map_err(|e| StoreError::Database(e.to_string()))
    }

    /// Get distinct entry IDs that have chunks of a specific type.
    ///
    /// Useful for listing all entries that contain code blocks, summaries, etc.
    pub fn get_entry_ids_with_chunk_type(&self, chunk_type: &str) -> Result<Vec<String>, StoreError> {
        self.vectors
            .get_entry_ids_with_chunk_type(chunk_type)
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

    /// Perform hybrid search combining vector search and Tantivy full-text search.
    ///
    /// Uses Reciprocal Rank Fusion to combine results from both sources.
    /// Vector search uses spawn_blocking to avoid blocking the async runtime.
    pub async fn hybrid_search(
        &self,
        query_text: &str,
        query_embedding: &[f32],
        limit: usize,
        filters: &SearchFilters,
    ) -> Result<Vec<SearchResult>, StoreError> {
        // 1. Vector search in VectorStore (via spawn_blocking)
        let vectors = self.vectors.clone();
        let vec_filter = filters_to_vec_filter(filters);
        let query_embedding = query_embedding.to_vec();
        let vector_limit = limit * 2;

        let vector_results = tokio::task::spawn_blocking(move || {
            vectors.vector_search(&query_embedding, vector_limit, vec_filter.as_ref())
        })
        .await
        .map_err(|e| StoreError::Internal(format!("spawn_blocking failed: {}", e)))?
        .map_err(|e| StoreError::Database(e.to_string()))?;

        // 2. Tantivy full-text search
        let tantivy_filters = EntrySearchFilters {
            entry_type: filters.entry_type.clone(),
            source_domain: filters.source_domain.clone(),
            tag: filters.tag.clone(),
        };
        let fts_results = self.search
            .search_entries(query_text, limit * 2, &tantivy_filters)
            .map_err(|e| StoreError::Database(format!("Tantivy search failed: {}", e)))?;

        // 3. Combine using Reciprocal Rank Fusion
        let combined = reciprocal_rank_fusion(&vector_results, &fts_results, limit);

        Ok(combined)
    }

    /// Perform vector-only search.
    ///
    /// Uses spawn_blocking to avoid blocking the async runtime,
    /// since VectorStore uses a blocking mutex internally.
    pub async fn vector_search(
        &self,
        query_embedding: &[f32],
        limit: usize,
        filters: &SearchFilters,
    ) -> Result<Vec<SearchResult>, StoreError> {
        let vectors = self.vectors.clone();
        let vec_filter = filters_to_vec_filter(filters);
        let query_embedding = query_embedding.to_vec();

        let results = tokio::task::spawn_blocking(move || {
            vectors.vector_search(&query_embedding, limit, vec_filter.as_ref())
        })
        .await
        .map_err(|e| StoreError::Internal(format!("spawn_blocking failed: {}", e)))?
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
    // Work Item Operations (SQLite)
    // =========================================================================

    /// Insert a work item into SQLite and Tantivy search index.
    pub async fn insert_work_item(&self, item: &WorkItemRecord) -> Result<(), StoreError> {
        self.sqlite
            .insert_work_item(item)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))?;

        // Sync to Tantivy search index
        let doc = work_item_record_to_document(item);
        self.search
            .index_issue(&doc)
            .map_err(|e| StoreError::Database(format!("Search sync failed: {}", e)))?;

        Ok(())
    }

    /// Get a work item by ID.
    pub async fn get_work_item(&self, id: &str) -> Result<Option<WorkItemRecord>, StoreError> {
        self.sqlite
            .get_work_item(id)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))
    }

    /// List work items for a project.
    pub async fn list_work_items(
        &self,
        project_id: &str,
        state: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<WorkItemRecord>, StoreError> {
        self.sqlite
            .list_work_items(project_id, state, limit, offset)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))
    }

    /// Update a work item in SQLite and Tantivy search index.
    pub async fn update_work_item(&self, item: &WorkItemRecord) -> Result<bool, StoreError> {
        let result = self.sqlite
            .update_work_item(item)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))?;

        // Sync to Tantivy search index
        if result {
            let doc = work_item_record_to_document(item);
            self.search
                .index_issue(&doc)
                .map_err(|e| StoreError::Database(format!("Search sync failed: {}", e)))?;
        }

        Ok(result)
    }

    /// Delete a work item from SQLite and Tantivy search index.
    pub async fn delete_work_item(&self, id: &str) -> Result<bool, StoreError> {
        // Delete from Tantivy search index first
        self.search
            .delete_issue(id)
            .map_err(|e| StoreError::Database(format!("Search delete failed: {}", e)))?;

        self.sqlite
            .delete_work_item(id)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))
    }

    /// Get a work item by number within a project.
    pub async fn get_work_item_by_number(
        &self,
        project_id: &str,
        number: u32,
    ) -> Result<Option<WorkItemRecord>, StoreError> {
        self.sqlite
            .get_work_item_by_number(project_id, number)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))
    }

    /// Get the next work item number for a project.
    pub async fn next_work_item_number(&self, project_id: &str) -> Result<u32, StoreError> {
        self.sqlite
            .next_work_item_number(project_id)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))
    }

    /// Count work items for a project.
    pub async fn work_item_count(&self, project_id: &str) -> Result<usize, StoreError> {
        self.sqlite
            .work_item_count(project_id)
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

        // Clear Tantivy search indexes
        self.search
            .clear_all()
            .map_err(|e| StoreError::Database(format!("Failed to clear search: {}", e)))?;

        info!("Cleared all data from KIX store");
        Ok(())
    }

    // =========================================================================
    // Tantivy Search Operations (when feature enabled)
    // =========================================================================

    /// Full reindex of all entries, pages, and issues from SQLite to Tantivy.
    ///
    /// This is useful when rebuilding the search index.
    pub async fn full_reindex(&self) -> Result<SearchReindexStats, StoreError> {
        use tracing::warn;

        info!("Starting full reindex to Tantivy search");

        // Clear existing indexes
        self.search
            .clear_all()
            .map_err(|e| StoreError::Database(format!("Failed to clear search: {}", e)))?;

        let mut stats = SearchReindexStats::default();

        // Reindex entries
        let entries = self.list_entries(None, None, 100000, 0).await?;
        info!("Reindexing {} entries", entries.len());
        for entry in &entries {
            let doc = entry_record_to_document(entry);
            match self.search.index_entry(&doc) {
                Ok(_) => stats.entries_indexed += 1,
                Err(e) => {
                    warn!(entry_id = %entry.id, error = %e, "Failed to index entry");
                    stats.errors.push(format!("Entry {}: {}", entry.id, e));
                }
            }
        }

        // Reindex pages (via entries)
        info!("Reindexing pages for {} entries", entries.len());
        for entry in &entries {
            let pages = self.sqlite.get_pages_by_source(&entry.id).await
                .map_err(|e| StoreError::Database(e.to_string()))?;
            for page in &pages {
                let doc = page_record_to_document(page);
                match self.search.index_page(&doc) {
                    Ok(_) => stats.pages_indexed += 1,
                    Err(e) => {
                        warn!(page_id = %page.page_id, error = %e, "Failed to index page");
                        stats.errors.push(format!("Page {}: {}", page.page_id, e));
                    }
                }
            }
        }

        // Reindex work items (for all projects)
        let projects = self.list_projects(true).await?;
        for project in &projects {
            let items = self.list_work_items(&project.id, None, 100000, 0).await?;
            info!("Reindexing {} work items for project {}", items.len(), project.id);
            for item in &items {
                let doc = work_item_record_to_document(item);
                match self.search.index_issue(&doc) {
                    Ok(_) => stats.issues_indexed += 1,
                    Err(e) => {
                        warn!(item_id = %item.id, error = %e, "Failed to index work item");
                        stats.errors.push(format!("WorkItem {}: {}", item.id, e));
                    }
                }
            }
        }

        info!(
            entries = stats.entries_indexed,
            pages = stats.pages_indexed,
            issues = stats.issues_indexed,
            errors = stats.errors.len(),
            "Full reindex complete"
        );

        Ok(stats)
    }

    /// Get Tantivy search index statistics.
    pub fn search_stats(&self) -> Result<kix_search::SearchStats, StoreError> {
        self.search
            .stats()
            .map_err(|e| StoreError::Database(format!("Failed to get search stats: {}", e)))
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
    ///
    /// Uses spawn_blocking to avoid blocking the async runtime,
    /// since VectorStore uses a blocking mutex internally.
    pub async fn get_chunks_by_entry_id(&self, entry_id: &str) -> Result<Vec<VectorSearchResult>, StoreError> {
        let vectors = self.vectors.clone();
        let entry_id = entry_id.to_string();

        tokio::task::spawn_blocking(move || {
            vectors.get_chunks_by_entry(&entry_id)
        })
        .await
        .map_err(|e| StoreError::Internal(format!("spawn_blocking failed: {}", e)))?
        .map_err(|e| StoreError::Database(e.to_string()))
    }

    /// Get the cached embedding for an entry.
    ///
    /// Returns the first chunk's embedding (chunk_index = 0), which is typically
    /// the most representative of the entry's content. This avoids regenerating
    /// embeddings via Ollama when finding related entries.
    ///
    /// Uses spawn_blocking to avoid blocking the async runtime.
    ///
    /// # Returns
    /// - `Ok(Some(embedding))` - Cached embedding found
    /// - `Ok(None)` - Entry has no chunks (fallback to generation needed)
    /// - `Err(...)` - Database error
    pub async fn get_entry_embedding(&self, entry_id: &str) -> Result<Option<Vec<f32>>, StoreError> {
        let vectors = self.vectors.clone();
        let entry_id = entry_id.to_string();

        tokio::task::spawn_blocking(move || {
            vectors.get_entry_embedding(&entry_id)
        })
        .await
        .map_err(|e| StoreError::Internal(format!("spawn_blocking failed: {}", e)))?
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

/// Reciprocal Rank Fusion to combine vector and Tantivy full-text search results.
fn reciprocal_rank_fusion(
    vector_results: &[VectorSearchResult],
    tantivy_results: &[kix_search::TextSearchResult],
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

    // Add Tantivy scores (Tantivy results are entry-level, we boost chunks from those entries)
    for (rank, tantivy_result) in tantivy_results.iter().enumerate() {
        let rrf_score = 1.0 / (K + rank as f32);
        // Find chunks belonging to this entry and boost them
        for result in vector_results.iter() {
            if result.entry_id == tantivy_result.id {
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

// =========================================================================
// Tantivy Search Integration
// =========================================================================

/// Convert EntryRecord to Tantivy EntryDocument.
fn entry_record_to_document(entry: &EntryRecord) -> EntryDocument {
    let tags: Vec<String> = entry
        .tags
        .as_ref()
        .and_then(|t| serde_json::from_str(t).ok())
        .unwrap_or_default();

    let created_at = chrono::DateTime::parse_from_rfc3339(&entry.created_at)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .unwrap_or_else(|_| chrono::Utc::now());

    EntryDocument {
        id: entry.id.clone(),
        title: entry.title.clone(),
        description: entry.description.clone(),
        content: entry.content.clone(),
        entry_type: entry.entry_type.clone(),
        source_domain: entry.source_domain.clone(),
        source_path: entry.source_path.clone(),
        tags,
        created_at,
    }
}

/// Convert PageRecord to Tantivy PageDocument.
fn page_record_to_document(page: &PageRecord) -> PageDocument {
    let created_at = chrono::DateTime::parse_from_rfc3339(&page.created_at)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .unwrap_or_else(|_| chrono::Utc::now());

    PageDocument {
        page_id: page.page_id.clone(),
        entry_id: page.source_id.clone(),
        url: page.url.clone(),
        title: page.title.clone(),
        content: page.full_content.clone(),
        created_at,
    }
}

/// Convert WorkItemRecord to Tantivy IssueDocument.
fn work_item_record_to_document(item: &WorkItemRecord) -> IssueDocument {
    let labels: Vec<String> = item
        .labels
        .as_ref()
        .and_then(|l| serde_json::from_str(l).ok())
        .unwrap_or_default();

    let created_at = chrono::DateTime::parse_from_rfc3339(&item.created_at)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .unwrap_or_else(|_| chrono::Utc::now());

    IssueDocument {
        id: item.id.clone(),
        project_id: item.project_id.clone(),
        number: item.number,
        title: item.title.clone(),
        body: item.body.clone(),
        state: item.state.clone(),
        labels,
        created_at,
    }
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
