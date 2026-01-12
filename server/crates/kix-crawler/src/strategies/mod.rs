//! Crawling Strategies
//!
//! Different crawling strategies for various use cases:
//! - SinglePage: One URL with retries and framework detection
//! - Batch: Parallel batch crawling with memory-adaptive dispatch
//! - Recursive: Depth-first recursive crawling with visited tracking
//! - Sitemap: XML sitemap parsing and systematic crawling

mod batch;
mod recursive;
mod single_page;
mod sitemap;

pub use batch::BatchStrategy;
pub use recursive::RecursiveStrategy;
pub use single_page::SinglePageStrategy;
pub use sitemap::SitemapStrategy;

use crate::progress::SharedProgressTracker;
use crate::CrawlerError;
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;
use url::Url;

/// Result of crawling a single page
#[derive(Debug, Clone)]
pub struct CrawlPageResult {
    /// The URL that was crawled
    pub url: Url,
    /// Raw HTML content
    pub html: String,
    /// Extracted markdown content (if available)
    pub markdown: Option<String>,
    /// Page title
    pub title: Option<String>,
    /// HTTP status code
    pub status_code: u16,
    /// Content type from headers
    pub content_type: Option<String>,
    /// Time taken to crawl in milliseconds
    pub crawl_time_ms: u64,
    /// URLs discovered on this page (for recursive crawling)
    pub discovered_urls: Vec<Url>,
}

/// Result of a complete crawl operation
#[derive(Debug)]
pub struct CrawlResult {
    /// All successfully crawled pages
    pub pages: Vec<CrawlPageResult>,
    /// URLs that failed to crawl
    pub failed_urls: Vec<(Url, String)>,
    /// Total time taken in milliseconds
    pub total_time_ms: u64,
    /// Whether the crawl was cancelled
    pub was_cancelled: bool,
}

/// Configuration for crawl strategies
#[derive(Debug, Clone)]
pub struct StrategyConfig {
    /// Maximum number of pages to crawl
    pub max_pages: usize,
    /// Maximum depth for recursive crawling
    pub max_depth: usize,
    /// Timeout per page in seconds (for network/browser)
    pub page_timeout_secs: u64,
    /// Maximum retries per page
    pub max_retries: u8,
    /// Delay between retries in milliseconds
    pub retry_delay_ms: u64,
    /// Maximum concurrent requests for batch strategies
    pub max_concurrent: usize,
    /// Whether to render JavaScript
    pub render_js: bool,
    /// Wait selectors for JavaScript rendering
    pub wait_selectors: Vec<String>,
    /// User agent string
    pub user_agent: String,
    /// Per-task timeout in seconds (entire task including retries)
    /// Default: 60 seconds - aggressive to prevent individual tasks from blocking
    pub task_timeout_secs: u64,
}

impl Default for StrategyConfig {
    fn default() -> Self {
        Self {
            max_pages: 1000,
            max_depth: 5,
            page_timeout_secs: 30,
            max_retries: 3,
            retry_delay_ms: 1000,
            max_concurrent: 10,
            render_js: true,
            wait_selectors: Vec::new(),
            user_agent: "KIX/1.0 (+https://github.com/kix)".to_string(),
            task_timeout_secs: 60, // Aggressive: 60 seconds per task
        }
    }
}

/// Trait for crawling strategies
#[async_trait]
pub trait CrawlStrategy: Send + Sync {
    /// Execute the crawl strategy
    async fn crawl(
        &self,
        urls: Vec<Url>,
        config: &StrategyConfig,
        progress: SharedProgressTracker,
        cancel_token: CancellationToken,
    ) -> Result<CrawlResult, CrawlerError>;

    /// Get the name of this strategy
    fn name(&self) -> &'static str;
}

/// Strategy selector based on crawl type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrawlStrategyType {
    /// Single page with retries
    SinglePage,
    /// Parallel batch of URLs
    Batch,
    /// Depth-first recursive crawl
    Recursive,
    /// Sitemap-based crawl
    Sitemap,
}

impl CrawlStrategyType {
    /// Get a human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            Self::SinglePage => "Single page with retries",
            Self::Batch => "Parallel batch crawling",
            Self::Recursive => "Depth-first recursive crawl",
            Self::Sitemap => "Sitemap-based systematic crawl",
        }
    }

    /// Create the appropriate strategy instance
    pub fn create_strategy(&self) -> Box<dyn CrawlStrategy> {
        match self {
            Self::SinglePage => Box::new(SinglePageStrategy::new()),
            Self::Batch => Box::new(BatchStrategy::new()),
            Self::Recursive => Box::new(RecursiveStrategy::new()),
            Self::Sitemap => Box::new(SitemapStrategy::new()),
        }
    }
}

/// Detect framework from HTML for optimal wait selectors
pub fn detect_framework_selectors(html: &str) -> Vec<String> {
    let mut selectors = Vec::new();

    // Next.js
    if html.contains("__NEXT_DATA__") || html.contains("_next") {
        selectors.push("div#__next".to_string());
    }

    // Nuxt.js
    if html.contains("_nuxt") || html.contains("__nuxt") {
        selectors.push("div#__nuxt".to_string());
    }

    // Angular
    if html.contains("ng-version") || html.contains("ng-app") {
        selectors.push("[ng-version]".to_string());
    }

    // React (generic)
    if html.contains("data-reactroot") || html.contains("react-root") {
        selectors.push("[data-reactroot]".to_string());
    }

    // Vue.js
    if html.contains("v-app") || html.contains("data-v-") {
        selectors.push("[data-v-app]".to_string());
    }

    // Svelte/SvelteKit
    if html.contains("svelte") || html.contains("__sveltekit") {
        selectors.push("[data-sveltekit-hydrate]".to_string());
    }

    // Docusaurus
    if html.contains("docusaurus") {
        selectors.push("div#__docusaurus".to_string());
    }

    // VitePress
    if html.contains("vitepress") {
        selectors.push("div#app".to_string());
    }

    // Gatsby
    if html.contains("gatsby") || html.contains("___gatsby") {
        selectors.push("div#___gatsby".to_string());
    }

    selectors
}

/// Calculate exponential backoff delay
pub fn calculate_backoff(attempt: u8, base_delay_ms: u64) -> u64 {
    let multiplier = 2u64.pow(attempt as u32);
    base_delay_ms * multiplier
}
