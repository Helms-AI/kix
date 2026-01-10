//! Robots.txt and sitemap handling

use std::time::{Duration, Instant};

use dashmap::DashMap;
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use reqwest::Client;
use tracing::{debug, info, warn};
use url::Url;

/// Cached robots.txt data
#[derive(Clone, Debug)]
pub struct RobotsData {
    /// Disallowed paths
    pub disallowed: Vec<String>,
    /// Allowed paths (overrides disallowed)
    pub allowed: Vec<String>,
    /// Crawl delay
    pub crawl_delay: Option<Duration>,
    /// Sitemaps
    pub sitemaps: Vec<String>,
    /// Cache timestamp
    pub cached_at: Instant,
}

impl RobotsData {
    /// Check if a path is allowed
    pub fn is_allowed(&self, path: &str) -> bool {
        // Check allowed first (more specific)
        for pattern in &self.allowed {
            if path_matches(path, pattern) {
                return true;
            }
        }

        // Check disallowed
        for pattern in &self.disallowed {
            if path_matches(path, pattern) {
                return false;
            }
        }

        true
    }
}

/// Robots.txt checker with caching
pub struct RobotsChecker {
    cache: DashMap<String, RobotsData>,
    client: Client,
    user_agent: String,
    cache_ttl: Duration,
}

impl RobotsChecker {
    /// Create a new robots checker
    pub fn new(client: Client, user_agent: impl Into<String>) -> Self {
        Self {
            cache: DashMap::new(),
            client,
            user_agent: user_agent.into(),
            cache_ttl: Duration::from_secs(3600), // 1 hour cache
        }
    }

    /// Check if a URL is allowed by robots.txt
    pub async fn is_allowed(&self, url: &Url) -> bool {
        let domain = match url.domain() {
            Some(d) => d.to_string(),
            None => return true,
        };

        let robots_data = self.get_or_fetch(&domain, url.scheme()).await;

        match robots_data {
            Some(data) => data.is_allowed(url.path()),
            None => true, // Allow if no robots.txt
        }
    }

    /// Get crawl delay for a domain
    pub async fn get_crawl_delay(&self, url: &Url) -> Option<Duration> {
        let domain = url.domain()?;
        let robots_data = self.get_or_fetch(domain, url.scheme()).await?;
        robots_data.crawl_delay
    }

    /// Get sitemaps for a domain
    pub async fn get_sitemaps(&self, url: &Url) -> Vec<String> {
        let domain = match url.domain() {
            Some(d) => d,
            None => return vec![],
        };

        match self.get_or_fetch(domain, url.scheme()).await {
            Some(data) => data.sitemaps.clone(),
            None => vec![],
        }
    }

