//! MCP tool implementations.
//!
//! All tools are implemented in the `server` module using the `#[tool]` macro.
//! This module re-exports parameter types for external use.

pub use crate::server::{
    // Search tools
    ComparePatternsParams, ExplainPatternParams, FindRelatedParams, GetCategoryOverviewParams,
    GetPatternParams, ListPatternsParams, PatternSequenceParams, SearchByProblemParams,
    SearchByTechnologyParams, SearchPatternsParams, SuggestArchitectureParams,
    // Indexing tools
    BatchDocument, ContentSource, DeleteDocumentParams, DocumentSchema, GenericSchema,
    GetIndexStatusParams, IndexBatchParams, IndexDocumentParams, PatternSchema,
};
