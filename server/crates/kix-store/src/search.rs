//! Search types for the Knowledge Indexer store.

use serde::{Deserialize, Serialize};

// Re-export EntryChunk from kix_parser for convenience
pub use kix_parser::EntryChunk;

/// Summary of an entry/pattern for API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternSummary {
    /// Entry ID
    pub id: String,
    /// Entry title
    pub title: String,
    /// Entry type (document, article, pdf, etc.)
    pub entry_type: String,
    /// Tags
    pub tags: Vec<String>,
    /// Description/summary
    pub description: String,
    /// Source type (url, file_upload, etc.)
    pub source_type: Option<String>,
    /// Source domain (e.g., "docs.example.com")
    pub source_domain: Option<String>,
    /// Source path/URL
    pub source_path: Option<String>,
    /// Created timestamp
    pub created_at: Option<String>,
    /// Updated timestamp
    pub updated_at: Option<String>,
}

impl From<kix_sqlite::EntryRecord> for PatternSummary {
    fn from(entry: kix_sqlite::EntryRecord) -> Self {
        // Parse tags from JSON if present
        let tags: Vec<String> = entry
            .tags
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();

        Self {
            id: entry.id,
            title: entry.title,
            entry_type: entry.entry_type,
            tags,
            description: entry.description.unwrap_or_default(),
            source_type: Some(entry.source_type),
            source_domain: entry.source_domain,
            source_path: Some(entry.source_path),
            created_at: Some(entry.created_at),
            updated_at: Some(entry.updated_at),
        }
    }
}

/// Search result from the chunks table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// Chunk ID
    pub chunk_id: String,
    /// Parent entry ID
    pub entry_id: String,
    /// Page ID for RAG context retrieval
    pub page_id: Option<String>,
    /// Entry title
    pub entry_title: String,
    /// Chunk text content
    pub text: String,
    /// Relevance score (0-1, higher is better)
    pub score: f32,
    /// Entry type (document, article, pdf, etc.)
    pub entry_type: String,
    /// Tags
    pub tags: Vec<String>,
    /// Chunk type (summary, content, code, header)
    pub chunk_type: Option<String>,
    /// Source domain (e.g., "docs.example.com")
    pub source_domain: Option<String>,
}

/// Filters for search queries.
#[derive(Debug, Clone, Default)]
pub struct SearchFilters {
    /// Filter by entry type (e.g., "document", "article", "pdf")
    pub entry_type: Option<String>,
    /// Filter by chunk type (e.g., "summary", "content")
    pub chunk_type: Option<String>,
    /// Filter by tag
    pub tag: Option<String>,
    /// Filter by source domain (e.g., "docs.example.com")
    pub source_domain: Option<String>,
}

impl SearchFilters {
    /// Creates a new empty filter.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the entry type filter.
    pub fn with_entry_type(mut self, entry_type: &str) -> Self {
        self.entry_type = Some(entry_type.to_string());
        self
    }

    /// Sets the chunk type filter.
    pub fn with_chunk_type(mut self, chunk_type: &str) -> Self {
        self.chunk_type = Some(chunk_type.to_string());
        self
    }

    /// Sets the tag filter.
    pub fn with_tag(mut self, tag: &str) -> Self {
        self.tag = Some(tag.to_string());
        self
    }

    /// Sets the source domain filter.
    pub fn with_source_domain(mut self, domain: &str) -> Self {
        self.source_domain = Some(domain.to_string());
        self
    }

    /// Check if no filters are set.
    pub fn is_empty(&self) -> bool {
        self.entry_type.is_none()
            && self.chunk_type.is_none()
            && self.tag.is_none()
            && self.source_domain.is_none()
    }
}
