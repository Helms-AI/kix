//! Stage-based Progress Tracking
//!
//! Implements progress tracking with stage-based mapping for
//! accurate, monotonic progress reporting across crawling pipeline stages.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

/// Crawling pipeline stages for progress tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CrawlStage {
    /// Initial setup and configuration (0-5%)
    Starting,
    /// URL discovery via llms.txt, sitemap, robots.txt (5-15%)
    Discovery,
    /// Analyzing discovered URLs and building crawl plan (15-25%)
    Analyzing,
    /// Active crawling of pages (25-60%)
    Crawling,
    /// Processing crawled content (60-75%)
    Processing,
    /// Creating source records in database (75-80%)
    SourceCreation,
    /// Storing documents and chunks (80-90%)
    DocumentStorage,
    /// Extracting and validating code blocks (90-95%)
    CodeExtraction,
    /// Final cleanup and summary (95-100%)
    Finalization,
}

impl CrawlStage {
    /// Get a human-readable name for the stage
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Starting => "Starting",
            Self::Discovery => "Discovering URLs",
            Self::Analyzing => "Analyzing",
            Self::Crawling => "Crawling pages",
            Self::Processing => "Processing content",
            Self::SourceCreation => "Creating sources",
            Self::DocumentStorage => "Storing documents",
            Self::CodeExtraction => "Extracting code",
            Self::Finalization => "Finalizing",
        }
    }

    /// Get the stage range as (start%, end%)
    pub fn range(&self) -> (u8, u8) {
        match self {
            Self::Starting => (0, 5),
            Self::Discovery => (5, 15),
            Self::Analyzing => (15, 25),
            Self::Crawling => (25, 60),
            Self::Processing => (60, 75),
            Self::SourceCreation => (75, 80),
            Self::DocumentStorage => (80, 90),
            Self::CodeExtraction => (90, 95),
            Self::Finalization => (95, 100),
        }
    }

    /// Get all stages in order
    pub fn all() -> &'static [CrawlStage] {
        &[
            Self::Starting,
            Self::Discovery,
            Self::Analyzing,
            Self::Crawling,
            Self::Processing,
            Self::SourceCreation,
            Self::DocumentStorage,
            Self::CodeExtraction,
            Self::Finalization,
        ]
    }
}

/// Progress tracking with stage mapping and monotonicity enforcement
#[derive(Debug)]
pub struct ProgressTracker {
    /// Pre-computed stage ranges for fast lookup
    stage_ranges: HashMap<CrawlStage, (u8, u8)>,
    /// Last reported overall progress (for monotonicity)
    last_reported: AtomicU8,
    /// Current stage
    current_stage: parking_lot::RwLock<CrawlStage>,
    /// Current stage progress (0-100)
    stage_progress: AtomicU8,
    /// Total pages discovered
    pages_total: parking_lot::RwLock<Option<u32>>,
    /// Pages processed so far
    pages_processed: parking_lot::RwLock<u32>,
    /// Current message
    message: parking_lot::RwLock<String>,
}

impl Default for ProgressTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl ProgressTracker {
    /// Create a new progress tracker
    pub fn new() -> Self {
        let mut stage_ranges = HashMap::new();
        for stage in CrawlStage::all() {
            stage_ranges.insert(*stage, stage.range());
        }

        Self {
            stage_ranges,
            last_reported: AtomicU8::new(0),
            current_stage: parking_lot::RwLock::new(CrawlStage::Starting),
            stage_progress: AtomicU8::new(0),
            pages_total: parking_lot::RwLock::new(None),
            pages_processed: parking_lot::RwLock::new(0),
            message: parking_lot::RwLock::new(String::new()),
        }
    }

    /// Set the current stage
    pub fn set_stage(&self, stage: CrawlStage) {
        *self.current_stage.write() = stage;
        self.stage_progress.store(0, Ordering::SeqCst);
        *self.message.write() = stage.display_name().to_string();
    }

    /// Update progress within the current stage (0-100)
    pub fn set_stage_progress(&self, progress: u8) {
        let clamped = progress.min(100);
        self.stage_progress.store(clamped, Ordering::SeqCst);
    }

    /// Set a custom message
    pub fn set_message(&self, message: impl Into<String>) {
        *self.message.write() = message.into();
    }

    /// Set the total number of pages
    pub fn set_pages_total(&self, total: u32) {
        *self.pages_total.write() = Some(total);
    }

    /// Increment pages processed
    pub fn increment_pages_processed(&self) {
        let mut processed = self.pages_processed.write();
        *processed += 1;

        // Auto-update stage progress if in crawling stage
        if *self.current_stage.read() == CrawlStage::Crawling {
            if let Some(total) = *self.pages_total.read() {
                if total > 0 {
                    let pct = ((*processed as f64 / total as f64) * 100.0) as u8;
                    self.stage_progress.store(pct.min(100), Ordering::SeqCst);
                }
            }
        }
    }

