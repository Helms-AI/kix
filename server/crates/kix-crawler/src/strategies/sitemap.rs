//! Sitemap Strategy
//!
//! Sitemap-based systematic crawling.
//! Parses XML sitemaps and crawls all listed URLs with:
//! - Sitemap index support (recursive sitemap parsing)
//! - Priority and lastmod awareness
//! - Batch processing for efficiency

use super::{BatchStrategy, CrawlResult, CrawlStrategy, StrategyConfig};
use crate::progress::{CrawlStage, SharedProgressTracker};
use crate::CrawlerError;
use async_trait::async_trait;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::time::Instant;
use tokio_util::sync::CancellationToken;
use url::Url;

/// Sitemap entry with metadata
#[derive(Debug, Clone)]
pub struct SitemapEntry {
    /// URL from the sitemap
    pub url: Url,
    /// Last modification date (if available) - reserved for future freshness-based crawling
    #[allow(dead_code)]
    pub lastmod: Option<String>,
    /// Priority (0.0 - 1.0)
    pub priority: Option<f32>,
    /// Change frequency - reserved for future scheduling optimization
    #[allow(dead_code)]
    pub changefreq: Option<String>,
}

/// Sitemap-based crawl strategy
pub struct SitemapStrategy {
    /// HTTP client for fetching sitemaps
    client: reqwest::Client,
    /// Batch strategy for crawling discovered URLs
    batch_strategy: BatchStrategy,
}

