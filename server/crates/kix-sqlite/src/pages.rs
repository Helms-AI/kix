//! Page CRUD operations for SQLite.
//!
//! Pages store full content for RAG context retrieval (two-layer storage).

use crate::error::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

/// Page record for SQLite storage.
///
/// Stores full page content for RAG context retrieval.
#[derive(Clone, Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct PageRecord {
    /// Unique page identifier (UUID)
    pub page_id: String,

    /// FK to the source entry
    pub source_id: String,

    /// Original URL
    pub url: String,

    /// Page title
    pub title: Option<String>,

    /// Full markdown content
    pub full_content: String,

    /// SHA256 hash for deduplication
    pub content_hash: String,

    /// Content length in characters
    pub content_length: i64,

    /// Number of code blocks
    pub code_block_count: i64,

    /// Additional metadata as JSON
    pub metadata: Option<String>,

    /// Crawl time in milliseconds
    pub crawl_time_ms: Option<i64>,

    /// Creation timestamp (RFC3339)
    pub created_at: String,
}

impl PageRecord {
    /// Create a new page record.
    pub fn new(
        source_id: impl Into<String>,
        url: impl Into<String>,
        full_content: impl Into<String>,
    ) -> Self {
        let content = full_content.into();
        let content_len = content.len() as i64;

        // Count code blocks
        let code_block_count = (content.matches("```").count() / 2) as i64;

        // Generate content hash
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let content_hash = format!("{:x}", hasher.finalize());

        Self {
            page_id: uuid::Uuid::new_v4().to_string(),
            source_id: source_id.into(),
            url: url.into(),
            title: None,
            full_content: content,
            content_hash,
            content_length: content_len,
            code_block_count,
            metadata: None,
            crawl_time_ms: None,
            created_at: Utc::now().to_rfc3339(),
        }
    }

    /// Set the page title.
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set crawl time.
    pub fn with_crawl_time(mut self, ms: u64) -> Self {
        self.crawl_time_ms = Some(ms as i64);
        self
    }

    /// Set metadata.
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata.to_string());
        self
    }
}

/// Insert a new page.
pub async fn insert_page(pool: &SqlitePool, page: &PageRecord) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO pages (
            page_id, source_id, url, title, full_content,
            content_hash, content_length, code_block_count,
            metadata, crawl_time_ms, created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&page.page_id)
    .bind(&page.source_id)
    .bind(&page.url)
    .bind(&page.title)
    .bind(&page.full_content)
    .bind(&page.content_hash)
    .bind(page.content_length)
    .bind(page.code_block_count)
    .bind(&page.metadata)
    .bind(page.crawl_time_ms)
    .bind(&page.created_at)
    .execute(pool)
    .await?;

    Ok(())
}

/// Get a page by ID.
pub async fn get_page(pool: &SqlitePool, page_id: &str) -> Result<Option<PageRecord>> {
    let page = sqlx::query_as::<_, PageRecord>("SELECT * FROM pages WHERE page_id = ?")
        .bind(page_id)
        .fetch_optional(pool)
        .await?;

    Ok(page)
}

/// Get all pages for a source/entry.
pub async fn get_pages_by_source(pool: &SqlitePool, source_id: &str) -> Result<Vec<PageRecord>> {
    let pages = sqlx::query_as::<_, PageRecord>(
        "SELECT * FROM pages WHERE source_id = ? ORDER BY created_at",
    )
    .bind(source_id)
    .fetch_all(pool)
    .await?;

    Ok(pages)
}

/// Check if a page exists by content hash.
pub async fn exists_by_hash(pool: &SqlitePool, content_hash: &str) -> Result<bool> {
    let count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM pages WHERE content_hash = ?")
            .bind(content_hash)
            .fetch_one(pool)
            .await?;

    Ok(count.0 > 0)
}

/// Delete a page by ID.
pub async fn delete_page(pool: &SqlitePool, page_id: &str) -> Result<bool> {
    let result = sqlx::query("DELETE FROM pages WHERE page_id = ?")
        .bind(page_id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected() > 0)
}

/// Delete all pages for a source.
pub async fn delete_by_source(pool: &SqlitePool, source_id: &str) -> Result<usize> {
    let result = sqlx::query("DELETE FROM pages WHERE source_id = ?")
        .bind(source_id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected() as usize)
}

/// Count total pages.
pub async fn count(pool: &SqlitePool) -> Result<usize> {
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM pages")
        .fetch_one(pool)
        .await?;

    Ok(count.0 as usize)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entries::{insert_entry, EntryRecord};
    use crate::pool::{create_pool, run_migrations};
    use tempfile::TempDir;

    async fn setup_test_db() -> (SqlitePool, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let pool = create_pool(&db_path).await.unwrap();
        run_migrations(&pool).await.unwrap();
        (pool, temp_dir)
    }

    async fn create_test_entry(pool: &SqlitePool, id: &str) {
        let entry = EntryRecord::new(id, "Test Entry", "document", "url", "https://example.com", "hash");
        insert_entry(pool, &entry).await.unwrap();
    }

    #[tokio::test]
    async fn test_insert_and_get_page() {
        let (pool, _temp_dir) = setup_test_db().await;

        // Create parent entry
        create_test_entry(&pool, "entry-1").await;

        let page = PageRecord::new("entry-1", "https://example.com/page1", "# Hello\n\nThis is content.")
            .with_title("Hello Page");

        insert_page(&pool, &page).await.unwrap();

        let retrieved = get_page(&pool, &page.page_id).await.unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.title, Some("Hello Page".to_string()));
        assert!(retrieved.content_length > 0);
    }

    #[tokio::test]
    async fn test_page_content_hash() {
        let (pool, _temp_dir) = setup_test_db().await;
        create_test_entry(&pool, "entry-2").await;

        let page = PageRecord::new("entry-2", "https://example.com/page2", "Test content");
        insert_page(&pool, &page).await.unwrap();

        assert!(exists_by_hash(&pool, &page.content_hash).await.unwrap());
        assert!(!exists_by_hash(&pool, "nonexistent").await.unwrap());
    }

    #[tokio::test]
    async fn test_get_pages_by_source() {
        let (pool, _temp_dir) = setup_test_db().await;
        create_test_entry(&pool, "entry-3").await;

        // Insert multiple pages for same source
        for i in 0..3 {
            let page = PageRecord::new("entry-3", format!("https://example.com/page{}", i), format!("Content {}", i));
            insert_page(&pool, &page).await.unwrap();
        }

        let pages = get_pages_by_source(&pool, "entry-3").await.unwrap();
        assert_eq!(pages.len(), 3);
    }

    #[tokio::test]
    async fn test_code_block_counting() {
        let content = r#"
# Code Example

```rust
fn main() {
    println!("Hello");
}
```

Some text between.

```python
print("world")
```
"#;

        let page = PageRecord::new("src-1", "https://example.com", content);
        assert_eq!(page.code_block_count, 2);
    }
}
