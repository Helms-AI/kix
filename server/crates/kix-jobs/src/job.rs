//! Job definition and state management

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

/// Job state enumeration
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum JobState {
    /// Job is waiting to be processed
    Pending {
        created_at: DateTime<Utc>,
    },
    /// Job is queued for execution
    Queued {
        queued_at: DateTime<Utc>,
        priority: u8,
    },
    /// Job is currently running
    Running {
        started_at: DateTime<Utc>,
        executor_id: String,
    },
    /// Job completed successfully
    Completed {
        finished_at: DateTime<Utc>,
        result: JobResult,
    },
    /// Job failed with error
    Failed {
        failed_at: DateTime<Utc>,
        error: String,
        retry_count: u32,
    },
    /// Job was cancelled
    Cancelled {
        cancelled_at: DateTime<Utc>,
        reason: String,
    },
}

impl JobState {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            JobState::Completed { .. } | JobState::Failed { .. } | JobState::Cancelled { .. }
        )
    }

    pub fn is_running(&self) -> bool {
        matches!(self, JobState::Running { .. })
    }

    /// Get the state name as a string
    pub fn name(&self) -> &'static str {
        match self {
            JobState::Pending { .. } => "pending",
            JobState::Queued { .. } => "queued",
            JobState::Running { .. } => "running",
            JobState::Completed { .. } => "completed",
            JobState::Failed { .. } => "failed",
            JobState::Cancelled { .. } => "cancelled",
        }
    }
}

/// Job result data
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct JobResult {
    pub items_processed: usize,
    pub chunks_created: usize,
    pub embeddings_generated: usize,
    pub errors: Vec<String>,
    pub duration_ms: u64,
}

/// Type of indexing job
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum JobType {
    /// Index a URL with optional depth crawling
    Url {
        url: String,
        depth: usize,
        respect_robots: bool,
        render_js: bool,
    },
    /// Index uploaded files
    FileUpload {
        /// Temporary file paths for uploaded files
        file_paths: Vec<PathBuf>,
        /// Original file names
        file_names: Vec<String>,
        /// Whether to extract ZIP files
        extract_archives: bool,
    },
    /// Re-index existing content
    Reindex {
        collection_id: String,
        filters: Option<ReindexFilters>,
    },
}

/// Filters for re-indexing
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReindexFilters {
    pub pattern_types: Option<Vec<String>>,
    pub categories: Option<Vec<String>>,
    pub modified_after: Option<DateTime<Utc>>,
}

/// Job configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JobConfig {
    /// Maximum items to process (0 = unlimited)
    pub max_items: usize,
    /// Priority (higher = more urgent)
    pub priority: u8,
    /// Timeout in seconds (0 = no timeout)
    pub timeout_secs: u64,
    /// Maximum retries on failure
    pub max_retries: u32,
    /// Rate limit (items per second, 0 = unlimited)
    pub rate_limit: f64,
    /// Generate embeddings during indexing
    pub generate_embeddings: bool,
    /// Store in database
    pub persist_results: bool,
    /// Link to existing patterns
    pub link_patterns: bool,
}

impl Default for JobConfig {
    fn default() -> Self {
        Self {
            max_items: 0,
            priority: 5,
            timeout_secs: 3600, // 1 hour
            max_retries: 3,
            rate_limit: 0.0,
            generate_embeddings: true,
            persist_results: true,
            link_patterns: true,
        }
    }
}

/// A job to be executed
#[derive(Debug)]
pub struct Job {
    pub id: Uuid,
    pub job_type: JobType,
    pub config: JobConfig,
    pub state: Arc<RwLock<JobState>>,
    pub cancellation_token: CancellationToken,
    pub created_at: DateTime<Utc>,
}

impl Job {
    /// Create a new job
    pub fn new(job_type: JobType, config: JobConfig) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            job_type,
            config,
            state: Arc::new(RwLock::new(JobState::Pending { created_at: now })),
            cancellation_token: CancellationToken::new(),
            created_at: now,
        }
    }

    /// Create a URL indexing job
    pub fn url(url: impl Into<String>, depth: usize) -> Self {
        Self::new(
            JobType::Url {
                url: url.into(),
                depth,
                respect_robots: true,
                render_js: false,
            },
            JobConfig::default(),
        )
    }

    /// Create a file upload indexing job
    pub fn file_upload(
        file_paths: Vec<PathBuf>,
        file_names: Vec<String>,
        extract_archives: bool,
    ) -> Self {
        Self::new(
            JobType::FileUpload {
                file_paths,
                file_names,
                extract_archives,
            },
            JobConfig::default(),
        )
    }

    /// Check if job is cancelled
    pub fn is_cancelled(&self) -> bool {
        self.cancellation_token.is_cancelled()
    }

    /// Cancel the job
    pub fn cancel(&self, _reason: impl Into<String>) {
        self.cancellation_token.cancel();
        // State update handled by executor
    }

    /// Get current state
    pub async fn get_state(&self) -> JobState {
        self.state.read().await.clone()
    }

    /// Update state
    pub async fn set_state(&self, new_state: JobState) {
        let mut state = self.state.write().await;
        *state = new_state;
    }
}

/// Serializable job summary
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JobSummary {
    pub id: Uuid,
    pub job_type: String,
    pub state: String,
    pub created_at: DateTime<Utc>,
    pub progress: Option<crate::progress::Progress>,
}

impl JobSummary {
    /// Create a JobSummary from a Job, reading the actual state asynchronously
    pub async fn from_job(job: &Job) -> Self {
        let job_type = match &job.job_type {
            JobType::Url { .. } => "url",
            JobType::FileUpload { .. } => "file_upload",
            JobType::Reindex { .. } => "reindex",
        };

        let state = job.get_state().await;

        Self {
            id: job.id,
            job_type: job_type.to_string(),
            state: state.name().to_string(),
            created_at: job.created_at,
            progress: None,
        }
    }
}