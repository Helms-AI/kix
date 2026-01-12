//! Crawler Service
//!
//! High-level orchestration layer that integrates advanced crawling components:
//! - URL Discovery (llms.txt → sitemap → robots priority)
//! - Crawling Strategies (single, batch, recursive, sitemap)
//! - Content Extraction (markdown with code preservation)
//! - Code Extraction (30+ patterns)
//! - Progress Tracking (9-stage pipeline with monotonicity)
//! - Cancellation Support (global registry with job guards)
//!
//! This service provides a unified interface with the following pipeline stages:
//! 1. starting (0-5%)
//! 2. discovery (5-15%)
//! 3. analyzing (15-25%)
//! 4. crawling (25-60%)
//! 5. processing (60-75%)
//! 6. source_creation (75-80%)
//! 7. document_storage (80-90%)
//! 8. code_extraction (90-95%)
//! 9. finalization (95-100%)

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;
use std::time::{Duration, Instant};

use scraper::Html;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use url::Url;
use uuid::Uuid;

use crate::cancellation::{global_registry, JobGuard};
use crate::code::{CodeExtractionService, ExtractedCode};
use crate::discovery::{DiscoveryConfig, DiscoveryMethod, DiscoveryService};
use crate::extractor::{ContentExtractor, ExtractedContent, MarkdownConfig};
use crate::progress::{new_shared_tracker, CrawlStage, SharedProgressTracker};
use crate::ssrf::SsrfValidator;
use crate::strategies::{
    BatchStrategy, CrawlPageResult, CrawlStrategy, CrawlStrategyType, RecursiveStrategy,
    SinglePageStrategy, SitemapStrategy, StrategyConfig,
};
use crate::CrawlerError;

// ============================================================================
// Service Configuration
// ============================================================================

/// Configuration for the crawler service.
#[derive(Clone, Debug)]
pub struct CrawlerServiceConfig {
    /// Discovery configuration
    pub discovery: DiscoveryConfig,

    /// Strategy configuration
    pub strategy: StrategyConfig,

    /// Content extraction configuration
    pub markdown: MarkdownConfig,

    /// Whether to extract code blocks separately
    pub extract_code: bool,

    /// Enable URL discovery (llms.txt, sitemap, robots)
    pub enable_discovery: bool,

    /// Default crawling strategy
    pub default_strategy: CrawlStrategyType,

    /// Overall crawl timeout in seconds (0 = no timeout)
    /// This is a watchdog that cancels the entire crawl if it takes too long.
    /// Default: 300 seconds (5 minutes) - aggressive to prevent hangs
    pub crawl_timeout_secs: u64,
}

impl Default for CrawlerServiceConfig {
    fn default() -> Self {
        Self {
            discovery: DiscoveryConfig::default(),
            strategy: StrategyConfig::default(),
            markdown: MarkdownConfig::default(),
            extract_code: true,
            enable_discovery: true,
            default_strategy: CrawlStrategyType::Batch,
            crawl_timeout_secs: 300, // 5 minutes - aggressive watchdog
        }
    }
}

impl CrawlerServiceConfig {
    /// Create config for single page crawl.
    pub fn single_page() -> Self {
        Self {
            default_strategy: CrawlStrategyType::SinglePage,
            enable_discovery: false,
            ..Default::default()
        }
    }

    /// Create config for sitemap-based crawl.
    pub fn sitemap_based() -> Self {
        Self {
            default_strategy: CrawlStrategyType::Sitemap,
            enable_discovery: true,
            ..Default::default()
        }
    }

    /// Create config for recursive crawl.
    pub fn recursive(max_depth: usize) -> Self {
        let mut strategy = StrategyConfig::default();
        strategy.max_depth = max_depth;

        Self {
            strategy,
            default_strategy: CrawlStrategyType::Recursive,
            enable_discovery: false,
            ..Default::default()
        }
    }
}

// ============================================================================
// Crawl Job
// ============================================================================

/// A crawl job with all associated metadata.
#[derive(Clone, Debug)]
pub struct CrawlJob {
    /// Unique job identifier
    pub id: Uuid,

    /// Seed URL to crawl
    pub url: Url,

    /// Maximum crawl depth
    pub max_depth: usize,

    /// Maximum pages to crawl
    pub max_pages: usize,

    /// Crawling strategy to use
    pub strategy: CrawlStrategyType,
}

