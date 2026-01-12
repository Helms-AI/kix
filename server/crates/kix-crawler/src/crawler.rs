//! URL crawler with depth control

use std::sync::Arc;
use std::time::Duration;

use htmd::HtmlToMarkdown;
use reqwest::Client;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};
use url::Url;

use crate::frontier::{CrawlFrontier, CrawlTask, FrontierStats};
use crate::rate_limiter::DomainRateLimiter;
use crate::robots::RobotsChecker;
use crate::CrawlerError;

#[cfg(feature = "browser")]
use crate::browser::{BrowserConfig, BrowserPool};

/// Crawler configuration
#[derive(Clone, Debug)]
pub struct CrawlerConfig {
    /// Maximum crawl depth
    pub max_depth: usize,
    /// Maximum pages to crawl
    pub max_pages: usize,
    /// Maximum pages per domain
    pub max_pages_per_domain: Option<usize>,
    /// Concurrent requests
    pub concurrent_requests: usize,
    /// Request timeout
    pub timeout: Duration,
    /// Respect robots.txt
    pub respect_robots: bool,
    /// Follow redirects
    pub follow_redirects: bool,
    /// User agent string
    pub user_agent: String,
    /// Allowed domains (empty = all)
    pub allowed_domains: Vec<String>,
    /// Blocked domains
    pub blocked_domains: Vec<String>,
    /// Render JavaScript using headless browser (default: true)
    pub render_js: bool,
    /// Timeout for browser rendering (default: 30 seconds)
    pub render_timeout: Duration,
    /// Use sitemaps to discover URLs (from robots.txt or default locations)
    pub use_sitemaps: bool,
}

impl Default for CrawlerConfig {
    fn default() -> Self {
        Self {
            max_depth: 3,
            max_pages: 10000,            // Increased from 1000 for large crawls
            max_pages_per_domain: Some(500),  // Increased from 100
            // Aligned with browser max_contexts (8) to prevent queueing bottleneck
            // when render_js is enabled. Higher values cause 56+ requests to wait.
            concurrent_requests: 8,
            timeout: Duration::from_secs(30),
            respect_robots: true,
            follow_redirects: true,
            user_agent: "EIP-Crawler/1.0 (+https://github.com/eip-knowledge)".to_string(),
            allowed_domains: vec![],
            blocked_domains: vec![],
            render_js: true,             // Enable browser rendering by default for JS-heavy pages
            render_timeout: Duration::from_secs(30),  // Default timeout for browser rendering
            use_sitemaps: true,          // Enable sitemap discovery by default
        }
    }
}

/// A code block extracted from HTML content
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CodeBlock {
    /// Programming language (if detected from class)
    pub language: Option<String>,
    /// The code content
    pub content: String,
    /// Number of lines
    pub line_count: usize,
}

/// A header extracted from HTML content
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExtractedHeader {
    /// Header level (1-6)
    pub level: u8,
    /// Header text
    pub text: String,
}

/// Structured content extracted from HTML
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ExtractedContent {
    /// Content converted to markdown (preserves structure)
    pub markdown: String,
    /// Extracted code blocks (preserved separately for special chunking)
    pub code_blocks: Vec<CodeBlock>,
    /// Extracted headers for hierarchy
    pub headers: Vec<ExtractedHeader>,
    /// Plain text for fallback/embedding
    pub plain_text: String,
}

/// Result of crawling a single page
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CrawlResult {
    /// The URL that was crawled
    pub url: Url,
    /// HTTP status code
    pub status: u16,
    /// Page title
    pub title: Option<String>,
    /// Extracted text content (plain text, for backward compatibility)
    pub content: String,
    /// Structured content with preserved markdown, code blocks, headers
    pub structured_content: ExtractedContent,
    /// Links found on the page
    pub links: Vec<Url>,
    /// Content type
    pub content_type: String,
    /// Response size in bytes
    pub size: usize,
    /// Time taken to fetch
    pub fetch_time_ms: u64,
    /// Source domain for filtering
    pub source_domain: Option<String>,
}

