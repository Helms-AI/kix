//! Job Store - Job History Persistence
//!
//! Implements job history persistence using LanceDB.
//!
//! ## Architecture
//!
//! - **Jobs**: Store completed job metadata and aggregated statistics
//! - **Job Items**: Store per-page/file details for detailed job history
//!
//! Jobs are persisted when they complete (success, failure, or cancellation)
//! and can be queried for historical analysis.

use arrow_array::{
    Array, ArrayRef, Float64Array, RecordBatch, RecordBatchIterator, StringArray, UInt32Array,
    UInt64Array,
};
use arrow_schema::ArrowError;
use chrono::{DateTime, Utc};
use futures::TryStreamExt;
use lancedb::query::{ExecutableQuery, QueryBase};
use lancedb::{connect, Connection, Table};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;

use crate::error::StoreError;
use crate::schema::{job_item_schema, job_schema};

// ============================================================================
// Job Record Types
// ============================================================================

/// A persisted job record with metadata and aggregated statistics.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JobRecord {
    /// Unique job identifier (UUID)
    pub job_id: String,

    /// Type of job: "url", "file_upload", "reindex"
    pub job_type: String,

    /// Final status: "completed", "failed", "cancelled"
    pub status: String,

    /// Job creation timestamp
    pub created_at: DateTime<Utc>,

    /// Job start timestamp
    pub started_at: Option<DateTime<Utc>>,

    /// Job completion timestamp
    pub completed_at: DateTime<Utc>,

    /// Source URL (for URL jobs)
    pub source_url: Option<String>,

    /// Extracted domain (for URL jobs)
    pub source_domain: Option<String>,

    /// Job configuration as JSON
    pub config: serde_json::Value,

    /// Aggregated statistics
    pub stats: JobStats,

    /// List of error messages
    pub errors: Vec<String>,
}

/// Aggregated job statistics.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct JobStats {
    /// Number of items successfully processed
    pub items_processed: u32,

    /// Number of items discovered (for URL crawls)
    pub items_discovered: u32,

    /// Total chunks created
    pub chunks_created: u32,

    /// Total embeddings generated
    pub embeddings_generated: u32,

    /// Number of errors encountered
    pub error_count: u32,

    /// Total duration in milliseconds
    pub duration_ms: u64,

    /// Processing rate (items per second)
    pub processing_rate: f64,
}

/// A per-item record for job history (page or file).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JobItemRecord {
    /// Unique item identifier (UUID)
    pub item_id: String,

    /// FK to parent job
    pub job_id: String,

    /// URL or file path
    pub item_path: String,

    /// Type: "page", "file"
    pub item_type: String,

    /// Status: "completed", "error", "skipped"
    pub status: String,

    /// Parent URL (for crawled pages)
    pub parent_url: Option<String>,

    /// Crawl depth
    pub depth: u32,

    /// When the item was discovered
    pub discovered_at: DateTime<Utc>,

    /// When processing started
    pub started_at: Option<DateTime<Utc>>,

    /// When processing completed
    pub completed_at: Option<DateTime<Utc>>,

    /// Number of chunks created
    pub chunks_created: u32,

    /// Number of embeddings generated
    pub embeddings_generated: u32,

    /// Processing duration in milliseconds
    pub duration_ms: u64,

    /// Error message if failed
    pub error_message: Option<String>,
}