impl CrawlJob {
    /// Create a new crawl job.
    pub fn new(url: Url) -> Self {
        Self {
            id: Uuid::new_v4(),
            url,
            max_depth: 3,
            max_pages: 100,
            strategy: CrawlStrategyType::Batch,
        }
    }

    /// Create a job with custom depth.
    pub fn with_depth(url: Url, max_depth: usize) -> Self {
        Self {
            max_depth,
            ..Self::new(url)
        }
    }

    /// Create a job with custom page limit.
    pub fn with_max_pages(url: Url, max_pages: usize) -> Self {
        Self {
            max_pages,
            ..Self::new(url)
        }
    }
}

// ============================================================================
// Crawl Result (Full Pipeline)
// ============================================================================

/// Result of a complete crawl pipeline execution.
#[derive(Debug)]
pub struct CrawlPipelineResult {
    /// Job that was executed
    pub job_id: Uuid,

    /// All crawled pages with extracted content
    pub pages: Vec<ProcessedPage>,

    /// URLs that failed to crawl
    pub failed_urls: Vec<(Url, String)>,

    /// Total execution time in milliseconds
    pub total_time_ms: u64,

    /// Whether the job was cancelled
    pub was_cancelled: bool,

    /// Discovery method used
    pub discovery_method: Option<String>,

    /// URLs discovered via sitemap/llms.txt
    pub discovered_urls: usize,
}

/// A fully processed page with extracted content and code.
#[derive(Debug, Clone)]
pub struct ProcessedPage {
    /// Page URL
    pub url: Url,

    /// Page title
    pub title: Option<String>,

    /// Extracted markdown content
    pub content: ExtractedContent,

    /// Extracted code blocks (validated)
    pub code_blocks: Vec<ExtractedCode>,

    /// HTTP status code
    pub status_code: u16,

    /// Content type
    pub content_type: Option<String>,

    /// Crawl time for this page
    pub crawl_time_ms: u64,
}

// ============================================================================
// Crawler Service
// ============================================================================

/// High-level crawler service that orchestrates the full crawling pipeline.
pub struct CrawlerService {
    config: CrawlerServiceConfig,
    discovery: DiscoveryService,
    extractor: ContentExtractor,
    code_extractor: CodeExtractionService,
    ssrf_validator: SsrfValidator,
}

impl CrawlerService {
    /// Create a new crawler service with default configuration.
    pub fn new() -> Self {
        Self::with_config(CrawlerServiceConfig::default())
    }

    /// Create a crawler service with custom configuration.
    pub fn with_config(config: CrawlerServiceConfig) -> Self {
        // Create discovery service (uses default if creation fails)
        // Note: If both attempts fail, this is a critical system error
        // (e.g., TLS not available) that cannot be gracefully recovered
        let discovery = DiscoveryService::new().unwrap_or_else(|e| {
            warn!(error = %e, "Failed to create discovery service, using custom config");
            DiscoveryService::with_config(config.discovery.clone())
                .expect("Failed to create discovery service - check TLS/HTTP configuration")
        });

        Self {
            discovery,
            extractor: ContentExtractor::new(config.markdown.clone()),
            code_extractor: CodeExtractionService::new(),
            ssrf_validator: SsrfValidator::new(),
            config,
        }
    }

