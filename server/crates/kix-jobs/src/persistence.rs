//! Job persistence layer

use async_trait::async_trait;
use std::path::PathBuf;
use uuid::Uuid;

use crate::job::{Job, JobConfig, JobResult, JobState, JobSummary, JobType};
use crate::JobError;

/// Trait for job persistence
#[async_trait]
pub trait JobPersistence: Send + Sync {
    /// Save a job
    async fn save_job(&self, job: &Job) -> Result<(), JobError>;

    /// Load a job by ID
    async fn load_job(&self, id: Uuid) -> Result<Option<Job>, JobError>;

    /// List jobs with optional filters
    async fn list_jobs(&self, filter: JobFilter) -> Result<Vec<JobSummary>, JobError>;

    /// Delete a job
    async fn delete_job(&self, id: Uuid) -> Result<(), JobError>;

    /// Save job result
    async fn save_result(&self, id: Uuid, result: &JobResult) -> Result<(), JobError>;

    /// Load job result
    async fn load_result(&self, id: Uuid) -> Result<Option<JobResult>, JobError>;

    /// Cleanup old jobs
    async fn cleanup(&self, older_than: chrono::Duration) -> Result<usize, JobError>;
}

/// Filter for listing jobs
#[derive(Default, Clone, Debug)]
pub struct JobFilter {
    /// Filter by state
    pub states: Option<Vec<String>>,
    /// Filter by job type
    pub job_types: Option<Vec<String>>,
    /// Limit results
    pub limit: Option<usize>,
    /// Offset for pagination
    pub offset: Option<usize>,
    /// Order by field
    pub order_by: Option<String>,
    /// Order direction
    pub ascending: bool,
}

/// In-memory persistence for development
pub struct MemoryPersistence {
    jobs: dashmap::DashMap<Uuid, JobData>,
    results: dashmap::DashMap<Uuid, JobResult>,
}

#[derive(Clone)]
struct JobData {
    id: Uuid,
    job_type: JobType,
    #[allow(dead_code)]
    config: JobConfig,
    state: JobState,
    created_at: chrono::DateTime<chrono::Utc>,
}

impl MemoryPersistence {
    pub fn new() -> Self {
        Self {
            jobs: dashmap::DashMap::new(),
            results: dashmap::DashMap::new(),
        }
    }
}

impl Default for MemoryPersistence {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl JobPersistence for MemoryPersistence {
    async fn save_job(&self, job: &Job) -> Result<(), JobError> {
        let state = job.get_state().await;
        self.jobs.insert(
            job.id,
            JobData {
                id: job.id,
                job_type: job.job_type.clone(),
                config: job.config.clone(),
                state,
                created_at: job.created_at,
            },
        );
        Ok(())
    }

    async fn load_job(&self, _id: Uuid) -> Result<Option<Job>, JobError> {
        // In-memory doesn't support full job reconstruction
        // This would need state restoration which isn't possible
        // with our current Job structure
        Ok(None)
    }

    async fn list_jobs(&self, filter: JobFilter) -> Result<Vec<JobSummary>, JobError> {
        let mut jobs: Vec<JobSummary> = self
            .jobs
            .iter()
            .map(|r| {
                let data = r.value();
                let job_type = match &data.job_type {
                    JobType::Url { .. } => "url",
                    JobType::FileUpload { .. } => "file_upload",
                    JobType::Reindex { .. } => "reindex",
                };
                let state = match &data.state {
                    JobState::Pending { .. } => "pending",
                    JobState::Queued { .. } => "queued",
                    JobState::Running { .. } => "running",
                    JobState::Completed { .. } => "completed",
                    JobState::Failed { .. } => "failed",
                    JobState::Cancelled { .. } => "cancelled",
                };

                JobSummary {
                    id: data.id,
                    job_type: job_type.to_string(),
                    state: state.to_string(),
                    created_at: data.created_at,
                    progress: None,
                }
            })
            .collect();

        // Apply filters
        if let Some(states) = &filter.states {
            jobs.retain(|j| states.contains(&j.state));
        }
        if let Some(types) = &filter.job_types {
            jobs.retain(|j| types.contains(&j.job_type));
        }

        // Sort
        jobs.sort_by(|a, b| {
            if filter.ascending {
                a.created_at.cmp(&b.created_at)
            } else {
                b.created_at.cmp(&a.created_at)
            }
        });

        // Pagination
        if let Some(offset) = filter.offset {
            jobs = jobs.into_iter().skip(offset).collect();
        }
        if let Some(limit) = filter.limit {
            jobs.truncate(limit);
        }

        Ok(jobs)
    }

    async fn delete_job(&self, id: Uuid) -> Result<(), JobError> {
        self.jobs.remove(&id);
        self.results.remove(&id);
        Ok(())
    }

    async fn save_result(&self, id: Uuid, result: &JobResult) -> Result<(), JobError> {
        self.results.insert(id, result.clone());
        Ok(())
    }

    async fn load_result(&self, id: Uuid) -> Result<Option<JobResult>, JobError> {
        Ok(self.results.get(&id).map(|r| r.clone()))
    }

    async fn cleanup(&self, older_than: chrono::Duration) -> Result<usize, JobError> {
        let cutoff = chrono::Utc::now() - older_than;
        let to_remove: Vec<Uuid> = self
            .jobs
            .iter()
            .filter(|r| r.value().created_at < cutoff)
            .filter(|r| r.value().state.is_terminal())
            .map(|r| *r.key())
            .collect();

        let count = to_remove.len();
        for id in to_remove {
            self.jobs.remove(&id);
            self.results.remove(&id);
        }

        Ok(count)
    }
}

/// SQLite persistence (feature-gated)
#[cfg(feature = "persistence")]
pub mod sqlite {
    use super::*;

    pub struct SqlitePersistence {
        // TODO: Implement SQLite persistence
        _db_path: PathBuf,
    }

    impl SqlitePersistence {
        pub async fn new(db_path: PathBuf) -> Result<Self, JobError> {
            // TODO: Initialize SQLite database
            Ok(Self { _db_path: db_path })
        }
    }
}