impl JobItemRecord {
    /// Create a new job item record for a discovered URL.
    pub fn new_page(job_id: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            item_id: uuid::Uuid::new_v4().to_string(),
            job_id: job_id.into(),
            item_path: url.into(),
            item_type: "page".to_string(),
            status: "pending".to_string(),
            parent_url: None,
            depth: 0,
            discovered_at: Utc::now(),
            started_at: None,
            completed_at: None,
            chunks_created: 0,
            embeddings_generated: 0,
            duration_ms: 0,
            error_message: None,
        }
    }

    /// Create a new job item record for a file.
    pub fn new_file(job_id: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            item_id: uuid::Uuid::new_v4().to_string(),
            job_id: job_id.into(),
            item_path: path.into(),
            item_type: "file".to_string(),
            status: "pending".to_string(),
            parent_url: None,
            depth: 0,
            discovered_at: Utc::now(),
            started_at: None,
            completed_at: None,
            chunks_created: 0,
            embeddings_generated: 0,
            duration_ms: 0,
            error_message: None,
        }
    }

    /// Set parent URL and depth (for crawled pages).
    pub fn with_parent(mut self, parent_url: impl Into<String>, depth: u32) -> Self {
        self.parent_url = Some(parent_url.into());
        self.depth = depth;
        self
    }

    /// Mark as started.
    pub fn mark_started(&mut self) {
        self.started_at = Some(Utc::now());
        self.status = "running".to_string();
    }

    /// Mark as completed with stats.
    pub fn mark_completed(&mut self, chunks: u32, embeddings: u32, duration_ms: u64) {
        self.completed_at = Some(Utc::now());
        self.status = "completed".to_string();
        self.chunks_created = chunks;
        self.embeddings_generated = embeddings;
        self.duration_ms = duration_ms;
    }

    /// Mark as failed with error message.
    pub fn mark_error(&mut self, error: impl Into<String>) {
        self.completed_at = Some(Utc::now());
        self.status = "error".to_string();
        self.error_message = Some(error.into());
    }
}

// ============================================================================
// Job Store
// ============================================================================

/// Store for job history persistence.
///
/// Uses a separate LanceDB file (jobs.lance) to store completed jobs
/// and their per-item details.
pub struct JobStore {
    db: Connection,
    jobs_table: Option<Table>,
    items_table: Option<Table>,
}

impl JobStore {
    /// Create a new job store at the specified path.
    ///
    /// The path should point to the jobs.lance directory.
    pub async fn new(db_path: &str) -> Result<Self, StoreError> {
        info!("Connecting to JobStore at: {}", db_path);

        let db = connect(db_path)
            .execute()
            .await
            .map_err(|e| StoreError::Database(e.to_string()))?;

        Ok(Self {
            db,
            jobs_table: None,
            items_table: None,
        })
    }

    /// Initialize tables, creating them if they don't exist.
    pub async fn init_tables(&mut self) -> Result<(), StoreError> {
        self.jobs_table = Some(self.get_or_create_jobs_table().await?);
        self.items_table = Some(self.get_or_create_items_table().await?);

        info!("JobStore tables initialized (jobs, job_items)");
        Ok(())
    }

    /// Get or create the jobs table.
    async fn get_or_create_jobs_table(&self) -> Result<Table, StoreError> {
        let table_names = self
            .db
            .table_names()
            .execute()
            .await
            .map_err(|e| StoreError::Database(e.to_string()))?;

        if table_names.contains(&"jobs".to_string()) {
            info!("Opening existing jobs table");
            self.db
                .open_table("jobs")
                .execute()
                .await
                .map_err(|e| StoreError::Database(e.to_string()))
        } else {
            info!("Creating new jobs table");
            self.db
                .create_empty_table("jobs", job_schema())
                .execute()
                .await
                .map_err(|e| StoreError::Database(e.to_string()))
        }
    }

    /// Get or create the job items table.
    async fn get_or_create_items_table(&self) -> Result<Table, StoreError> {
        let table_names = self
            .db
            .table_names()
            .execute()
            .await
            .map_err(|e| StoreError::Database(e.to_string()))?;

        if table_names.contains(&"job_items".to_string()) {
            info!("Opening existing job_items table");
            self.db
                .open_table("job_items")
                .execute()
                .await
                .map_err(|e| StoreError::Database(e.to_string()))
        } else {
            info!("Creating new job_items table");
            self.db
                .create_empty_table("job_items", job_item_schema())
                .execute()
                .await
                .map_err(|e| StoreError::Database(e.to_string()))
        }
    }

    // ========================================================================
    // Write Operations
    // ========================================================================