/// Web crawler
pub struct Crawler {
    client: Client,
    config: CrawlerConfig,
    frontier: Arc<CrawlFrontier>,
    rate_limiter: Arc<DomainRateLimiter>,
    robots_checker: Arc<RobotsChecker>,
    semaphore: Arc<Semaphore>,
    cancellation: CancellationToken,
    /// Browser pool for JavaScript rendering (per-job, created fresh for each crawler)
    #[cfg(feature = "browser")]
    browser_pool: Option<Arc<BrowserPool>>,
}

impl Crawler {
    /// Create a new crawler (async to allow browser pool initialization)
    pub async fn new(config: CrawlerConfig) -> Result<Self, CrawlerError> {
        let client = Client::builder()
            .user_agent(&config.user_agent)
            .timeout(config.timeout)
            .redirect(if config.follow_redirects {
                reqwest::redirect::Policy::limited(10)
            } else {
                reqwest::redirect::Policy::none()
            })
            .build()?;

        let robots_checker = Arc::new(RobotsChecker::new(client.clone(), &config.user_agent));

        // Initialize browser pool if JS rendering is enabled
        #[cfg(feature = "browser")]
        let browser_pool = if config.render_js {
            let browser_config = BrowserConfig {
                timeout: config.render_timeout,
                ..Default::default()
            };
            info!("Initializing browser pool for JavaScript rendering");
            Some(Arc::new(BrowserPool::new(browser_config).await?))
        } else {
            None
        };

        // Cap concurrency at browser max_contexts when using browser rendering
        // to prevent queueing bottleneck (56+ requests waiting for 8 contexts)
        #[cfg(feature = "browser")]
        let effective_concurrency = if config.render_js {
            let browser_max = BrowserConfig::default().max_contexts;
            if config.concurrent_requests > browser_max {
                info!(
                    requested = config.concurrent_requests,
                    capped = browser_max,
                    "Capping concurrency to browser max_contexts"
                );
            }
            config.concurrent_requests.min(browser_max)
        } else {
            config.concurrent_requests
        };

        #[cfg(not(feature = "browser"))]
        let effective_concurrency = config.concurrent_requests;

        let semaphore = Arc::new(Semaphore::new(effective_concurrency));

        Ok(Self {
            client,
            config,
            frontier: Arc::new(CrawlFrontier::with_defaults()),
            rate_limiter: Arc::new(DomainRateLimiter::with_defaults()),
            robots_checker,
            semaphore,
            cancellation: CancellationToken::new(),
            #[cfg(feature = "browser")]
            browser_pool,
        })
    }

    /// Shutdown the crawler and release resources (call when job completes)
    pub async fn shutdown(&self) -> Result<(), CrawlerError> {
        #[cfg(feature = "browser")]
        if let Some(ref pool) = self.browser_pool {
            info!("Shutting down browser pool");
            pool.close().await?;
        }
        Ok(())
    }

    /// Get current frontier statistics
    pub async fn get_frontier_stats(&self) -> FrontierStats {
        self.frontier.stats().await
    }

    /// Check if frontier is exhausted
    pub async fn is_frontier_empty(&self) -> bool {
        self.frontier.is_empty().await
    }

    /// Filter sitemap URLs to only include those related to the seed URL
    /// Uses path prefix matching to find related pages in the same section
    fn filter_related_sitemap_urls(seed_url: &Url, sitemap_urls: Vec<Url>) -> Vec<Url> {
        let seed_path = seed_url.path();

        // Find the "section" path - go up one level from the seed URL
        // e.g., /docs/en/agents-and-tools/agent-skills/overview
        //    -> /docs/en/agents-and-tools/ (parent section)
        let path_parts: Vec<&str> = seed_path.split('/').filter(|s| !s.is_empty()).collect();

        // Use parent directory as the filter prefix (at least 2 levels deep)
        let filter_depth = path_parts.len().saturating_sub(1).max(2);
        let filter_prefix = if filter_depth > 0 && filter_depth <= path_parts.len() {
            format!("/{}/", path_parts[..filter_depth].join("/"))
        } else {
            "/".to_string()
        };

        info!(
            seed_path = seed_path,
            filter_prefix = %filter_prefix,
            "Filtering sitemap URLs by path prefix"
        );

        sitemap_urls
            .into_iter()
            .filter(|url| {
                url.host() == seed_url.host() && url.path().starts_with(&filter_prefix)
            })
            .collect()
    }

