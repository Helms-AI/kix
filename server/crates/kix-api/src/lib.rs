//! EIP API - REST API for the EIP Knowledge System dashboard.
//!
//! This crate provides REST endpoints for the web dashboard to interact
//! with the EIP store, including real-time indexing with SSE progress updates.

pub mod error;
pub mod routes;
pub mod indexing_routes;
pub mod admin;

pub use error::ApiError;
pub use routes::{create_router, AppState};
pub use indexing_routes::{create_indexing_router, IndexingState};
pub use admin::admin_routes;
