//! Spider-based crawler adapter.
//!
//! This module provides an adapter layer over the spider crate,
//! exposing raw HTML for code extraction while providing markdown
//! for content processing.

use spider::website::Website;
use spider::page::Page;
use spider_transformations::transformation::content::{self, TransformConfig, ReturnFormat};
use tokio_stream::Stream;
use std::pin::Pin;
use tokio::sync::mpsc;

use super::config::{SpiderConfig, CrawlMode};
use crate::extraction::{CodeExtractor, CodeExtractionConfig, ExtractionResult};

/// Crawled page with raw HTML, markdown, and extracted code
#[derive(Debug, Clone)]
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

    /// Content hash for deduplication
    pub content_hash: String,

    /// Code extraction result (from CodeExtractor)
    pub code_extraction: Option<ExtractionResult>,
}

/// Spider-based crawler adapter
pub struct SpiderCrawler {
    config: SpiderConfig,
}

impl SpiderCrawler {
    /// Create a new spider crawler with the given configuration
    pub fn new(config: SpiderConfig) -> Self {
        Self { config }
    }

    /// Create a crawler with default configuration
    pub fn with_defaults() -> Self {
        Self::new(SpiderConfig::default())
    }

    /// Crawl a URL and return a stream of pages
    pub async fn crawl(
        &self,
        url: &str,
    ) -> Pin<Box<dyn Stream<Item = Result<CrawledPage, CrawlError>> + Send>> {
        let (tx, rx) = mpsc::unbounded_channel();
        let config = self.config.clone();
        let url = url.to_string();

        // Spawn crawl task
        tokio::spawn(async move {
            let mut website = Website::new(&url);
            Self::configure_website_static(&mut website, &config);

            // Subscribe to page events
            let mut rx_pages = website.subscribe(16).expect("Failed to subscribe");

            // Start crawling in background
            let crawl_handle = tokio::spawn(async move {
                website.crawl().await;
                website.unsubscribe();
            });

            // Process pages as they come in
            while let Ok(page) = rx_pages.recv().await {
                let result = Self::process_page_static(&page, &config);
                if tx.send(result).is_err() {
                    break; // Receiver dropped
                }
            }

            // Wait for crawl to complete
            let _ = crawl_handle.await;
        });

        Box::pin(tokio_stream::wrappers::UnboundedReceiverStream::new(rx))
    }

    /// Crawl a single page
    pub async fn crawl_single(&self, url: &str) -> Result<CrawledPage, CrawlError> {
        let mut website = Website::new(url);
        Self::configure_website_static(&mut website, &self.config);

        // For single page, set depth to 0
        website.with_depth(0);

        // Scrape the page
        website.scrape().await;

        let pages = website.get_pages();
        match pages {
            Some(pages_map) => {
                // Get the first page from the DashMap
                if let Some(entry) = pages_map.iter().next() {
                    return Self::process_page_static(&entry, &self.config);
                }
                Err(CrawlError::NoPages)
            }
            None => Err(CrawlError::NoPages),
        }
    }

    /// Configure a spider Website with our settings
    fn configure_website_static(website: &mut Website, config: &SpiderConfig) {
        // Robots.txt
        website.with_respect_robots_txt(config.respect_robots_txt);

        // Depth
        if let Some(depth) = config.max_depth {
            website.with_depth(depth);
        }

        // Request timeout
        website.with_request_timeout(Some(config.timeout));

        // Rate limiting
        if let Some(delay) = config.rate_limit {
            website.with_delay(delay.as_millis() as u64);
        }

        // Smart mode detection
        match config.mode {
            CrawlMode::HttpOnly => {
                // Default HTTP-only mode, no special config
            }
            CrawlMode::Smart => {
                // Enable smart mode for JS detection
                // Spider will use HTTP first and fall back to rendering if needed
            }
            CrawlMode::JsRequired => {
                // Chrome rendering would require the chrome feature
                // For now, this falls back to HTTP
                tracing::warn!("JsRequired mode requires chrome feature; falling back to HTTP");
            }
        }

        // Caching
        if config.enable_cache {
            website.with_caching(true);
        }
    }

    /// Process a spider Page into our CrawledPage format
    fn process_page_static(page: &Page, _config: &SpiderConfig) -> Result<CrawledPage, CrawlError> {
        let url = page.get_url().to_string();
        let html = page.get_html().to_string();
        let status = page.status_code.as_u16();

        // Skip non-successful responses
        if status >= 400 {
            return Err(CrawlError::HttpError { url, status });
        }

        // Convert to markdown using spider_transformations
        let transform_config = TransformConfig {
            return_format: ReturnFormat::Markdown,
            ..Default::default()
        };
        let markdown = content::transform_content(page, &transform_config, &None, &None, &None);

        // Extract title
        let title = Self::extract_title(&html);

        // Compute content hash
        let content_hash = Self::compute_hash(&html);

        // Extract code blocks from HTML
        let extractor = CodeExtractor::new(CodeExtractionConfig::default());
        let code_extraction = Some(extractor.extract(&html));

        Ok(CrawledPage {
            url,
            html,
            markdown,
            status,
            title,
            content_hash,
            code_extraction,
        })
    }

    /// Extract title from HTML
    fn extract_title(html: &str) -> Option<String> {
        // Simple title extraction - look for <title> tag
        let html_lower = html.to_lowercase();
        let start = html_lower.find("<title>")?;
        let end = html_lower.find("</title>")?;

        if end <= start + 7 {
            return None;
        }

        // Get the original case title
        let title = &html[start + 7..end];
        let trimmed = title.trim();

        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    }

    /// Compute SHA256 hash of content
    fn compute_hash(content: &str) -> String {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}

/// Errors that can occur during crawling
#[derive(Debug, thiserror::Error)]
pub enum CrawlError {
    /// HTTP error response
    #[error("HTTP error {status} for {url}")]
    HttpError { url: String, status: u16 },

    /// No pages returned from crawl
    #[error("No pages returned")]
    NoPages,

    /// Crawl was cancelled
    #[error("Crawl cancelled")]
    Cancelled,

    /// Spider internal error
    #[error("Spider error: {0}")]
    SpiderError(String),

    /// URL parse error
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_title_extraction() {
        let html = r#"<html><head><title>Test Page Title</title></head><body></body></html>"#;
        let title = SpiderCrawler::extract_title(html);
        assert_eq!(title, Some("Test Page Title".to_string()));
    }

    #[test]
    fn test_title_extraction_empty() {
        let html = r#"<html><head><title></title></head><body></body></html>"#;
        let title = SpiderCrawler::extract_title(html);
        assert_eq!(title, None);
    }

    #[test]
    fn test_title_extraction_no_title() {
        let html = r#"<html><head></head><body></body></html>"#;
        let title = SpiderCrawler::extract_title(html);
        assert_eq!(title, None);
    }

    #[test]
    fn test_content_hash() {
        let content = "Hello, World!";
        let hash = SpiderCrawler::compute_hash(content);
        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 64); // SHA256 produces 64 hex characters
    }

    #[test]
    fn test_crawl_mode_default() {
        let config = SpiderConfig::default();
        assert_eq!(config.mode, CrawlMode::Smart);
    }
}