    /// Execute a crawl job with full pipeline processing.
    ///
    /// This method executes the complete crawling pipeline:
    /// 1. Validate URL (SSRF protection)
    /// 2. Discovery phase (find URLs via llms.txt, sitemap, robots)
    /// 3. Crawling phase (fetch pages using selected strategy)
    /// 4. Processing phase (extract markdown, code blocks)
    /// 5. Finalization
    ///
    /// A watchdog timer is spawned to cancel the crawl if it exceeds the timeout.
    pub async fn execute(&self, job: CrawlJob) -> Result<CrawlPipelineResult, CrawlerError> {
        let start = Instant::now();
        let progress = new_shared_tracker();

        // Register with cancellation registry
        let registry = global_registry();
        let cancel_token = registry.register(job.id);
        let _guard = JobGuard::new(registry.clone(), job.id);

        // Spawn watchdog timer if timeout is configured
        let watchdog_handle = if self.config.crawl_timeout_secs > 0 {
            let cancel = cancel_token.clone();
            let timeout = Duration::from_secs(self.config.crawl_timeout_secs);
            let job_id = job.id;
            Some(tokio::spawn(async move {
                tokio::time::sleep(timeout).await;
                warn!(
                    job_id = %job_id,
                    timeout_secs = timeout.as_secs(),
                    "Crawl watchdog triggered - cancelling job to prevent hang"
                );
                cancel.cancel();
            }))
        } else {
            None
        };

        info!(job_id = %job.id, url = %job.url, "Starting crawl pipeline");

        // Stage 1: Starting (0-5%)
        progress.set_stage(CrawlStage::Starting);
        progress.set_message("Initializing crawl...".to_string());

        // Validate URL
        if let Err(e) = self.ssrf_validator.validate(&job.url) {
            warn!(url = %job.url, error = %e, "URL validation failed");
            if let Some(ref h) = watchdog_handle { h.abort(); }
            return Err(CrawlerError::Parse(format!("SSRF validation failed: {}", e)));
        }

        if cancel_token.is_cancelled() {
            if let Some(ref h) = watchdog_handle { h.abort(); }
            return Ok(self.create_cancelled_result(job.id, start.elapsed().as_millis() as u64));
        }

        // Stage 2: Discovery (5-15%)
        progress.set_stage(CrawlStage::Discovery);
        progress.set_message("Discovering URLs...".to_string());

        let (urls_to_crawl, discovery_method) = if self.config.enable_discovery {
            self.discover_urls(&job.url, &progress, &cancel_token).await?
        } else {
            (vec![job.url.clone()], None)
        };

        let discovered_count = urls_to_crawl.len();
        info!(count = discovered_count, method = ?discovery_method, "URLs discovered");

        if cancel_token.is_cancelled() {
            if let Some(ref h) = watchdog_handle { h.abort(); }
            return Ok(self.create_cancelled_result(job.id, start.elapsed().as_millis() as u64));
        }

        // Stage 3: Analyzing (15-25%)
        progress.set_stage(CrawlStage::Analyzing);
        progress.set_message(format!("Analyzing {} URLs...", discovered_count));

        // Apply limits
        let urls_to_crawl: Vec<Url> = urls_to_crawl
            .into_iter()
            .take(job.max_pages)
            .collect();

        // Stage 4: Crawling (25-60%)
        progress.set_stage(CrawlStage::Crawling);
        progress.set_pages_total(urls_to_crawl.len() as u32);
        progress.set_message(format!("Crawling {} pages...", urls_to_crawl.len()));

        let mut strategy_config = self.config.strategy.clone();
        strategy_config.max_pages = job.max_pages;
        strategy_config.max_depth = job.max_depth;

        let crawl_result = self
            .execute_crawl(
                urls_to_crawl,
                job.strategy,
                &strategy_config,
                progress.clone(),
                cancel_token.clone(),
            )
            .await?;

        if crawl_result.was_cancelled {
            if let Some(ref h) = watchdog_handle { h.abort(); }
            return Ok(CrawlPipelineResult {
                job_id: job.id,
                pages: Vec::new(),
                failed_urls: crawl_result.failed_urls,
                total_time_ms: start.elapsed().as_millis() as u64,
                was_cancelled: true,
                discovery_method: discovery_method.map(|m| m.to_string()),
                discovered_urls: discovered_count,
            });
        }

        // Stage 5: Processing (60-75%)
        progress.set_stage(CrawlStage::Processing);
        progress.set_message("Processing page content...".to_string());

        let mut processed_pages = Vec::with_capacity(crawl_result.pages.len());
        for (idx, page) in crawl_result.pages.into_iter().enumerate() {
            if cancel_token.is_cancelled() {
                break;
            }

            let processed = self.process_page(page).await;
            processed_pages.push(processed);

            let pct = ((idx + 1) as f32 / processed_pages.capacity() as f32 * 100.0) as u8;
            progress.set_stage_progress(pct);
        }

        // Stage 6: Source Creation (75-80%)
        progress.set_stage(CrawlStage::SourceCreation);
        progress.set_message("Creating source records...".to_string());
        // This would be handled by the caller (processor.rs)

        // Stage 7: Document Storage (80-90%)
        progress.set_stage(CrawlStage::DocumentStorage);
        progress.set_message("Storing documents...".to_string());
        // This would be handled by the caller (processor.rs)

        // Stage 8: Code Extraction (90-95%)
        progress.set_stage(CrawlStage::CodeExtraction);
        progress.set_message("Extracting code blocks...".to_string());
        // Already done during processing

        // Stage 9: Finalization (95-100%)
        progress.set_stage(CrawlStage::Finalization);
        progress.set_message("Finalizing...".to_string());
        progress.set_stage_progress(100);

        let total_time = start.elapsed().as_millis() as u64;

        info!(
            job_id = %job.id,
            pages = processed_pages.len(),
            failed = crawl_result.failed_urls.len(),
            time_ms = total_time,
            "Crawl pipeline completed"
        );

        // Abort watchdog timer - crawl completed successfully
        if let Some(ref h) = watchdog_handle { h.abort(); }

        Ok(CrawlPipelineResult {
            job_id: job.id,
            pages: processed_pages,
            failed_urls: crawl_result.failed_urls,
            total_time_ms: total_time,
            was_cancelled: false,
            discovery_method: discovery_method.map(|m| m.to_string()),
            discovered_urls: discovered_count,
        })
    }

