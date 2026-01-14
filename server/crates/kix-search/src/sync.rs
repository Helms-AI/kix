//! Database to search index synchronization.
//!
//! This module provides synchronization between the SeaORM database and
//! the Tantivy search indexes. It supports:
//!
//! - Full reindex from database
//! - Incremental sync on CRUD operations
//! - Batch sync for efficient bulk operations

use sea_orm::{DatabaseConnection, EntityTrait, QueryOrder};
use tracing::{debug, info, warn};

use crate::{EntryDocument, IssueDocument, PageDocument, SearchEngine, SearchResult};

// Re-export entity types from kix-sqlite for convenience
use kix_sqlite::entities::{entry, issue, page};

/// Synchronizer for keeping search indexes in sync with the database.
///
/// The IndexSynchronizer provides methods to sync data between SeaORM
/// entities and Tantivy search indexes.
pub struct IndexSynchronizer<'a> {
    db: &'a DatabaseConnection,
    search: &'a SearchEngine,
}

impl<'a> IndexSynchronizer<'a> {
    /// Create a new synchronizer.
    pub fn new(db: &'a DatabaseConnection, search: &'a SearchEngine) -> Self {
        Self { db, search }
    }

    // =========================================================================
    // Full Reindex Operations
    // =========================================================================

    /// Perform a full reindex of all entries from the database.
    ///
    /// This clears the entry index and rebuilds it from the database.
    pub async fn full_reindex_entries(&self) -> SearchResult<SyncStats> {
        info!("Starting full reindex of entries");

        // Clear existing index
        self.search.entry_writer().clear_all()?;

        // Load all entries from database
        let entries = entry::Entity::find()
            .order_by_asc(entry::Column::CreatedAt)
            .all(self.db)
            .await
            .map_err(|e| crate::error::SearchError::Index(format!("Database error: {}", e)))?;

        let total = entries.len();
        info!(count = total, "Loaded entries from database");

        // Convert and index in batches
        let batch_size = 1000;
        let mut indexed = 0;

        for chunk in entries.chunks(batch_size) {
            let docs: Vec<EntryDocument> = chunk.iter().map(entry_to_document).collect();
            let count = self.search.index_entries(&docs)?;
            indexed += count;
            debug!(indexed = indexed, total = total, "Indexed entry batch");
        }

        let stats = SyncStats {
            entries_synced: indexed,
            pages_synced: 0,
            issues_synced: 0,
            errors: vec![],
        };

        info!(indexed = indexed, "Entry reindex complete");
        Ok(stats)
    }

    /// Perform a full reindex of all pages from the database.
    pub async fn full_reindex_pages(&self) -> SearchResult<SyncStats> {
        info!("Starting full reindex of pages");

        self.search.page_writer().clear_all()?;

        let pages = page::Entity::find()
            .order_by_asc(page::Column::CreatedAt)
            .all(self.db)
            .await
            .map_err(|e| crate::error::SearchError::Index(format!("Database error: {}", e)))?;

        let total = pages.len();
        info!(count = total, "Loaded pages from database");

        let batch_size = 1000;
        let mut indexed = 0;

        for chunk in pages.chunks(batch_size) {
            let docs: Vec<PageDocument> = chunk.iter().map(page_to_document).collect();
            let count = self.search.index_pages(&docs)?;
            indexed += count;
            debug!(indexed = indexed, total = total, "Indexed page batch");
        }

        let stats = SyncStats {
            entries_synced: 0,
            pages_synced: indexed,
            issues_synced: 0,
            errors: vec![],
        };

        info!(indexed = indexed, "Page reindex complete");
        Ok(stats)
    }

    /// Perform a full reindex of all issues from the database.
    pub async fn full_reindex_issues(&self) -> SearchResult<SyncStats> {
        info!("Starting full reindex of issues");

        self.search.issue_writer().clear_all()?;

        let issues = issue::Entity::find()
            .order_by_asc(issue::Column::CreatedAt)
            .all(self.db)
            .await
            .map_err(|e| crate::error::SearchError::Index(format!("Database error: {}", e)))?;

        let total = issues.len();
        info!(count = total, "Loaded issues from database");

        let batch_size = 1000;
        let mut indexed = 0;

        for chunk in issues.chunks(batch_size) {
            let docs: Vec<IssueDocument> = chunk.iter().map(issue_to_document).collect();
            let count = self.search.index_issues(&docs)?;
            indexed += count;
            debug!(indexed = indexed, total = total, "Indexed issue batch");
        }

        let stats = SyncStats {
            entries_synced: 0,
            pages_synced: 0,
            issues_synced: indexed,
            errors: vec![],
        };

        info!(indexed = indexed, "Issue reindex complete");
        Ok(stats)
    }

