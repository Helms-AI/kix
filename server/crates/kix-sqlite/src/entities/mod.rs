//! SeaORM entity definitions for KIX SQLite database.
//!
//! This module contains all the entity definitions that map to database tables.
//! Entities use SeaORM's derive macros for type-safe database operations.
//!
//! ## Conversions
//!
//! The `conversions` submodule provides `From` implementations to convert between
//! SeaORM `Model` types and the application's `Record` types for backward compatibility.

pub mod conversions;
pub mod entry;
pub mod github_token;
pub mod issue;
pub mod job;
pub mod job_item;
pub mod page;
pub mod project;
pub mod project_entry;
pub mod sync_conflict;
pub mod sync_history;
pub mod sync_state;

pub use entry::Entity as Entry;
pub use github_token::Entity as GithubToken;
pub use issue::Entity as Issue;
pub use job::Entity as Job;
pub use job_item::Entity as JobItem;
pub use page::Entity as Page;
pub use project::Entity as Project;
pub use project_entry::Entity as ProjectEntry;
pub use sync_conflict::Entity as SyncConflict;
pub use sync_history::Entity as SyncHistory;
pub use sync_state::Entity as SyncState;
