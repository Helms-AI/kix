//! URL Discovery Service
//!
//! Discovers URLs for crawling using a priority order:
//! 1. llms.txt - AI-specific content hints
//! 2. sitemap.xml - Standard XML sitemaps
//! 3. robots.txt - Crawl directives with sitemap references
//!
//! Falls back to crawling from base URL if no discovery method succeeds.

use crate::ssrf::{SsrfError, SsrfValidator};
use quick_xml::events::Event;
use quick_xml::Reader;
use reqwest::Client;
use std::collections::HashSet;
use std::time::Duration;
use thiserror::Error;
use url::Url;

#[derive(Error, Debug)]
pub enum DiscoveryError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("URL parse error: {0}")]
    UrlParse(#[from] url::ParseError),

    #[error("SSRF validation failed: {0}")]
    Ssrf(#[from] SsrfError),

    #[error("XML parse error: {0}")]
    XmlParse(String),

    #[error("No discovery method available")]
    NoDiscoveryAvailable,

    #[error("Discovery timeout")]
    Timeout,
}

/// Result of URL discovery
#[derive(Debug, Clone)]
pub struct DiscoveryResult {
    /// The discovery method that succeeded
    pub method: DiscoveryMethod,
    /// Discovered URLs
    pub urls: Vec<Url>,
    /// Total number of URLs found
    pub total_found: usize,
}

/// Discovery method used
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoveryMethod {
    /// URLs from llms.txt
    LlmsTxt,
    /// URLs from sitemap.xml
    Sitemap,
    /// URLs from robots.txt (sitemap references)
    RobotsTxt,
    /// No discovery available, will crawl from base
    None,
}

impl DiscoveryMethod {
    /// Get a human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            Self::LlmsTxt => "llms.txt",
            Self::Sitemap => "sitemap.xml",
            Self::RobotsTxt => "robots.txt",
            Self::None => "manual crawl",
        }
    }
}

/// Configuration for discovery service
#[derive(Debug, Clone)]
pub struct DiscoveryConfig {
    /// HTTP request timeout
    pub timeout: Duration,
    /// Maximum URLs to return from discovery
    pub max_urls: usize,
    /// User agent for requests
    pub user_agent: String,
    /// SSRF validator settings
    pub ssrf_validator: SsrfValidator,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            max_urls: 10000,
            user_agent: "KIX/1.0 (+https://github.com/kix)".to_string(),
            ssrf_validator: SsrfValidator::new(),
        }
    }
}

/// URL Discovery Service following a priority order
pub struct DiscoveryService {
    client: Client,
    config: DiscoveryConfig,
}

impl DiscoveryService {
    /// Create a new discovery service with default configuration
    pub fn new() -> Result<Self, DiscoveryError> {
        Self::with_config(DiscoveryConfig::default())
    }

    /// Create a discovery service with custom configuration
    pub fn with_config(config: DiscoveryConfig) -> Result<Self, DiscoveryError> {
        let client = Client::builder()
            .timeout(config.timeout)
            .user_agent(&config.user_agent)
            .build()?;

        Ok(Self { client, config })
    }

    /// Discover URLs for a given base URL using a priority order
    ///
    /// Priority:
    /// 1. llms.txt - AI-specific content hints
    /// 2. sitemap.xml - Standard sitemap
    /// 3. robots.txt - For sitemap references
    /// 4. None - Fall back to manual crawling
    pub async fn discover(&self, base_url: &Url) -> Result<DiscoveryResult, DiscoveryError> {
        // Validate the base URL for SSRF
        self.config.ssrf_validator.validate(base_url)?;

        // Try llms.txt first (AI-specific)
        if let Ok(urls) = self.fetch_llms_txt(base_url).await {
            if !urls.is_empty() {
                tracing::info!("Discovered {} URLs from llms.txt", urls.len());
                return Ok(DiscoveryResult {
                    method: DiscoveryMethod::LlmsTxt,
                    total_found: urls.len(),
                    urls: self.limit_urls(urls),
                });
            }
        }

        // Try sitemap.xml
        if let Ok(urls) = self.fetch_sitemap(base_url).await {
            if !urls.is_empty() {
                tracing::info!("Discovered {} URLs from sitemap.xml", urls.len());
                return Ok(DiscoveryResult {
                    method: DiscoveryMethod::Sitemap,
                    total_found: urls.len(),
                    urls: self.limit_urls(urls),
                });
            }
        }

        // Try robots.txt for sitemap references
        if let Ok(urls) = self.fetch_robots_sitemaps(base_url).await {
            if !urls.is_empty() {
                tracing::info!("Discovered {} URLs from robots.txt sitemaps", urls.len());
                return Ok(DiscoveryResult {
                    method: DiscoveryMethod::RobotsTxt,
                    total_found: urls.len(),
                    urls: self.limit_urls(urls),
                });
            }
        }

        // No discovery available
        tracing::info!(
            "No discovery methods available for {}, will crawl from base",
            base_url
        );
        Ok(DiscoveryResult {
            method: DiscoveryMethod::None,
            total_found: 1,
            urls: vec![base_url.clone()],
        })
    }