    /// Save a completed job with its items.
    ///
    /// This should be called when a job finishes (success, failure, or cancel).
    pub async fn save_job(
        &self,
        job: &JobRecord,
        items: &[JobItemRecord],
    ) -> Result<(), StoreError> {
        // Insert job record
        let jobs_table = self
            .jobs_table
            .as_ref()
            .ok_or_else(|| StoreError::Database("Jobs table not initialized".to_string()))?;

        let job_batch = Self::job_to_batch(job)?;
        let schema = job_batch.schema();
        let batches: Vec<Result<RecordBatch, ArrowError>> = vec![Ok(job_batch)];
        let reader = RecordBatchIterator::new(batches, schema);

        jobs_table
            .add(Box::new(reader))
            .execute()
            .await
            .map_err(|e| StoreError::Database(e.to_string()))?;

        info!("Saved job record: {}", job.job_id);

        // Insert items if any
        if !items.is_empty() {
            let items_table = self
                .items_table
                .as_ref()
                .ok_or_else(|| StoreError::Database("Items table not initialized".to_string()))?;

            let items_batch = Self::items_to_batch(items)?;
            let schema = items_batch.schema();
            let batches: Vec<Result<RecordBatch, ArrowError>> = vec![Ok(items_batch)];
            let reader = RecordBatchIterator::new(batches, schema);

            items_table
                .add(Box::new(reader))
                .execute()
                .await
                .map_err(|e| StoreError::Database(e.to_string()))?;

            info!("Saved {} job items for job: {}", items.len(), job.job_id);
        }

        Ok(())
    }

    // ========================================================================
    // Read Operations
    // ========================================================================

