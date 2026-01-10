//! Error types for embedding generation.

use thiserror::Error;

/// Errors that can occur during embedding generation.
#[derive(Error, Debug)]
pub enum EmbeddingError {
    /// Failed to initialize the embedding model.
    #[error("Failed to initialize embedding model: {0}")]
    ModelInit(String),

    /// Failed to generate embeddings.
    #[error("Failed to generate embeddings: {0}")]
    Generation(String),

    /// Batch size exceeded limit.
    #[error("Batch size {0} exceeds maximum {1}")]
    BatchTooLarge(usize, usize),

    /// Empty input provided.
    #[error("Cannot generate embeddings for empty input")]
    EmptyInput,

    /// No embedding backend available.
    #[error("No embedding backend available: {0}")]
    NoBackendAvailable(String),

    /// Backend initialization failed.
    #[error("Backend initialization failed: {0}")]
    BackendInit(String),
}
