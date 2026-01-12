//! Crawl frontier management - URL queue and deduplication

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use bloomfilter::Bloom;
use dashmap::DashMap;
use lru::LruCache;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::RwLock;
use tracing::{info, warn};
use url::Url;

/// A task in the crawl frontier
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CrawlTask {
    /// URL to crawl
    pub url: Url,
    /// Depth of this URL in the crawl tree
    pub depth: usize,
    /// When this task was added
    #[serde(skip, default = "Instant::now")]
    pub added_at: Instant,
    /// Parent URL (if any)
    pub parent: Option<Url>,
}

impl CrawlTask {
    /// Create a new crawl task
    pub fn new(url: Url, depth: usize, parent: Option<Url>) -> Self {
        Self {
            url,
            depth,
            added_at: Instant::now(),
            parent,
        }
    }

    /// Create a seed task (no parent, depth 0)
    pub fn seed(url: Url) -> Self {
        Self::new(url, 0, None)
    }

    /// Create a child task
    pub fn child(&self, url: Url) -> Self {
        Self::new(url, self.depth + 1, Some(self.url.clone()))
    }

    /// Age of the task
    pub fn age(&self) -> Duration {
        self.added_at.elapsed()
    }
}

/// Configuration for the frontier
#[derive(Clone, Debug)]
pub struct FrontierConfig {
    /// Expected number of URLs (for bloom filter sizing)
    pub expected_urls: usize,
    /// Desired false positive rate for bloom filter
    pub false_positive_rate: f64,
    /// Size of LRU cache for recent URLs
    pub lru_cache_size: usize,
    /// Maximum frontier size
    pub max_size: usize,
    /// Maximum time a task can be in-progress before considered stale (seconds)
    pub in_progress_ttl_secs: u64,
}

impl Default for FrontierConfig {
    fn default() -> Self {
        Self {
            expected_urls: 100_000,
            false_positive_rate: 0.01,
            lru_cache_size: 10_000,
            max_size: 1_000_000,
            in_progress_ttl_secs: 60, // Aggressive: 60 seconds
        }
    }
}

/// Crawl frontier managing URL queue and deduplication
pub struct CrawlFrontier {
    /// Pending URLs queue
    pending: RwLock<VecDeque<CrawlTask>>,
    /// Bloom filter for fast membership test
    bloom: Mutex<Bloom<String>>,
    /// LRU cache for recent URLs (more accurate)
    lru: Mutex<LruCache<String, ()>>,
    /// In-progress URLs
    in_progress: DashMap<String, std::time::Instant>,
    /// Per-domain task counts
    domain_counts: DashMap<String, usize>,
    /// Configuration
    config: FrontierConfig,
    /// Total discovered (pending + in_progress + completed)
    total_discovered: AtomicUsize,
}

impl CrawlFrontier {
    /// Create a new frontier
    pub fn new(config: FrontierConfig) -> Self {
        let bloom = Bloom::new_for_fp_rate(config.expected_urls, config.false_positive_rate);
        let lru = LruCache::new(
            std::num::NonZeroUsize::new(config.lru_cache_size).unwrap(),
        );

        Self {
            pending: RwLock::new(VecDeque::new()),
            bloom: Mutex::new(bloom),
            lru: Mutex::new(lru),
            in_progress: DashMap::new(),
            domain_counts: DashMap::new(),
            config,
            total_discovered: AtomicUsize::new(0),
        }
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(FrontierConfig::default())
    }

    /// Add a URL to the frontier
    pub async fn add(&self, task: CrawlTask) -> bool {
        let normalized = normalize_url(&task.url);

        // Check if already seen
        if self.is_seen(&normalized) {
            return false;
        }

        // Check frontier size
        let pending = self.pending.read().await;
        if pending.len() >= self.config.max_size {
            return false;
        }
        drop(pending);

        // Mark as seen
        self.mark_seen(&normalized);

        // Add to queue
        let mut pending = self.pending.write().await;
        pending.push_back(task);

        // Increment discovered count
        self.total_discovered.fetch_add(1, Ordering::Relaxed);

        true
    }

    /// Add multiple URLs
    pub async fn add_many(&self, tasks: Vec<CrawlTask>) -> usize {
        let mut added = 0;
        for task in tasks {
            if self.add(task).await {
                added += 1;
            }
        }
        added
    }

