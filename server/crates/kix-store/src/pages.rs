//! Pages Store - Two-Layer Storage Implementation
//!
//! Implements a two-layer storage pattern for RAG context retrieval.
//!
//! ## Architecture
//!
//! - **Pages**: Store full page content for context retrieval
//! - **Chunks**: Store smaller pieces with embeddings for vector search
//!
//! When a search finds relevant chunks, the full page context can be retrieved
//! using the chunk's `page_id` foreign key.

use arrow_array::{Array, ArrayRef, RecordBatch, RecordBatchIterator, StringArray, UInt32Array, UInt64Array};
use arrow_schema::ArrowError;
use chrono::{DateTime, Utc};
use lancedb::query::{ExecutableQuery, QueryBase};
use lancedb::{Connection, Table};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;
use uuid::Uuid;

use crate::error::StoreError;
use crate::schema::page_schema;

// ============================================================================
// Page Record
// ============================================================================

/// A page record for the two-layer storage.
///
/// Pages store the full content of a crawled page for context retrieval.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PageRecord {
    /// Unique page identifier
    pub page_id: String,

    /// FK to the source/entry that this page belongs to
    pub source_id: String,

    /// Original URL of the page
    pub url: String,

    /// Page title
    pub title: Option<String>,

    /// Full markdown content of the page
    pub full_content: String,

    /// Content hash for deduplication
    pub content_hash: String,

    /// Content length in characters
    pub content_length: u32,

    /// Number of code blocks in the page
    pub code_block_count: u32,

    /// Additional metadata as JSON
    pub metadata: Option<serde_json::Value>,

    /// Time taken to crawl (milliseconds)
    pub crawl_time_ms: Option<u64>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,
}

impl PageRecord {
    /// Create a new page record.
    pub fn new(
        source_id: impl Into<String>,
        url: impl Into<String>,
        full_content: impl Into<String>,
    ) -> Self {
        let content = full_content.into();
        let content_len = content.len() as u32;

        // Count code blocks
        let code_block_count = content.matches("```").count() as u32 / 2;

        // Generate content hash
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let content_hash = format!("{:x}", hasher.finalize());

        Self {
            page_id: Uuid::new_v4().to_string(),
            source_id: source_id.into(),
            url: url.into(),
            title: None,
            full_content: content,
            content_hash,
            content_length: content_len,
            code_block_count,
            metadata: None,
            crawl_time_ms: None,
            created_at: Utc::now(),
        }
    }

    /// Set the title.
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set metadata.
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Set crawl time.
    pub fn with_crawl_time(mut self, ms: u64) -> Self {
        self.crawl_time_ms = Some(ms);
        self
    }

    /// Get a preview of the content (first N characters).
    pub fn content_preview(&self, max_chars: usize) -> &str {
        if self.full_content.len() <= max_chars {
            &self.full_content
        } else {
            &self.full_content[..max_chars]
        }
    }
}

// ============================================================================
// Pages Store
// ============================================================================

/// Store for page records (first layer of two-layer storage).
pub struct PageStore {
    table: Table,
}

impl PageStore {
    /// Create a new page store from an existing table.
    pub fn new(table: Table) -> Self {
        Self { table }
    }

    /// Create a page store, initializing the table if needed.
    pub async fn init(db: &Connection) -> Result<Self, StoreError> {
        let table_names = db
            .table_names()
            .execute()
            .await
            .map_err(|e| StoreError::Database(e.to_string()))?;

        let table = if table_names.contains(&"pages".to_string()) {
            info!("Opening existing pages table");
            db.open_table("pages")
                .execute()
                .await
                .map_err(|e| StoreError::Database(e.to_string()))?
        } else {
            info!("Creating new pages table");
            db.create_empty_table("pages", page_schema())
                .execute()
                .await
                .map_err(|e| StoreError::Database(e.to_string()))?
        };

        Ok(Self { table })
    }

    /// Insert page records.
    pub async fn insert_pages(&self, pages: &[PageRecord]) -> Result<(), StoreError> {
        if pages.is_empty() {
            return Ok(());
        }

        let batch = Self::pages_to_batch(pages)?;
        let schema = batch.schema();
        let batches: Vec<Result<RecordBatch, ArrowError>> = vec![Ok(batch)];
        let reader = RecordBatchIterator::new(batches, schema);

        self.table
            .add(Box::new(reader))
            .execute()
            .await
            .map_err(|e| StoreError::Database(e.to_string()))?;

        info!("Inserted {} pages", pages.len());
        Ok(())
    }

