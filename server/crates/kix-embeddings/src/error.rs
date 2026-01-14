//! Error types for embedding generation.

use thiserror::Error;

/// Errors that can occur during embedding generation.
#[derive(Error, Debug)]
pub enum EmbeddingError {
    /// Ollama API error
    #[error("Ollama error: {0}")]
    OllamaError(String),

    /// Empty response from Ollama
    #[error("Empty response from Ollama")]
    EmptyResponse,

    /// Dimension mismatch between expected and actual
    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    /// Semaphore acquisition error
    #[error("Semaphore error")]
    SemaphoreError,

    /// Connection error to Ollama server
    #[error("Connection error: {0}")]
    ConnectionError(String),

    /// Empty input provided
    #[error("Cannot generate embeddings for empty input")]
    EmptyInput,

    /// Failed to initialize embedding model
    #[error("Failed to initialize embedding model: {0}")]
    ModelInit(String),

    /// Failed to generate embeddings
    #[error("Failed to generate embeddings: {0}")]
    Generation(String),
}
