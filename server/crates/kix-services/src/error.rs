//! Error types for the shared service layer.
//!
//! ServiceError provides a unified error type that can be converted to both
//! HTTP StatusCode (for REST API) and MCP ErrorData (for MCP tools).

use axum::http::StatusCode;
use kix_store::StoreError;
use rmcp::ErrorData as McpErrorData;
use thiserror::Error;

/// Errors that can occur in service operations.
///
/// This error type provides a common abstraction between the REST API
/// and MCP server, allowing both to use the same business logic.
#[derive(Error, Debug)]
pub enum ServiceError {
    /// Resource not found.
    #[error("Not found: {0}")]
    NotFound(String),

    /// Invalid input or parameters.
    #[error("Validation error: {0}")]
    Validation(String),

    /// Store/database error.
    #[error("Store error: {0}")]
    Store(#[from] StoreError),

    /// Authentication/authorization error.
    #[error("Auth error: {0}")]
    Auth(String),

    /// External service error (e.g., crawling, embedding).
    #[error("External service error: {0}")]
    External(String),

    /// Internal server error.
    #[error("Internal error: {0}")]
    Internal(String),

    /// Configuration error.
    #[error("Configuration error: {0}")]
    Config(String),

    /// Conflict error (e.g., duplicate resource).
    #[error("Conflict: {0}")]
    Conflict(String),
}

/// Result type alias for service operations.
pub type ServiceResult<T> = Result<T, ServiceError>;

// Conversion to HTTP StatusCode for REST API
impl From<&ServiceError> for StatusCode {
    fn from(error: &ServiceError) -> Self {
        match error {
            ServiceError::NotFound(_) => StatusCode::NOT_FOUND,
            ServiceError::Validation(_) => StatusCode::BAD_REQUEST,
            ServiceError::Store(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ServiceError::Auth(_) => StatusCode::UNAUTHORIZED,
            ServiceError::External(_) => StatusCode::BAD_GATEWAY,
            ServiceError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ServiceError::Config(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ServiceError::Conflict(_) => StatusCode::CONFLICT,
        }
    }
}

impl From<ServiceError> for StatusCode {
    fn from(error: ServiceError) -> Self {
        StatusCode::from(&error)
    }
}

// Conversion to MCP ErrorData for MCP tools
impl From<ServiceError> for McpErrorData {
    fn from(error: ServiceError) -> Self {
        match &error {
            ServiceError::NotFound(msg) => McpErrorData::invalid_params(msg.clone(), None),
            ServiceError::Validation(msg) => McpErrorData::invalid_params(msg.clone(), None),
            ServiceError::Auth(msg) => {
                McpErrorData::internal_error(format!("Authentication error: {}", msg), None)
            }
            _ => McpErrorData::internal_error(error.to_string(), None),
        }
    }
}

// Convenience constructors
impl ServiceError {
    /// Create a not found error for a specific resource type.
    pub fn not_found(resource_type: &str, identifier: &str) -> Self {
        Self::NotFound(format!("{} '{}' not found", resource_type, identifier))
    }

    /// Create a validation error for a specific field.
    pub fn invalid_field(field: &str, reason: &str) -> Self {
        Self::Validation(format!("Invalid {}: {}", field, reason))
    }

    /// Create an internal error.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }

    /// Create a conflict error.
    pub fn conflict(message: impl Into<String>) -> Self {
        Self::Conflict(message.into())
    }

    /// Create a configuration error.
    pub fn config(message: impl Into<String>) -> Self {
        Self::Config(message.into())
    }

    /// Create a bad request error (alias for validation error).
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::Validation(message.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_not_found_to_status_code() {
        let err = ServiceError::not_found("Project", "test-123");
        assert_eq!(StatusCode::from(&err), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_validation_to_status_code() {
        let err = ServiceError::invalid_field("name", "cannot be empty");
        assert_eq!(StatusCode::from(&err), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_conflict_to_status_code() {
        let err = ServiceError::conflict("Resource already exists");
        assert_eq!(StatusCode::from(&err), StatusCode::CONFLICT);
    }

    #[test]
    fn test_not_found_to_mcp_error() {
        let err = ServiceError::not_found("Issue", "42");
        let mcp_err: McpErrorData = err.into();
        // MCP uses invalid_params for not found (client provided bad identifier)
        assert!(mcp_err.message.contains("not found"));
    }

    #[test]
    fn test_internal_to_mcp_error() {
        let err = ServiceError::internal("Database connection failed");
        let mcp_err: McpErrorData = err.into();
        assert!(mcp_err.message.contains("Database connection failed"));
    }
}
