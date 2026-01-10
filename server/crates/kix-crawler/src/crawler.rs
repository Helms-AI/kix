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

use crate::frontier::{CrawlFrontier, CrawlTask};
use crate::rate_limiter::DomainRateLimiter;
use crate::robots::RobotsChecker;
use crate::CrawlerError;

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
    /// Render JavaScript using headless browser
    pub render_js: bool,
    /// Use sitemaps to discover URLs (from robots.txt or default locations)
    pub use_sitemaps: bool,
}

impl Default for CrawlerConfig {
    fn default() -> Self {
        Self {
            max_depth: 3,
            max_pages: 10000,            // Increased from 1000 for large crawls
            max_pages_per_domain: Some(500),  // Increased from 100
            concurrent_requests: 64,     // Increased from 4 for high throughput
            timeout: Duration::from_secs(30),
            respect_robots: true,
            follow_redirects: true,
            user_agent: "EIP-Crawler/1.0 (+https://github.com/eip-knowledge)".to_string(),
            allowed_domains: vec![],
            blocked_domains: vec![],
            render_js: false,
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
}

impl Crawler {
    /// Create a new crawler
    pub fn new(config: CrawlerConfig) -> Result<Self, CrawlerError> {
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
        let semaphore = Arc::new(Semaphore::new(config.concurrent_requests));

        Ok(Self {
            client,
            config,
            frontier: Arc::new(CrawlFrontier::with_defaults()),
            rate_limiter: Arc::new(DomainRateLimiter::with_defaults()),
            robots_checker,
            semaphore,
            cancellation: CancellationToken::new(),
        })
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

    /// Start crawling from a seed URL
    pub async fn crawl<F>(&self, seed_url: Url, mut on_result: F) -> Result<CrawlStats, CrawlerError>
    where
        F: FnMut(CrawlResult) + Send,
    {
        info!(url = %seed_url, max_depth = self.config.max_depth, "Starting crawl");

        // Add seed URL
        self.frontier.add(CrawlTask::seed(seed_url.clone())).await;

        let mut stats = CrawlStats::default();

        // Fetch sitemap URLs if enabled
        if self.config.use_sitemaps {
            let sitemap_urls = self.robots_checker.fetch_all_sitemap_urls(&seed_url).await;
            if !sitemap_urls.is_empty() {
                info!(count = sitemap_urls.len(), "Discovered URLs from sitemap");
                stats.sitemap_urls_discovered = sitemap_urls.len();

                // Filter to only URLs related to the seed URL's section
                let related_urls = Self::filter_related_sitemap_urls(&seed_url, sitemap_urls);
                info!(
                    total = stats.sitemap_urls_discovered,
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

        while !self.frontier.is_done().await {
            // Check cancellation
            if self.cancellation.is_cancelled() {
                info!("Crawl cancelled");
                return Err(CrawlerError::Cancelled);
            }

            // Check page limit
            if stats.pages_crawled >= self.config.max_pages {
                info!(pages = stats.pages_crawled, "Maximum pages reached");
                break;
            }

            // Get next task
            let task = match self.frontier.next().await {
                Some(t) => t,
                None => {
                    // Wait a bit for in-progress tasks
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                }
            };

            // Check depth limit
            if task.depth > self.config.max_depth {
                self.frontier.complete(&task.url);
                continue;
            }

            // Check domain restrictions
            if !self.is_domain_allowed(&task.url) {
                debug!(url = %task.url, "Domain not allowed");
                self.frontier.complete(&task.url);
                continue;
            }

            // Acquire semaphore
            let _permit = self.semaphore.acquire().await.unwrap();

            // Check robots.txt
            if self.config.respect_robots {
                if !self.robots_checker.is_allowed(&task.url).await {
                    debug!(url = %task.url, "Blocked by robots.txt");
                    self.frontier.complete(&task.url);
                    stats.robots_blocked += 1;
                    continue;
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
            match self.fetch_page(&task.url).await {
                Ok(result) => {
                    stats.pages_crawled += 1;
                    stats.bytes_downloaded += result.size as u64;

                    // Extract and queue links
                    if task.depth < self.config.max_depth {
                        let new_tasks: Vec<CrawlTask> = result
                            .links
                            .iter()
                            .filter(|url| self.is_domain_allowed(url))
                            .map(|url| task.child(url.clone()))
                            .collect();

                        let added = self.frontier.add_many(new_tasks).await;
                        stats.links_discovered += result.links.len();
                        stats.links_queued += added;
                    }

                    // Call result handler
                    on_result(result);

                    self.frontier.complete(&task.url);
                }
                Err(e) => {
                    warn!(url = %task.url, error = %e, "Failed to fetch page");
                    stats.errors += 1;

                    // Retry on transient errors
                    let should_retry = matches!(&e,
                        CrawlerError::Http(e) if e.is_timeout() || e.is_connect()
                    );
                    self.frontier.fail(task, should_retry).await;
                }
            }
        }

        info!(
            pages = stats.pages_crawled,
            bytes = stats.bytes_downloaded,
            errors = stats.errors,
            "Crawl completed"
        );

        Ok(stats)
    }

    /// Fetch a single page
    async fn fetch_page(&self, url: &Url) -> Result<CrawlResult, CrawlerError> {
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

/// Extract headers from HTML (h1-h6)
fn extract_headers(document: &Html) -> Vec<ExtractedHeader> {
    let mut headers = Vec::new();

    for level in 1..=6 {
        let selector = Selector::parse(&format!("h{}", level)).unwrap();
        for el in document.select(&selector) {
            let text = el.text().collect::<String>().trim().to_string();
            if !text.is_empty() {
                headers.push(ExtractedHeader {
                    level: level as u8,
                    text,
                });
            }
        }
    }

    // Sort by document order (approximate by collecting them in order)
    // Note: The above loop collects in h1, h2, h3... order, not document order
    // For proper document order, we'd need to traverse the DOM differently
    // This is good enough for most use cases
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