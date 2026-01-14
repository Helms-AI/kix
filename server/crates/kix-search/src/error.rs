//! Error types for the search module.

use thiserror::Error;

/// Errors that can occur in search operations.
#[derive(Error, Debug)]
pub enum SearchError {
    /// Index creation or opening error.
    #[error("Index error: {0}")]
    Index(String),

    /// Query parsing error.
    #[error("Query error: {0}")]
    Query(String),

    /// Document indexing error.
    #[error("Indexing error: {0}")]
    Indexing(String),

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Tantivy error.
    #[error("Tantivy error: {0}")]
    Tantivy(#[from] tantivy::TantivyError),

    /// Schema field not found.
    #[error("Schema field not found: {0}")]
    SchemaField(String),

    /// Serialization error.
    #[error("Serialization error: {0}")]
    Serialization(String),
}

impl From<tantivy::query::QueryParserError> for SearchError {
    fn from(err: tantivy::query::QueryParserError) -> Self {
        SearchError::Query(err.to_string())
    }
}

/// Result type for search operations.
pub type SearchResult<T> = std::result::Result<T, SearchError>;
