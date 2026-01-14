//! Configuration for spider-based crawling.

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

/// Crawling mode for JavaScript handling
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CrawlMode {
    /// HTTP only, no JavaScript rendering
    HttpOnly,
    /// Smart mode: HTTP first, JS fallback if needed
    #[default]
    Smart,
    /// Always use JavaScript rendering (requires Chrome)
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

    /// Create config for shallow crawl (links from homepage only)
    pub fn shallow() -> Self {
        Self {
            max_depth: Some(1),
            max_pages: 50,
            ..Default::default()
        }
    }

    /// Create config with custom depth and page limit
    pub fn with_limits(depth: usize, max_pages: usize) -> Self {
        Self {
            max_depth: Some(depth),
            max_pages,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = SpiderConfig::default();
        assert_eq!(config.mode, CrawlMode::Smart);
        assert!(config.respect_robots_txt);
        assert!(config.use_sitemap);
        assert_eq!(config.max_depth, Some(3));
    }

    #[test]
    fn test_single_page_config() {
        let config = SpiderConfig::single_page();
        assert_eq!(config.max_depth, Some(0));
        assert_eq!(config.max_pages, 1);
        assert!(!config.use_sitemap);
    }

    #[test]
    fn test_documentation_config() {
        let config = SpiderConfig::documentation();
        assert_eq!(config.max_depth, Some(5));
        assert!(!config.exclude_patterns.is_empty());
    }
}
