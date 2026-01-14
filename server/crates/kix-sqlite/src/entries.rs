//! Entry CRUD operations for SQLite.

use crate::error::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

/// Entry record for SQLite storage.
///
/// Represents document metadata without embeddings (vectors stored in LanceDB).
#[derive(Clone, Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct EntryRecord {
    /// Unique entry identifier (UUID)
    pub id: String,

    /// Entry title
    pub title: String,

    /// Short description/summary
    pub description: Option<String>,

    /// Full content text
    pub content: Option<String>,

    /// Tags as JSON array string
    pub tags: Option<String>,

    /// Collection IDs as JSON array string
    pub collection_ids: Option<String>,

    /// Entry type (document, article, pdf, code, etc.)
    pub entry_type: String,

    /// Source type (url, file_upload, direct_input)
    pub source_type: String,

    /// Original file/URL path
    pub source_path: String,

    /// Domain for filtering (e.g., "docs.example.com")
    pub source_domain: Option<String>,

    /// URL-friendly identifier
    pub slug: String,

    /// Content hash for deduplication
    pub source_hash: String,

    /// Creation timestamp (RFC3339)
    pub created_at: String,

    /// Last update timestamp (RFC3339)
    pub updated_at: String,
}

impl EntryRecord {
    /// Create a new entry record.
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        entry_type: impl Into<String>,
        source_type: impl Into<String>,
        source_path: impl Into<String>,
        source_hash: impl Into<String>,
    ) -> Self {
        let title = title.into();
        let now = Utc::now().to_rfc3339();
        Self {
            id: id.into(),
            title: title.clone(),
            description: None,
            content: None,
            tags: None,
            collection_ids: None,
            entry_type: entry_type.into(),
            source_type: source_type.into(),
            source_path: source_path.into(),
            source_domain: None,
            slug: slugify(&title),
            source_hash: source_hash.into(),
            created_at: now.clone(),
            updated_at: now,
        }
    }

    /// Get tags as Vec<String>.
    pub fn tags_vec(&self) -> Vec<String> {
        self.tags
            .as_ref()
            .and_then(|t| serde_json::from_str(t).ok())
            .unwrap_or_default()
    }

    /// Set tags from Vec<String>.
    pub fn set_tags(&mut self, tags: Vec<String>) {
        self.tags = Some(serde_json::to_string(&tags).unwrap_or_else(|_| "[]".to_string()));
    }

    /// Get collection IDs as Vec<String>.
    pub fn collection_ids_vec(&self) -> Vec<String> {
        self.collection_ids
            .as_ref()
            .and_then(|t| serde_json::from_str(t).ok())
            .unwrap_or_default()
    }
}

/// Create a URL-friendly slug from a string.
fn slugify(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Insert a new entry.
pub async fn insert_entry(pool: &SqlitePool, entry: &EntryRecord) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO entries (
            id, title, description, content, tags, collection_ids,
            entry_type, source_type, source_path, source_domain,
            slug, source_hash, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&entry.id)
    .bind(&entry.title)
    .bind(&entry.description)
    .bind(&entry.content)
    .bind(&entry.tags)
    .bind(&entry.collection_ids)
    .bind(&entry.entry_type)
    .bind(&entry.source_type)
    .bind(&entry.source_path)
    .bind(&entry.source_domain)
    .bind(&entry.slug)
    .bind(&entry.source_hash)
    .bind(&entry.created_at)
    .bind(&entry.updated_at)
    .execute(pool)
    .await?;

    Ok(())
}