    /// Fetch and parse llms.txt
    ///
    /// llms.txt is a proposed standard for AI-specific content hints.
    /// Format: One URL per line, comments start with #
    async fn fetch_llms_txt(&self, base_url: &Url) -> Result<Vec<Url>, DiscoveryError> {
        let llms_url = base_url.join("/llms.txt")?;
        self.config.ssrf_validator.validate(&llms_url)?;

        let response = self
            .client
            .get(llms_url.as_str())
            .send()
            .await?
            .error_for_status()?;

        let text = response.text().await?;
        let urls = self.parse_llms_txt(&text, base_url)?;

        Ok(urls)
    }

    /// Parse llms.txt content
    fn parse_llms_txt(&self, content: &str, base_url: &Url) -> Result<Vec<Url>, DiscoveryError> {
        let mut urls = Vec::new();

        for line in content.lines() {
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Parse URL (absolute or relative)
            let url = if line.starts_with("http://") || line.starts_with("https://") {
                Url::parse(line)?
            } else {
                base_url.join(line)?
            };

            // Validate for SSRF
            if self.config.ssrf_validator.validate(&url).is_ok() {
                urls.push(url);
            }
        }

        Ok(urls)
    }

    /// Fetch and parse sitemap.xml
    async fn fetch_sitemap(&self, base_url: &Url) -> Result<Vec<Url>, DiscoveryError> {
        let sitemap_url = base_url.join("/sitemap.xml")?;
        self.config.ssrf_validator.validate(&sitemap_url)?;

        let response = self
            .client
            .get(sitemap_url.as_str())
            .send()
            .await?
            .error_for_status()?;

        let text = response.text().await?;
        let urls = self.parse_sitemap_xml(&text, base_url).await?;

        Ok(urls)
    }