    /// Perform a full reindex of all data.
    pub async fn full_reindex_all(&self) -> SearchResult<SyncStats> {
        info!("Starting full reindex of all data");

        let entry_stats = self.full_reindex_entries().await?;
        let page_stats = self.full_reindex_pages().await?;
        let issue_stats = self.full_reindex_issues().await?;

        let stats = SyncStats {
            entries_synced: entry_stats.entries_synced,
            pages_synced: page_stats.pages_synced,
            issues_synced: issue_stats.issues_synced,
            errors: vec![],
        };

        info!(
            entries = stats.entries_synced,
            pages = stats.pages_synced,
            issues = stats.issues_synced,
            "Full reindex complete"
        );

        Ok(stats)
    }

    // =========================================================================
    // Incremental Sync Operations
    // =========================================================================

    /// Sync a single entry by ID.
    ///
    /// If the entry exists in the database, it will be indexed.
    /// If it doesn't exist, it will be deleted from the index.
    pub async fn sync_entry(&self, entry_id: &str) -> SearchResult<()> {
        let entry = entry::Entity::find_by_id(entry_id)
            .one(self.db)
            .await
            .map_err(|e| crate::error::SearchError::Index(format!("Database error: {}", e)))?;

        match entry {
            Some(e) => {
                let doc = entry_to_document(&e);
                self.search.index_entry(&doc)?;
                debug!(id = entry_id, "Synced entry to index");
            }
            None => {
                self.search.delete_entry(entry_id)?;
                debug!(id = entry_id, "Deleted entry from index");
            }
        }

        Ok(())
    }

    /// Sync a single page by ID.
    pub async fn sync_page(&self, page_id: &str) -> SearchResult<()> {
        let page = page::Entity::find_by_id(page_id)
            .one(self.db)
            .await
            .map_err(|e| crate::error::SearchError::Index(format!("Database error: {}", e)))?;

        match page {
            Some(p) => {
                let doc = page_to_document(&p);
                self.search.index_page(&doc)?;
                debug!(id = page_id, "Synced page to index");
            }
            None => {
                self.search.delete_page(page_id)?;
                debug!(id = page_id, "Deleted page from index");
            }
        }

        Ok(())
    }

    /// Sync a single issue by ID.
    pub async fn sync_issue(&self, issue_id: &str) -> SearchResult<()> {
        let issue = issue::Entity::find_by_id(issue_id)
            .one(self.db)
            .await
            .map_err(|e| crate::error::SearchError::Index(format!("Database error: {}", e)))?;

        match issue {
            Some(i) => {
                let doc = issue_to_document(&i);
                self.search.index_issue(&doc)?;
                debug!(id = issue_id, "Synced issue to index");
            }
            None => {
                self.search.delete_issue(issue_id)?;
                debug!(id = issue_id, "Deleted issue from index");
            }
        }

        Ok(())
    }

    // =========================================================================
    // Batch Sync Operations
    // =========================================================================

    /// Sync multiple entries by ID.
    pub async fn sync_entries(&self, entry_ids: &[&str]) -> SearchResult<SyncStats> {
        let mut synced = 0;
        let mut errors = vec![];

        for id in entry_ids {
            match self.sync_entry(id).await {
                Ok(_) => synced += 1,
                Err(e) => {
                    warn!(id = id, error = %e, "Failed to sync entry");
                    errors.push(format!("Entry {}: {}", id, e));
                }
            }
        }

        Ok(SyncStats {
            entries_synced: synced,
            pages_synced: 0,
            issues_synced: 0,
            errors,
        })
    }

    /// Sync multiple pages by ID.
    pub async fn sync_pages(&self, page_ids: &[&str]) -> SearchResult<SyncStats> {
        let mut synced = 0;
        let mut errors = vec![];

        for id in page_ids {
            match self.sync_page(id).await {
                Ok(_) => synced += 1,
                Err(e) => {
                    warn!(id = id, error = %e, "Failed to sync page");
                    errors.push(format!("Page {}: {}", id, e));
                }
            }
        }

        Ok(SyncStats {
            entries_synced: 0,
            pages_synced: synced,
            issues_synced: 0,
            errors,
        })
    }

    /// Sync multiple issues by ID.
    pub async fn sync_issues(&self, issue_ids: &[&str]) -> SearchResult<SyncStats> {
        let mut synced = 0;
        let mut errors = vec![];

        for id in issue_ids {
            match self.sync_issue(id).await {
                Ok(_) => synced += 1,
                Err(e) => {
                    warn!(id = id, error = %e, "Failed to sync issue");
                    errors.push(format!("Issue {}: {}", id, e));
                }
            }
        }

        Ok(SyncStats {
            entries_synced: 0,
            pages_synced: 0,
            issues_synced: synced,
            errors,
        })
    }
}

/// Statistics from a sync operation.
#[derive(Debug, Clone, Default)]
pub struct SyncStats {
    /// Number of entries synced.
    pub entries_synced: usize,
    /// Number of pages synced.
    pub pages_synced: usize,
    /// Number of issues synced.
    pub issues_synced: usize,
    /// Errors encountered during sync.
    pub errors: Vec<String>,
}

