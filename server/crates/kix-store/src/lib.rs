//! Knowledge Indexer Store - Unified SQLite storage.
//!
//! This crate provides vector storage, indexing, and search operations
//! using SQLite for structured data and sqlite-vec for vector embeddings.
//!
//! ## Architecture
//!
//! - **kix.db** (via kix-sqlite): Structured data (entries, pages, projects, issues, etc.)
//! - **vectors.db** (via kix-vectors + sqlite-vec): Vector embeddings for semantic search
//!
//! ## Two-Layer Storage
//!
//! This store implements a two-layer storage pattern for RAG context retrieval:
//! - **Pages table**: Full page content for RAG context retrieval
//! - **Chunks table**: Smaller pieces with embeddings for vector search
//!
//! Chunks reference their parent page via `page_id` for context retrieval.

pub mod error;
pub mod job_store;
pub mod projects;
pub mod search;
pub mod store;
pub mod tokens;

pub use error::StoreError;
pub use job_store::{JobAggregateStats, JobStats, JobStore};
pub use projects::ProjectStore;
pub use search::{EntryChunk, PatternSummary, SearchFilters, SearchResult};
pub use store::{get_embedding_dim, KixStore, DEFAULT_EMBEDDING_DIM};
pub use tokens::{EncryptedTokenData, TokenStore};

// Re-export types and SqliteStore from kix-sqlite for convenience
pub use kix_sqlite::{
    EntryRecord, IssueRecord, JobRecord, JobItemRecord, PageRecord, ProjectEntryRecord, ProjectRecord,
    SqliteStore, TokenRecord,
};

// Re-export types from kix-vectors
pub use kix_vectors::{SearchFilter as VectorSearchFilter, VectorSearchResult, VectorStore};
