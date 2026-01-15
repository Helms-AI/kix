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
pub mod job;
pub mod job_item;
pub mod page;
pub mod project;
pub mod project_entry;
pub mod work_item;

pub use entry::Entity as Entry;
pub use job::Entity as Job;
pub use job_item::Entity as JobItem;
pub use page::Entity as Page;
pub use project::Entity as Project;
pub use project_entry::Entity as ProjectEntry;
pub use work_item::Entity as WorkItem;
