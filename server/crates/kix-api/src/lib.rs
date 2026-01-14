//! EIP API - REST API for the EIP Knowledge System dashboard.
//!
//! This crate provides REST endpoints for the web dashboard to interact
//! with the EIP store, including real-time indexing with SSE progress updates.

pub mod admin;
pub mod error;
pub mod indexing_routes;
pub mod project_routes;
pub mod routes;

pub use admin::admin_routes;
pub use error::ApiError;
pub use indexing_routes::{create_indexing_router, IndexingState};
pub use project_routes::{create_project_router, ProjectState};
pub use routes::{create_router, AppState};