    /// Map stage progress to overall progress (0-100)
    ///
    /// Implements the ProgressMapper pattern with monotonicity enforcement:
    /// - Maps stage-local progress to overall progress
    /// - Never reports progress lower than previously reported
    pub fn map_progress(&self, stage: CrawlStage, stage_pct: u8) -> u8 {
        let (start, end) = self.stage_ranges[&stage];
        let range = end - start;
        let mapped = start + ((range as f32 * (stage_pct.min(100) as f32 / 100.0)) as u8);

        // Monotonicity: never go backwards
        loop {
            let last = self.last_reported.load(Ordering::SeqCst);
            if mapped <= last {
                return last;
            }
            if self
                .last_reported
                .compare_exchange(last, mapped, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                return mapped;
            }
        }
    }

    /// Get the current overall progress (0-100)
    pub fn get_overall_progress(&self) -> u8 {
        let stage = *self.current_stage.read();
        let stage_pct = self.stage_progress.load(Ordering::SeqCst);
        self.map_progress(stage, stage_pct)
    }

    /// Get current progress snapshot
    pub fn snapshot(&self) -> ProgressSnapshot {
        let stage = *self.current_stage.read();
        let stage_progress = self.stage_progress.load(Ordering::SeqCst);
        let overall_progress = self.map_progress(stage, stage_progress);

        ProgressSnapshot {
            stage,
            stage_name: stage.display_name().to_string(),
            stage_progress,
            overall_progress,
            message: self.message.read().clone(),
            pages_processed: *self.pages_processed.read(),
            pages_total: *self.pages_total.read(),
        }
    }

    /// Reset tracker for a new crawl
    pub fn reset(&self) {
        *self.current_stage.write() = CrawlStage::Starting;
        self.stage_progress.store(0, Ordering::SeqCst);
        self.last_reported.store(0, Ordering::SeqCst);
        *self.pages_total.write() = None;
        *self.pages_processed.write() = 0;
        *self.message.write() = String::new();
    }
}

/// A snapshot of current progress state
#[derive(Debug, Clone)]
pub struct ProgressSnapshot {
    /// Current stage
    pub stage: CrawlStage,
    /// Human-readable stage name
    pub stage_name: String,
    /// Progress within current stage (0-100)
    pub stage_progress: u8,
    /// Overall progress (0-100)
    pub overall_progress: u8,
    /// Current status message
    pub message: String,
    /// Pages processed so far
    pub pages_processed: u32,
    /// Total pages to process (if known)
    pub pages_total: Option<u32>,
}

impl ProgressSnapshot {
    /// Convert to JSON-compatible format for SSE
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "stage": format!("{:?}", self.stage).to_lowercase(),
            "stage_name": self.stage_name,
            "stage_progress": self.stage_progress,
            "overall_progress": self.overall_progress,
            "message": self.message,
            "pages_processed": self.pages_processed,
            "pages_total": self.pages_total,
        })
    }
}

/// A shareable progress tracker
pub type SharedProgressTracker = Arc<ProgressTracker>;

/// Create a new shared progress tracker
pub fn new_shared_tracker() -> SharedProgressTracker {
    Arc::new(ProgressTracker::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stage_ranges_cover_full_range() {
        let stages = CrawlStage::all();
        let mut last_end = 0;

        for stage in stages {
            let (start, end) = stage.range();
            assert_eq!(
                start, last_end,
                "Stage {:?} should start at {}",
                stage, last_end
            );
            assert!(end > start, "Stage {:?} should have positive range", stage);
            last_end = end;
        }

        assert_eq!(last_end, 100, "Final stage should end at 100");
    }

    #[test]
    fn test_progress_mapping() {
        let tracker = ProgressTracker::new();

        // Starting stage: 0-5%
        assert_eq!(tracker.map_progress(CrawlStage::Starting, 0), 0);
        assert_eq!(tracker.map_progress(CrawlStage::Starting, 50), 2);
        assert_eq!(tracker.map_progress(CrawlStage::Starting, 100), 5);

        // Crawling stage: 25-60%
        // Reset for fresh test
        let tracker = ProgressTracker::new();
        assert_eq!(tracker.map_progress(CrawlStage::Crawling, 0), 25);
        assert_eq!(tracker.map_progress(CrawlStage::Crawling, 50), 42);
        assert_eq!(tracker.map_progress(CrawlStage::Crawling, 100), 60);
    }

    #[test]
    fn test_monotonicity() {
        let tracker = ProgressTracker::new();

        // Progress should never go backwards
        let p1 = tracker.map_progress(CrawlStage::Crawling, 50);
        let p2 = tracker.map_progress(CrawlStage::Starting, 0); // Earlier stage
        assert!(p2 >= p1, "Progress should never decrease");
    }

    #[test]
    fn test_snapshot() {
        let tracker = ProgressTracker::new();
        tracker.set_stage(CrawlStage::Crawling);
        tracker.set_stage_progress(50);
        tracker.set_message("Crawling page 5 of 10");
        tracker.set_pages_total(10);

        for _ in 0..5 {
            tracker.increment_pages_processed();
        }

        let snapshot = tracker.snapshot();
        assert_eq!(snapshot.stage, CrawlStage::Crawling);
        assert!(snapshot.overall_progress >= 25);
        assert_eq!(snapshot.pages_processed, 5);
        assert_eq!(snapshot.pages_total, Some(10));
    }
}