    /// Fetch and parse all URLs from a sitemap (with recursive sitemap index support)
    /// Returns a list of page URLs found in the sitemap
    pub fn fetch_sitemap_urls<'a>(
        &'a self,
        sitemap_url: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<Url>, SitemapError>> + Send + 'a>> {
        Box::pin(async move {
            info!(sitemap = sitemap_url, "Fetching sitemap");

            let response = self.client
                .get(sitemap_url)
                .send()
                .await
                .map_err(|e| SitemapError::Fetch(format!("Failed to fetch sitemap: {}", e)))?;

            if !response.status().is_success() {
                return Err(SitemapError::Fetch(format!(
                    "Sitemap returned status {}",
                    response.status()
                )));
            }

            let xml_content = response
                .text()
                .await
                .map_err(|e| SitemapError::Fetch(format!("Failed to read sitemap body: {}", e)))?;

            let parsed = parse_sitemap_xml(&xml_content)?;

            match parsed {
                SitemapContent::UrlSet(urls) => {
                    info!(count = urls.len(), "Found URLs in sitemap");
                    Ok(urls)
                }
                SitemapContent::SitemapIndex(nested_sitemaps) => {
                    // Recursively fetch nested sitemaps
                    info!(count = nested_sitemaps.len(), "Found sitemap index, fetching nested sitemaps");
                    let mut all_urls = Vec::new();

                    for nested_url in nested_sitemaps {
                        match self.fetch_sitemap_urls(&nested_url).await {
                            Ok(urls) => all_urls.extend(urls),
                            Err(e) => {
                                warn!(url = nested_url, error = %e, "Failed to fetch nested sitemap");
                            }
                        }
                    }

                    info!(total_count = all_urls.len(), "Total URLs from sitemap index");
                    Ok(all_urls)
                }
            }
        })
    }

    /// Fetch all URLs from all sitemaps declared in robots.txt for a domain
    pub async fn fetch_all_sitemap_urls(&self, base_url: &Url) -> Vec<Url> {
        let sitemaps = self.get_sitemaps(base_url).await;

        if sitemaps.is_empty() {
            // Try common sitemap locations if none declared in robots.txt
            let default_sitemaps = vec![
                format!("{}://{}/sitemap.xml", base_url.scheme(), base_url.domain().unwrap_or("")),
                format!("{}://{}/sitemap_index.xml", base_url.scheme(), base_url.domain().unwrap_or("")),
            ];

            for sitemap_url in default_sitemaps {
                if let Ok(urls) = self.fetch_sitemap_urls(&sitemap_url).await {
                    if !urls.is_empty() {
                        return urls;
                    }
                }
            }
            return vec![];
        }

        let mut all_urls = Vec::new();
        for sitemap_url in sitemaps {
            match self.fetch_sitemap_urls(&sitemap_url).await {
                Ok(urls) => all_urls.extend(urls),
                Err(e) => {
                    warn!(url = sitemap_url, error = %e, "Failed to fetch sitemap");
                }
            }
        }

        all_urls
    }

    /// Get or fetch robots.txt data
    async fn get_or_fetch(&self, domain: &str, scheme: &str) -> Option<RobotsData> {
        // Check cache
        if let Some(cached) = self.cache.get(domain) {
            if cached.cached_at.elapsed() < self.cache_ttl {
                return Some(cached.clone());
            }
        }

        // Fetch robots.txt
        let robots_url = format!("{}://{}/robots.txt", scheme, domain);
        let data = self.fetch_and_parse(&robots_url).await;

        if let Some(ref data) = data {
            self.cache.insert(domain.to_string(), data.clone());
        }

        data
    }

    /// Fetch and parse robots.txt
    async fn fetch_and_parse(&self, url: &str) -> Option<RobotsData> {
        let response = self.client.get(url).send().await.ok()?;

        if !response.status().is_success() {
            debug!(url = url, status = %response.status(), "robots.txt not found");
            return None;
        }

        let text = response.text().await.ok()?;
        Some(self.parse_robots_txt(&text))
    }

    /// Parse robots.txt content
    fn parse_robots_txt(&self, content: &str) -> RobotsData {
        let mut disallowed = Vec::new();
        let mut allowed = Vec::new();
        let mut crawl_delay = None;
        let mut sitemaps = Vec::new();

        let mut in_user_agent_block = false;
        let mut is_matching_agent = false;

        for line in content.lines() {
            let line = line.trim();

            // Skip comments and empty lines
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Split on first ':'
            let parts: Vec<&str> = line.splitn(2, ':').collect();
            if parts.len() != 2 {
                continue;
            }

            let key = parts[0].trim().to_lowercase();
            let value = parts[1].trim();

            match key.as_str() {
                "user-agent" => {
                    in_user_agent_block = true;
                    is_matching_agent = value == "*" || self.user_agent.contains(value);
                }
                "disallow" if in_user_agent_block && is_matching_agent => {
                    if !value.is_empty() {
                        disallowed.push(value.to_string());
                    }
                }
                "allow" if in_user_agent_block && is_matching_agent => {
                    if !value.is_empty() {
                        allowed.push(value.to_string());
                    }
                }
                "crawl-delay" if in_user_agent_block && is_matching_agent => {
                    if let Ok(secs) = value.parse::<f64>() {
                        crawl_delay = Some(Duration::from_secs_f64(secs));
                    }
                }
                "sitemap" => {
                    sitemaps.push(value.to_string());
                }
                _ => {}
            }
        }

        RobotsData {
            disallowed,
            allowed,
            crawl_delay,
            sitemaps,
            cached_at: Instant::now(),
        }
    }
}

/// Check if a path matches a robots.txt pattern
fn path_matches(path: &str, pattern: &str) -> bool {
    if pattern.is_empty() {
        return false;
    }

    // Handle * wildcard
    if pattern.contains('*') {
        let parts: Vec<&str> = pattern.split('*').collect();
        let mut remaining = path;

        for (i, part) in parts.iter().enumerate() {
            if part.is_empty() {
                continue;
            }

            if i == 0 {
                // First part must match from start
                if !remaining.starts_with(part) {
                    return false;
                }
                remaining = &remaining[part.len()..];
            } else {
                // Other parts can match anywhere
                match remaining.find(part) {
                    Some(pos) => remaining = &remaining[pos + part.len()..],
                    None => return false,
                }
            }
        }

        return true;
    }

    // Handle $ end anchor
    if pattern.ends_with('$') {
        let pattern = &pattern[..pattern.len() - 1];
        return path == pattern;
    }

    // Simple prefix match
    path.starts_with(pattern)
}

