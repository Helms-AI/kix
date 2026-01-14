//! KIX Shared Services Layer
//!
//! This crate provides the shared business logic layer for KIX, ensuring
//! consistency between the REST API (kix-api) and MCP server (kix-mcp).
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    Unified Service Architecture                  │
//! ├─────────────────────────────────────────────────────────────────┤
//! │                                                                   │
//! │  User ──────► REST API ──────┐                                   │
//! │              (kix-api)        │                                   │
//! │                               ├──► kix-services ◄── Store Layer  │
//! │  MCP Client ► MCP Server ────┘    (shared logic)                 │
//! │              (kix-mcp)                                            │
//! │                                                                   │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Design Principles
//!
//! 1. **Every MCP tool has a corresponding REST API endpoint**
//!    - Both call the same service functions
//!    - Handlers are thin wrappers that transform inputs/outputs
//!
//! 2. **Services are stateless**
//!    - Stores and event buses are passed as parameters
//!    - Enables easy testing and flexibility
//!
//! 3. **Events emit from services, not handlers**
//!    - Services receive an optional `SharedEventBus`
//!    - Both API and MCP trigger the same events
//!
//! # Modules
//!
//! - [`error`] - Unified error types that map to HTTP and MCP errors
//! - [`retrieval`] - Search, document retrieval, and RAG context (TODO)
//! - [`projects`] - Project CRUD operations (TODO)
//! - [`issues`] - Issue CRUD with GitHub sync (TODO)
//! - [`github`] - Token management and GitHub API (TODO)
//! - [`indexing`] - URL/file indexing and job management (TODO)
//! - [`knowledge`] - Entry linking and project-scoped search (TODO)

pub mod error;
pub mod github;
pub mod indexing;
pub mod issues;
pub mod knowledge;
pub mod projects;
pub mod retrieval;

// Re-export commonly used types
pub use error::{ServiceError, ServiceResult};

// Re-export retrieval types and functions
pub use retrieval::{
    find_related, get_document, get_page_context, search_knowledge,
    ChunkInfo, Document, PageContext, Pagination, QueryFilters, RelatedEntry,
    SearchMode, SearchResultItem, SearchResults,
};

// Re-export project types and functions
pub use projects::{
    delete_project, get_project, list_projects, update_project, DeleteProjectOptions,
    DeleteProjectResult, ProjectDetail, ProjectFilters, ProjectList, ProjectStats,
    ProjectSummary, ProjectUpdates,
};

// Re-export issue types and functions
pub use issues::{
    create_issue, delete_issue, get_issue, list_issues, update_issue,
    CreateIssueData, CreateIssueResult, DeleteIssueOptions, DeleteIssueResult,
    IssueFilters, IssueList, IssueOptions, IssueSummary, IssueUpdates,
};

// Re-export GitHub types and functions
pub use github::{
    get_authenticated_user, get_token_for_project, get_token_status, list_organizations,
    search_repositories, set_token, verify_repo_access, OrgItem, RepoItem, RepoListResult,
    RepoSearchQuery, SetTokenResult, TokenScope, TokenStatus, VerifyAccessResult,
};

// Re-export external types used by GitHub services
pub use kix_projects::github::models::AuthenticatedUser;

// Re-export indexing types and functions
pub use indexing::{
    is_terminal_state, parse_job_id, DeleteResult, DeleteTarget, FileIndexConfig, FileInput,
    IndexStats, JobList, JobStatus, SyncIndexOptions, SyncIndexResult, UrlIndexConfig,
    // Code extraction types (Phase 5)
    list_code_patterns, list_supported_languages,
    CodeBlockResponse, CodeExtractionStats, LanguageInfo, LanguageStats,
    PatternInfo, PatternStats, RejectionReason, ValidationSummary,
};

// Re-export knowledge linking types and functions
pub use knowledge::{
    link_entry, list_linked_entries, unlink_entry, EntrySearchHit, IssueSearchHit,
    LinkedEntry, LinkedEntryFilters, LinkedEntryList, LinkResult, ProjectSearchOptions,
    ProjectSearchResult,
};