    /// Execute crawl with progress callback.
    pub async fn execute_with_callback<F>(
        &self,
        job: CrawlJob,
        on_progress: F,
    ) -> Result<CrawlPipelineResult, CrawlerError>
    where
        F: Fn(CrawlStage, u8, &str) + Send + Sync + 'static,
    {
        let progress = new_shared_tracker();

        // Spawn progress monitor
        let progress_clone = progress.clone();
        let callback = Arc::new(on_progress);
        let callback_clone = callback.clone();

        tokio::spawn(async move {
            let mut last_overall = 0u8;
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;

                let snapshot = progress_clone.snapshot();
                let overall = snapshot.overall_progress;

                if overall != last_overall {
                    callback_clone(snapshot.stage, overall, &snapshot.message);
                    last_overall = overall;
                }

                if overall >= 100 {
                    break;
                }
            }
        });

        self.execute(job).await
    }

    /// Cancel a running job.
    pub fn cancel(&self, job_id: &Uuid) -> bool {
        global_registry().cancel(job_id)
    }

    // ========================================================================
    // Private Methods
    // ========================================================================

    /// Discover URLs using the priority order: llms.txt → sitemap → robots.
    async fn discover_urls(
        &self,
        base_url: &Url,
        progress: &SharedProgressTracker,
        _cancel_token: &CancellationToken,
    ) -> Result<(Vec<Url>, Option<&'static str>), CrawlerError> {
        progress.set_message("Checking llms.txt...".to_string());
        progress.set_stage_progress(20);

        match self.discovery.discover(base_url).await {
            Ok(result) => {
                let method_str = match result.method {
                    DiscoveryMethod::LlmsTxt => "llms.txt",
                    DiscoveryMethod::Sitemap => "sitemap.xml",
                    DiscoveryMethod::RobotsTxt => "robots.txt",
                    DiscoveryMethod::None => "none",
                };

                if result.urls.is_empty() {
                    Ok((vec![base_url.clone()], None))
                } else {
                    Ok((result.urls, Some(method_str)))
                }
            }
            Err(e) => {
                warn!(error = %e, "Discovery failed, using seed URL only");
                Ok((vec![base_url.clone()], None))
            }
        }
    }

    /// Execute the crawling strategy.
    async fn execute_crawl(
        &self,
        urls: Vec<Url>,
        strategy_type: CrawlStrategyType,
        config: &StrategyConfig,
        progress: SharedProgressTracker,
        cancel_token: CancellationToken,
    ) -> Result<crate::strategies::CrawlResult, CrawlerError> {
        match strategy_type {
            CrawlStrategyType::SinglePage => {
                let strategy = SinglePageStrategy::new();
                strategy.crawl(urls, config, progress, cancel_token).await
            }
            CrawlStrategyType::Batch => {
                let strategy = BatchStrategy::new();
                strategy.crawl(urls, config, progress, cancel_token).await
            }
            CrawlStrategyType::Recursive => {
                let strategy = RecursiveStrategy::new();
                strategy.crawl(urls, config, progress, cancel_token).await
            }
            CrawlStrategyType::Sitemap => {
                let strategy = SitemapStrategy::new();
                strategy.crawl(urls, config, progress, cancel_token).await
            }
        }
    }

    /// Process a crawled page to extract content and code.
    ///
    /// Uses panic isolation to prevent one page's processing failure from
    /// crashing the entire crawl job. This is important for handling edge cases
    /// like malformed UTF-8 content or unexpected HTML structures.
    async fn process_page(&self, page: CrawlPageResult) -> ProcessedPage {
        let url = page.url.clone();

        // Wrap content extraction in catch_unwind for panic isolation
        // This prevents one page's panic from crashing the entire crawl
        let extraction_result = catch_unwind(AssertUnwindSafe(|| {
            // Parse HTML for code extraction
            let document = Html::parse_document(&page.html);

            // Extract content using the extractor
            let content = self.extractor.extract(&page.html, &page.url);

            // Extract code blocks if enabled
            let code_blocks = if self.config.extract_code {
                self.code_extractor.extract_all(&document)
            } else {
                Vec::new()
            };

            (content, code_blocks)
        }));

        match extraction_result {
            Ok((content, code_blocks)) => ProcessedPage {
                url: page.url,
                title: page.title,
                content,
                code_blocks,
                status_code: page.status_code,
                content_type: page.content_type,
                crawl_time_ms: page.crawl_time_ms,
            },
            Err(panic_info) => {
                // Log the panic but don't propagate it
                error!(
                    url = %url,
                    panic = ?panic_info,
                    "Content extraction panicked, returning fallback content"
                );

                // Return a fallback ProcessedPage with empty content
                ProcessedPage {
                    url,
                    title: page.title,
                    content: ExtractedContent::empty(),
                    code_blocks: Vec::new(),
                    status_code: page.status_code,
                    content_type: page.content_type,
                    crawl_time_ms: page.crawl_time_ms,
                }
            }
        }
    }

    /// Create a cancelled result.
    fn create_cancelled_result(&self, job_id: Uuid, elapsed_ms: u64) -> CrawlPipelineResult {
        CrawlPipelineResult {
            job_id,
            pages: Vec::new(),
            failed_urls: Vec::new(),
            total_time_ms: elapsed_ms,
            was_cancelled: true,
            discovery_method: None,
            discovered_urls: 0,
        }
    }
}

