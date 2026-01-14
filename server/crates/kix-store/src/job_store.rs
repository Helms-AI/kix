//! Job history store wrapper.
//!
//! This module provides a `JobStore` that wraps `SqliteStore` for backward compatibility.
//! All job operations are performed through the unified SQLite database.

use crate::error::StoreError;
use kix_sqlite::{JobItemRecord, JobRecord, SqliteStore};
use std::path::Path;

/// Job history store.
///
/// Wraps `SqliteStore` to provide job-specific operations.
/// This exists for backward compatibility with code that expects a separate JobStore.
pub struct JobStore {
    sqlite: SqliteStore,
}

/// Job statistics for a single job execution.
#[derive(Debug, Clone, Default)]
pub struct JobStats {
    /// Number of items processed
    pub items_processed: usize,
    /// Number of items discovered
    pub items_discovered: usize,
    /// Number of chunks created
    pub chunks_created: usize,
    /// Number of embeddings generated
    pub embeddings_generated: usize,
    /// Number of errors
    pub error_count: usize,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Processing rate (items/second)
    pub processing_rate: f64,
}

/// Aggregate job statistics.
#[derive(Debug, Clone, Default)]
pub struct JobAggregateStats {
    /// Total jobs
    pub total_jobs: usize,
    /// Completed jobs
    pub completed_jobs: usize,
    /// Failed jobs
    pub failed_jobs: usize,
    /// Running jobs
    pub running_jobs: usize,
}

impl JobStore {
    /// Create a new job store at the given SQLite database path.
    pub async fn new(db_path: &str) -> Result<Self, StoreError> {
        let path = Path::new(db_path);
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                StoreError::Database(format!("Failed to create parent directory: {}", e))
            })?;
        }

        let sqlite = SqliteStore::new(path).await.map_err(|e| {
            StoreError::Database(format!("Failed to create SQLite store: {}", e))
        })?;

        Ok(Self { sqlite })
    }

    /// Create a job store from an existing SqliteStore.
    pub fn from_sqlite(sqlite: SqliteStore) -> Self {
        Self { sqlite }
    }

    /// Initialize tables (no-op as SqliteStore handles migrations).
    pub async fn init_tables(&mut self) -> Result<(), StoreError> {
        // Tables are created during SqliteStore::new() via migrations
        Ok(())
    }

    /// Insert a job record.
    pub async fn insert_job(&self, job: &JobRecord) -> Result<(), StoreError> {
        self.sqlite.insert_job(job).await.map_err(|e| {
            StoreError::Database(format!("Failed to insert job: {}", e))
        })
    }

    /// Save (insert or update) a job record with its items.
    /// Alias for insert_job for backward compatibility.
    pub async fn save_job(&self, job: &JobRecord) -> Result<(), StoreError> {
        self.insert_job(job).await
    }

    /// Save a job with associated items.
    pub async fn save_job_with_items(&self, job: &JobRecord, items: &[JobItemRecord]) -> Result<(), StoreError> {
        self.insert_job(job).await?;
        if !items.is_empty() {
            self.insert_job_items(items).await?;
        }
        Ok(())
    }

    /// Get a job by ID.
    pub async fn get_job(&self, job_id: &str) -> Result<Option<JobRecord>, StoreError> {
        self.sqlite.get_job(job_id).await.map_err(|e| {
            StoreError::Database(format!("Failed to get job: {}", e))
        })
    }

    /// List jobs with optional status filter.
    pub async fn list_jobs(
        &self,
        status: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<JobRecord>, StoreError> {
        self.sqlite.list_jobs(status, limit, offset).await.map_err(|e| {
            StoreError::Database(format!("Failed to list jobs: {}", e))
        })
    }

    /// Insert job items.
    pub async fn insert_job_items(&self, items: &[JobItemRecord]) -> Result<(), StoreError> {
        self.sqlite.insert_job_items(items).await.map_err(|e| {
            StoreError::Database(format!("Failed to insert job items: {}", e))
        })
    }

    /// Get job items for a job.
    pub async fn get_job_items(&self, job_id: &str) -> Result<Vec<JobItemRecord>, StoreError> {
        self.sqlite.get_job_items(job_id).await.map_err(|e| {
            StoreError::Database(format!("Failed to get job items: {}", e))
        })
    }

    /// Get aggregate job statistics.
    pub async fn get_aggregate_stats(&self) -> Result<JobAggregateStats, StoreError> {
        let all_jobs = self.sqlite.list_jobs(None, 10000, 0).await.map_err(|e| {
            StoreError::Database(format!("Failed to list jobs for stats: {}", e))
        })?;

        let mut stats = JobAggregateStats::default();
        stats.total_jobs = all_jobs.len();

        for job in &all_jobs {
            match job.status.as_str() {
                "completed" => stats.completed_jobs += 1,
                "failed" => stats.failed_jobs += 1,
                "running" | "in_progress" => stats.running_jobs += 1,
                _ => {}
            }
        }

        Ok(stats)
    }

    /// Close the store.
    pub async fn close(&self) {
        self.sqlite.close().await;
    }
}