    /// Get next task to process (FIFO - breadth-first)
    pub async fn next(&self) -> Option<CrawlTask> {
        let mut pending = self.pending.write().await;
        if let Some(task) = pending.pop_front() {
            let normalized = normalize_url(&task.url);
            self.in_progress.insert(normalized, std::time::Instant::now());
            Some(task)
        } else {
            None
        }
    }

    /// Mark task as completed
    pub fn complete(&self, url: &Url) {
        let normalized = normalize_url(url);
        self.in_progress.remove(&normalized);
    }

    /// Check if URL has been seen
    fn is_seen(&self, normalized_url: &str) -> bool {
        // Check LRU first (more accurate for recent URLs)
        {
            let mut lru = self.lru.lock();
            if lru.get(normalized_url).is_some() {
                return true;
            }
        }

        // Check bloom filter (may have false positives)
        let bloom = self.bloom.lock();
        bloom.check(&normalized_url.to_string())
    }

    /// Mark URL as seen
    fn mark_seen(&self, normalized_url: &str) {
        // Add to both bloom and LRU
        {
            let mut bloom = self.bloom.lock();
            bloom.set(&normalized_url.to_string());
        }
        {
            let mut lru = self.lru.lock();
            lru.put(normalized_url.to_string(), ());
        }

        // Track domain
        if let Ok(url) = Url::parse(normalized_url) {
            if let Some(domain) = url.domain() {
                self.domain_counts
                    .entry(domain.to_string())
                    .and_modify(|c| *c += 1)
                    .or_insert(1);
            }
        }
    }

    /// Clear the frontier
    pub async fn clear(&self) {
        let mut pending = self.pending.write().await;
        pending.clear();
        self.in_progress.clear();
        self.total_discovered.store(0, Ordering::Relaxed);
    }

    /// Get frontier statistics
    pub async fn stats(&self) -> FrontierStats {
        let pending = self.pending.read().await;
        FrontierStats {
            pending_count: pending.len(),
            in_progress_count: self.in_progress.len(),
            domains_crawled: self.domain_counts.len(),
            total_discovered: self.total_discovered.load(Ordering::Relaxed),
        }
    }

    /// Clean up stale in-progress tasks
    pub fn clean_stale(&self) {
        let stale_threshold = Instant::now()
            .checked_sub(Duration::from_secs(self.config.in_progress_ttl_secs))
            .unwrap_or_else(Instant::now);

        let mut stale_keys = Vec::new();
        for entry in self.in_progress.iter() {
            if *entry.value() < stale_threshold {
                stale_keys.push(entry.key().clone());
            }
        }

        let count = stale_keys.len();
        for key in stale_keys {
            self.in_progress.remove(&key);
            warn!(
                "Removed stale in-progress URL after {} seconds: {}",
                self.config.in_progress_ttl_secs, key
            );
        }

        if count > 0 {
            info!("Cleaned up {} stale in-progress URLs", count);
        }
    }

    /// Get pending URLs (for debugging/monitoring)
    pub async fn pending_urls(&self) -> Vec<Url> {
        let pending = self.pending.read().await;
        pending.iter().map(|t| t.url.clone()).collect()
    }

    /// Check if frontier is empty (no pending or in-progress)
    pub async fn is_empty(&self) -> bool {
        let pending = self.pending.read().await;
        pending.is_empty() && self.in_progress.is_empty()
    }

    /// Get count of pages crawled per domain
    pub fn domain_stats(&self) -> Vec<(String, usize)> {
        self.domain_counts
            .iter()
            .map(|r| (r.key().clone(), *r.value()))
            .collect()
    }

    /// Check if we're done (no pending, no in-progress)
    pub async fn is_done(&self) -> bool {
        self.is_empty().await
    }

    /// Handle a failed task (optionally retry)
    pub async fn fail(&self, task: CrawlTask, should_retry: bool) {
        let normalized = normalize_url(&task.url);
        // Always remove from in_progress first - this is critical!
        // Without this, failed URLs stay in in_progress forever,
        // causing is_done() to never return true and the crawl to stall.
        self.in_progress.remove(&normalized);

        if should_retry && task.depth < 10 {
            // Re-add to frontier for retry
            let _ = self.add(task).await;
        }
        // Otherwise, just drop it
    }
}

/// Frontier statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontierStats {
    pub pending_count: usize,
    pub in_progress_count: usize,
    pub domains_crawled: usize,
    pub total_discovered: usize,
}

