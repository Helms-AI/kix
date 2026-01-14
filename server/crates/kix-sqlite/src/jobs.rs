//! Job history operations for SQLite.

use crate::error::Result;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

/// Job record for SQLite storage.
#[derive(Clone, Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct JobRecord {
    /// Unique job identifier (UUID)
    pub job_id: String,

    /// Job type: "url", "file_upload", or "reindex"
    pub job_type: String,

    /// Status: "completed", "failed", or "cancelled"
    pub status: String,

    /// Creation timestamp (RFC3339)
    pub created_at: String,

    /// Start timestamp (RFC3339)
    pub started_at: Option<String>,

    /// Completion timestamp (RFC3339)
    pub completed_at: String,

    /// Source URL for URL jobs
    pub source_url: Option<String>,

    /// Source domain
    pub source_domain: Option<String>,

    /// Job configuration as JSON
    pub config: String,

    /// Number of items processed
    pub items_processed: i64,

    /// Number of items discovered
    pub items_discovered: i64,

    /// Number of chunks created
    pub chunks_created: i64,

    /// Number of embeddings generated
    pub embeddings_generated: i64,

    /// Number of errors
    pub error_count: i64,

    /// Duration in milliseconds
    pub duration_ms: i64,

    /// Processing rate (items/second)
    pub processing_rate: f64,

    /// Error messages as JSON array
    pub errors: Option<String>,
}

impl JobRecord {
    /// Create a new job record.
    pub fn new(job_id: impl Into<String>, job_type: impl Into<String>) -> Self {
        Self {
            job_id: job_id.into(),
            job_type: job_type.into(),
            status: "completed".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            started_at: None,
            completed_at: chrono::Utc::now().to_rfc3339(),
            source_url: None,
            source_domain: None,
            config: "{}".to_string(),
            items_processed: 0,
            items_discovered: 0,
            chunks_created: 0,
            embeddings_generated: 0,
            error_count: 0,
            duration_ms: 0,
            processing_rate: 0.0,
            errors: None,
        }
    }

    /// Get errors as Vec<String>.
    pub fn errors_vec(&self) -> Vec<String> {
        self.errors
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default()
    }

    /// Set errors from Vec<String>.
    pub fn set_errors(&mut self, errors: Vec<String>) {
        self.errors = Some(serde_json::to_string(&errors).unwrap_or_else(|_| "[]".to_string()));
    }
}

/// Job item record for per-item details.
#[derive(Clone, Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct JobItemRecord {
    /// Unique item identifier (UUID)
    pub item_id: String,

    /// FK to jobs table
    pub job_id: String,

    /// URL or file path
    pub item_path: String,

    /// Item type: "page" or "file"
    pub item_type: String,

    /// Status: "completed", "error", or "skipped"
    pub status: String,

    /// Parent URL (for crawled pages)
    pub parent_url: Option<String>,

    /// Crawl depth
    pub depth: i64,

    /// Discovery timestamp (RFC3339)
    pub discovered_at: String,

    /// Start timestamp (RFC3339)
    pub started_at: Option<String>,

    /// Completion timestamp (RFC3339)
    pub completed_at: Option<String>,

    /// Number of chunks created
    pub chunks_created: i64,

    /// Number of embeddings generated
    pub embeddings_generated: i64,

    /// Duration in milliseconds
    pub duration_ms: i64,

    /// Error message if failed
    pub error_message: Option<String>,
}

impl JobItemRecord {
    /// Create a new job item record.
    pub fn new(
        job_id: impl Into<String>,
        item_path: impl Into<String>,
        item_type: impl Into<String>,
    ) -> Self {
        Self {
            item_id: uuid::Uuid::new_v4().to_string(),
            job_id: job_id.into(),
            item_path: item_path.into(),
            item_type: item_type.into(),
            status: "pending".to_string(),
            parent_url: None,
            depth: 0,
            discovered_at: chrono::Utc::now().to_rfc3339(),
            started_at: None,
            completed_at: None,
            chunks_created: 0,
            embeddings_generated: 0,
            duration_ms: 0,
            error_message: None,
        }
    }

    /// Create a new page item record.
    pub fn new_page(job_id: impl Into<String>, url: impl Into<String>) -> Self {
        Self::new(job_id, url, "page")
    }

    /// Create a new file item record.
    pub fn new_file(job_id: impl Into<String>, path: impl Into<String>) -> Self {
        Self::new(job_id, path, "file")
    }

    /// Set parent URL.
    pub fn with_parent(mut self, parent_url: impl Into<String>) -> Self {
        self.parent_url = Some(parent_url.into());
        self
    }

    /// Set depth.
    pub fn with_depth(mut self, depth: i64) -> Self {
        self.depth = depth;
        self
    }

    /// Mark as started.
    pub fn mark_started(&mut self) {
        self.started_at = Some(chrono::Utc::now().to_rfc3339());
        self.status = "processing".to_string();
    }

    /// Mark as completed with stats.
    pub fn mark_completed(&mut self, chunks: i64, embeddings: i64, duration_ms: i64) {
        self.completed_at = Some(chrono::Utc::now().to_rfc3339());
        self.status = "completed".to_string();
        self.chunks_created = chunks;
        self.embeddings_generated = embeddings;
        self.duration_ms = duration_ms;
    }

    /// Mark as errored.
    pub fn mark_error(&mut self, message: impl Into<String>) {
        self.completed_at = Some(chrono::Utc::now().to_rfc3339());
        self.status = "error".to_string();
        self.error_message = Some(message.into());
    }