    /// Parse sitemap XML content
    ///
    /// Supports both regular sitemaps and sitemap indexes
    async fn parse_sitemap_xml(
        &self,
        content: &str,
        _base_url: &Url,
    ) -> Result<Vec<Url>, DiscoveryError> {
        let mut urls = Vec::new();
        let mut sitemap_indexes = Vec::new();
        let mut reader = Reader::from_str(content);
        reader.config_mut().trim_text(true);

        let mut buf = Vec::new();
        let mut in_url = false;
        let mut in_loc = false;
        let mut in_sitemap = false;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => match e.name().as_ref() {
                    b"url" => in_url = true,
                    b"sitemap" => in_sitemap = true,
                    b"loc" => in_loc = true,
                    _ => {}
                },
                Ok(Event::End(e)) => match e.name().as_ref() {
                    b"url" => in_url = false,
                    b"sitemap" => in_sitemap = false,
                    b"loc" => in_loc = false,
                    _ => {}
                },
                Ok(Event::Text(e)) if in_loc => {
                    let text = e
                        .unescape()
                        .map_err(|e| DiscoveryError::XmlParse(e.to_string()))?;
                    let url_str = text.trim();

                    if let Ok(url) = Url::parse(url_str) {
                        if self.config.ssrf_validator.validate(&url).is_ok() {
                            if in_sitemap {
                                sitemap_indexes.push(url);
                            } else if in_url {
                                urls.push(url);
                            }
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => return Err(DiscoveryError::XmlParse(e.to_string())),
                _ => {}
            }
            buf.clear();
        }

        // If this was a sitemap index, fetch the child sitemaps
        for sitemap_url in sitemap_indexes {
            if let Ok(response) = self.client.get(sitemap_url.as_str()).send().await {
                if let Ok(text) = response.text().await {
                    // Recursively parse (but limit depth to prevent infinite loops)
                    if let Ok(child_urls) = self.parse_sitemap_xml_no_recurse(&text) {
                        urls.extend(child_urls);
                    }
                }
            }
        }

        Ok(urls)
    }

    /// Parse sitemap XML without recursing into sitemap indexes
    fn parse_sitemap_xml_no_recurse(&self, content: &str) -> Result<Vec<Url>, DiscoveryError> {
        let mut urls = Vec::new();
        let mut reader = Reader::from_str(content);
        reader.config_mut().trim_text(true);

        let mut buf = Vec::new();
        let mut in_url = false;
        let mut in_loc = false;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => match e.name().as_ref() {
                    b"url" => in_url = true,
                    b"loc" if in_url => in_loc = true,
                    _ => {}
                },
                Ok(Event::End(e)) => match e.name().as_ref() {
                    b"url" => in_url = false,
                    b"loc" => in_loc = false,
                    _ => {}
                },
                Ok(Event::Text(e)) if in_loc && in_url => {
                    let text = e
                        .unescape()
                        .map_err(|e| DiscoveryError::XmlParse(e.to_string()))?;
                    let url_str = text.trim();

                    if let Ok(url) = Url::parse(url_str) {
                        if self.config.ssrf_validator.validate(&url).is_ok() {
                            urls.push(url);
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => return Err(DiscoveryError::XmlParse(e.to_string())),
                _ => {}
            }
            buf.clear();
        }

        Ok(urls)
    }

    /// Fetch robots.txt and extract sitemap URLs
    async fn fetch_robots_sitemaps(&self, base_url: &Url) -> Result<Vec<Url>, DiscoveryError> {
        let robots_url = base_url.join("/robots.txt")?;
        self.config.ssrf_validator.validate(&robots_url)?;

        let response = self
            .client
            .get(robots_url.as_str())
            .send()
            .await?
            .error_for_status()?;

        let text = response.text().await?;
        let sitemap_urls = self.parse_robots_txt_sitemaps(&text)?;

        // Fetch all referenced sitemaps
        let mut all_urls = Vec::new();
        for sitemap_url in sitemap_urls {
            if let Ok(response) = self.client.get(sitemap_url.as_str()).send().await {
                if let Ok(sitemap_text) = response.text().await {
                    if let Ok(urls) = self.parse_sitemap_xml(&sitemap_text, base_url).await {
                        all_urls.extend(urls);
                    }
                }
            }
        }

        Ok(all_urls)
    }

    /// Parse robots.txt for Sitemap: directives
    fn parse_robots_txt_sitemaps(&self, content: &str) -> Result<Vec<Url>, DiscoveryError> {
        let mut sitemaps = Vec::new();

        for line in content.lines() {
            let line = line.trim();

            // Look for Sitemap: directives (case insensitive)
            if line.to_lowercase().starts_with("sitemap:") {
                let url_str = line[8..].trim();
                if let Ok(url) = Url::parse(url_str) {
                    if self.config.ssrf_validator.validate(&url).is_ok() {
                        sitemaps.push(url);
                    }
                }
            }
        }

        Ok(sitemaps)
    }

    /// Limit URLs to configured maximum, deduplicating first
    fn limit_urls(&self, urls: Vec<Url>) -> Vec<Url> {
        let unique: HashSet<_> = urls.into_iter().collect();
        unique
            .into_iter()
            .take(self.config.max_urls)
            .collect()
    }
}

impl Default for DiscoveryService {
    fn default() -> Self {
        Self::new().expect("Failed to create default DiscoveryService")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_llms_txt() {
        let service = DiscoveryService::new().unwrap();
        let base_url = Url::parse("https://example.com").unwrap();

        let content = r#"
            # This is a comment
            /docs/overview
            /docs/api
            https://example.com/blog

            # Another comment
            /tutorials
        "#;

        let urls = service.parse_llms_txt(content, &base_url).unwrap();
        assert_eq!(urls.len(), 4);
    }

    #[test]
    fn test_parse_robots_sitemaps() {
        let service = DiscoveryService::new().unwrap();

        let content = r#"
            User-agent: *
            Allow: /

            Sitemap: https://example.com/sitemap.xml
            Sitemap: https://example.com/sitemap-blog.xml
        "#;

        let sitemaps = service.parse_robots_txt_sitemaps(content).unwrap();
        assert_eq!(sitemaps.len(), 2);
    }

    #[test]
    fn test_discovery_method_description() {
        assert_eq!(DiscoveryMethod::LlmsTxt.description(), "llms.txt");
        assert_eq!(DiscoveryMethod::Sitemap.description(), "sitemap.xml");
        assert_eq!(DiscoveryMethod::RobotsTxt.description(), "robots.txt");
        assert_eq!(DiscoveryMethod::None.description(), "manual crawl");
    }
}
