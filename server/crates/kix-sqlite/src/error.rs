//! Error types for kix-sqlite crate

use thiserror::Error;

/// Errors that can occur in SQLite operations
#[derive(Error, Debug)]
pub enum SqliteError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),

    #[error("Entry not found: {0}")]
    EntryNotFound(String),

    #[error("Page not found: {0}")]
    PageNotFound(String),

    #[error("Project not found: {0}")]
    ProjectNotFound(String),

    #[error("Issue not found: {0}")]
    IssueNotFound(String),

    #[error("Token not found for scope: {0}")]
    TokenNotFound(String),

    #[error("Job not found: {0}")]
    JobNotFound(String),

    #[error("Duplicate entry: {0}")]
    DuplicateEntry(String),

    #[error("Invalid JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid data: {0}")]
    InvalidData(String),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("SeaORM error: {0}")]
    SeaOrm(#[from] sea_orm::DbErr),
}

/// Result type for SQLite operations
pub type Result<T> = std::result::Result<T, SqliteError>;