    /// Mark as skipped.
    pub fn mark_skipped(&mut self) {
        self.completed_at = Some(chrono::Utc::now().to_rfc3339());
        self.status = "skipped".to_string();
    }
}

/// Insert a job record.
pub async fn insert_job(pool: &SqlitePool, job: &JobRecord) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO jobs (
            job_id, job_type, status, created_at, started_at, completed_at,
            source_url, source_domain, config, items_processed, items_discovered,
            chunks_created, embeddings_generated, error_count, duration_ms,
            processing_rate, errors
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&job.job_id)
    .bind(&job.job_type)
    .bind(&job.status)
    .bind(&job.created_at)
    .bind(&job.started_at)
    .bind(&job.completed_at)
    .bind(&job.source_url)
    .bind(&job.source_domain)
    .bind(&job.config)
    .bind(job.items_processed)
    .bind(job.items_discovered)
    .bind(job.chunks_created)
    .bind(job.embeddings_generated)
    .bind(job.error_count)
    .bind(job.duration_ms)
    .bind(job.processing_rate)
    .bind(&job.errors)
    .execute(pool)
    .await?;

    Ok(())
}

/// Get a job by ID.
pub async fn get_job(pool: &SqlitePool, job_id: &str) -> Result<Option<JobRecord>> {
    let job = sqlx::query_as::<_, JobRecord>("SELECT * FROM jobs WHERE job_id = ?")
        .bind(job_id)
        .fetch_optional(pool)
        .await?;

    Ok(job)
}

/// List jobs with optional status filter.
pub async fn list_jobs(
    pool: &SqlitePool,
    status: Option<&str>,
    limit: usize,
    offset: usize,
) -> Result<Vec<JobRecord>> {
    let query = match status {
        Some(s) => sqlx::query_as::<_, JobRecord>(
            "SELECT * FROM jobs WHERE status = ? ORDER BY completed_at DESC LIMIT ? OFFSET ?",
        )
        .bind(s)
        .bind(limit as i64)
        .bind(offset as i64),
        None => sqlx::query_as::<_, JobRecord>(
            "SELECT * FROM jobs ORDER BY completed_at DESC LIMIT ? OFFSET ?",
        )
        .bind(limit as i64)
        .bind(offset as i64),
    };

    let jobs = query.fetch_all(pool).await?;
    Ok(jobs)
}

/// Insert job items.
pub async fn insert_job_items(pool: &SqlitePool, items: &[JobItemRecord]) -> Result<()> {
    for item in items {
        sqlx::query(
            r#"
            INSERT INTO job_items (
                item_id, job_id, item_path, item_type, status,
                parent_url, depth, discovered_at, started_at, completed_at,
                chunks_created, embeddings_generated, duration_ms, error_message
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&item.item_id)
        .bind(&item.job_id)
        .bind(&item.item_path)
        .bind(&item.item_type)
        .bind(&item.status)
        .bind(&item.parent_url)
        .bind(item.depth)
        .bind(&item.discovered_at)
        .bind(&item.started_at)
        .bind(&item.completed_at)
        .bind(item.chunks_created)
        .bind(item.embeddings_generated)
        .bind(item.duration_ms)
        .bind(&item.error_message)
        .execute(pool)
        .await?;
    }

    Ok(())
}

/// Get job items for a job.
pub async fn get_job_items(pool: &SqlitePool, job_id: &str) -> Result<Vec<JobItemRecord>> {
    let items = sqlx::query_as::<_, JobItemRecord>(
        "SELECT * FROM job_items WHERE job_id = ? ORDER BY discovered_at",
    )
    .bind(job_id)
    .fetch_all(pool)
    .await?;

    Ok(items)
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
    async fn test_insert_and_get_job() {
        let (pool, _temp_dir) = setup_test_db().await;

        let mut job = JobRecord::new("job-123", "url");
        job.source_url = Some("https://example.com".to_string());
        job.items_processed = 10;
        job.chunks_created = 50;

        insert_job(&pool, &job).await.unwrap();

        let retrieved = get_job(&pool, "job-123").await.unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.job_type, "url");
        assert_eq!(retrieved.items_processed, 10);
    }

    #[tokio::test]
    async fn test_list_jobs_by_status() {
        let (pool, _temp_dir) = setup_test_db().await;

        // Create jobs with different statuses
        let mut completed = JobRecord::new("job-1", "url");
        completed.status = "completed".to_string();
        insert_job(&pool, &completed).await.unwrap();

        let mut failed = JobRecord::new("job-2", "url");
        failed.status = "failed".to_string();
        insert_job(&pool, &failed).await.unwrap();

        // List by status
        let completed_jobs = list_jobs(&pool, Some("completed"), 100, 0).await.unwrap();
        assert_eq!(completed_jobs.len(), 1);

        let all_jobs = list_jobs(&pool, None, 100, 0).await.unwrap();
        assert_eq!(all_jobs.len(), 2);
    }

    #[tokio::test]
    async fn test_job_items() {
        let (pool, _temp_dir) = setup_test_db().await;

        // Create parent job
        let job = JobRecord::new("job-items-test", "url");
        insert_job(&pool, &job).await.unwrap();

        // Create items
        let items: Vec<JobItemRecord> = (0..5)
            .map(|i| {
                let mut item = JobItemRecord::new(
                    "job-items-test",
                    format!("https://example.com/page{}", i),
                    "page",
                );
                item.chunks_created = (i + 1) as i64;
                item
            })
            .collect();

        insert_job_items(&pool, &items).await.unwrap();

        let retrieved = get_job_items(&pool, "job-items-test").await.unwrap();
        assert_eq!(retrieved.len(), 5);
    }
}
