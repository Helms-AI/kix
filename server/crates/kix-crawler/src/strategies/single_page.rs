//! Single Page Strategy
//!
//! Crawls a single page with retry logic and framework detection.
//! Features:
//! - Exponential backoff retries
//! - Framework-specific wait selectors
//! - JavaScript rendering support

use super::{
    calculate_backoff, detect_framework_selectors, CrawlPageResult, CrawlResult, CrawlStrategy,
    StrategyConfig,
};
use crate::progress::{CrawlStage, SharedProgressTracker};
use crate::CrawlerError;
use async_trait::async_trait;
use std::time::Instant;
use tokio_util::sync::CancellationToken;
use url::Url;

/// Single page crawl strategy with retries
pub struct SinglePageStrategy {
    /// HTTP client for requests
    client: reqwest::Client,
}

impl SinglePageStrategy {
    /// Create a new single page strategy
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
        }
    }

    /// Crawl a single page with retry logic
    async fn crawl_page_with_retry(
        &self,
        url: &Url,
        config: &StrategyConfig,
        cancel_token: &CancellationToken,
    ) -> Result<CrawlPageResult, CrawlerError> {
        let mut last_error = None;

        for attempt in 0..=config.max_retries {
            // Check for cancellation
            if cancel_token.is_cancelled() {
                return Err(CrawlerError::Cancelled);
            }

            // Add delay between retries
            if attempt > 0 {
                let delay = calculate_backoff(attempt, config.retry_delay_ms);
                tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
            }

            match self.try_crawl_page(url, config).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    tracing::warn!(
                        "Crawl attempt {} failed for {}: {}",
                        attempt + 1,
                        url,
                        e
                    );
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| CrawlerError::Parse("Max retries exceeded".to_string())))
    }

    /// Try to crawl a page once
    async fn try_crawl_page(
        &self,
        url: &Url,
        config: &StrategyConfig,
    ) -> Result<CrawlPageResult, CrawlerError> {
        let start = Instant::now();

        let response = self
            .client
            .get(url.as_str())
            .header("User-Agent", &config.user_agent)
            .timeout(std::time::Duration::from_secs(config.page_timeout_secs))
            .send()
            .await?;

        let status_code = response.status().as_u16();
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let html = response.text().await?;
        let crawl_time_ms = start.elapsed().as_millis() as u64;

        // Detect framework selectors for potential JS rendering
        let _framework_selectors = detect_framework_selectors(&html);

        // Extract title
        let title = self.extract_title(&html);

        // Discover URLs from the page
        let discovered_urls = self.extract_urls(&html, url);

        Ok(CrawlPageResult {
            url: url.clone(),
            html,
            markdown: None, // Will be processed later by extractor
            title,
            status_code,
            content_type,
            crawl_time_ms,
            discovered_urls,
        })
    }

    /// Extract title from HTML
    fn extract_title(&self, html: &str) -> Option<String> {
        // Simple title extraction
        if let Some(start) = html.find("<title>") {
            if let Some(end) = html[start..].find("</title>") {
                let title = &html[start + 7..start + end];
                return Some(title.trim().to_string());
            }
        }
        None
    }

    /// Extract URLs from HTML content
    fn extract_urls(&self, html: &str, base_url: &Url) -> Vec<Url> {
        let mut urls = Vec::new();
        let mut search_start = 0;

        // Simple href extraction
        while let Some(href_pos) = html[search_start..].find("href=\"") {
            let start = search_start + href_pos + 6;
            if let Some(end_pos) = html[start..].find('"') {
                let href = &html[start..start + end_pos];

                // Try to parse as absolute or relative URL
                if let Ok(url) = Url::parse(href).or_else(|_| base_url.join(href)) {
                    // Only include same-origin URLs
                    if url.host() == base_url.host() {
                        urls.push(url);
                    }
                }

                search_start = start + end_pos;
            } else {
                break;
            }
        }

        urls
    }
}

impl Default for SinglePageStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CrawlStrategy for SinglePageStrategy {
    async fn crawl(
        &self,
        urls: Vec<Url>,
        config: &StrategyConfig,
        progress: SharedProgressTracker,
        cancel_token: CancellationToken,
    ) -> Result<CrawlResult, CrawlerError> {
        let start = Instant::now();
        let mut pages = Vec::new();
        let mut failed_urls = Vec::new();

        // Update progress
        progress.set_stage(CrawlStage::Crawling);
        progress.set_pages_total(urls.len() as u32);

        for (idx, url) in urls.iter().enumerate() {
            // Check for cancellation
            if cancel_token.is_cancelled() {
                return Ok(CrawlResult {
                    pages,
                    failed_urls,
                    total_time_ms: start.elapsed().as_millis() as u64,
                    was_cancelled: true,
                });
            }

            match self.crawl_page_with_retry(url, config, &cancel_token).await {
                Ok(result) => {
                    pages.push(result);
                    progress.increment_pages_processed();
                }
                Err(e) => {
                    failed_urls.push((url.clone(), e.to_string()));
                }
            }

            // Update progress
            let pct = ((idx + 1) as f32 / urls.len() as f32 * 100.0) as u8;
            progress.set_stage_progress(pct);
        }

        Ok(CrawlResult {
            pages,
            failed_urls,
            total_time_ms: start.elapsed().as_millis() as u64,
            was_cancelled: false,
        })
    }

    fn name(&self) -> &'static str {
        "SinglePage"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_title() {
        let strategy = SinglePageStrategy::new();

        let html = "<html><head><title>Test Page</title></head><body></body></html>";
        assert_eq!(strategy.extract_title(html), Some("Test Page".to_string()));

        let html_no_title = "<html><body></body></html>";
        assert_eq!(strategy.extract_title(html_no_title), None);
    }

    #[test]
    fn test_detect_framework_selectors() {
        let nextjs_html = r#"<script id="__NEXT_DATA__">{"page":"/"}</script>"#;
        let selectors = detect_framework_selectors(nextjs_html);
        assert!(selectors.contains(&"div#__next".to_string()));

        let angular_html = r#"<app-root ng-version="16.0.0"></app-root>"#;
        let selectors = detect_framework_selectors(angular_html);
        assert!(selectors.contains(&"[ng-version]".to_string()));
    }

    #[test]
    fn test_calculate_backoff() {
        assert_eq!(calculate_backoff(0, 1000), 1000);
        assert_eq!(calculate_backoff(1, 1000), 2000);
        assert_eq!(calculate_backoff(2, 1000), 4000);
        assert_eq!(calculate_backoff(3, 1000), 8000);
    }
}
