//! # kix-projects
//!
//! Local-first project management system for Kix.
//!
//! This crate provides:
//! - Project and work item management
//! - Kanban board with epic lanes
//! - AI-assisted project planning with knowledge base context
//! - Project-scoped search across work items and knowledge
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                      kix-projects                                │
//! ├─────────────────────────────────────────────────────────────────┤
//! │  project.rs    → Project data model and configuration           │
//! │  issue.rs      → Work item data model and CRUD                  │
//! │  knowledge.rs  → Project-entry linking                          │
//! │  planning.rs   → AI planning data structures                    │
//! │  templates.rs  → Board templates                                │
//! │  error.rs      → Error types                                    │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Key Features
//!
//! - **Projects**: Containers for work items organized by epics
//! - **Work Items**: Hierarchical items (epic → story → task → subtask)
//! - **Kanban Board**: Epic lanes with drag-drop support
//! - **AI Planning**: Use knowledge base to help plan tasks
//! - **Real-time Events**: MCP → UI events for live updates
//!
//! ## Usage
//!
//! ```rust,ignore
//! use kix_projects::{Project, Issue};
//!
//! // Create a new project
//! let project = Project::new("My App".to_string());
//!
//! // Create a work item
//! let issue = Issue::new(
//!     project.id.clone(),
//!     1,
//!     "Implement login".to_string(),
//! );
//! ```

pub mod error;
pub mod events;
pub mod issue;
pub mod knowledge;
pub mod planning;
pub mod project;
pub mod search;
pub mod templates;

// Re-export main types for convenience
pub use error::ProjectError;
pub use issue::{
    CreateIssueParams, Issue, IssueFilters, IssueState, UpdateIssueParams,
};
pub use knowledge::{KnowledgeReference, LinkEntryParams, ProjectEntry};
pub use planning::{
    BreakdownTaskParams, GetProjectContextParams, IssueContext, PlanProjectParams, PlanResult,
    PlanTemplate, PlannedTask, ProjectContext, ProjectContextStats, SuggestTasksParams,
    TaskBreakdown, TaskSuggestion, TaskSuggestions,
};
pub use project::{BoardColumn, IssueType, Project, ProjectStats};
pub use templates::{CustomFieldDefinition, CustomFieldType, ProjectTemplate, ProjectView, TemplateConfig};
pub use search::{
    IssueSearchResult, KnowledgeSearchResult, ProjectSearchParams, ProjectSearchResult,
    SearchResultMerger, SearchType, calculate_text_score, generate_excerpt,
};

// Event bus re-exports
pub use events::{
    ProjectEvent, ProjectEventBus, ProjectEventType, SharedEventBus, create_event_bus,
};

/// Result type for project operations.
pub type Result<T> = std::result::Result<T, ProjectError>;