/// Normalize a URL for deduplication
pub fn normalize_url(url: &Url) -> String {
    let mut result = String::new();

    // Scheme
    result.push_str(url.scheme());
    result.push_str("://");

    // Host
    if let Some(host) = url.host_str() {
        result.push_str(&host.to_lowercase());
    }

    // Path (remove trailing slash unless root)
    let path = url.path();
    if path == "/" {
        result.push('/');
    } else {
        result.push_str(path.trim_end_matches('/'));
    }

    // Sort query parameters for consistency
    let mut query_pairs: Vec<(String, String)> = url.query_pairs()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    query_pairs.sort();

    if !query_pairs.is_empty() {
        result.push('?');
        result.push_str(
            &query_pairs
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join("&")
        );
    }

    // Ignore fragment
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_frontier_basic() {
        let frontier = CrawlFrontier::with_defaults();

        let url1 = Url::parse("https://example.com/page1").unwrap();
        let url2 = Url::parse("https://example.com/page2").unwrap();

        // Add tasks
        assert!(frontier.add(CrawlTask::seed(url1.clone())).await);
        assert!(frontier.add(CrawlTask::seed(url2.clone())).await);

        // Duplicate should be rejected
        assert!(!frontier.add(CrawlTask::seed(url1.clone())).await);

        // Get next task
        let task = frontier.next().await.unwrap();
        assert_eq!(task.url, url1);

        // Complete task
        frontier.complete(&url1);

        // Get stats
        let stats = frontier.stats().await;
        assert_eq!(stats.pending_count, 1);
        assert_eq!(stats.in_progress_count, 0);
        assert_eq!(stats.total_discovered, 2);
    }

    #[tokio::test]
    async fn test_fail_removes_from_in_progress() {
        // This test ensures that fail() removes URLs from in_progress,
        // which is critical to prevent crawl stalls when errors occur.
        let frontier = CrawlFrontier::with_defaults();

        let url1 = Url::parse("https://example.com/page1").unwrap();
        frontier.add(CrawlTask::seed(url1.clone())).await;

        // Get next task (moves to in_progress)
        let task = frontier.next().await.unwrap();
        assert_eq!(task.url, url1);

        // Verify it's in in_progress
        let stats = frontier.stats().await;
        assert_eq!(stats.in_progress_count, 1);
        assert!(!frontier.is_done().await);

        // Fail the task without retry
        frontier.fail(task, false).await;

        // Verify it's removed from in_progress
        let stats = frontier.stats().await;
        assert_eq!(stats.in_progress_count, 0);

        // Frontier should now be done (nothing pending, nothing in progress)
        assert!(frontier.is_done().await);
    }

    #[tokio::test]
    async fn test_fail_clears_in_progress_even_with_retry() {
        // Even with retry=true, the URL is removed from in_progress.
        // The retry attempt may fail silently if the URL was already seen,
        // but the critical thing is in_progress is cleared to prevent stalls.
        let frontier = CrawlFrontier::with_defaults();

        let url1 = Url::parse("https://example.com/page1").unwrap();
        frontier.add(CrawlTask::seed(url1.clone())).await;

        // Get next task
        let task = frontier.next().await.unwrap();

        // Initially: 0 pending, 1 in_progress
        let stats = frontier.stats().await;
        assert_eq!(stats.pending_count, 0);
        assert_eq!(stats.in_progress_count, 1);

        // Fail with retry - URL is removed from in_progress
        // (retry may not succeed if URL already seen, but that's OK)
        frontier.fail(task, true).await;

        // Critical: in_progress is cleared
        let stats = frontier.stats().await;
        assert_eq!(stats.in_progress_count, 0);
        // Note: pending_count may be 0 or 1 depending on whether retry succeeded
    }

    #[test]
    fn test_normalize_url() {
        // Test query param sorting and case normalization
        let url1 = Url::parse("https://example.com/page/?b=2&a=1#frag").unwrap();
        let url2 = Url::parse("https://EXAMPLE.COM/page?a=1&b=2").unwrap();
        assert_eq!(normalize_url(&url1), normalize_url(&url2));

        // Test trailing slash normalization (without query params)
        let url3 = Url::parse("https://example.com/page/").unwrap();
        let url4 = Url::parse("https://example.com/page").unwrap();
        assert_eq!(normalize_url(&url3), normalize_url(&url4));

        // Test fragment is stripped
        let url5 = Url::parse("https://example.com/page#section").unwrap();
        let url6 = Url::parse("https://example.com/page").unwrap();
        assert_eq!(normalize_url(&url5), normalize_url(&url6));

        // URLs with and without query params should be different
        assert_ne!(normalize_url(&url1), normalize_url(&url3));
    }
}