    /// Start crawling from a seed URL with PARALLEL processing per depth level
    pub async fn crawl<F>(&self, seed_url: Url, on_result: F) -> Result<CrawlStats, CrawlerError>
    where
        F: Fn(CrawlResult) + Send + Sync,
    {
        use futures::stream::{self, StreamExt};
        use std::sync::atomic::{AtomicUsize, Ordering};

        info!(url = %seed_url, max_depth = self.config.max_depth, concurrency = self.config.concurrent_requests, "Starting parallel crawl");

        // Add seed URL
        self.frontier.add(CrawlTask::seed(seed_url.clone())).await;

        // Use atomic counters for parallel access
        let pages_crawled = AtomicUsize::new(0);
        let bytes_downloaded = std::sync::atomic::AtomicU64::new(0);
        let links_discovered = AtomicUsize::new(0);
        let links_queued = AtomicUsize::new(0);
        let robots_blocked = AtomicUsize::new(0);
        let errors = AtomicUsize::new(0);
        let mut sitemap_urls_discovered = 0;

        // Fetch sitemap URLs if enabled
        if self.config.use_sitemaps {
            let sitemap_urls = self.robots_checker.fetch_all_sitemap_urls(&seed_url).await;
            if !sitemap_urls.is_empty() {
                info!(count = sitemap_urls.len(), "Discovered URLs from sitemap");
                sitemap_urls_discovered = sitemap_urls.len();

                // Filter to only URLs related to the seed URL's section
                let related_urls = Self::filter_related_sitemap_urls(&seed_url, sitemap_urls);
                info!(
                    total = sitemap_urls_discovered,
                    related = related_urls.len(),
                    "Filtered sitemap URLs to related section"
                );

                // Add filtered sitemap URLs to frontier at depth 0
                let sitemap_tasks: Vec<CrawlTask> = related_urls
                    .into_iter()
                    .filter(|url| self.is_domain_allowed(url))
                    .map(CrawlTask::seed)
                    .collect();

                let added = self.frontier.add_many(sitemap_tasks).await;
                info!(added = added, "Added sitemap URLs to frontier");
            }
        }

        // Main parallel crawl loop
        let mut loop_iteration = 0u64;
        while !self.frontier.is_done().await {
            loop_iteration += 1;

            // Log every 100 iterations when potentially stuck in empty batch loop
            if loop_iteration % 100 == 0 {
                let stats = self.frontier.stats().await;
                info!(
                    iteration = loop_iteration,
                    pages = pages_crawled.load(Ordering::Relaxed),
                    pending = stats.pending_count,
                    in_progress = stats.in_progress_count,
                    "Crawl loop iteration"
                );
            }

            // Check cancellation
            if self.cancellation.is_cancelled() {
                info!("Crawl cancelled");
                return Err(CrawlerError::Cancelled);
            }

            // Check page limit
            if pages_crawled.load(Ordering::Relaxed) >= self.config.max_pages {
                info!(pages = pages_crawled.load(Ordering::Relaxed), "Maximum pages reached");
                break;
            }

            // Collect a batch of tasks to process in parallel
            let batch_size = self.config.concurrent_requests.min(
                self.config.max_pages.saturating_sub(pages_crawled.load(Ordering::Relaxed))
            );

            let mut batch: Vec<CrawlTask> = Vec::with_capacity(batch_size);
            for _ in 0..batch_size {
                if let Some(task) = self.frontier.next().await {
                    // Pre-filter tasks
                    if task.depth > self.config.max_depth {
                        self.frontier.complete(&task.url);
                        continue;
                    }
                    if !self.is_domain_allowed(&task.url) {
                        debug!(url = %task.url, "Domain not allowed");
                        self.frontier.complete(&task.url);
                        continue;
                    }
                    batch.push(task);
                } else {
                    break;
                }
            }

            if batch.is_empty() {
                // Log frontier state when batch is empty but not done
                let frontier_stats = self.frontier.stats().await;
                // Use WARN level since this is an unusual state worth investigating
                warn!(
                    pending = frontier_stats.pending_count,
                    in_progress = frontier_stats.in_progress_count,
                    total_discovered = frontier_stats.total_discovered,
                    "Empty batch but not done - waiting for in-progress tasks"
                );
                // Wait a bit for in-progress tasks
                tokio::time::sleep(Duration::from_millis(50)).await;
                continue;
            }

            debug!(batch_size = batch.len(), "Processing batch in parallel with streaming");

            // Process batch in parallel using buffer_unordered - STREAMING results as they complete
            let result_stream = stream::iter(batch)
                .map(|task| async {
                    // Acquire semaphore permit (handle closed semaphore gracefully)
                    let _permit = match self.semaphore.acquire().await {
                        Ok(p) => p,
                        Err(_) => {
                            return (task.clone(), Err(CrawlerError::BrowserError(
                                "Semaphore closed".to_string()
                            )));
                        }
                    };

                    // Check robots.txt
                    if self.config.respect_robots {
                        if !self.robots_checker.is_allowed(&task.url).await {
                            debug!(url = %task.url, "Blocked by robots.txt");
                            let url_string = task.url.to_string();
                            return (task, Err(CrawlerError::RobotsDisallowed(url_string)));
                        }

                        // Apply crawl delay from robots.txt
                        if let Some(delay) = self.robots_checker.get_crawl_delay(&task.url).await {
                            if let Some(domain) = task.url.domain() {
                                self.rate_limiter.set_crawl_delay(domain, delay);
                            }
                        }
                    }

                    // Apply rate limiting
                    if let Some(domain) = task.url.domain() {
                        self.rate_limiter.acquire(domain).await;
                    }

                    // Fetch the page
                    let result = self.fetch_page(&task.url).await;
                    (task, result)
                })
                .buffer_unordered(self.config.concurrent_requests);

            // Pin the stream for iteration
            futures::pin_mut!(result_stream);

            // Process results IMMEDIATELY as each page completes (no waiting for batch)
            while let Some((task, result)) = result_stream.next().await {
                match result {
                    Ok(mut crawl_result) => {
                        pages_crawled.fetch_add(1, Ordering::Relaxed);
                        bytes_downloaded.fetch_add(crawl_result.size as u64, Ordering::Relaxed);

                        // Extract and queue links - only if we're not at max depth
                        if task.depth < self.config.max_depth {
                            let new_tasks: Vec<CrawlTask> = crawl_result
                                .links
                                .iter()
                                .filter(|url| self.is_domain_allowed(url))
                                .map(|url| task.child(url.clone()))
                                .collect();

                            let added = self.frontier.add_many(new_tasks).await;
                            links_discovered.fetch_add(crawl_result.links.len(), Ordering::Relaxed);
                            links_queued.fetch_add(added, Ordering::Relaxed);
                        } else {
                            // At max depth - clear links so they're not reported as "discovered"
                            // since they won't be crawled. This ensures discovered == processable.
                            crawl_result.links.clear();
                        }

                        // Call result handler IMMEDIATELY - no waiting for other pages
                        on_result(crawl_result);

                        self.frontier.complete(&task.url);
                    }
                    Err(CrawlerError::RobotsDisallowed(_)) => {
                        robots_blocked.fetch_add(1, Ordering::Relaxed);
                        self.frontier.complete(&task.url);
                    }
                    Err(e) => {
                        warn!(url = %task.url, error = %e, "Failed to fetch page");
                        errors.fetch_add(1, Ordering::Relaxed);

                        // Retry on transient errors
                        let should_retry = matches!(&e,
                            CrawlerError::Http(e) if e.is_timeout() || e.is_connect()
                        );
                        self.frontier.fail(task, should_retry).await;
                    }
                }
            }
            // Log after batch completes
            debug!(
                pages = pages_crawled.load(Ordering::Relaxed),
                errors = errors.load(Ordering::Relaxed),
                "Batch processing completed"
            );
        }

        let stats = CrawlStats {
            pages_crawled: pages_crawled.load(Ordering::Relaxed),
            bytes_downloaded: bytes_downloaded.load(Ordering::Relaxed),
            links_discovered: links_discovered.load(Ordering::Relaxed),
            links_queued: links_queued.load(Ordering::Relaxed),
            robots_blocked: robots_blocked.load(Ordering::Relaxed),
            errors: errors.load(Ordering::Relaxed),
            sitemap_urls_discovered,
        };

        info!(
            pages = stats.pages_crawled,
            bytes = stats.bytes_downloaded,
            errors = stats.errors,
            "Crawl completed"
        );

        Ok(stats)
    }

