# Phase 1: Spider Integration

**Duration**: 3-4 days
**Dependencies**: Phase 0
**Status**: Not Started

---

## Objective

Replace kix-crawler's custom crawling infrastructure with the spider crate while maintaining access to raw HTML for code extraction.

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    Spider Integration Layer                      │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  kix-jobs/processor.rs                                          │
│         │                                                        │
│         ▼                                                        │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  SpiderCrawler (NEW)                                     │    │
│  │  ├─ config: SpiderConfig                                │    │
│  │  ├─ crawl(url) → Stream<CrawledPage>                   │    │
│  │  └─ process_page(Page) → CrawledPage                   │    │
│  └─────────────────────────────────────────────────────────┘    │
│                    │                                             │
│         ┌─────────┴─────────┐                                   │
│         ▼                   ▼                                    │
│  ┌─────────────┐     ┌─────────────────┐                        │
│  │ spider      │     │ spider_         │                        │
│  │ Website     │     │ transformations │                        │
│  │ .crawl()    │     │ → markdown      │                        │
│  └─────────────┘     └─────────────────┘                        │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## Tasks

### 1.1 Add Dependencies

**File**: `server/Cargo.toml` (workspace)

```toml
[workspace.dependencies]
spider = { version = "2", features = ["sync", "smart", "cache", "sitemap", "chrome"] }
spider_transformations = "2"
```

**File**: `server/crates/kix-jobs/Cargo.toml`

```toml
[dependencies]
spider = { workspace = true }
spider_transformations = { workspace = true }
```

**Verification**:
```bash
cargo check -p kix-jobs
```

---

### 1.2 Create SpiderConfig

**File**: `server/crates/kix-jobs/src/crawler/config.rs` (NEW)

```rust
use std::time::Duration;

/// Configuration for spider-based crawling
#[derive(Debug, Clone)]
pub struct SpiderConfig {
    /// Crawling mode
    pub mode: CrawlMode,

    /// Respect robots.txt
    pub respect_robots_txt: bool,

    /// Use sitemap for discovery
    pub use_sitemap: bool,

    /// Enable HTTP caching (ETag/Last-Modified)
    pub enable_cache: bool,

    /// Maximum crawl depth
    pub max_depth: Option<usize>,

    /// Concurrent requests
    pub concurrency: usize,

    /// Rate limit delay between requests
    pub rate_limit: Option<Duration>,

    /// Request timeout
    pub timeout: Duration,

    /// Maximum pages to crawl (0 = unlimited)
    pub max_pages: usize,

    /// URL patterns to include (regex)
    pub include_patterns: Vec<String>,

    /// URL patterns to exclude (regex)
    pub exclude_patterns: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrawlMode {
    /// HTTP only, no JavaScript rendering
    HttpOnly,
    /// Smart mode: HTTP first, JS fallback if needed
    Smart,
    /// Always use JavaScript rendering
    JsRequired,
}

impl Default for SpiderConfig {
    fn default() -> Self {
        Self {
            mode: CrawlMode::Smart,
            respect_robots_txt: true,
            use_sitemap: true,
            enable_cache: true,
            max_depth: Some(3),
            concurrency: 10,
            rate_limit: Some(Duration::from_millis(100)),
            timeout: Duration::from_secs(30),
            max_pages: 0,
            include_patterns: vec![],
            exclude_patterns: vec![],
        }
    }
}

impl SpiderConfig {
    /// Create config for single page crawl
    pub fn single_page() -> Self {
        Self {
            max_depth: Some(0),
            max_pages: 1,
            use_sitemap: false,
            ..Default::default()
        }
    }

    /// Create config for documentation site
    pub fn documentation() -> Self {
        Self {
            mode: CrawlMode::Smart,
            max_depth: Some(5),
            exclude_patterns: vec![
                r".*\.(png|jpg|gif|svg|css|js)$".to_string(),
                r".*/search.*".to_string(),
            ],
            ..Default::default()
        }
    }
}
```

---

### 1.3 Create SpiderCrawler Adapter

**File**: `server/crates/kix-jobs/src/crawler/spider_adapter.rs` (NEW)

