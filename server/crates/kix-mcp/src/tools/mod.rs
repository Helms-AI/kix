//! MCP tool parameter and response types.
//!
//! This module re-exports all parameter and response types used by the RAG tools.
//!
//! ## Tool Categories
//!
//! **Retrieval Tools:**
//! - `search` - [`SearchParams`], [`SearchResponse`]
//! - `get_context` - [`GetContextParams`], [`PageContext`]
//! - `get_document` - [`GetDocumentParams`], [`Document`]
//!
//! **Indexing Tools:**
//! - `index` - [`IndexParams`], [`IndexResult`]
//! - `index_async` - [`IndexAsyncParams`], [`JobCreated`]
//! - `job_status` - [`JobStatusParams`], [`JobStatusResponse`]
//! - `delete` - [`DeleteParams`], [`DeleteResult`]
//!
//! **Status Tool:**
//! - `status` - [`StatusParams`], [`IndexStatus`]

pub use crate::server::{
    // Search/Retrieval types
    SearchParams,
    SearchMode,
    QueryFilters,
    SearchResultItem,
    SearchResponse,
    GetContextParams,
    PageContext,
    GetDocumentParams,
    ChunkInfo,
    Document,

    // Indexing types
    ContentSource,
    IndexParams,
    IndexResult,
    UrlSource,
    AsyncSource,
    IndexAsyncParams,
    JobCreated,
    JobStatusParams,
    JobProgress,
    JobResult,
    JobStatusResponse,
    DeleteFilter,
    DeleteParams,
    DeleteResult,

    // Status types
    StatusParams,
    StatusBreakdown,
    IndexStatus,
};
