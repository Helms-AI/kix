//! GitHub integration module for REST and GraphQL APIs.
//!
//! This module provides:
//! - REST API client for issues (`rest_client`)
//! - GraphQL client for Projects V2 (`graphql_client`)
//! - Projects V2 orchestration service (`project_v2_service`)
//! - Issue sync service (`sync`)
//! - Secure token storage (`tokens`)
//! - Unified token service (`token_service`)
//! - Data models for GitHub API responses (`models`)

pub mod bidirectional_sync;
pub mod graphql_client;
pub mod models;
pub mod project_v2_service;
pub mod rest_client;
pub mod sync;
pub mod token_service;
pub mod tokens;

// Re-export main types
pub use graphql_client::GitHubGraphQLClient;
pub use models::*;
pub use project_v2_service::ProjectV2Service;
pub use rest_client::GitHubRestClient;
pub use sync::{GitHubSyncService, IssueInfo, SyncConfig, SyncDirection, SyncResult};
pub use bidirectional_sync::{BidirectionalSyncService, EnhancedSyncResult, ChangeDetail, ChangeAction};
pub use token_service::{ProjectTokenType, TokenService, TokenStoreResult};
pub use tokens::{
    get_token_with_fallback, EncryptedToken, GitHubTokenManager, InMemoryTokenStorage,
    TokenScope, TokenStorage,
};