```rust
use spider::website::Website;
use spider::page::Page;
use spider_transformations::transformation::content::{self, TransformConfig, ReturnFormat};
use tokio_stream::{Stream, StreamExt};
use url::Url;

use super::config::{SpiderConfig, CrawlMode};

/// Crawled page with raw HTML and markdown
pub struct CrawledPage {
    /// Page URL
    pub url: String,

    /// Raw HTML (for code extraction)
    pub html: String,

    /// Markdown content (from spider_transformations)
    pub markdown: String,

    /// HTTP status code
    pub status: u16,

    /// Page title (if found)
    pub title: Option<String>,
}

/// Spider-based crawler adapter
pub struct SpiderCrawler {
    config: SpiderConfig,
}

impl SpiderCrawler {
    pub fn new(config: SpiderConfig) -> Self {
        Self { config }
    }

    /// Crawl a URL and return a stream of pages
    pub async fn crawl(&self, url: &str) -> impl Stream<Item = Result<CrawledPage, CrawlError>> {
        let mut website = Website::new(url);

        // Apply configuration
        self.configure_website(&mut website);

        // Start crawling
        let rx = website.subscribe(0).unwrap();

        // Spawn crawl task
        let handle = tokio::spawn(async move {
            website.crawl().await;
        });

        // Convert receiver to stream
        tokio_stream::wrappers::UnboundedReceiverStream::new(rx)
            .map(|page| self.process_page(page))
    }

    /// Crawl a single page
    pub async fn crawl_single(&self, url: &str) -> Result<CrawledPage, CrawlError> {
        let mut website = Website::new(url);
        self.configure_website(&mut website);
        website.configuration.depth = 0;

        website.scrape().await;

        let pages = website.get_pages();
        pages
            .first()
            .map(|p| self.process_page_sync(p))
            .ok_or(CrawlError::NoPages)?
    }

    fn configure_website(&self, website: &mut Website) {
        let config = &self.config;

        // Robots.txt
        website.configuration.respect_robots_txt = config.respect_robots_txt;

        // Depth
        if let Some(depth) = config.max_depth {
            website.configuration.depth = depth;
        }

        // Concurrency
        website.configuration.request_timeout = Some(config.timeout);

        // Rate limiting
        if let Some(delay) = config.rate_limit {
            website.configuration.delay = delay.as_millis() as u64;
        }

        // Smart mode / Chrome
        match config.mode {
            CrawlMode::HttpOnly => {
                // Default HTTP mode
            }
            CrawlMode::Smart => {
                website.configuration.smart = true;
            }
            CrawlMode::JsRequired => {
                website.configuration.chrome = true;
            }
        }

        // Caching
        if config.enable_cache {
            website.configuration.cache = true;
        }
    }

    fn process_page(&self, page: Page) -> Result<CrawledPage, CrawlError> {
        self.process_page_sync(&page)
    }

    fn process_page_sync(&self, page: &Page) -> Result<CrawledPage, CrawlError> {
        let url = page.get_url().to_string();
        let html = page.get_html().to_string();
        let status = page.status_code.as_u16();

        // Skip non-successful responses
        if status >= 400 {
            return Err(CrawlError::HttpError { url, status });
        }

        // Convert to markdown using spider_transformations
        let mut conf = TransformConfig::default();
        conf.return_format = ReturnFormat::Markdown;
        let markdown = content::transform_content(page, &conf, &None, &None)
            .unwrap_or_default();

        // Extract title
        let title = self.extract_title(&html);

        Ok(CrawledPage {
            url,
            html,
            markdown,
            status,
            title,
        })
    }

    fn extract_title(&self, html: &str) -> Option<String> {
        // Simple title extraction
        let start = html.find("<title>")?;
        let end = html.find("</title>")?;
        let title = &html[start + 7..end];
        Some(title.trim().to_string())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CrawlError {
    #[error("HTTP error {status} for {url}")]
    HttpError { url: String, status: u16 },

    #[error("No pages returned")]
    NoPages,

    #[error("Crawl cancelled")]
    Cancelled,

    #[error("Spider error: {0}")]
    SpiderError(String),
}
```

---

### 1.4 Create Module Structure

**File**: `server/crates/kix-jobs/src/crawler/mod.rs` (NEW)

```rust
mod config;
mod spider_adapter;

pub use config::{SpiderConfig, CrawlMode};
pub use spider_adapter::{SpiderCrawler, CrawledPage, CrawlError};
```

**Update**: `server/crates/kix-jobs/src/lib.rs`

```rust
pub mod crawler;

// Re-export for convenience
pub use crawler::{SpiderCrawler, SpiderConfig, CrawledPage};
```

---

### 1.5 Integration with Processor

**File**: `server/crates/kix-jobs/src/processor.rs` (MODIFY)

