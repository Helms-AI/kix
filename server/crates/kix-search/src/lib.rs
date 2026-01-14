//! Tantivy-based full-text search engine for KIX.
//!
//! This crate provides full-text search capabilities using Tantivy, replacing
//! SQLite FTS5. It supports:
//!
//! - BM25 ranking for relevance scoring
//! - Fuzzy search and phrase matching
//! - Faceted filtering by type, domain, state
//! - Snippet generation with highlighting
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────┐     ┌─────────────────┐
//! │   KixStore      │     │   SearchEngine  │
//! │   (SeaORM)      │────▶│   (Tantivy)     │
//! │   CRUD ops      │     │   FTS search    │
//! └─────────────────┘     └─────────────────┘
//!          │                       │
//!          └───────────┬───────────┘
//!                      ▼
//!               Hybrid Search
//!          (RRF fusion of results)
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use kix_search::SearchEngine;
//! use std::path::Path;
//!
//! // Create search engine
//! let engine = SearchEngine::new(Path::new("./search_index"))?;
//!
//! // Index an entry
//! engine.index_entry(&entry)?;
//!
//! // Search
//! let results = engine.search_entries("content router", 10)?;
//! ```

pub mod error;
pub mod indexer;
pub mod schema;
pub mod searcher;

#[cfg(feature = "sync")]
pub mod sync;

use std::path::Path;

use tantivy::Index;
use tracing::info;

pub use error::{SearchError, SearchResult};
pub use indexer::{EntryDocument, EntryIndexWriter, IssueDocument, IssueIndexWriter, PageDocument, PageIndexWriter};
pub use schema::{
    build_entry_schema, build_issue_schema, build_page_schema, EntryFields, IssueFields,
    PageFields,
};
pub use searcher::{
    EntrySearchFilters, EntrySearcher, IssueSearchFilters, IssueSearcher, PageSearcher,
    TextSearchResult,
};

#[cfg(feature = "sync")]
pub use sync::{IndexSynchronizer, SyncStats};

/// Unified search engine managing all Tantivy indexes.
///
/// The SearchEngine provides a single interface for managing entries, pages,
/// and issues search indexes. It handles index creation, document indexing,
/// and search queries.
pub struct SearchEngine {
    /// Entry index and components
    entry_index: Index,
    entry_schema: tantivy::schema::Schema,
    entry_fields: EntryFields,

    /// Page index and components
    page_index: Index,
    page_schema: tantivy::schema::Schema,
    page_fields: PageFields,

    /// Issue index and components
    issue_index: Index,
    issue_schema: tantivy::schema::Schema,
    issue_fields: IssueFields,

    /// Base index path
    index_path: std::path::PathBuf,
}

impl SearchEngine {
    /// Create a new search engine with indexes at the given path.
    ///
    /// Creates three separate indexes:
    /// - `{path}/entries` - Entry documents
    /// - `{path}/pages` - Page documents
    /// - `{path}/issues` - Issue documents
    pub fn new(index_path: &Path) -> SearchResult<Self> {
        std::fs::create_dir_all(index_path)?;

        // Create entry index
        let entry_path = index_path.join("entries");
        std::fs::create_dir_all(&entry_path)?;
        let (entry_schema, entry_fields) = build_entry_schema();
        let entry_index = Self::open_or_create_index(&entry_path, entry_schema.clone())?;

        // Create page index
        let page_path = index_path.join("pages");
        std::fs::create_dir_all(&page_path)?;
        let (page_schema, page_fields) = build_page_schema();
        let page_index = Self::open_or_create_index(&page_path, page_schema.clone())?;

        // Create issue index
        let issue_path = index_path.join("issues");
        std::fs::create_dir_all(&issue_path)?;
        let (issue_schema, issue_fields) = build_issue_schema();
        let issue_index = Self::open_or_create_index(&issue_path, issue_schema.clone())?;

        info!(path = %index_path.display(), "Search engine initialized");

        Ok(Self {
            entry_index,
            entry_schema,
            entry_fields,
            page_index,
            page_schema,
            page_fields,
            issue_index,
            issue_schema,
            issue_fields,
            index_path: index_path.to_path_buf(),
        })
    }

    fn open_or_create_index(
        path: &Path,
        schema: tantivy::schema::Schema,
    ) -> SearchResult<Index> {
        if path.join("meta.json").exists() {
            Index::open_in_dir(path)
                .map_err(|e| SearchError::Index(format!("Failed to open index: {}", e)))
        } else {
            Index::create_in_dir(path, schema)
                .map_err(|e| SearchError::Index(format!("Failed to create index: {}", e)))
        }
    }

    // =========================================================================
    // Entry Operations
    // =========================================================================

    /// Get an entry index writer for bulk operations.
    pub fn entry_writer(&self) -> EntryIndexWriter {
        EntryIndexWriter::new(
            self.entry_index.clone(),
            self.entry_schema.clone(),
            self.entry_fields.clone(),
        )
    }

