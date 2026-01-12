//! SSE event types and serialization

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Type of SSE event
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventType {
    /// Job has started processing
    JobStarted {
        job_id: Uuid,
        source: SourceType,
        total_items: Option<usize>,
        timestamp: DateTime<Utc>,
    },

    /// Progress update for a job
    Progress {
        job_id: Uuid,
        processed: usize,
        total: usize,
        current_item: Option<String>,
        rate: f64, // items per second
        percentage: f32,
    },

    /// Single item processed
    ItemProcessed {
        job_id: Uuid,
        item_path: String,
        chunks_created: usize,
        embeddings_generated: usize,
        duration_ms: u64,
    },

    /// Error occurred during processing
    Error {
        job_id: Uuid,
        item_path: Option<String>,
        error_message: String,
        recoverable: bool,
    },

    /// Job completed successfully
    JobCompleted {
        job_id: Uuid,
        total_processed: usize,
        total_chunks: usize,
        duration_ms: u64,
        errors: Vec<String>,
    },

    /// Job was cancelled
    JobCancelled {
        job_id: Uuid,
        reason: String,
    },

    /// Job history was updated (job persisted to history store)
    /// Frontend should refresh job history when receiving this event.
    JobHistoryUpdated {
        job_id: Uuid,
        status: String,
    },

    /// Item (URL/file) was discovered and added to processing queue
    ItemDiscovered {
        job_id: Uuid,
        item_path: String,
        discovered_at: DateTime<Utc>,
        /// Parent URL that contained this link (for crawls)
        parent_url: Option<String>,
        /// Depth in crawl tree (0 for seeds)
        depth: usize,
    },

    /// Item processing has started
    ItemStarted {
        job_id: Uuid,
        item_path: String,
        started_at: DateTime<Utc>,
    },

    /// Heartbeat to keep connection alive
    Heartbeat {
        timestamp: DateTime<Utc>,
    },
}

/// Source type for indexing
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    Url { url: String, depth: usize },
    File { path: String },
    Directory { path: String, recursive: bool },
}

/// SSE event wrapper
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub data: EventType,
}

impl Event {
    /// Create a new event
    pub fn new(data: EventType) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            data,
        }
    }

    /// Convert to SSE format
    pub fn to_sse_data(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&self)
    }
}