/// Get an entry by ID.
pub async fn get_entry(pool: &SqlitePool, id: &str) -> Result<Option<EntryRecord>> {
    let entry = sqlx::query_as::<_, EntryRecord>("SELECT * FROM entries WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;

    Ok(entry)
}

/// Get an entry by slug.
pub async fn get_entry_by_slug(pool: &SqlitePool, slug: &str) -> Result<Option<EntryRecord>> {
    let entry = sqlx::query_as::<_, EntryRecord>("SELECT * FROM entries WHERE slug = ?")
        .bind(slug)
        .fetch_optional(pool)
        .await?;

    Ok(entry)
}

/// Check if an entry exists by source hash.
pub async fn exists_by_hash(pool: &SqlitePool, source_hash: &str) -> Result<bool> {
    let count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM entries WHERE source_hash = ?")
            .bind(source_hash)
            .fetch_one(pool)
            .await?;

    Ok(count.0 > 0)
}

/// Delete an entry by ID.
pub async fn delete_entry(pool: &SqlitePool, id: &str) -> Result<bool> {
    let result = sqlx::query("DELETE FROM entries WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected() > 0)
}

/// List entries with optional filters.
pub async fn list_entries(
    pool: &SqlitePool,
    entry_type: Option<&str>,
    source_domain: Option<&str>,
    limit: usize,
    offset: usize,
) -> Result<Vec<EntryRecord>> {
    let mut query = String::from("SELECT * FROM entries WHERE 1=1");
    let mut binds: Vec<String> = vec![];

    if let Some(et) = entry_type {
        query.push_str(" AND entry_type = ?");
        binds.push(et.to_string());
    }

    if let Some(sd) = source_domain {
        query.push_str(" AND (source_domain = ? OR source_domain LIKE ?)");
        binds.push(sd.to_string());
        binds.push(format!("%.{}", sd));
    }

    query.push_str(" ORDER BY created_at DESC LIMIT ? OFFSET ?");

    // Build the query dynamically
    let mut q = sqlx::query_as::<_, EntryRecord>(&query);

    for bind in &binds {
        q = q.bind(bind);
    }

    q = q.bind(limit as i64).bind(offset as i64);

    let entries = q.fetch_all(pool).await?;

    Ok(entries)
}

/// Count total entries.
pub async fn count(pool: &SqlitePool) -> Result<usize> {
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM entries")
        .fetch_one(pool)
        .await?;

    Ok(count.0 as usize)
}

/// Update an entry.
pub async fn update_entry(pool: &SqlitePool, entry: &EntryRecord) -> Result<bool> {
    let result = sqlx::query(
        r#"
        UPDATE entries SET
            title = ?, description = ?, content = ?, tags = ?,
            collection_ids = ?, entry_type = ?, source_type = ?,
            source_path = ?, source_domain = ?, slug = ?,
            source_hash = ?, updated_at = ?
        WHERE id = ?
        "#,
    )
    .bind(&entry.title)
    .bind(&entry.description)
    .bind(&entry.content)
    .bind(&entry.tags)
    .bind(&entry.collection_ids)
    .bind(&entry.entry_type)
    .bind(&entry.source_type)
    .bind(&entry.source_path)
    .bind(&entry.source_domain)
    .bind(&entry.slug)
    .bind(&entry.source_hash)
    .bind(Utc::now().to_rfc3339())
    .bind(&entry.id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::{create_pool, run_migrations};
    use tempfile::TempDir;

    async fn setup_test_db() -> (SqlitePool, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let pool = create_pool(&db_path).await.unwrap();
        run_migrations(&pool).await.unwrap();
        (pool, temp_dir)
    }

    #[tokio::test]
    async fn test_insert_and_get_entry() {
        let (pool, _temp_dir) = setup_test_db().await;

        let entry = EntryRecord::new(
            "test-id-123",
            "Test Entry",
            "document",
            "url",
            "https://example.com",
            "hash123",
        );

        insert_entry(&pool, &entry).await.unwrap();

        let retrieved = get_entry(&pool, "test-id-123").await.unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.title, "Test Entry");
        assert_eq!(retrieved.entry_type, "document");
    }

    #[tokio::test]
    async fn test_entry_exists_by_hash() {
        let (pool, _temp_dir) = setup_test_db().await;

        let entry = EntryRecord::new(
            "test-id-456",
            "Another Entry",
            "article",
            "file",
            "/path/to/file",
            "unique-hash-456",
        );

        insert_entry(&pool, &entry).await.unwrap();

        assert!(exists_by_hash(&pool, "unique-hash-456").await.unwrap());
        assert!(!exists_by_hash(&pool, "nonexistent-hash").await.unwrap());
    }

    #[tokio::test]
    async fn test_delete_entry() {
        let (pool, _temp_dir) = setup_test_db().await;

        let entry = EntryRecord::new(
            "to-delete",
            "Delete Me",
            "document",
            "url",
            "https://delete.me",
            "delete-hash",
        );

        insert_entry(&pool, &entry).await.unwrap();
        assert!(get_entry(&pool, "to-delete").await.unwrap().is_some());

        let deleted = delete_entry(&pool, "to-delete").await.unwrap();
        assert!(deleted);
        assert!(get_entry(&pool, "to-delete").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_list_entries_with_filters() {
        let (pool, _temp_dir) = setup_test_db().await;

        // Insert test entries
        for i in 0..5 {
            let mut entry = EntryRecord::new(
                format!("entry-{}", i),
                format!("Entry {}", i),
                if i % 2 == 0 { "document" } else { "article" },
                "url",
                format!("https://example.com/{}", i),
                format!("hash-{}", i),
            );
            entry.source_domain = Some("example.com".to_string());
            insert_entry(&pool, &entry).await.unwrap();
        }

        // Test filter by entry_type
        let docs = list_entries(&pool, Some("document"), None, 10, 0).await.unwrap();
        assert_eq!(docs.len(), 3); // 0, 2, 4

        // Test filter by source_domain
        let by_domain = list_entries(&pool, None, Some("example.com"), 10, 0).await.unwrap();
        assert_eq!(by_domain.len(), 5);

        // Test pagination
        let page1 = list_entries(&pool, None, None, 2, 0).await.unwrap();
        let page2 = list_entries(&pool, None, None, 2, 2).await.unwrap();
        assert_eq!(page1.len(), 2);
        assert_eq!(page2.len(), 2);
    }

    #[tokio::test]
    async fn test_slugify() {
        assert_eq!(slugify("Hello World"), "hello-world");
        assert_eq!(slugify("Test 123 Entry"), "test-123-entry");
        assert_eq!(slugify("CAPS and lowercase"), "caps-and-lowercase");
        assert_eq!(slugify("Special!@#$Characters"), "special-characters");
    }
}