    /// Get an entry searcher.
    pub fn entry_searcher(&self) -> SearchResult<EntrySearcher> {
        EntrySearcher::new(
            self.entry_index.clone(),
            self.entry_schema.clone(),
            self.entry_fields.clone(),
        )
    }

    /// Index a single entry.
    pub fn index_entry(&self, entry: &EntryDocument) -> SearchResult<()> {
        self.entry_writer().index_entry(entry)
    }

    /// Index multiple entries in batch.
    pub fn index_entries(&self, entries: &[EntryDocument]) -> SearchResult<usize> {
        self.entry_writer().index_batch(entries)
    }

    /// Delete an entry from the index.
    pub fn delete_entry(&self, entry_id: &str) -> SearchResult<()> {
        self.entry_writer().delete_entry(entry_id)
    }

    /// Search entries.
    pub fn search_entries(
        &self,
        query: &str,
        limit: usize,
        filters: &EntrySearchFilters,
    ) -> SearchResult<Vec<TextSearchResult>> {
        self.entry_searcher()?.search(query, limit, filters)
    }

    // =========================================================================
    // Page Operations
    // =========================================================================

    /// Get a page index writer for bulk operations.
    pub fn page_writer(&self) -> PageIndexWriter {
        PageIndexWriter::new(
            self.page_index.clone(),
            self.page_schema.clone(),
            self.page_fields.clone(),
        )
    }

    /// Get a page searcher.
    pub fn page_searcher(&self) -> SearchResult<PageSearcher> {
        PageSearcher::new(
            self.page_index.clone(),
            self.page_schema.clone(),
            self.page_fields.clone(),
        )
    }

    /// Index a single page.
    pub fn index_page(&self, page: &PageDocument) -> SearchResult<()> {
        self.page_writer().index_page(page)
    }

    /// Index multiple pages in batch.
    pub fn index_pages(&self, pages: &[PageDocument]) -> SearchResult<usize> {
        self.page_writer().index_batch(pages)
    }

    /// Delete a page from the index.
    pub fn delete_page(&self, page_id: &str) -> SearchResult<()> {
        self.page_writer().delete_page(page_id)
    }

    /// Search pages.
    pub fn search_pages(&self, query: &str, limit: usize) -> SearchResult<Vec<TextSearchResult>> {
        self.page_searcher()?.search(query, limit)
    }

    // =========================================================================
    // Issue Operations
    // =========================================================================

    /// Get an issue index writer for bulk operations.
    pub fn issue_writer(&self) -> IssueIndexWriter {
        IssueIndexWriter::new(
            self.issue_index.clone(),
            self.issue_schema.clone(),
            self.issue_fields.clone(),
        )
    }

    /// Get an issue searcher.
    pub fn issue_searcher(&self) -> SearchResult<IssueSearcher> {
        IssueSearcher::new(
            self.issue_index.clone(),
            self.issue_schema.clone(),
            self.issue_fields.clone(),
        )
    }

    /// Index a single issue.
    pub fn index_issue(&self, issue: &IssueDocument) -> SearchResult<()> {
        self.issue_writer().index_issue(issue)
    }

    /// Index multiple issues in batch.
    pub fn index_issues(&self, issues: &[IssueDocument]) -> SearchResult<usize> {
        self.issue_writer().index_batch(issues)
    }

    /// Delete an issue from the index.
    pub fn delete_issue(&self, issue_id: &str) -> SearchResult<()> {
        self.issue_writer().delete_issue(issue_id)
    }

    /// Search issues.
    pub fn search_issues(
        &self,
        query: &str,
        limit: usize,
        filters: &IssueSearchFilters,
    ) -> SearchResult<Vec<TextSearchResult>> {
        self.issue_searcher()?.search(query, limit, filters)
    }

    // =========================================================================
    // Utility Operations
    // =========================================================================

    /// Clear all indexes.
    pub fn clear_all(&self) -> SearchResult<()> {
        self.entry_writer().clear_all()?;
        self.page_writer().clear_all()?;
        self.issue_writer().clear_all()?;
        info!("Cleared all search indexes");
        Ok(())
    }

    /// Get index statistics.
    pub fn stats(&self) -> SearchResult<SearchStats> {
        let entry_searcher = self.entry_searcher()?;
        let page_searcher = self.page_searcher()?;
        let issue_searcher = self.issue_searcher()?;

        Ok(SearchStats {
            entry_count: entry_searcher.doc_count(),
            page_count: page_searcher.doc_count(),
            issue_count: issue_searcher.doc_count(),
            index_path: self.index_path.to_string_lossy().to_string(),
        })
    }

    /// Reload all index readers to see recent commits.
    pub fn reload_all(&self) -> SearchResult<()> {
        self.entry_searcher()?.reload()?;
        self.page_searcher()?.reload()?;
        self.issue_searcher()?.reload()?;
        Ok(())
    }
}