Add spider integration alongside existing crawler:

```rust
use crate::crawler::{SpiderCrawler, SpiderConfig, CrawledPage};

impl ContentProcessor {
    /// Process URL using spider
    pub async fn process_url_with_spider(
        &self,
        url: &str,
        config: SpiderConfig,
        progress: Option<&SharedProgressTracker>,
    ) -> Result<ProcessingResult, ProcessorError> {
        let crawler = SpiderCrawler::new(config);

        // Update progress: starting
        if let Some(p) = progress {
            p.update(CrawlStage::Starting, 0.0);
        }

        // Crawl pages
        let mut stream = crawler.crawl(url).await;
        let mut results = Vec::new();
        let mut pages_processed = 0;

        while let Some(page_result) = stream.next().await {
            match page_result {
                Ok(page) => {
                    // Process the page
                    let entry = self.process_crawled_page(&page).await?;
                    results.push(entry);
                    pages_processed += 1;

                    // Update progress
                    if let Some(p) = progress {
                        p.update(CrawlStage::Processing,
                            (pages_processed as f32 / 100.0) * 100.0);
                    }
                }
                Err(e) => {
                    tracing::warn!("Crawl error: {}", e);
                }
            }
        }

        Ok(ProcessingResult {
            entries: results,
            pages_crawled: pages_processed,
            // ... other fields
        })
    }

    async fn process_crawled_page(&self, page: &CrawledPage) -> Result<Entry, ProcessorError> {
        // Create entry from crawled page
        // This will be enhanced in Phase 2 with CodeExtractor

        let entry = Entry {
            url: page.url.clone(),
            title: page.title.clone().unwrap_or_default(),
            content: page.markdown.clone(),
            source_type: SourceType::Web,
            // ... other fields
        };

        Ok(entry)
    }
}
```

---

### 1.6 Write Tests

**File**: `server/crates/kix-jobs/src/crawler/tests.rs` (NEW)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_spider_single_page() {
        let config = SpiderConfig::single_page();
        let crawler = SpiderCrawler::new(config);

        let result = crawler.crawl_single("https://example.com").await;

        assert!(result.is_ok());
        let page = result.unwrap();
        assert!(!page.html.is_empty());
        assert!(!page.markdown.is_empty());
        assert_eq!(page.status, 200);
    }

    #[tokio::test]
    async fn test_spider_config_modes() {
        // HTTP Only
        let http_config = SpiderConfig {
            mode: CrawlMode::HttpOnly,
            ..Default::default()
        };
        assert!(!http_config.mode == CrawlMode::JsRequired);

        // Smart mode (default)
        let smart_config = SpiderConfig::default();
        assert_eq!(smart_config.mode, CrawlMode::Smart);

        // Documentation preset
        let doc_config = SpiderConfig::documentation();
        assert_eq!(doc_config.max_depth, Some(5));
    }

    #[tokio::test]
    async fn test_spider_markdown_output() {
        let config = SpiderConfig::single_page();
        let crawler = SpiderCrawler::new(config);

        // Test with a page known to have content
        let result = crawler.crawl_single("https://httpbin.org/html").await;

        assert!(result.is_ok());
        let page = result.unwrap();

        // Should have markdown content
        assert!(page.markdown.contains("#") || page.markdown.len() > 100);
    }
}
```

---

## Deliverables

| Deliverable | File | Description |
|-------------|------|-------------|
| SpiderConfig | `crawler/config.rs` | Configuration struct |
| SpiderCrawler | `crawler/spider_adapter.rs` | Adapter layer |
| Module exports | `crawler/mod.rs` | Public API |
| Integration | `processor.rs` | Connected to job processor |
| Tests | `crawler/tests.rs` | Unit tests |

---

## Exit Criteria

- [ ] `cargo check -p kix-jobs` passes
- [ ] Spider crawls single page successfully
- [ ] Spider crawls multi-page site with sitemap
- [ ] Smart mode falls back to JS when needed
- [ ] HTTP caching works (verify with headers)
- [ ] Raw HTML accessible for code extraction (Phase 2)
- [ ] All existing tests still pass

---

## Testing Commands

```bash
# Run spider tests
cargo test -p kix-jobs crawler --release

# Manual verification
cargo run --release -p kix-cli -- \
  index url https://docs.example.com \
  --depth 1 \
  --mode smart
```

---

## Next Phase

Upon completion, proceed to [Phase 2: CodeExtractor Module](./phase-2-code-extractor.md).
