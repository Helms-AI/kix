//! Conversion utilities between SeaORM entities and Record types.
//!
//! This module provides bidirectional conversion between:
//! - SeaORM `Model` types (used internally for ORM operations)
//! - `Record` types (used by the rest of the application)
//!
//! Since both types map to the same database schema, conversions are 1:1.

use super::{entry, github_token, issue, job, job_item, page, project, project_entry};

// ============================================================================
// Entry conversions
// ============================================================================

impl From<entry::Model> for crate::entries::EntryRecord {
    fn from(model: entry::Model) -> Self {
        Self {
            id: model.id,
            title: model.title,
            description: model.description,
            content: model.content,
            tags: model.tags,
            collection_ids: model.collection_ids,
            entry_type: model.entry_type,
            source_type: model.source_type,
            source_path: model.source_path,
            source_domain: model.source_domain,
            slug: model.slug,
            source_hash: model.source_hash,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

impl From<&crate::entries::EntryRecord> for entry::ActiveModel {
    fn from(record: &crate::entries::EntryRecord) -> Self {
        use sea_orm::ActiveValue::Set;
        Self {
            id: Set(record.id.clone()),
            title: Set(record.title.clone()),
            description: Set(record.description.clone()),
            content: Set(record.content.clone()),
            tags: Set(record.tags.clone()),
            collection_ids: Set(record.collection_ids.clone()),
            entry_type: Set(record.entry_type.clone()),
            source_type: Set(record.source_type.clone()),
            source_path: Set(record.source_path.clone()),
            source_domain: Set(record.source_domain.clone()),
            slug: Set(record.slug.clone()),
            source_hash: Set(record.source_hash.clone()),
            created_at: Set(record.created_at.clone()),
            updated_at: Set(record.updated_at.clone()),
        }
    }
}

// ============================================================================
// Page conversions
// ============================================================================

impl From<page::Model> for crate::pages::PageRecord {
    fn from(model: page::Model) -> Self {
        Self {
            page_id: model.page_id,
            source_id: model.source_id,
            url: model.url,
            title: model.title,
            full_content: model.full_content,
            content_hash: model.content_hash,
            content_length: model.content_length,
            code_block_count: model.code_block_count,
            metadata: model.metadata,
            crawl_time_ms: model.crawl_time_ms,
            created_at: model.created_at,
        }
    }
}

impl From<&crate::pages::PageRecord> for page::ActiveModel {
    fn from(record: &crate::pages::PageRecord) -> Self {
        use sea_orm::ActiveValue::Set;
        Self {
            page_id: Set(record.page_id.clone()),
            source_id: Set(record.source_id.clone()),
            url: Set(record.url.clone()),
            title: Set(record.title.clone()),
            full_content: Set(record.full_content.clone()),
            content_hash: Set(record.content_hash.clone()),
            content_length: Set(record.content_length),
            code_block_count: Set(record.code_block_count),
            metadata: Set(record.metadata.clone()),
            crawl_time_ms: Set(record.crawl_time_ms),
            created_at: Set(record.created_at.clone()),
        }
    }
}

// ============================================================================
// Project conversions
// ============================================================================

impl From<project::Model> for crate::projects::ProjectRecord {
    fn from(model: project::Model) -> Self {
        Self {
            id: model.id,
            name: model.name,
            slug: model.slug,
            description: model.description,
            color: model.color,
            github_config: model.github_config,
            archived: model.archived,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

impl From<&crate::projects::ProjectRecord> for project::ActiveModel {
    fn from(record: &crate::projects::ProjectRecord) -> Self {
        use sea_orm::ActiveValue::Set;
        Self {
            id: Set(record.id.clone()),
            name: Set(record.name.clone()),
            slug: Set(record.slug.clone()),
            description: Set(record.description.clone()),
            color: Set(record.color.clone()),
            github_config: Set(record.github_config.clone()),
            archived: Set(record.archived),
            created_at: Set(record.created_at.clone()),
            updated_at: Set(record.updated_at.clone()),
        }
    }
}

// ============================================================================
// Issue conversions
// ============================================================================

impl From<issue::Model> for crate::issues::IssueRecord {
    fn from(model: issue::Model) -> Self {
        Self {
            id: model.id,
            project_id: model.project_id,
            number: model.number,
            title: model.title,
            body: model.body,
            state: model.state,
            labels: model.labels,
            assignees: model.assignees,
            priority: model.priority,
            github_number: model.github_number,
            github_node_id: model.github_node_id,
            github_url: model.github_url,
            github_project_item_id: model.github_project_item_id,
            source: model.source,
            created_at: model.created_at,
            updated_at: model.updated_at,
            closed_at: model.closed_at,
            synced_at: model.synced_at,
        }
    }
}

impl From<&crate::issues::IssueRecord> for issue::ActiveModel {
    fn from(record: &crate::issues::IssueRecord) -> Self {
        use sea_orm::ActiveValue::Set;
        Self {
            id: Set(record.id.clone()),
            project_id: Set(record.project_id.clone()),
            number: Set(record.number),
            title: Set(record.title.clone()),
            body: Set(record.body.clone()),
            state: Set(record.state.clone()),
            labels: Set(record.labels.clone()),
            assignees: Set(record.assignees.clone()),
            priority: Set(record.priority),
            github_number: Set(record.github_number),
            github_node_id: Set(record.github_node_id.clone()),
            github_url: Set(record.github_url.clone()),
            github_project_item_id: Set(record.github_project_item_id.clone()),
            source: Set(record.source.clone()),
            created_at: Set(record.created_at.clone()),
            updated_at: Set(record.updated_at.clone()),
            closed_at: Set(record.closed_at.clone()),
            synced_at: Set(record.synced_at.clone()),
        }
    }
}

// ============================================================================
// ProjectEntry (link) conversions
// ============================================================================

impl From<project_entry::Model> for crate::links::ProjectEntryRecord {
    fn from(model: project_entry::Model) -> Self {
        Self {
            id: model.id,
            project_id: model.project_id,
            entry_id: model.entry_id,
            relevance: model.relevance,
            notes: model.notes,
            linked_at: model.linked_at,
        }
    }
}

impl From<&crate::links::ProjectEntryRecord> for project_entry::ActiveModel {
    fn from(record: &crate::links::ProjectEntryRecord) -> Self {
        use sea_orm::ActiveValue::Set;
        Self {
            id: Set(record.id.clone()),
            project_id: Set(record.project_id.clone()),
            entry_id: Set(record.entry_id.clone()),
            relevance: Set(record.relevance),
            notes: Set(record.notes.clone()),
            linked_at: Set(record.linked_at.clone()),
        }
    }
}

// ============================================================================
// GitHub Token conversions
// ============================================================================

impl From<github_token::Model> for crate::tokens::TokenRecord {
    fn from(model: github_token::Model) -> Self {
        Self {
            scope: model.scope,
            encrypted_token: model.encrypted_token,
            token_suffix: model.token_suffix,
            token_format: model.token_format,
            verified_at: model.verified_at,
            verified_user: model.verified_user,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

impl From<&crate::tokens::TokenRecord> for github_token::ActiveModel {
    fn from(record: &crate::tokens::TokenRecord) -> Self {
        use sea_orm::ActiveValue::Set;
        Self {
            scope: Set(record.scope.clone()),
            encrypted_token: Set(record.encrypted_token.clone()),
            token_suffix: Set(record.token_suffix.clone()),
            token_format: Set(record.token_format.clone()),
            verified_at: Set(record.verified_at.clone()),
            verified_user: Set(record.verified_user.clone()),
            created_at: Set(record.created_at.clone()),
            updated_at: Set(record.updated_at.clone()),
        }
    }
}

// ============================================================================
// Job conversions
// ============================================================================

impl From<job::Model> for crate::jobs::JobRecord {
    fn from(model: job::Model) -> Self {
        Self {
            job_id: model.job_id,
            job_type: model.job_type,
            status: model.status,
            created_at: model.created_at,
            started_at: model.started_at,
            completed_at: model.completed_at,
            source_url: model.source_url,
            source_domain: model.source_domain,
            config: model.config,
            items_processed: model.items_processed,
            items_discovered: model.items_discovered,
            chunks_created: model.chunks_created,
            embeddings_generated: model.embeddings_generated,
            error_count: model.error_count,
            duration_ms: model.duration_ms,
            processing_rate: model.processing_rate,
            errors: model.errors,
            code_extraction_stats: model.code_extraction_stats,
        }
    }
}

impl From<&crate::jobs::JobRecord> for job::ActiveModel {
    fn from(record: &crate::jobs::JobRecord) -> Self {
        use sea_orm::ActiveValue::Set;
        Self {
            job_id: Set(record.job_id.clone()),
            job_type: Set(record.job_type.clone()),
            status: Set(record.status.clone()),
            created_at: Set(record.created_at.clone()),
            started_at: Set(record.started_at.clone()),
            completed_at: Set(record.completed_at.clone()),
            source_url: Set(record.source_url.clone()),
            source_domain: Set(record.source_domain.clone()),
            config: Set(record.config.clone()),
            items_processed: Set(record.items_processed),
            items_discovered: Set(record.items_discovered),
            chunks_created: Set(record.chunks_created),
            embeddings_generated: Set(record.embeddings_generated),
            error_count: Set(record.error_count),
            duration_ms: Set(record.duration_ms),
            processing_rate: Set(record.processing_rate),
            errors: Set(record.errors.clone()),
            code_extraction_stats: Set(record.code_extraction_stats.clone()),
        }
    }
}

// ============================================================================
// Job Item conversions
// ============================================================================

impl From<job_item::Model> for crate::jobs::JobItemRecord {
    fn from(model: job_item::Model) -> Self {
        Self {
            item_id: model.item_id,
            job_id: model.job_id,
            item_path: model.item_path,
            item_type: model.item_type,
            status: model.status,
            parent_url: model.parent_url,
            depth: model.depth,
            discovered_at: model.discovered_at,
            started_at: model.started_at,
            completed_at: model.completed_at,
            chunks_created: model.chunks_created,
            embeddings_generated: model.embeddings_generated,
            duration_ms: model.duration_ms,
            error_message: model.error_message,
        }
    }
}

impl From<&crate::jobs::JobItemRecord> for job_item::ActiveModel {
    fn from(record: &crate::jobs::JobItemRecord) -> Self {
        use sea_orm::ActiveValue::Set;
        Self {
            item_id: Set(record.item_id.clone()),
            job_id: Set(record.job_id.clone()),
            item_path: Set(record.item_path.clone()),
            item_type: Set(record.item_type.clone()),
            status: Set(record.status.clone()),
            parent_url: Set(record.parent_url.clone()),
            depth: Set(record.depth),
            discovered_at: Set(record.discovered_at.clone()),
            started_at: Set(record.started_at.clone()),
            completed_at: Set(record.completed_at.clone()),
            chunks_created: Set(record.chunks_created),
            embeddings_generated: Set(record.embeddings_generated),
            duration_ms: Set(record.duration_ms),
            error_message: Set(record.error_message.clone()),
        }
    }
}
