//! Error types for the MCP server.

use thiserror::Error;

/// Errors that can occur in MCP server operations.
#[derive(Error, Debug)]
pub enum McpError {
    /// Store error.
    #[error("Store error: {0}")]
    Store(String),

    /// Embedding error.
    #[error("Embedding error: {0}")]
    Embedding(String),

    /// Invalid request.
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    /// Pattern not found.
    #[error("Pattern not found: {0}")]
    NotFound(String),
}