    /// Fetch a single page (routes to browser or raw based on config)
    async fn fetch_page(&self, url: &Url) -> Result<CrawlResult, CrawlerError> {
        #[cfg(feature = "browser")]
        if let Some(ref browser_pool) = self.browser_pool {
            return self.fetch_page_with_browser(url, browser_pool).await;
        }

        // Fall back to raw HTML fetching
        self.fetch_page_raw(url).await
    }

    /// Fetch a page using browser rendering (for JavaScript-heavy sites)
    #[cfg(feature = "browser")]
    async fn fetch_page_with_browser(
        &self,
        url: &Url,
        browser_pool: &BrowserPool,
    ) -> Result<CrawlResult, CrawlerError> {
        let start = std::time::Instant::now();

        debug!(url = %url, "Rendering page with browser");

        // Render with browser
        let render_result = browser_pool.render(url).await.map_err(|e| {
            CrawlerError::BrowserError(format!(
                "Browser rendering failed for {}: {}. Ensure Chromium is installed.",
                url, e
            ))
        })?;

        // Parse rendered HTML
        let document = Html::parse_document(&render_result.html);

        // Extract title from rendered content
        let title_selector = Selector::parse("title").unwrap();
        let title = document
            .select(&title_selector)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string())
            .or(render_result.title);