// ============ Sitemap Processing ============

/// Error type for sitemap operations
#[derive(Debug, thiserror::Error)]
pub enum SitemapError {
    #[error("Failed to fetch sitemap: {0}")]
    Fetch(String),
    #[error("Failed to parse sitemap XML: {0}")]
    Parse(String),
    #[error("Invalid URL in sitemap: {0}")]
    InvalidUrl(String),
}

/// Content type parsed from sitemap XML
#[derive(Debug)]
pub enum SitemapContent {
    /// Regular sitemap with page URLs (<urlset>)
    UrlSet(Vec<Url>),
    /// Sitemap index with nested sitemap URLs (<sitemapindex>)
    SitemapIndex(Vec<String>),
}

/// Parse sitemap XML content
/// Supports both regular sitemaps (<urlset>) and sitemap indexes (<sitemapindex>)
fn parse_sitemap_xml(xml_content: &str) -> Result<SitemapContent, SitemapError> {
    let mut reader = Reader::from_str(xml_content);
    reader.config_mut().trim_text(true);

    let mut urls: Vec<Url> = Vec::new();
    let mut sitemap_urls: Vec<String> = Vec::new();
    let mut in_url = false;
    let mut in_sitemap = false;
    let mut in_loc = false;
    let mut is_sitemap_index = false;

    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = e.local_name();
                match name.as_ref() {
                    b"sitemapindex" => {
                        is_sitemap_index = true;
                    }
                    b"url" => {
                        in_url = true;
                    }
                    b"sitemap" => {
                        in_sitemap = true;
                    }
                    b"loc" => {
                        in_loc = true;
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let name = e.local_name();
                match name.as_ref() {
                    b"url" => in_url = false,
                    b"sitemap" => in_sitemap = false,
                    b"loc" => in_loc = false,
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) => {
                if in_loc {
                    let text = e.unescape()
                        .map_err(|e| SitemapError::Parse(format!("Failed to unescape text: {}", e)))?
                        .trim()
                        .to_string();

                    if !text.is_empty() {
                        if is_sitemap_index && in_sitemap {
                            // This is a nested sitemap URL
                            sitemap_urls.push(text);
                        } else if in_url {
                            // This is a page URL
                            match Url::parse(&text) {
                                Ok(url) => urls.push(url),
                                Err(e) => {
                                    debug!(url = text, error = %e, "Invalid URL in sitemap, skipping");
                                }
                            }
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(SitemapError::Parse(format!(
                    "XML parse error at position {}: {}",
                    reader.buffer_position(),
                    e
                )));
            }
            _ => {}
        }
        buf.clear();
    }

    if is_sitemap_index {
        Ok(SitemapContent::SitemapIndex(sitemap_urls))
    } else {
        Ok(SitemapContent::UrlSet(urls))
    }
}

#[cfg(test)]
mod sitemap_tests {
    use super::*;

    #[test]
    fn test_parse_urlset_sitemap() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
            <url>
                <loc>https://example.com/page1</loc>
                <lastmod>2024-01-01</lastmod>
            </url>
            <url>
                <loc>https://example.com/page2</loc>
            </url>
        </urlset>"#;

        let result = parse_sitemap_xml(xml).unwrap();
        match result {
            SitemapContent::UrlSet(urls) => {
                assert_eq!(urls.len(), 2);
                assert_eq!(urls[0].as_str(), "https://example.com/page1");
                assert_eq!(urls[1].as_str(), "https://example.com/page2");
            }
            _ => panic!("Expected UrlSet"),
        }
    }

    #[test]
    fn test_parse_sitemap_index() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
            <sitemap>
                <loc>https://example.com/sitemap1.xml</loc>
            </sitemap>
            <sitemap>
                <loc>https://example.com/sitemap2.xml</loc>
            </sitemap>
        </sitemapindex>"#;

        let result = parse_sitemap_xml(xml).unwrap();
        match result {
            SitemapContent::SitemapIndex(sitemaps) => {
                assert_eq!(sitemaps.len(), 2);
                assert_eq!(sitemaps[0], "https://example.com/sitemap1.xml");
                assert_eq!(sitemaps[1], "https://example.com/sitemap2.xml");
            }
            _ => panic!("Expected SitemapIndex"),
        }
    }
}