/// Statistics about the search indexes.
#[derive(Debug, Clone)]
pub struct SearchStats {
    /// Number of entry documents indexed.
    pub entry_count: u64,
    /// Number of page documents indexed.
    pub page_count: u64,
    /// Number of issue documents indexed.
    pub issue_count: u64,
    /// Path to index directory.
    pub index_path: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn create_test_entry() -> EntryDocument {
        EntryDocument {
            id: "test-1".to_string(),
            title: "Test Entry".to_string(),
            description: Some("A test entry for searching".to_string()),
            content: Some("This is the content of the test entry".to_string()),
            entry_type: "document".to_string(),
            source_domain: Some("test.example.com".to_string()),
            source_path: "/test".to_string(),
            tags: vec!["test".to_string()],
            created_at: chrono::Utc::now(),
        }
    }

    fn create_test_page() -> PageDocument {
        PageDocument {
            page_id: "page-1".to_string(),
            entry_id: "test-1".to_string(),
            url: "https://test.example.com/page".to_string(),
            title: Some("Test Page".to_string()),
            content: "Full page content for testing".to_string(),
            created_at: chrono::Utc::now(),
        }
    }

    fn create_test_issue() -> IssueDocument {
        IssueDocument {
            id: "issue-1".to_string(),
            project_id: "project-1".to_string(),
            number: 1,
            title: "Test Issue".to_string(),
            body: Some("This is a test issue description".to_string()),
            state: "open".to_string(),
            labels: vec!["bug".to_string()],
            created_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn test_search_engine_creation() {
        let dir = tempdir().unwrap();
        let engine = SearchEngine::new(dir.path()).unwrap();

        let stats = engine.stats().unwrap();
        assert_eq!(stats.entry_count, 0);
        assert_eq!(stats.page_count, 0);
        assert_eq!(stats.issue_count, 0);
    }

    #[test]
    fn test_entry_indexing_and_search() {
        let dir = tempdir().unwrap();
        let engine = SearchEngine::new(dir.path()).unwrap();

        // Index entry
        let entry = create_test_entry();
        engine.index_entry(&entry).unwrap();

        // Search
        let results = engine
            .search_entries("test", 10, &EntrySearchFilters::default())
            .unwrap();

        assert!(!results.is_empty());
        assert_eq!(results[0].id, "test-1");
    }

    #[test]
    fn test_page_indexing_and_search() {
        let dir = tempdir().unwrap();
        let engine = SearchEngine::new(dir.path()).unwrap();

        let page = create_test_page();
        engine.index_page(&page).unwrap();

        let results = engine.search_pages("content", 10).unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_issue_indexing_and_search() {
        let dir = tempdir().unwrap();
        let engine = SearchEngine::new(dir.path()).unwrap();

        let issue = create_test_issue();
        engine.index_issue(&issue).unwrap();

        let results = engine
            .search_issues("test", 10, &IssueSearchFilters::default())
            .unwrap();

        assert!(!results.is_empty());
        assert_eq!(results[0].id, "issue-1");
    }

    #[test]
    fn test_clear_all() {
        let dir = tempdir().unwrap();
        let engine = SearchEngine::new(dir.path()).unwrap();

        // Index some documents
        engine.index_entry(&create_test_entry()).unwrap();
        engine.index_page(&create_test_page()).unwrap();
        engine.index_issue(&create_test_issue()).unwrap();

        // Clear all
        engine.clear_all().unwrap();
        engine.reload_all().unwrap();

        // Verify empty
        let stats = engine.stats().unwrap();
        assert_eq!(stats.entry_count, 0);
        assert_eq!(stats.page_count, 0);
        assert_eq!(stats.issue_count, 0);
    }

    #[test]
    fn test_delete_entry() {
        let dir = tempdir().unwrap();
        let engine = SearchEngine::new(dir.path()).unwrap();

        let entry = create_test_entry();
        engine.index_entry(&entry).unwrap();

        // Verify indexed
        let results = engine
            .search_entries("test", 10, &EntrySearchFilters::default())
            .unwrap();
        assert!(!results.is_empty());

        // Delete
        engine.delete_entry(&entry.id).unwrap();

        // Verify deleted (may need reload for consistency)
        engine.reload_all().unwrap();
        let results = engine
            .search_entries("test", 10, &EntrySearchFilters::default())
            .unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_entry_search_with_filters() {
        let dir = tempdir().unwrap();
        let engine = SearchEngine::new(dir.path()).unwrap();

        // Index multiple entries with different types
        let entry1 = EntryDocument {
            id: "doc-1".to_string(),
            entry_type: "document".to_string(),
            ..create_test_entry()
        };
        let entry2 = EntryDocument {
            id: "pattern-1".to_string(),
            entry_type: "pattern".to_string(),
            title: "Test Pattern".to_string(),
            ..create_test_entry()
        };

        engine.index_entry(&entry1).unwrap();
        engine.index_entry(&entry2).unwrap();

        // Search with filter
        let filters = EntrySearchFilters {
            entry_type: Some("pattern".to_string()),
            ..Default::default()
        };
        let results = engine.search_entries("test", 10, &filters).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "pattern-1");
    }
}
