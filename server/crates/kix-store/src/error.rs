//! Error types for the store module.

use thiserror::Error;

/// Errors that can occur in store operations.
#[derive(Error, Debug)]
pub enum StoreError {
    /// Database connection error.
    #[error("Database error: {0}")]
    Database(String),

    /// Record not found.
    #[error("Record not found: {0}")]
    NotFound(String),

    /// Query execution error.
    #[error("Query error: {0}")]
    Query(String),

    /// Index creation error.
    #[error("Index error: {0}")]
    Index(String),

    /// Serialization error.
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Internal error (e.g., spawn_blocking failures).
    #[error("Internal error: {0}")]
    Internal(String),
}