        // Extract links
        let link_selector = Selector::parse("a[href]").unwrap();
        let links: Vec<Url> = document
            .select(&link_selector)
            .filter_map(|el| el.value().attr("href"))
            .filter_map(|href| url.join(href).ok())
            .filter(|u| u.scheme() == "http" || u.scheme() == "https")
            .collect();

        // Extract structured content from RENDERED HTML
        let structured_content = extract_structured_content(&render_result.html, &document);
        let content = structured_content.plain_text.clone();
        let source_domain = url.domain().map(|d| d.to_string());

        info!(
            url = %url,
            render_time_ms = render_result.render_time_ms,
            content_len = content.len(),
            "Page rendered with browser"
        );

        Ok(CrawlResult {
            url: render_result.url,
            status: 200,
            title,
            content,
            structured_content,
            links,
            content_type: "text/html".to_string(),
            size: render_result.html.len(),
            fetch_time_ms: start.elapsed().as_millis() as u64,
            source_domain,
        })
    }

    /// Fetch a single page using raw HTTP (no JavaScript rendering)
    async fn fetch_page_raw(&self, url: &Url) -> Result<CrawlResult, CrawlerError> {
        let start = std::time::Instant::now();

        let response = self.client.get(url.as_str()).send().await?;

        let status = response.status().as_u16();
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("text/html")
            .to_string();

        // Extract source domain
        let source_domain = url.domain().map(|d| d.to_string());

        // Only process HTML pages
        if !content_type.contains("text/html") && !content_type.contains("application/xhtml") {
            return Ok(CrawlResult {
                url: url.clone(),
                status,
                title: None,
                content: String::new(),
                structured_content: ExtractedContent::default(),
                links: vec![],
                content_type,
                size: 0,
                fetch_time_ms: start.elapsed().as_millis() as u64,
                source_domain,
            });
        }

        let body = response.text().await?;
        let size = body.len();

        // Parse HTML
        let document = Html::parse_document(&body);

        // Extract title
        let title_selector = Selector::parse("title").unwrap();
        let title = document
            .select(&title_selector)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string());

        // Extract links
        let link_selector = Selector::parse("a[href]").unwrap();
        let links: Vec<Url> = document
            .select(&link_selector)
            .filter_map(|el| el.value().attr("href"))
            .filter_map(|href| url.join(href).ok())
            .filter(|u| u.scheme() == "http" || u.scheme() == "https")
            .collect();

        // Extract structured content (markdown, code blocks, headers)
        let structured_content = extract_structured_content(&body, &document);

        // Keep plain text for backward compatibility
        let content = structured_content.plain_text.clone();

        Ok(CrawlResult {
            url: url.clone(),
            status,
            title,
            content,
            structured_content,
            links,
            content_type,
            size,
            fetch_time_ms: start.elapsed().as_millis() as u64,
            source_domain,
        })
    }

    /// Check if a domain is allowed
    fn is_domain_allowed(&self, url: &Url) -> bool {
        let domain = match url.domain() {
            Some(d) => d,
            None => return false,
        };

        // Check blocked domains
        for blocked in &self.config.blocked_domains {
            if domain.ends_with(blocked) {
                return false;
            }
        }

        // Check allowed domains (if specified)
        if !self.config.allowed_domains.is_empty() {
            return self.config.allowed_domains.iter().any(|allowed| domain.ends_with(allowed));
        }

        true
    }

    /// Cancel the crawl
    pub fn cancel(&self) {
        self.cancellation.cancel();
    }

    /// Get frontier statistics
    pub async fn stats(&self) -> crate::frontier::FrontierStats {
        self.frontier.stats().await
    }
}

