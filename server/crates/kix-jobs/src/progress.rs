//! Progress tracking for jobs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

/// Progress information
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Progress {
    /// Current step description
    pub current_step: String,
    /// Current item being processed
    pub current_item: Option<String>,
    /// Number of items processed
    pub processed: usize,
    /// Total items to process (0 if unknown)
    pub total: usize,
    /// Processing rate (items per second)
    pub rate: f64,
    /// Estimated time remaining in seconds
    pub eta_secs: Option<u64>,
    /// Percentage complete (0-100)
    pub percentage: f32,
    /// Detailed metrics
    pub metrics: ProgressMetrics,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

impl Default for Progress {
    fn default() -> Self {
        Self {
            current_step: "Initializing".to_string(),
            current_item: None,
            processed: 0,
            total: 0,
            rate: 0.0,
            eta_secs: None,
            percentage: 0.0,
            metrics: ProgressMetrics::default(),
            updated_at: Utc::now(),
        }
    }
}

/// Detailed progress metrics
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProgressMetrics {
    /// Number of URLs/files discovered
    pub items_discovered: usize,
    /// Number of chunks created
    pub chunks_created: usize,
    /// Number of embeddings generated
    pub embeddings_generated: usize,
    /// Bytes processed
    pub bytes_processed: u64,
    /// Number of errors encountered
    pub errors_encountered: usize,
    /// Number of items skipped
    pub items_skipped: usize,
}

/// Progress tracker for a job
pub struct ProgressTracker {
    processed: AtomicUsize,
    total: AtomicUsize,
    progress: Arc<RwLock<Progress>>,
    start_time: Instant,
    rate_window: RwLock<RateWindow>,
}

/// Sliding window for rate calculation
struct RateWindow {
    samples: Vec<(Instant, usize)>,
    window_size: std::time::Duration,
}

impl RateWindow {
    fn new(window_size: std::time::Duration) -> Self {
        Self {
            samples: Vec::with_capacity(100),
            window_size,
        }
    }

    fn add_sample(&mut self, count: usize) {
        let now = Instant::now();
        self.samples.push((now, count));

        // Remove old samples
        self.samples
            .retain(|(time, _)| now.duration_since(*time) < self.window_size);
    }

    fn calculate_rate(&self) -> f64 {
        if self.samples.len() < 2 {
            return 0.0;
        }

        let first = self.samples.first().unwrap();
        let last = self.samples.last().unwrap();
        let duration = last.0.duration_since(first.0).as_secs_f64();

        if duration > 0.0 {
            let total: usize = self.samples.iter().map(|(_, c)| c).sum();
            total as f64 / duration
        } else {
            0.0
        }
    }
}

impl ProgressTracker {
    /// Create a new progress tracker
    pub fn new() -> Self {
        Self {
            processed: AtomicUsize::new(0),
            total: AtomicUsize::new(0),
            progress: Arc::new(RwLock::new(Progress::default())),
            start_time: Instant::now(),
            rate_window: RwLock::new(RateWindow::new(std::time::Duration::from_secs(60))),
        }
    }

    /// Set total items count
    pub fn set_total(&self, total: usize) {
        self.total.store(total, Ordering::SeqCst);
    }

    /// Increment processed count
    pub async fn increment(&self, count: usize) {
        let _new_processed = self.processed.fetch_add(count, Ordering::SeqCst) + count;

        // Update rate window
        {
            let mut window = self.rate_window.write().await;
            window.add_sample(count);
        }

        self.update_progress().await;
    }

    /// Update current step
    pub async fn set_step(&self, step: impl Into<String>) {
        let mut progress = self.progress.write().await;
        progress.current_step = step.into();
        progress.updated_at = Utc::now();
    }

    /// Update current item
    pub async fn set_current_item(&self, item: Option<String>) {
        let mut progress = self.progress.write().await;
        progress.current_item = item;
        progress.updated_at = Utc::now();
    }

    /// Update metrics
    pub async fn update_metrics<F>(&self, updater: F)
    where
        F: FnOnce(&mut ProgressMetrics),
    {
        let mut progress = self.progress.write().await;
        updater(&mut progress.metrics);
        progress.updated_at = Utc::now();
    }

    /// Get current progress
    pub async fn get_progress(&self) -> Progress {
        self.progress.read().await.clone()
    }

    /// Update progress calculations
    async fn update_progress(&self) {
        let processed = self.processed.load(Ordering::SeqCst);
        let total = self.total.load(Ordering::SeqCst);

        let rate = {
            let window = self.rate_window.read().await;
            window.calculate_rate()
        };

        let percentage = if total > 0 {
            (processed as f32 / total as f32) * 100.0
        } else {
            0.0
        };

        let eta_secs = if rate > 0.0 && total > processed {
            Some(((total - processed) as f64 / rate) as u64)
        } else {
            None
        };

        let mut progress = self.progress.write().await;
        progress.processed = processed;
        progress.total = total;
        progress.rate = rate;
        progress.percentage = percentage;
        progress.eta_secs = eta_secs;
        progress.updated_at = Utc::now();
    }

    /// Get elapsed time
    pub fn elapsed(&self) -> std::time::Duration {
        self.start_time.elapsed()
    }
}

impl Default for ProgressTracker {
    fn default() -> Self {
        Self::new()
    }
}