//! Job queue management

use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::sync::Arc;

use dashmap::DashMap;
use tokio::sync::{Mutex, Notify};
use tracing::{debug, info};
use uuid::Uuid;

use crate::job::{Job, JobState, JobSummary};
use crate::JobError;

/// Configuration for job queue
#[derive(Clone, Debug)]
pub struct QueueConfig {
    /// Maximum jobs in queue
    pub max_queue_size: usize,
    /// Maximum concurrent running jobs
    pub max_concurrent: usize,
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            max_queue_size: 1000,
            max_concurrent: 8,  // Increased from 4 to match executor's 8 workers
        }
    }
}

/// Wrapper for priority queue ordering
struct PrioritizedJob {
    job: Arc<Job>,
    priority: u8,
    created_at: chrono::DateTime<chrono::Utc>,
}

impl PartialEq for PrioritizedJob {
    fn eq(&self, other: &Self) -> bool {
        self.job.id == other.job.id
    }
}

impl Eq for PrioritizedJob {}

impl PartialOrd for PrioritizedJob {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PrioritizedJob {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher priority first, then older jobs first
        match self.priority.cmp(&other.priority) {
            Ordering::Equal => other.created_at.cmp(&self.created_at),
            ord => ord,
        }
    }
}

/// Job queue for managing pending jobs
pub struct JobQueue {
    /// All jobs (pending, running, completed)
    jobs: DashMap<Uuid, Arc<Job>>,
    /// Priority queue for pending jobs
    pending: Mutex<BinaryHeap<PrioritizedJob>>,
    /// Currently running jobs
    running: DashMap<Uuid, Arc<Job>>,
    /// Notification for new jobs
    notify: Notify,
    /// Configuration
    config: QueueConfig,
}

impl JobQueue {
    /// Create a new job queue
    pub fn new(config: QueueConfig) -> Self {
        Self {
            jobs: DashMap::new(),
            pending: Mutex::new(BinaryHeap::new()),
            running: DashMap::new(),
            notify: Notify::new(),
            config,
        }
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(QueueConfig::default())
    }

    /// Submit a new job to the queue
    pub async fn submit(&self, job: Job) -> Result<Uuid, JobError> {
        let job_id = job.id;
        let priority = job.config.priority;
        let created_at = job.created_at;

        // Check queue size
        let pending = self.pending.lock().await;
        if pending.len() >= self.config.max_queue_size {
            return Err(JobError::ResourceLimit("Queue is full".to_string()));
        }
        drop(pending);

        // Store and queue the job
        let job = Arc::new(job);
        self.jobs.insert(job_id, job.clone());

        let prioritized = PrioritizedJob {
            job,
            priority,
            created_at,
        };

        {
            let mut pending = self.pending.lock().await;
            pending.push(prioritized);
        }

        // Notify waiting executors
        self.notify.notify_one();

        info!(job_id = %job_id, priority = priority, "Job submitted to queue");
        Ok(job_id)
    }

    /// Get the next job to execute
    pub async fn next(&self) -> Option<Arc<Job>> {
        // Check if we can run more jobs
        if self.running.len() >= self.config.max_concurrent {
            return None;
        }

        let mut pending = self.pending.lock().await;
        if let Some(prioritized) = pending.pop() {
            let job = prioritized.job;

            // Update job state
            job.set_state(JobState::Running {
                started_at: chrono::Utc::now(),
                executor_id: format!("executor-{}", uuid::Uuid::new_v4()),
            })
            .await;

            // Track as running
            self.running.insert(job.id, job.clone());

            debug!(job_id = %job.id, "Job dequeued for execution");
            Some(job)
        } else {
            None
        }
    }

    /// Wait for a job to become available
    pub async fn wait_for_job(&self) -> Arc<Job> {
        loop {
            if let Some(job) = self.next().await {
                return job;
            }
            self.notify.notified().await;
        }
    }

    /// Mark a job as completed
    pub async fn complete(&self, job_id: Uuid, result: crate::job::JobResult) {
        if let Some((_, job)) = self.running.remove(&job_id) {
            job.set_state(JobState::Completed {
                finished_at: chrono::Utc::now(),
                result,
            })
            .await;

            info!(job_id = %job_id, "Job completed");
        }

        // Notify for capacity
        self.notify.notify_one();
    }

    /// Mark a job as failed
    pub async fn fail(&self, job_id: Uuid, error: String, retry_count: u32) {
        if let Some((_, job)) = self.running.remove(&job_id) {
            job.set_state(JobState::Failed {
                failed_at: chrono::Utc::now(),
                error,
                retry_count,
            })
            .await;

            info!(job_id = %job_id, "Job failed");
        }

        self.notify.notify_one();
    }

    /// Cancel a job
    pub async fn cancel(&self, job_id: Uuid, reason: String) -> Result<(), JobError> {
        if let Some(job) = self.jobs.get(&job_id) {
            job.cancel(&reason);
            job.set_state(JobState::Cancelled {
                cancelled_at: chrono::Utc::now(),
                reason,
            })
            .await;

            // Remove from running if present
            self.running.remove(&job_id);

            info!(job_id = %job_id, "Job cancelled");
            Ok(())
        } else {
            Err(JobError::NotFound(job_id))
        }
    }

    /// Get job by ID
    pub fn get(&self, job_id: Uuid) -> Option<Arc<Job>> {
        self.jobs.get(&job_id).map(|r| r.clone())
    }

    /// Get all job summaries
    pub async fn list(&self) -> Vec<JobSummary> {
        let mut summaries = Vec::with_capacity(self.jobs.len());
        for r in self.jobs.iter() {
            summaries.push(JobSummary::from_job(r.value().as_ref()).await);
        }
        summaries
    }

    /// Get running jobs count
    pub fn running_count(&self) -> usize {
        self.running.len()
    }

    /// Get pending jobs count
    pub async fn pending_count(&self) -> usize {
        self.pending.lock().await.len()
    }

    /// Get total jobs count
    pub fn total_count(&self) -> usize {
        self.jobs.len()
    }
}

impl Default for JobQueue {
    fn default() -> Self {
        Self::with_defaults()
    }
}