    /// Get a page by ID.
    pub async fn get_page(&self, page_id: &str) -> Result<Option<PageRecord>, StoreError> {
        use futures::TryStreamExt;

        let filter = format!("page_id = '{}'", page_id.replace('\'', "''"));

        let batches = self
            .table
            .query()
            .only_if(filter)
            .limit(1)
            .execute()
            .await
            .map_err(|e| StoreError::Database(e.to_string()))?
            .try_collect::<Vec<_>>()
            .await
            .map_err(|e| StoreError::Database(e.to_string()))?;

        if batches.is_empty() || batches[0].num_rows() == 0 {
            return Ok(None);
        }

        Self::batch_to_page(&batches[0], 0).map(Some)
    }

    /// Get all pages for a source.
    pub async fn get_pages_by_source(
        &self,
        source_id: &str,
    ) -> Result<Vec<PageRecord>, StoreError> {
        use futures::TryStreamExt;

        let filter = format!("source_id = '{}'", source_id.replace('\'', "''"));

        let batches = self
            .table
            .query()
            .only_if(filter)
            .execute()
            .await
            .map_err(|e| StoreError::Database(e.to_string()))?
            .try_collect::<Vec<_>>()
            .await
            .map_err(|e| StoreError::Database(e.to_string()))?;

        let mut pages = Vec::new();
        for batch in &batches {
            for i in 0..batch.num_rows() {
                pages.push(Self::batch_to_page(batch, i)?);
            }
        }

        Ok(pages)
    }

