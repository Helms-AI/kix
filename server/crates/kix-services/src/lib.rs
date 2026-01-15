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
//! - [`projects`] - Project CRUD operations
//! - [`work_items`] - Work item CRUD operations
//! - [`indexing`] - URL/file indexing and job management
//! - [`knowledge`] - Entry linking and project-scoped search

pub mod error;
pub mod explorer;
pub mod indexing;
pub mod work_items;
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
    create_project, delete_project, get_project, list_projects, update_project,
    CreateProjectData, CreateProjectResult, DeleteProjectOptions, DeleteProjectResult,
    ProjectDetail, ProjectFilters, ProjectList, ProjectStats, ProjectSummary, ProjectUpdates,
};

// Re-export work item types and functions
pub use work_items::{
    create_work_item, delete_work_item, get_board, get_child_work_items, get_column_counts,
    get_work_item, list_work_items, move_card, update_work_item,
    BoardColumn, BoardSwimlane, BoardView, ColumnCounts, CreateWorkItemData, CreateWorkItemResult,
    WorkItemFilters, WorkItemList, WorkItemSummary, WorkItemUpdates,
};

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
    link_entry, list_linked_entries, search_project, unlink_entry, EntrySearchHit, WorkItemSearchHit,
    LinkedEntry, LinkedEntryFilters, LinkedEntryList, LinkResult, ProjectSearchOptions,
    ProjectSearchResult,
};

// Re-export data explorer types and functions
pub use explorer::{
    discover_databases, execute_sqlite_query, get_query_templates, get_sqlite_schema,
    get_table_data, ColumnInfo, DataModifyRequest, DataModifyResult, DataOperation,
    DatabaseInfo, DatabaseStatus, DatabaseType, ForeignKeyInfo, IndexInfo,
    QueryHistoryEntry, QueryRequest, QueryResult, QueryTemplate, QueryType,
    TableSchema, TemplateParameter,
};