impl SyncStats {
    /// Total documents synced.
    pub fn total(&self) -> usize {
        self.entries_synced + self.pages_synced + self.issues_synced
    }

    /// Whether the sync had any errors.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

// ============================================================================
// Conversion Functions
// ============================================================================

/// Convert a SeaORM entry model to a Tantivy document.
fn entry_to_document(entry: &entry::Model) -> EntryDocument {
    // Parse created_at timestamp
    let created_at = chrono::DateTime::parse_from_rfc3339(&entry.created_at)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .unwrap_or_else(|_| chrono::Utc::now());

    // Parse tags from JSON
    let tags: Vec<String> = entry
        .tags
        .as_ref()
        .and_then(|t| serde_json::from_str(t).ok())
        .unwrap_or_default();

    EntryDocument {
        id: entry.id.clone(),
        title: entry.title.clone(),
        description: entry.description.clone(),
        content: entry.content.clone(),
        entry_type: entry.entry_type.clone(),
        source_domain: entry.source_domain.clone(),
        source_path: entry.source_path.clone(),
        tags,
        created_at,
    }
}

/// Convert a SeaORM page model to a Tantivy document.
fn page_to_document(page: &page::Model) -> PageDocument {
    let created_at = chrono::DateTime::parse_from_rfc3339(&page.created_at)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .unwrap_or_else(|_| chrono::Utc::now());

    PageDocument {
        page_id: page.page_id.clone(),
        entry_id: page.source_id.clone(),
        url: page.url.clone(),
        title: page.title.clone(),
        content: page.full_content.clone(),
        created_at,
    }
}

/// Convert a SeaORM issue model to a Tantivy document.
fn issue_to_document(issue: &issue::Model) -> IssueDocument {
    let created_at = chrono::DateTime::parse_from_rfc3339(&issue.created_at)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .unwrap_or_else(|_| chrono::Utc::now());

    // Parse labels from JSON
    let labels: Vec<String> = issue
        .labels
        .as_ref()
        .and_then(|l| serde_json::from_str(l).ok())
        .unwrap_or_default();

    IssueDocument {
        id: issue.id.clone(),
        project_id: issue.project_id.clone(),
        number: issue.number,
        title: issue.title.clone(),
        body: issue.body.clone(),
        state: issue.state.clone(),
        labels,
        created_at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entry_to_document() {
        let entry = entry::Model {
            id: "entry-1".to_string(),
            title: "Test Entry".to_string(),
            description: Some("Description".to_string()),
            content: Some("Content".to_string()),
            tags: Some(r#"["tag1", "tag2"]"#.to_string()),
            collection_ids: None,
            entry_type: "document".to_string(),
            source_type: "url".to_string(),
            source_path: "https://example.com".to_string(),
            source_domain: Some("example.com".to_string()),
            slug: "test-entry".to_string(),
            source_hash: "hash123".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        };

        let doc = entry_to_document(&entry);

        assert_eq!(doc.id, "entry-1");
        assert_eq!(doc.title, "Test Entry");
        assert_eq!(doc.tags, vec!["tag1", "tag2"]);
    }

    #[test]
    fn test_page_to_document() {
        let page = page::Model {
            page_id: "page-1".to_string(),
            source_id: "entry-1".to_string(),
            url: "https://example.com/page".to_string(),
            title: Some("Test Page".to_string()),
            full_content: "Page content".to_string(),
            content_hash: "hash".to_string(),
            content_length: 12,
            code_block_count: 0,
            metadata: None,
            crawl_time_ms: None,
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };

        let doc = page_to_document(&page);

        assert_eq!(doc.page_id, "page-1");
        assert_eq!(doc.entry_id, "entry-1");
        assert_eq!(doc.content, "Page content");
    }

    #[test]
    fn test_issue_to_document() {
        let issue = issue::Model {
            id: "issue-1".to_string(),
            project_id: "project-1".to_string(),
            number: 42,
            title: "Test Issue".to_string(),
            body: Some("Issue body".to_string()),
            state: "open".to_string(),
            labels: Some(r#"["bug", "p1"]"#.to_string()),
            assignees: None,
            priority: Some(1),
            github_number: None,
            github_node_id: None,
            github_url: None,
            github_project_item_id: None,
            source: "local".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
            closed_at: None,
            synced_at: None,
        };

        let doc = issue_to_document(&issue);

        assert_eq!(doc.id, "issue-1");
        assert_eq!(doc.number, 42);
        assert_eq!(doc.labels, vec!["bug", "p1"]);
    }

    #[test]
    fn test_sync_stats() {
        let stats = SyncStats {
            entries_synced: 10,
            pages_synced: 20,
            issues_synced: 5,
            errors: vec![],
        };

        assert_eq!(stats.total(), 35);
        assert!(!stats.has_errors());

        let stats_with_errors = SyncStats {
            entries_synced: 10,
            pages_synced: 20,
            issues_synced: 5,
            errors: vec!["Error 1".to_string()],
        };

        assert!(stats_with_errors.has_errors());
    }
}
