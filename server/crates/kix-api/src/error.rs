//! Error types for the API.

use thiserror::Error;

/// Errors that can occur in API operations.
#[derive(Error, Debug)]
pub enum ApiError {
    /// Store error.
    #[error("Store error: {0}")]
    Store(String),

    /// Not found.
    #[error("Not found: {0}")]
    NotFound(String),

    /// Bad request.
    #[error("Bad request: {0}")]
    BadRequest(String),
}