    /// Delete a page by ID.
    pub async fn delete_page(&self, page_id: &str) -> Result<(), StoreError> {
        let filter = format!("page_id = '{}'", page_id.replace('\'', "''"));
        self.table
            .delete(&filter)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))?;

        info!("Deleted page: {}", page_id);
        Ok(())
    }

    /// Delete all pages for a source.
    pub async fn delete_pages_by_source(&self, source_id: &str) -> Result<(), StoreError> {
        let filter = format!("source_id = '{}'", source_id.replace('\'', "''"));
        self.table
            .delete(&filter)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))?;

        info!("Deleted pages for source: {}", source_id);
        Ok(())
    }

    /// Get page count.
    pub async fn count(&self) -> Result<usize, StoreError> {
        self.table
            .count_rows(None)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))
    }

    /// Check if a page exists by content hash.
    pub async fn exists_by_hash(&self, content_hash: &str) -> Result<bool, StoreError> {
        let filter = format!("content_hash = '{}'", content_hash.replace('\'', "''"));
        let count = self
            .table
            .count_rows(Some(filter))
            .await
            .map_err(|e| StoreError::Database(e.to_string()))?;

        Ok(count > 0)
    }

    // ========================================================================
    // Private Helpers
    // ========================================================================

    /// Convert page records to a RecordBatch.
    fn pages_to_batch(pages: &[PageRecord]) -> Result<RecordBatch, StoreError> {
        let page_ids: Vec<&str> = pages.iter().map(|p| p.page_id.as_str()).collect();
        let source_ids: Vec<&str> = pages.iter().map(|p| p.source_id.as_str()).collect();
        let urls: Vec<&str> = pages.iter().map(|p| p.url.as_str()).collect();
        let titles: Vec<Option<&str>> = pages.iter().map(|p| p.title.as_deref()).collect();
        let contents: Vec<&str> = pages.iter().map(|p| p.full_content.as_str()).collect();
        let hashes: Vec<&str> = pages.iter().map(|p| p.content_hash.as_str()).collect();
        let lengths: Vec<u32> = pages.iter().map(|p| p.content_length).collect();
        let code_counts: Vec<u32> = pages.iter().map(|p| p.code_block_count).collect();
        let metadata: Vec<Option<String>> = pages
            .iter()
            .map(|p| p.metadata.as_ref().map(|m| m.to_string()))
            .collect();
        let crawl_times: Vec<Option<u64>> = pages.iter().map(|p| p.crawl_time_ms).collect();
        let created_ats: Vec<String> = pages.iter().map(|p| p.created_at.to_rfc3339()).collect();

        let columns: Vec<ArrayRef> = vec![
            Arc::new(StringArray::from(page_ids)),
            Arc::new(StringArray::from(source_ids)),
            Arc::new(StringArray::from(urls)),
            Arc::new(StringArray::from(titles)),
            Arc::new(StringArray::from(contents)),
            Arc::new(StringArray::from(hashes)),
            Arc::new(UInt32Array::from(lengths)),
            Arc::new(UInt32Array::from(code_counts)),
            Arc::new(StringArray::from(
                metadata
                    .iter()
                    .map(|m| m.as_deref())
                    .collect::<Vec<_>>(),
            )),
            Arc::new(UInt64Array::from(
                crawl_times
                    .iter()
                    .map(|t| t.unwrap_or(0))
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                created_ats.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            )),
        ];

        RecordBatch::try_new(page_schema(), columns)
            .map_err(|e| StoreError::Serialization(e.to_string()))
    }

    /// Convert a RecordBatch row to a PageRecord.
    fn batch_to_page(batch: &RecordBatch, row: usize) -> Result<PageRecord, StoreError> {
        let page_id = batch
            .column_by_name("page_id")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
            .and_then(|a| a.value(row).into())
            .unwrap_or_default();

        let source_id = batch
            .column_by_name("source_id")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
            .and_then(|a| Some(a.value(row).to_string()))
            .unwrap_or_default();

        let url = batch
            .column_by_name("url")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
            .and_then(|a| Some(a.value(row).to_string()))
            .unwrap_or_default();

        let title = batch
            .column_by_name("title")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
            .and_then(|a| {
                if a.is_null(row) {
                    None
                } else {
                    Some(a.value(row).to_string())
                }
            });

        let full_content = batch
            .column_by_name("full_content")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
            .and_then(|a| Some(a.value(row).to_string()))
            .unwrap_or_default();

        let content_hash = batch
            .column_by_name("content_hash")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
            .and_then(|a| Some(a.value(row).to_string()))
            .unwrap_or_default();

        let content_length = batch
            .column_by_name("content_length")
            .and_then(|c| c.as_any().downcast_ref::<UInt32Array>())
            .map(|a| a.value(row))
            .unwrap_or(0);

        let code_block_count = batch
            .column_by_name("code_block_count")
            .and_then(|c| c.as_any().downcast_ref::<UInt32Array>())
            .map(|a| a.value(row))
            .unwrap_or(0);

        let metadata = batch
            .column_by_name("metadata")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
            .and_then(|a| {
                if a.is_null(row) {
                    None
                } else {
                    serde_json::from_str(a.value(row)).ok()
                }
            });

        let crawl_time_ms = batch
            .column_by_name("crawl_time_ms")
            .and_then(|c| c.as_any().downcast_ref::<UInt64Array>())
            .map(|a| a.value(row));

        let created_at = batch
            .column_by_name("created_at")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
            .and_then(|a| DateTime::parse_from_rfc3339(a.value(row)).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);

        Ok(PageRecord {
            page_id: page_id.to_string(),
            source_id,
            url,
            title,
            full_content,
            content_hash,
            content_length,
            code_block_count,
            metadata,
            crawl_time_ms,
            created_at,
        })
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_record_creation() {
        let page = PageRecord::new("source-123", "https://example.com/page", "# Hello World\n\nSome content here.");

        assert!(!page.page_id.is_empty());
        assert_eq!(page.source_id, "source-123");
        assert_eq!(page.url, "https://example.com/page");
        assert!(page.content_length > 0);
        assert!(!page.content_hash.is_empty());
    }

    #[test]
    fn test_page_record_with_code_blocks() {
        let content = r#"# Example

```rust
fn main() {
    println!("Hello");
}
```

Some text

```python
print("Hello")
```
"#;

        let page = PageRecord::new("source-123", "https://example.com", content);
        assert_eq!(page.code_block_count, 2);
    }

    #[test]
    fn test_page_record_builder_pattern() {
        let page = PageRecord::new("src", "url", "content")
            .with_title("My Title")
            .with_crawl_time(1500)
            .with_metadata(serde_json::json!({"key": "value"}));

        assert_eq!(page.title, Some("My Title".to_string()));
        assert_eq!(page.crawl_time_ms, Some(1500));
        assert!(page.metadata.is_some());
    }
}
