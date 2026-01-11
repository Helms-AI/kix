//! Knowledge Indexer Store - Vector storage using LanceDB.
//!
//! This crate provides vector storage, indexing, and search operations
//! using LanceDB as the underlying database.

pub mod error;
pub mod indexer;
pub mod schema;
pub mod search;
pub mod store;

pub use error::StoreError;
pub use store::{KixStore, EipStore, DEFAULT_EMBEDDING_DIM, get_embedding_dim};
pub use search::{SearchResult, SearchFilters, EntrySummary, PatternSummary, EntryChunk};
