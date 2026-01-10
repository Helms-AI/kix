//! Crawl frontier management - URL queue and deduplication

use std::collections::VecDeque;

use bloomfilter::Bloom;
use dashmap::DashMap;
use lru::LruCache;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use url::Url;

/// A task in the crawl frontier
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CrawlTask {
    /// URL to crawl
    pub url: Url,
    /// Current depth from seed
    pub depth: usize,
    /// Parent URL (if any)
    pub parent_url: Option<Url>,
    /// Priority (higher = more urgent)
    pub priority: f32,
    /// Retry count
    pub retry_count: u8,
}

impl CrawlTask {
    /// Create a new crawl task
    pub fn new(url: Url, depth: usize) -> Self {
        Self {
            url,
            depth,
            parent_url: None,
            priority: 1.0,
            retry_count: 0,
        }
    }

    /// Create a seed task (depth 0)
    pub fn seed(url: Url) -> Self {
        Self::new(url, 0)
    }

    /// Create a child task from this task
    pub fn child(&self, url: Url) -> Self {
        Self {
            url,
            depth: self.depth + 1,
            parent_url: Some(self.url.clone()),
            priority: self.priority * 0.9, // Reduce priority for deeper pages
            retry_count: 0,
        }
    }
}

/// Configuration for the frontier
#[derive(Clone, Debug)]
pub struct FrontierConfig {
    /// Expected number of URLs for bloom filter sizing
    pub expected_urls: usize,
    /// False positive rate for bloom filter
    pub false_positive_rate: f64,
    /// LRU cache size for recent URLs
    pub lru_cache_size: usize,
    /// Maximum frontier size
    pub max_size: usize,
}

impl Default for FrontierConfig {
    fn default() -> Self {
        Self {
            expected_urls: 100_000,
            false_positive_rate: 0.01,
            lru_cache_size: 10_000,
            max_size: 1_000_000,
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

        // Update domain count
        if let Some(domain) = url.domain() {
            self.domain_counts
                .entry(domain.to_string())
                .and_modify(|c| *c += 1)
                .or_insert(1);
        }
    }

    /// Mark task as failed (may retry)
    pub async fn fail(&self, task: CrawlTask, retry: bool) {
        let normalized = normalize_url(&task.url);
        self.in_progress.remove(&normalized);

        if retry && task.retry_count < 3 {
            let mut retry_task = task;
            retry_task.retry_count += 1;
            retry_task.priority *= 0.5; // Lower priority for retries

            let mut pending = self.pending.write().await;
            pending.push_back(retry_task);
        }
    }

    /// Check if URL has been seen
    fn is_seen(&self, normalized: &str) -> bool {
        // Check LRU cache first (more accurate)
        {
            let mut lru = self.lru.lock();
            if lru.get(normalized).is_some() {
                return true;
            }
        }

        // Check bloom filter
        let bloom = self.bloom.lock();
        bloom.check(&normalized.to_string())
    }

    /// Mark URL as seen
    fn mark_seen(&self, normalized: &str) {
        // Add to LRU
        {
            let mut lru = self.lru.lock();
            lru.put(normalized.to_string(), ());
        }

        // Add to bloom filter
        {
            let mut bloom = self.bloom.lock();
            bloom.set(&normalized.to_string());
        }
    }

    /// Get frontier statistics
    pub async fn stats(&self) -> FrontierStats {
        let pending = self.pending.read().await;
        FrontierStats {
            pending_count: pending.len(),
            in_progress_count: self.in_progress.len(),
            domains_crawled: self.domain_counts.len(),
        }
    }

    /// Check if frontier is empty and no tasks in progress
    pub async fn is_done(&self) -> bool {
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
}

/// Frontier statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontierStats {
    pub pending_count: usize,
    pub in_progress_count: usize,
    pub domains_crawled: usize,
}

/// Normalize a URL for deduplication
pub fn normalize_url(url: &Url) -> String {
    let mut normalized = url.clone();

    // Remove fragment
    normalized.set_fragment(None);

    // Lowercase scheme and host
    let scheme = normalized.scheme().to_lowercase();
    let host = normalized.host_str().unwrap_or("").to_lowercase();

    // Remove default ports
    let port = normalized.port().filter(|p| {
        !((scheme == "http" && *p == 80) || (scheme == "https" && *p == 443))
    });

    // Sort query parameters
    let mut query_pairs: Vec<_> = normalized.query_pairs().collect();
    query_pairs.sort_by(|a, b| a.0.cmp(&b.0));

    // Rebuild URL
    let mut result = format!("{}://{}", scheme, host);
    if let Some(p) = port {
        result.push_str(&format!(":{}", p));
    }
    result.push_str(normalized.path());

    if !query_pairs.is_empty() {
        result.push('?');
        result.push_str(
            &query_pairs
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join("&"),
        );
    }

    result
}

impl Default for CrawlFrontier {
    fn default() -> Self {
        Self::with_defaults()
    }
}