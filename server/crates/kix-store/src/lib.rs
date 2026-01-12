//! Knowledge Indexer Store - Vector storage using LanceDB.
//!
//! This crate provides vector storage, indexing, and search operations
//! using LanceDB as the underlying database.
//!
//! ## Two-Layer Storage
//!
//! This store implements a two-layer storage pattern for RAG context retrieval:
//! - **Pages table**: Full page content for RAG context retrieval
//! - **Chunks table**: Smaller pieces with embeddings for vector search
//!
//! Chunks reference their parent page via `page_id` for context retrieval.
//!
//! ## Job History Storage
//!
//! Job history is stored separately in `jobs.lance`:
//! - **Jobs table**: Completed job metadata and aggregated statistics
//! - **Job items table**: Per-page/file details for detailed history

pub mod error;
pub mod indexer;
pub mod jobs;
pub mod pages;
pub mod schema;
pub mod search;
pub mod store;

pub use error::StoreError;
pub use jobs::{JobItemRecord, JobRecord, JobStats, JobStore};
pub use pages::{PageRecord, PageStore};
pub use schema::{job_item_schema, job_schema, page_schema};
pub use search::{EntryChunk, EntrySummary, PatternSummary, SearchFilters, SearchResult};
pub use store::{get_embedding_dim, EipStore, KixStore, DEFAULT_EMBEDDING_DIM};
