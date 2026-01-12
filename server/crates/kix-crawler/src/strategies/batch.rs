//! Batch Strategy
//!
//! Parallel batch crawling with streaming results.
//! Features:
//! - Concurrent request limiting via semaphore
//! - Streaming results - progress updates per task, not per batch
//! - Error isolation (failures don't stop other pages)
//! - No batch barriers - slow URLs don't block others

use super::{CrawlResult, CrawlStrategy, SinglePageStrategy, StrategyConfig};
use crate::progress::{CrawlStage, SharedProgressTracker};
use crate::CrawlerError;
use async_trait::async_trait;
use futures::stream::{self, StreamExt};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, warn};
use url::Url;

/// Batch parallel crawl strategy
pub struct BatchStrategy;

impl BatchStrategy {
    /// Create a new batch strategy
    pub fn new() -> Self {
        Self
    }
}

impl Default for BatchStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CrawlStrategy for BatchStrategy {
    /// Crawl URLs using streaming - no batch barriers
    ///
    /// Key improvements over the old batch approach:
    /// - Progress updates immediately as each task completes
    /// - Slow URLs don't block other URLs from completing
    /// - No artificial batch boundaries
    async fn crawl(
        &self,
        urls: Vec<Url>,
        config: &StrategyConfig,
        progress: SharedProgressTracker,
        cancel_token: CancellationToken,
    ) -> Result<CrawlResult, CrawlerError> {
        let start = Instant::now();
        let total_urls = urls.len();

        if total_urls == 0 {
            return Ok(CrawlResult {
                pages: Vec::new(),
                failed_urls: Vec::new(),
                total_time_ms: 0,
                was_cancelled: false,
            });
        }

        // Update progress
        progress.set_stage(CrawlStage::Crawling);
        progress.set_pages_total(total_urls as u32);
        progress.set_message(format!("Crawling {} pages...", total_urls));

        // Create semaphore for concurrency control
        let semaphore = Arc::new(Semaphore::new(config.max_concurrent));

        // Atomic counter for progress tracking
        let completed = Arc::new(AtomicUsize::new(0));

        // Shared collections for results (using tokio Mutex for async-safe access)
        let all_pages = Arc::new(tokio::sync::Mutex::new(Vec::with_capacity(total_urls)));
        let all_failed = Arc::new(tokio::sync::Mutex::new(Vec::new()));

        // Create the streaming pipeline with panic-isolated spawned tasks
        let mut stream = stream::iter(urls)
            .map(|url| {
                // Clone all shared state for this iteration (outside async move)
                let config = config.clone();
                let cancel = cancel_token.clone();
                let sem = semaphore.clone();
                let progress = progress.clone();
                let completed = completed.clone();
                let total = total_urls;
                let pages = all_pages.clone();
                let failed = all_failed.clone();

                // Clone again for panic handler (before async move captures)
                let all_failed_for_panic = failed.clone();
                let completed_for_panic = completed.clone();
                let progress_for_panic = progress.clone();

                async move {
                    // Spawn task for panic isolation - panics won't crash the stream
                    let url_for_handle = url.clone();

                    let handle = tokio::spawn(async move {
                        // Acquire semaphore permit (limits concurrency)
                        let permit = match sem.acquire().await {
                            Ok(p) => p,
                            Err(_) => {
                                warn!(url = %url, "Semaphore closed, marking as failed");
                                let mut f = failed.lock().await;
                                f.push((url, "Semaphore closed".to_string()));
                                return;
                            }
                        };

                        // Check cancellation
                        if cancel.is_cancelled() {
                            drop(permit);
                            return;
                        }

                        // Crawl the page with per-task timeout
                        let single = SinglePageStrategy::new();
                        let task_timeout = Duration::from_secs(config.task_timeout_secs);

                        let result = tokio::time::timeout(
                            task_timeout,
                            single.crawl(vec![url.clone()], &config, progress.clone(), cancel)
                        ).await;

                        // Process result immediately (no waiting for batch)
                        match result {
                            Ok(Ok(mut crawl_result)) => {
                                if let Some(page) = crawl_result.pages.pop() {
                                    debug!(url = %url, "Page crawled successfully");
                                    let mut p = pages.lock().await;
                                    p.push(page);
                                } else if let Some((_, error)) = crawl_result.failed_urls.pop() {
                                    warn!(url = %url, error = %error, "Page failed");
                                    let mut f = failed.lock().await;
                                    f.push((url, error));
                                } else {
                                    warn!(url = %url, "No result from single page strategy");
                                    let mut f = failed.lock().await;
                                    f.push((url, "No result".to_string()));
                                }
                            }
                            Ok(Err(e)) => {
                                warn!(url = %url, error = %e, "Crawl error");
                                let mut f = failed.lock().await;
                                f.push((url, e.to_string()));
                            }
                            Err(_elapsed) => {
                                warn!(
                                    url = %url,
                                    timeout_secs = task_timeout.as_secs(),
                                    "Task timed out - preventing hang"
                                );
                                let mut f = failed.lock().await;
                                f.push((url, format!("Timeout after {}s", task_timeout.as_secs())));
                            }
                        }

                        // Update progress IMMEDIATELY after each task
                        let done = completed.fetch_add(1, Ordering::SeqCst) + 1;
                        let pct = (done as f32 / total as f32 * 100.0) as u8;
                        progress.set_stage_progress(pct);
                        progress.set_message(format!("Crawled {}/{} pages", done, total));

                        drop(permit);
                    });

                    // Handle panic isolation - if the spawned task panics, we catch it here
                    match handle.await {
                        Ok(()) => {
                            // Task completed normally (success or handled error)
                        }
                        Err(e) if e.is_panic() => {
                            // Task panicked - log and record as failed
                            error!(
                                url = %url_for_handle,
                                panic = ?e,
                                "Task panicked - isolated from crashing the crawl"
                            );
                            let mut f = all_failed_for_panic.lock().await;
                            f.push((url_for_handle, "Task panicked".to_string()));

                            // Still update progress
                            let done = completed_for_panic.fetch_add(1, Ordering::SeqCst) + 1;
                            let pct = (done as f32 / total as f32 * 100.0) as u8;
                            progress_for_panic.set_stage_progress(pct);
                        }
                        Err(_) => {
                            // Task was cancelled - just update progress
                            let done = completed_for_panic.fetch_add(1, Ordering::SeqCst) + 1;
                            let pct = (done as f32 / total as f32 * 100.0) as u8;
                            progress_for_panic.set_stage_progress(pct);
                        }
                    }
                }
            })
            .buffer_unordered(config.max_concurrent);

        // Stream results - process as they complete (no batch barriers!)
        while stream.next().await.is_some() {
            // Check cancellation periodically
            if cancel_token.is_cancelled() {
                break;
            }
        }

        // Extract final results (stream is exhausted, safe to take ownership)
        let pages: Vec<_> = all_pages.lock().await.drain(..).collect();
        let failed: Vec<_> = all_failed.lock().await.drain(..).collect();

        let was_cancelled = cancel_token.is_cancelled();
        let total_time_ms = start.elapsed().as_millis() as u64;

        tracing::info!(
            pages = pages.len(),
            failed = failed.len(),
            cancelled = was_cancelled,
            time_ms = total_time_ms,
            "Batch crawl complete"
        );

        Ok(CrawlResult {
            pages,
            failed_urls: failed,
            total_time_ms,
            was_cancelled,
        })
    }

    fn name(&self) -> &'static str {
        "Batch"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_strategy_creation() {
        let strategy = BatchStrategy::new();
        assert_eq!(strategy.name(), "Batch");
    }
}