impl Default for CrawlerService {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Convenience Functions
// ============================================================================

/// Crawl a single URL and return processed content.
pub async fn crawl_url(url: &str) -> Result<ProcessedPage, CrawlerError> {
    let url = Url::parse(url).map_err(|e| CrawlerError::Parse(e.to_string()))?;
    let service = CrawlerService::with_config(CrawlerServiceConfig::single_page());

    let job = CrawlJob {
        id: Uuid::new_v4(),
        url,
        max_depth: 0,
        max_pages: 1,
        strategy: CrawlStrategyType::SinglePage,
    };

    let result = service.execute(job).await?;
    result
        .pages
        .into_iter()
        .next()
        .ok_or_else(|| CrawlerError::Parse("No pages crawled".to_string()))
}

/// Crawl a URL with discovery and return all processed pages.
pub async fn crawl_with_discovery(
    url: &str,
    max_pages: usize,
) -> Result<CrawlPipelineResult, CrawlerError> {
    let url = Url::parse(url).map_err(|e| CrawlerError::Parse(e.to_string()))?;
    let service = CrawlerService::new();

    let job = CrawlJob {
        id: Uuid::new_v4(),
        url,
        max_depth: 3,
        max_pages,
        strategy: CrawlStrategyType::Batch,
    };

    service.execute(job).await
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = CrawlerServiceConfig::default();
        assert!(config.enable_discovery);
        assert!(config.extract_code);
        assert_eq!(config.default_strategy, CrawlStrategyType::Batch);
    }

    #[test]
    fn test_config_single_page() {
        let config = CrawlerServiceConfig::single_page();
        assert!(!config.enable_discovery);
        assert_eq!(config.default_strategy, CrawlStrategyType::SinglePage);
    }

    #[test]
    fn test_config_recursive() {
        let config = CrawlerServiceConfig::recursive(5);
        assert_eq!(config.strategy.max_depth, 5);
        assert_eq!(config.default_strategy, CrawlStrategyType::Recursive);
    }

    #[test]
    fn test_crawl_job_creation() {
        let url = Url::parse("https://example.com").unwrap();
        let job = CrawlJob::new(url.clone());

        assert_eq!(job.url, url);
        assert_eq!(job.max_depth, 3);
        assert_eq!(job.max_pages, 100);
    }

    #[test]
    fn test_service_creation() {
        let service = CrawlerService::new();
        assert!(service.config.enable_discovery);
    }
}
