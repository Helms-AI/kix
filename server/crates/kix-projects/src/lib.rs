//! # kix-projects
//!
//! AI-powered project management system for Kix with GitHub integration.
//!
//! This crate provides:
//! - Project and issue management with GitHub synchronization
//! - GitHub Projects V2 integration (Kanban boards, bug tracking, etc.)
//! - AI-assisted project planning with knowledge base context
//! - Project-scoped search across issues and knowledge
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                      kix-projects                                │
//! ├─────────────────────────────────────────────────────────────────┤
//! │  project.rs    → Project data model and configuration           │
//! │  issue.rs      → Issue data model and CRUD                      │
//! │  knowledge.rs  → Project-entry linking                          │
//! │  templates.rs  → GitHub Project V2 templates                    │
//! │  planning.rs   → AI planning data structures                    │
//! │  github/       → GitHub REST + GraphQL integration              │
//! │  error.rs      → Error types                                    │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Key Features
//!
//! - **Projects**: Bounded containers for knowledge and issues
//! - **GitHub Required**: Projects must connect to a GitHub repo
//! - **Projects V2**: Create Kanban, Bug Tracking, Sprint boards
//! - **AI Planning**: Use knowledge base to help plan tasks
//! - **Real-time Sync**: MCP → UI events for live updates
//!
//! ## Usage
//!
//! ```rust,ignore
//! use kix_projects::{Project, Issue, ProjectTemplate};
//!
//! // Create a new project
//! let project = Project::new(
//!     "My App".to_string(),
//!     "myorg".to_string(),
//!     "myapp".to_string(),
//! );
//!
//! // Create an issue
//! let issue = Issue::new(
//!     project.id.clone(),
//!     1,
//!     "Implement login".to_string(),
//! );
//! ```

pub mod error;
pub mod events;
pub mod github;
pub mod issue;
pub mod knowledge;
pub mod planning;
pub mod project;
pub mod search;
pub mod sync_state;
pub mod templates;

// Re-export main types for convenience
pub use error::ProjectError;
pub use github::models::{
    GitHubFieldOption, GitHubFieldValue, GitHubGraphQLError, GitHubIssue, GitHubLabel,
    GitHubProjectField, GitHubProjectItem, GitHubProjectItemContent, GitHubProjectV2,
    GitHubRepository, GitHubUser,
};
pub use issue::{
    CreateIssueParams, GitHubIssueData, Issue, IssueFilters, IssueSource, UpdateIssueParams,
};
pub use knowledge::{KnowledgeReference, LinkEntryParams, ProjectEntry};
pub use planning::{
    BreakdownTaskParams, GetProjectContextParams, IssueContext, PlanProjectParams, PlanResult,
    PlanTemplate, PlannedTask, ProjectContext, ProjectContextStats, SuggestTasksParams,
    TaskBreakdown, TaskSuggestion, TaskSuggestions,
};
pub use project::{
    CustomFieldConfig, GitHubConfig, GitHubProjectV2Config, GitHubSyncConfig, IssueState, Project,
    ProjectFieldType, ProjectStats, StatusOption,
};
pub use templates::{CustomFieldDefinition, CustomFieldType, ProjectTemplate, ProjectView, TemplateConfig};
pub use search::{
    IssueSearchResult, KnowledgeSearchResult, ProjectSearchParams, ProjectSearchResult,
    SearchResultMerger, SearchType, calculate_text_score, generate_excerpt,
};
pub use sync_state::{
    ConflictDetector, ConflictType, ContentHasher, IssueSyncState, MergeResult,
    MergeStrategy, SmartMerger, SyncStatus,
    SyncDirection as SyncStateDirection, SyncResult as SyncStateResult,
};

// GitHub integration re-exports
pub use github::{
    EncryptedToken, GitHubGraphQLClient, GitHubRestClient, GitHubSyncService,
    GitHubTokenManager, InMemoryTokenStorage, IssueInfo, ProjectTokenType, ProjectV2Service,
    SyncConfig, SyncDirection, SyncResult, TokenScope, TokenService, TokenStorage,
    TokenStoreResult, get_token_with_fallback,
};

// Event bus re-exports
pub use events::{
    ProjectEvent, ProjectEventBus, ProjectEventType, SharedEventBus, create_event_bus,
};

/// Result type for project operations.
pub type Result<T> = std::result::Result<T, ProjectError>;