    /// List jobs with pagination, ordered by completed_at descending.
    pub async fn list_jobs(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<JobRecord>, StoreError> {
        let table = self
            .jobs_table
            .as_ref()
            .ok_or_else(|| StoreError::Database("Jobs table not initialized".to_string()))?;

        // Note: LanceDB doesn't support OFFSET directly, so we fetch more and skip
        let fetch_limit = limit + offset;

        let batches = table
            .query()
            .limit(fetch_limit)
            .execute()
            .await
            .map_err(|e| StoreError::Database(e.to_string()))?
            .try_collect::<Vec<_>>()
            .await
            .map_err(|e| StoreError::Database(e.to_string()))?;

        let mut jobs = Vec::new();
        let mut count = 0;

        for batch in &batches {
            for i in 0..batch.num_rows() {
                if count >= offset {
                    jobs.push(Self::batch_to_job(batch, i)?);
                }
                count += 1;
                if jobs.len() >= limit {
                    break;
                }
            }
            if jobs.len() >= limit {
                break;
            }
        }

        // Sort by completed_at descending (most recent first)
        jobs.sort_by(|a, b| b.completed_at.cmp(&a.completed_at));

        Ok(jobs)
    }

    /// Get a job by ID.
    pub async fn get_job(&self, job_id: &str) -> Result<Option<JobRecord>, StoreError> {
        let table = self
            .jobs_table
            .as_ref()
            .ok_or_else(|| StoreError::Database("Jobs table not initialized".to_string()))?;

        let filter = format!("job_id = '{}'", job_id.replace('\'', "''"));

        let batches = table
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

        Self::batch_to_job(&batches[0], 0).map(Some)
    }

    /// Get all items for a job.
    pub async fn get_job_items(&self, job_id: &str) -> Result<Vec<JobItemRecord>, StoreError> {
        let table = self
            .items_table
            .as_ref()
            .ok_or_else(|| StoreError::Database("Items table not initialized".to_string()))?;

        let filter = format!("job_id = '{}'", job_id.replace('\'', "''"));

        let batches = table
            .query()
            .only_if(filter)
            .execute()
            .await
            .map_err(|e| StoreError::Database(e.to_string()))?
            .try_collect::<Vec<_>>()
            .await
            .map_err(|e| StoreError::Database(e.to_string()))?;

        let mut items = Vec::new();
        for batch in &batches {
            for i in 0..batch.num_rows() {
                items.push(Self::batch_to_item(batch, i)?);
            }
        }

        // Sort by discovered_at
        items.sort_by(|a, b| a.discovered_at.cmp(&b.discovered_at));

        Ok(items)
    }

    /// Get total job count.
    pub async fn job_count(&self) -> Result<usize, StoreError> {
        let table = self
            .jobs_table
            .as_ref()
            .ok_or_else(|| StoreError::Database("Jobs table not initialized".to_string()))?;

        table
            .count_rows(None)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))
    }

    /// Delete a job and its items by ID.
    pub async fn delete_job(&self, job_id: &str) -> Result<(), StoreError> {
        let filter = format!("job_id = '{}'", job_id.replace('\'', "''"));

        // Delete items first
        if let Some(items_table) = &self.items_table {
            items_table
                .delete(&filter)
                .await
                .map_err(|e| StoreError::Database(e.to_string()))?;
        }

        // Delete job
        if let Some(jobs_table) = &self.jobs_table {
            jobs_table
                .delete(&filter)
                .await
                .map_err(|e| StoreError::Database(e.to_string()))?;
        }

        info!("Deleted job and items: {}", job_id);
        Ok(())
    }

    // ========================================================================
    // Conversion Helpers
    // ========================================================================

    /// Convert a JobRecord to a RecordBatch.
    fn job_to_batch(job: &JobRecord) -> Result<RecordBatch, StoreError> {
        let errors_json = serde_json::to_string(&job.errors).unwrap_or_else(|_| "[]".to_string());
        let config_json = job.config.to_string();

        let columns: Vec<ArrayRef> = vec![
            Arc::new(StringArray::from(vec![job.job_id.as_str()])),
            Arc::new(StringArray::from(vec![job.job_type.as_str()])),
            Arc::new(StringArray::from(vec![job.status.as_str()])),
            Arc::new(StringArray::from(vec![job.created_at.to_rfc3339()])),
            Arc::new(StringArray::from(vec![job
                .started_at
                .map(|dt| dt.to_rfc3339())])),
            Arc::new(StringArray::from(vec![job.completed_at.to_rfc3339()])),
            Arc::new(StringArray::from(vec![job.source_url.as_deref()])),
            Arc::new(StringArray::from(vec![job.source_domain.as_deref()])),
            Arc::new(StringArray::from(vec![config_json.as_str()])),
            Arc::new(UInt32Array::from(vec![job.stats.items_processed])),
            Arc::new(UInt32Array::from(vec![job.stats.items_discovered])),
            Arc::new(UInt32Array::from(vec![job.stats.chunks_created])),
            Arc::new(UInt32Array::from(vec![job.stats.embeddings_generated])),
            Arc::new(UInt32Array::from(vec![job.stats.error_count])),
            Arc::new(UInt64Array::from(vec![job.stats.duration_ms])),
            Arc::new(Float64Array::from(vec![job.stats.processing_rate])),
            Arc::new(StringArray::from(vec![Some(errors_json.as_str())])),
        ];

        RecordBatch::try_new(job_schema(), columns)
            .map_err(|e| StoreError::Serialization(e.to_string()))
    }

    /// Convert job items to a RecordBatch.
    fn items_to_batch(items: &[JobItemRecord]) -> Result<RecordBatch, StoreError> {
        let item_ids: Vec<&str> = items.iter().map(|i| i.item_id.as_str()).collect();
        let job_ids: Vec<&str> = items.iter().map(|i| i.job_id.as_str()).collect();
        let paths: Vec<&str> = items.iter().map(|i| i.item_path.as_str()).collect();
        let types: Vec<&str> = items.iter().map(|i| i.item_type.as_str()).collect();
        let statuses: Vec<&str> = items.iter().map(|i| i.status.as_str()).collect();
        let parents: Vec<Option<&str>> = items.iter().map(|i| i.parent_url.as_deref()).collect();
        let depths: Vec<u32> = items.iter().map(|i| i.depth).collect();
        let discovered: Vec<String> = items.iter().map(|i| i.discovered_at.to_rfc3339()).collect();
        let started: Vec<Option<String>> = items
            .iter()
            .map(|i| i.started_at.map(|dt| dt.to_rfc3339()))
            .collect();
        let completed: Vec<Option<String>> = items
            .iter()
            .map(|i| i.completed_at.map(|dt| dt.to_rfc3339()))
            .collect();
        let chunks: Vec<u32> = items.iter().map(|i| i.chunks_created).collect();
        let embeddings: Vec<u32> = items.iter().map(|i| i.embeddings_generated).collect();
        let durations: Vec<u64> = items.iter().map(|i| i.duration_ms).collect();
        let errors: Vec<Option<&str>> = items.iter().map(|i| i.error_message.as_deref()).collect();

        let columns: Vec<ArrayRef> = vec![
            Arc::new(StringArray::from(item_ids)),
            Arc::new(StringArray::from(job_ids)),
            Arc::new(StringArray::from(paths)),
            Arc::new(StringArray::from(types)),
            Arc::new(StringArray::from(statuses)),
            Arc::new(StringArray::from(parents)),
            Arc::new(UInt32Array::from(depths)),
            Arc::new(StringArray::from(
                discovered.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                started
                    .iter()
                    .map(|s| s.as_deref())
                    .collect::<Vec<Option<&str>>>(),
            )),
            Arc::new(StringArray::from(
                completed
                    .iter()
                    .map(|s| s.as_deref())
                    .collect::<Vec<Option<&str>>>(),
            )),
            Arc::new(UInt32Array::from(chunks)),
            Arc::new(UInt32Array::from(embeddings)),
            Arc::new(UInt64Array::from(durations)),
            Arc::new(StringArray::from(errors)),
        ];

        RecordBatch::try_new(job_item_schema(), columns)
            .map_err(|e| StoreError::Serialization(e.to_string()))
    }

    /// Convert a RecordBatch row to a JobRecord.
    fn batch_to_job(batch: &RecordBatch, row: usize) -> Result<JobRecord, StoreError> {
        let job_id = Self::get_string(batch, "job_id", row);
        let job_type = Self::get_string(batch, "job_type", row);
        let status = Self::get_string(batch, "status", row);
        let created_at = Self::get_datetime(batch, "created_at", row);
        let started_at = Self::get_optional_datetime(batch, "started_at", row);
        let completed_at = Self::get_datetime(batch, "completed_at", row);
        let source_url = Self::get_optional_string(batch, "source_url", row);
        let source_domain = Self::get_optional_string(batch, "source_domain", row);
        let config_str = Self::get_string(batch, "config", row);
        let config: serde_json::Value =
            serde_json::from_str(&config_str).unwrap_or(serde_json::json!({}));

        let items_processed = Self::get_u32(batch, "items_processed", row);
        let items_discovered = Self::get_u32(batch, "items_discovered", row);
        let chunks_created = Self::get_u32(batch, "chunks_created", row);
        let embeddings_generated = Self::get_u32(batch, "embeddings_generated", row);
        let error_count = Self::get_u32(batch, "error_count", row);
        let duration_ms = Self::get_u64(batch, "duration_ms", row);
        let processing_rate = Self::get_f64(batch, "processing_rate", row);

        let errors_str = Self::get_optional_string(batch, "errors", row).unwrap_or_default();
        let errors: Vec<String> = serde_json::from_str(&errors_str).unwrap_or_default();

        Ok(JobRecord {
            job_id,
            job_type,
            status,
            created_at,
            started_at,
            completed_at,
            source_url,
            source_domain,
            config,
            stats: JobStats {
                items_processed,
                items_discovered,
                chunks_created,
                embeddings_generated,
                error_count,
                duration_ms,
                processing_rate,
            },
            errors,
        })
    }

    /// Convert a RecordBatch row to a JobItemRecord.
    fn batch_to_item(batch: &RecordBatch, row: usize) -> Result<JobItemRecord, StoreError> {
        Ok(JobItemRecord {
            item_id: Self::get_string(batch, "item_id", row),
            job_id: Self::get_string(batch, "job_id", row),
            item_path: Self::get_string(batch, "item_path", row),
            item_type: Self::get_string(batch, "item_type", row),
            status: Self::get_string(batch, "status", row),
            parent_url: Self::get_optional_string(batch, "parent_url", row),
            depth: Self::get_u32(batch, "depth", row),
            discovered_at: Self::get_datetime(batch, "discovered_at", row),
            started_at: Self::get_optional_datetime(batch, "started_at", row),
            completed_at: Self::get_optional_datetime(batch, "completed_at", row),
            chunks_created: Self::get_u32(batch, "chunks_created", row),
            embeddings_generated: Self::get_u32(batch, "embeddings_generated", row),
            duration_ms: Self::get_u64(batch, "duration_ms", row),
            error_message: Self::get_optional_string(batch, "error_message", row),
        })
    }

    // Helper methods for extracting values from RecordBatch
    fn get_string(batch: &RecordBatch, col: &str, row: usize) -> String {
        batch
            .column_by_name(col)
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
            .map(|a| a.value(row).to_string())
            .unwrap_or_default()
    }

    fn get_optional_string(batch: &RecordBatch, col: &str, row: usize) -> Option<String> {
        batch
            .column_by_name(col)
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
            .and_then(|a| {
                if a.is_null(row) {
                    None
                } else {
                    Some(a.value(row).to_string())
                }
            })
    }

    fn get_u32(batch: &RecordBatch, col: &str, row: usize) -> u32 {
        batch
            .column_by_name(col)
            .and_then(|c| c.as_any().downcast_ref::<UInt32Array>())
            .map(|a| a.value(row))
            .unwrap_or(0)
    }

    fn get_u64(batch: &RecordBatch, col: &str, row: usize) -> u64 {
        batch
            .column_by_name(col)
            .and_then(|c| c.as_any().downcast_ref::<UInt64Array>())
            .map(|a| a.value(row))
            .unwrap_or(0)
    }

    fn get_f64(batch: &RecordBatch, col: &str, row: usize) -> f64 {
        batch
            .column_by_name(col)
            .and_then(|c| c.as_any().downcast_ref::<Float64Array>())
            .map(|a| a.value(row))
            .unwrap_or(0.0)
    }

    fn get_datetime(batch: &RecordBatch, col: &str, row: usize) -> DateTime<Utc> {
        batch
            .column_by_name(col)
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
            .and_then(|a| DateTime::parse_from_rfc3339(a.value(row)).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(Utc::now)
    }

    fn get_optional_datetime(batch: &RecordBatch, col: &str, row: usize) -> Option<DateTime<Utc>> {
        batch
            .column_by_name(col)
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
            .and_then(|a| {
                if a.is_null(row) {
                    None
                } else {
                    DateTime::parse_from_rfc3339(a.value(row))
                        .ok()
                        .map(|dt| dt.with_timezone(&Utc))
                }
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
    fn test_job_item_record_creation() {
        let item = JobItemRecord::new_page("job-123", "https://example.com/page");

        assert!(!item.item_id.is_empty());
        assert_eq!(item.job_id, "job-123");
        assert_eq!(item.item_path, "https://example.com/page");
        assert_eq!(item.item_type, "page");
        assert_eq!(item.status, "pending");
    }

    #[test]
    fn test_job_item_with_parent() {
        let item = JobItemRecord::new_page("job-123", "https://example.com/child")
            .with_parent("https://example.com", 1);

        assert_eq!(item.parent_url, Some("https://example.com".to_string()));
        assert_eq!(item.depth, 1);
    }

    #[test]
    fn test_job_item_mark_completed() {
        let mut item = JobItemRecord::new_page("job-123", "https://example.com");
        item.mark_started();
        assert!(item.started_at.is_some());
        assert_eq!(item.status, "running");

        item.mark_completed(10, 10, 1500);
        assert!(item.completed_at.is_some());
        assert_eq!(item.status, "completed");
        assert_eq!(item.chunks_created, 10);
        assert_eq!(item.duration_ms, 1500);
    }

    #[test]
    fn test_job_item_mark_error() {
        let mut item = JobItemRecord::new_page("job-123", "https://example.com");
        item.mark_error("Connection timeout");

        assert_eq!(item.status, "error");
        assert_eq!(item.error_message, Some("Connection timeout".to_string()));
    }

    #[test]
    fn test_job_stats_default() {
        let stats = JobStats::default();
        assert_eq!(stats.items_processed, 0);
        assert_eq!(stats.duration_ms, 0);
        assert_eq!(stats.processing_rate, 0.0);
    }
}