impl SitemapStrategy {
    /// Create a new sitemap strategy
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            batch_strategy: BatchStrategy::new(),
        }
    }

    /// Fetch and parse a sitemap
    async fn fetch_sitemap(
        &self,
        sitemap_url: &Url,
        config: &StrategyConfig,
    ) -> Result<Vec<SitemapEntry>, CrawlerError> {
        let response = self
            .client
            .get(sitemap_url.as_str())
            .header("User-Agent", &config.user_agent)
            .send()
            .await?
            .error_for_status()?;

        let content = response.text().await?;
        self.parse_sitemap(&content, sitemap_url, config).await
    }

    /// Parse sitemap XML content
    async fn parse_sitemap(
        &self,
        content: &str,
        base_url: &Url,
        config: &StrategyConfig,
    ) -> Result<Vec<SitemapEntry>, CrawlerError> {
        let mut entries = Vec::new();
        let mut sitemap_indexes = Vec::new();

        let mut reader = Reader::from_str(content);
        reader.config_mut().trim_text(true);

        let mut buf = Vec::new();
        let mut in_url = false;
        let mut in_sitemap = false;
        let mut in_loc = false;
        let mut in_lastmod = false;
        let mut in_priority = false;
        let mut in_changefreq = false;

        let mut current_url: Option<String> = None;
        let mut current_lastmod: Option<String> = None;
        let mut current_priority: Option<f32> = None;
        let mut current_changefreq: Option<String> = None;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => match e.name().as_ref() {
                    b"url" => {
                        in_url = true;
                        current_url = None;
                        current_lastmod = None;
                        current_priority = None;
                        current_changefreq = None;
                    }
                    b"sitemap" => {
                        in_sitemap = true;
                        current_url = None;
                    }
                    b"loc" => in_loc = true,
                    b"lastmod" => in_lastmod = true,
                    b"priority" => in_priority = true,
                    b"changefreq" => in_changefreq = true,
                    _ => {}
                },
                Ok(Event::End(e)) => match e.name().as_ref() {
                    b"url" => {
                        if let Some(url_str) = current_url.take() {
                            if let Ok(url) = Url::parse(&url_str) {
                                entries.push(SitemapEntry {
                                    url,
                                    lastmod: current_lastmod.take(),
                                    priority: current_priority.take(),
                                    changefreq: current_changefreq.take(),
                                });
                            }
                        }
                        in_url = false;
                    }
                    b"sitemap" => {
                        if let Some(url_str) = current_url.take() {
                            if let Ok(url) = Url::parse(&url_str) {
                                sitemap_indexes.push(url);
                            }
                        }
                        in_sitemap = false;
                    }
                    b"loc" => in_loc = false,
                    b"lastmod" => in_lastmod = false,
                    b"priority" => in_priority = false,
                    b"changefreq" => in_changefreq = false,
                    _ => {}
                },
                Ok(Event::Text(e)) => {
                    if in_loc && (in_url || in_sitemap) {
                        if let Ok(text) = e.unescape() {
                            current_url = Some(text.trim().to_string());
                        }
                    } else if in_lastmod && in_url {
                        if let Ok(text) = e.unescape() {
                            current_lastmod = Some(text.trim().to_string());
                        }
                    } else if in_priority && in_url {
                        if let Ok(text) = e.unescape() {
                            current_priority = text.trim().parse().ok();
                        }
                    } else if in_changefreq && in_url {
                        if let Ok(text) = e.unescape() {
                            current_changefreq = Some(text.trim().to_string());
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    tracing::warn!("Error parsing sitemap: {}", e);
                    break;
                }
                _ => {}
            }
            buf.clear();
        }

        // Recursively fetch sitemap indexes
        for index_url in sitemap_indexes {
            // Only follow same-origin sitemaps
            if index_url.host() == base_url.host() {
                match Box::pin(self.fetch_sitemap(&index_url, config)).await {
                    Ok(child_entries) => entries.extend(child_entries),
                    Err(e) => {
                        tracing::warn!("Failed to fetch sitemap index {}: {}", index_url, e);
                    }
                }
            }
        }

        // Sort by priority (highest first)
        entries.sort_by(|a, b| {
            let pa = a.priority.unwrap_or(0.5);
            let pb = b.priority.unwrap_or(0.5);
            pb.partial_cmp(&pa).unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(entries)
    }
}

impl Default for SitemapStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CrawlStrategy for SitemapStrategy {
    async fn crawl(
        &self,
        urls: Vec<Url>,
        config: &StrategyConfig,
        progress: SharedProgressTracker,
        cancel_token: CancellationToken,
    ) -> Result<CrawlResult, CrawlerError> {
        let start = Instant::now();

        // Update progress - discovery phase
        progress.set_stage(CrawlStage::Discovery);
        progress.set_message("Parsing sitemaps...".to_string());

        // Parse all provided sitemaps
        let mut all_entries = Vec::new();
        for sitemap_url in &urls {
            if cancel_token.is_cancelled() {
                return Ok(CrawlResult {
                    pages: Vec::new(),
                    failed_urls: Vec::new(),
                    total_time_ms: start.elapsed().as_millis() as u64,
                    was_cancelled: true,
                });
            }

            match self.fetch_sitemap(sitemap_url, config).await {
                Ok(entries) => {
                    tracing::info!(
                        "Found {} URLs in sitemap {}",
                        entries.len(),
                        sitemap_url
                    );
                    all_entries.extend(entries);
                }
                Err(e) => {
                    tracing::warn!("Failed to fetch sitemap {}: {}", sitemap_url, e);
                }
            }
        }

        if all_entries.is_empty() {
            return Ok(CrawlResult {
                pages: Vec::new(),
                failed_urls: urls.into_iter().map(|u| (u, "No URLs found".to_string())).collect(),
                total_time_ms: start.elapsed().as_millis() as u64,
                was_cancelled: false,
            });
        }

        // Limit to max_pages
        let urls_to_crawl: Vec<Url> = all_entries
            .into_iter()
            .take(config.max_pages)
            .map(|e| e.url)
            .collect();

        tracing::info!("Crawling {} URLs from sitemaps", urls_to_crawl.len());

        // Use batch strategy to crawl all URLs
        self.batch_strategy
            .crawl(urls_to_crawl, config, progress, cancel_token)
            .await
    }

    fn name(&self) -> &'static str {
        "Sitemap"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_parse_simple_sitemap() {
        let strategy = SitemapStrategy::new();
        let config = StrategyConfig::default();
        let base_url = Url::parse("https://example.com").unwrap();

        let sitemap_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
    <url>
        <loc>https://example.com/page1</loc>
        <lastmod>2024-01-01</lastmod>
        <priority>0.8</priority>
    </url>
    <url>
        <loc>https://example.com/page2</loc>
        <priority>0.5</priority>
    </url>
</urlset>"#;

        let entries = strategy
            .parse_sitemap(sitemap_xml, &base_url, &config)
            .await
            .unwrap();

        assert_eq!(entries.len(), 2);

        // Should be sorted by priority (highest first)
        assert_eq!(
            entries[0].url.as_str(),
            "https://example.com/page1"
        );
        assert_eq!(entries[0].priority, Some(0.8));
        assert_eq!(entries[0].lastmod, Some("2024-01-01".to_string()));

        assert_eq!(
            entries[1].url.as_str(),
            "https://example.com/page2"
        );
        assert_eq!(entries[1].priority, Some(0.5));
    }

    #[test]
    fn test_sitemap_strategy_creation() {
        let strategy = SitemapStrategy::new();
        assert_eq!(strategy.name(), "Sitemap");
    }
}
