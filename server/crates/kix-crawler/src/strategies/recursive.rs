//! Recursive Strategy
//!
//! Depth-first recursive crawling with visited tracking.
//! Features:
//! - Depth limiting
//! - Visited URL tracking
//! - Same-origin enforcement
//! - Progress tracking at each depth level

use super::{CrawlPageResult, CrawlResult, CrawlStrategy, SinglePageStrategy, StrategyConfig};
use crate::progress::{CrawlStage, SharedProgressTracker};
use crate::CrawlerError;
use async_trait::async_trait;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use url::Url;

/// Recursive depth-first crawl strategy
pub struct RecursiveStrategy {
    /// Single page strategy for individual page crawling
    single_page: SinglePageStrategy,
}

impl RecursiveStrategy {
    /// Create a new recursive strategy
    pub fn new() -> Self {
        Self {
            single_page: SinglePageStrategy::new(),
        }
    }

    /// Recursively crawl starting from a URL
    async fn crawl_recursive(
        &self,
        url: Url,
        depth: usize,
        config: &StrategyConfig,
        visited: Arc<RwLock<HashSet<String>>>,
        progress: &SharedProgressTracker,
        cancel_token: &CancellationToken,
        pages: &mut Vec<CrawlPageResult>,
        failed: &mut Vec<(Url, String)>,
    ) -> Result<(), CrawlerError> {
        // Check cancellation
        if cancel_token.is_cancelled() {
            return Err(CrawlerError::Cancelled);
        }

        // Check depth limit
        if depth > config.max_depth {
            return Ok(());
        }

        // Check page limit
        if pages.len() >= config.max_pages {
            return Ok(());
        }

        // Normalize URL for visited tracking
        let normalized = self.normalize_url(&url);

        // Check if already visited
        {
            let visited_read = visited.read().await;
            if visited_read.contains(&normalized) {
                return Ok(());
            }
        }

        // Mark as visited
        {
            let mut visited_write = visited.write().await;
            visited_write.insert(normalized);
        }

        // Crawl this page
        let result = self
            .single_page
            .crawl(
                vec![url.clone()],
                config,
                progress.clone(),
                cancel_token.clone(),
            )
            .await;

        match result {
            Ok(mut crawl_result) => {
                if let Some(page) = crawl_result.pages.pop() {
                    // Get discovered URLs before moving page
                    let discovered_urls = page.discovered_urls.clone();
                    pages.push(page);
                    progress.increment_pages_processed();

                    // Update progress
                    let pct = (pages.len() as f32 / config.max_pages as f32 * 100.0).min(100.0) as u8;
                    progress.set_stage_progress(pct);
                    progress.set_message(format!(
                        "Crawled {} pages (depth {})",
                        pages.len(),
                        depth
                    ));

                    // Recursively crawl discovered URLs
                    for discovered_url in discovered_urls {
                        // Check limits again before recursing
                        if cancel_token.is_cancelled() || pages.len() >= config.max_pages {
                            break;
                        }

                        // Only follow same-origin URLs
                        if discovered_url.host() == url.host() {
                            Box::pin(self.crawl_recursive(
                                discovered_url,
                                depth + 1,
                                config,
                                visited.clone(),
                                progress,
                                cancel_token,
                                pages,
                                failed,
                            ))
                            .await?;
                        }
                    }
                } else if let Some((url, error)) = crawl_result.failed_urls.pop() {
                    failed.push((url, error));
                }
            }
            Err(e) => {
                failed.push((url, e.to_string()));
            }
        }

        Ok(())
    }

    /// Normalize URL for deduplication
    fn normalize_url(&self, url: &Url) -> String {
        let mut normalized = url.clone();

        // Remove fragment
        normalized.set_fragment(None);

        // Remove trailing slash for paths
        let path = normalized.path().to_string();
        if path.len() > 1 && path.ends_with('/') {
            normalized.set_path(&path[..path.len() - 1]);
        }

        // Remove common tracking parameters
        let params_to_remove = [
            "utm_source",
            "utm_medium",
            "utm_campaign",
            "utm_term",
            "utm_content",
            "ref",
            "source",
        ];

        if let Some(query) = normalized.query() {
            let filtered: Vec<&str> = query
                .split('&')
                .filter(|param| {
                    let key = param.split('=').next().unwrap_or("");
                    !params_to_remove.contains(&key)
                })
                .collect();

            if filtered.is_empty() {
                normalized.set_query(None);
            } else {
                normalized.set_query(Some(&filtered.join("&")));
            }
        }

        normalized.to_string()
    }
}

impl Default for RecursiveStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CrawlStrategy for RecursiveStrategy {
    async fn crawl(
        &self,
        urls: Vec<Url>,
        config: &StrategyConfig,
        progress: SharedProgressTracker,
        cancel_token: CancellationToken,
    ) -> Result<CrawlResult, CrawlerError> {
        let start = Instant::now();
        let visited = Arc::new(RwLock::new(HashSet::new()));
        let mut pages = Vec::new();
        let mut failed = Vec::new();

        // Update progress
        progress.set_stage(CrawlStage::Crawling);
        progress.set_pages_total(config.max_pages as u32);

        // Start recursive crawl from each seed URL
        for seed_url in urls {
            if cancel_token.is_cancelled() || pages.len() >= config.max_pages {
                break;
            }

            self.crawl_recursive(
                seed_url,
                0,
                config,
                visited.clone(),
                &progress,
                &cancel_token,
                &mut pages,
                &mut failed,
            )
            .await?;
        }

        let was_cancelled = cancel_token.is_cancelled();

        Ok(CrawlResult {
            pages,
            failed_urls: failed,
            total_time_ms: start.elapsed().as_millis() as u64,
            was_cancelled,
        })
    }

    fn name(&self) -> &'static str {
        "Recursive"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_url() {
        let strategy = RecursiveStrategy::new();

        // Remove trailing slash
        let url = Url::parse("https://example.com/path/").unwrap();
        let normalized = strategy.normalize_url(&url);
        assert!(!normalized.ends_with('/') || normalized.ends_with("://example.com/path"));

        // Remove fragment
        let url = Url::parse("https://example.com/page#section").unwrap();
        let normalized = strategy.normalize_url(&url);
        assert!(!normalized.contains('#'));

        // Remove tracking parameters
        let url = Url::parse("https://example.com/page?utm_source=test&id=123").unwrap();
        let normalized = strategy.normalize_url(&url);
        assert!(!normalized.contains("utm_source"));
        assert!(normalized.contains("id=123"));
    }

    #[test]
    fn test_recursive_strategy_creation() {
        let strategy = RecursiveStrategy::new();
        assert_eq!(strategy.name(), "Recursive");
    }
}