/// Extract structured content from HTML including markdown, code blocks, and headers
fn extract_structured_content(html: &str, document: &Html) -> ExtractedContent {
    // Convert HTML to markdown using htmd
    let converter = HtmlToMarkdown::new();
    let markdown = converter.convert(html).unwrap_or_default();

    // Extract code blocks
    let code_blocks = extract_code_blocks(document);

    // Extract headers
    let headers = extract_headers(document);

    // Extract plain text for embeddings (fallback)
    let plain_text = extract_plain_text(document);

    ExtractedContent {
        markdown,
        code_blocks,
        headers,
        plain_text,
    }
}

/// Extract code blocks from HTML (pre, code elements)
fn extract_code_blocks(document: &Html) -> Vec<CodeBlock> {
    let mut blocks = Vec::new();

    // Match <pre><code> and standalone <pre> elements
    let pre_selector = Selector::parse("pre").unwrap();
    let code_selector = Selector::parse("code").unwrap();

    for pre in document.select(&pre_selector) {
        // Check if there's a nested code element
        let (content, language) = if let Some(code_el) = pre.select(&code_selector).next() {
            let content = code_el.text().collect::<String>();
            let language = extract_language_from_class(code_el.value());
            (content, language)
        } else {
            let content = pre.text().collect::<String>();
            let language = extract_language_from_class(pre.value());
            (content, language)
        };

        let trimmed = content.trim();
        if !trimmed.is_empty() {
            let line_count = trimmed.lines().count();
            blocks.push(CodeBlock {
                language,
                content: trimmed.to_string(),
                line_count,
            });
        }
    }

    // Also match standalone code elements (inline code, but significant ones)
    for code in document.select(&code_selector) {
        // Skip if already captured inside a pre
        if code.parent().and_then(|p| p.value().as_element()).map(|e| e.name() == "pre").unwrap_or(false) {
            continue;
        }

        let content = code.text().collect::<String>();
        let trimmed = content.trim();

        // Only capture multi-line code or significant code blocks
        if trimmed.lines().count() > 1 || trimmed.len() > 100 {
            let language = extract_language_from_class(code.value());
            blocks.push(CodeBlock {
                language,
                content: trimmed.to_string(),
                line_count: trimmed.lines().count(),
            });
        }
    }

    blocks
}

