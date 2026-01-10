//! Job management and orchestration system

pub mod job;
pub mod queue;
pub mod executor;
pub mod progress;
pub mod persistence;
pub mod processor;
pub mod linker;

pub use job::{Job, JobConfig, JobState, JobType};
pub use queue::{JobQueue, QueueConfig};
pub use executor::{JobExecutor, ExecutorConfig};
pub use progress::{Progress, ProgressTracker};
pub use processor::{ContentProcessor, ProcessorConfig, ProcessingResult};
pub use linker::{PatternLinker, PatternLink, LinkStrength, LinkingConfig, LinkSuggestions};

use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum JobError {
    #[error("Job not found: {0}")]
    NotFound(Uuid),

    #[error("Job already running: {0}")]
    AlreadyRunning(Uuid),

    #[error("Job cancelled: {0}")]
    Cancelled(Uuid),

    #[error("Job failed: {0}")]
    Failed(String),

    #[error("Persistence error: {0}")]
    Persistence(String),

    #[error("Resource limit exceeded: {0}")]
    ResourceLimit(String),

    #[error("Invalid job configuration: {0}")]
    InvalidConfig(String),

    #[error("Processing error: {0}")]
    Processing(String),
}