/// Extract language hint from class attribute (e.g., "language-rust", "lang-python", "rust")
fn extract_language_from_class(element: &scraper::node::Element) -> Option<String> {
    let class = element.attr("class")?;

    // Common patterns: "language-X", "lang-X", just "X"
    for part in class.split_whitespace() {
        if let Some(lang) = part.strip_prefix("language-") {
            return Some(lang.to_string());
        }
        if let Some(lang) = part.strip_prefix("lang-") {
            return Some(lang.to_string());
        }
        // Check if it's a known language name directly
        let known_langs = [
            "rust", "python", "javascript", "typescript", "java", "go", "c", "cpp",
            "csharp", "ruby", "php", "swift", "kotlin", "scala", "bash", "shell",
            "sql", "html", "css", "json", "yaml", "toml", "xml", "markdown",
        ];
        if known_langs.contains(&part.to_lowercase().as_str()) {
            return Some(part.to_lowercase());
        }
    }

    None
}

/// Extract headers from HTML (h1-h6) in DOM order
fn extract_headers(document: &Html) -> Vec<ExtractedHeader> {
    let mut headers = Vec::new();
    let body_selector = Selector::parse("body").unwrap();

    if let Some(body) = document.select(&body_selector).next() {
        // Traverse DOM in document order to preserve visual reading order
        for node in body.descendants() {
            if let Some(el) = node.value().as_element() {
                let name = el.name();
                // Check if it's a header element (h1-h6)
                let level = match name {
                    "h1" => Some(1u8),
                    "h2" => Some(2u8),
                    "h3" => Some(3u8),
                    "h4" => Some(4u8),
                    "h5" => Some(5u8),
                    "h6" => Some(6u8),
                    _ => None,
                };

                if let Some(level) = level {
                    if let Some(el_ref) = scraper::ElementRef::wrap(node) {
                        let text = el_ref.text().collect::<String>().trim().to_string();
                        if !text.is_empty() {
                            headers.push(ExtractedHeader { level, text });
                        }
                    }
                }
            }
        }
    }

    headers
}

/// Extract plain text from HTML (for embeddings)
fn extract_plain_text(document: &Html) -> String {
    let body_selector = Selector::parse("body").unwrap();
    let skip_selector = Selector::parse("script, style, nav, footer, header, noscript").unwrap();

    let mut text = String::new();

    if let Some(body) = document.select(&body_selector).next() {
        for node in body.descendants() {
            if let Some(_el) = node.value().as_element() {
                // Skip certain elements
                if skip_selector.matches(&scraper::ElementRef::wrap(node).unwrap()) {
                    continue;
                }
            }

            if let Some(t) = node.value().as_text() {
                let trimmed = t.trim();
                if !trimmed.is_empty() {
                    text.push_str(trimmed);
                    text.push(' ');
                }
            }
        }
    }

    text.trim().to_string()
}

/// Crawl statistics
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CrawlStats {
    pub pages_crawled: usize,
    pub bytes_downloaded: u64,
    pub links_discovered: usize,
    pub links_queued: usize,
    pub robots_blocked: usize,
    pub errors: usize,
    pub sitemap_urls_discovered: usize,
}