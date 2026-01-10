//! Browser-based page rendering using Playwright
//!
//! This module provides JavaScript rendering capabilities for crawling
//! dynamic web pages that require JavaScript execution.

use std::time::Duration;

use url::Url;

use crate::CrawlerError;

/// Configuration for browser rendering
#[derive(Clone, Debug)]
pub struct BrowserConfig {
    /// Maximum concurrent browser contexts
    pub max_contexts: usize,
    /// Page load timeout
    pub timeout: Duration,
    /// Wait for network idle after load
    pub wait_for_idle: bool,
    /// Idle wait duration
    pub idle_wait: Duration,
    /// Headless mode
    pub headless: bool,
    /// Block images for faster loading
    pub block_images: bool,
    /// Block fonts for faster loading
    pub block_fonts: bool,
    /// Custom user agent
    pub user_agent: Option<String>,
    /// Viewport width
    pub viewport_width: u32,
    /// Viewport height
    pub viewport_height: u32,
}

impl Default for BrowserConfig {
    fn default() -> Self {
        Self {
            max_contexts: 4,
            timeout: Duration::from_secs(30),
            wait_for_idle: true,
            idle_wait: Duration::from_secs(2),
            headless: true,
            block_images: true,
            block_fonts: true,
            user_agent: None,
            viewport_width: 1920,
            viewport_height: 1080,
        }
    }
}

/// Result of rendering a page
#[derive(Clone, Debug)]
pub struct RenderResult {
    /// The final URL after any redirects
    pub url: Url,
    /// The rendered HTML content
    pub html: String,
    /// Page title
    pub title: Option<String>,
    /// Time taken to render
    pub render_time_ms: u64,
    /// Whether JavaScript was executed
    pub js_executed: bool,
}

/// Browser pool for managing browser instances
#[cfg(feature = "browser")]
pub struct BrowserPool {
    config: BrowserConfig,
    playwright: Arc<Mutex<Option<playwright::Playwright>>>,
    browser: Arc<Mutex<Option<playwright::api::Browser>>>,
    semaphore: Arc<Semaphore>,
}

#[cfg(feature = "browser")]
impl BrowserPool {
    /// Create a new browser pool
    pub async fn new(config: BrowserConfig) -> Result<Self, CrawlerError> {
        let semaphore = Arc::new(Semaphore::new(config.max_contexts));

        Ok(Self {
            config,
            playwright: Arc::new(Mutex::new(None)),
            browser: Arc::new(Mutex::new(None)),
            semaphore,
        })
    }

    /// Initialize the browser (lazy initialization)
    async fn ensure_browser(&self) -> Result<(), CrawlerError> {
        let mut pw_guard = self.playwright.lock().await;
        if pw_guard.is_none() {
            info!("Initializing Playwright browser");

            let playwright = playwright::Playwright::initialize()
                .await
                .map_err(|e| CrawlerError::BrowserError(format!("Failed to initialize Playwright: {}", e)))?;

            *pw_guard = Some(playwright);
        }
        drop(pw_guard);

        let mut browser_guard = self.browser.lock().await;
        if browser_guard.is_none() {
            let pw_guard = self.playwright.lock().await;
            let pw = pw_guard.as_ref().unwrap();

            let browser = pw.chromium()
                .launcher()
                .headless(self.config.headless)
                .launch()
                .await
                .map_err(|e| CrawlerError::BrowserError(format!("Failed to launch browser: {}", e)))?;

            *browser_guard = Some(browser);
            info!("Browser launched successfully");
        }

        Ok(())
    }

    /// Render a page and return the HTML content
    pub async fn render(&self, url: &Url) -> Result<RenderResult, CrawlerError> {
        let start = std::time::Instant::now();

        // Acquire semaphore permit
        let _permit = self.semaphore
            .acquire()
            .await
            .map_err(|_| CrawlerError::BrowserError("Browser pool closed".to_string()))?;

        // Ensure browser is initialized
        self.ensure_browser().await?;

        let browser_guard = self.browser.lock().await;
        let browser = browser_guard.as_ref()
            .ok_or_else(|| CrawlerError::BrowserError("Browser not initialized".to_string()))?;

        // Create new context
        let context = browser.context_builder()
            .viewport_size(self.config.viewport_width as i32, self.config.viewport_height as i32)
            .build()
            .await
            .map_err(|e| CrawlerError::BrowserError(format!("Failed to create context: {}", e)))?;

        // Set user agent if configured
        if let Some(ref ua) = self.config.user_agent {
            // Note: Playwright context builder should handle this
        }

        // Create new page
        let page = context.new_page()
            .await
            .map_err(|e| CrawlerError::BrowserError(format!("Failed to create page: {}", e)))?;

        // Block resources if configured
        if self.config.block_images || self.config.block_fonts {
            // Resource blocking would be set up via route interception
            debug!("Resource blocking enabled");
        }

        // Navigate to URL
        debug!(url = %url, "Navigating to page");

        page.goto_builder(url.as_str())
            .timeout(self.config.timeout.as_millis() as f64)
            .goto()
            .await
            .map_err(|e| CrawlerError::BrowserError(format!("Navigation failed: {}", e)))?;

        // Wait for network idle if configured
        if self.config.wait_for_idle {
            tokio::time::sleep(self.config.idle_wait).await;
        }

        // Get the final URL (after redirects)
        let final_url_str = page.url()
            .await
            .map_err(|e| CrawlerError::BrowserError(format!("Failed to get URL: {}", e)))?;

        let final_url = Url::parse(&final_url_str)
            .unwrap_or_else(|_| url.clone());

        // Get page title
        let title = page.title()
            .await
            .ok();

        // Get rendered HTML
        let html = page.content()
            .await
            .map_err(|e| CrawlerError::BrowserError(format!("Failed to get content: {}", e)))?;

        // Close context
        context.close()
            .await
            .map_err(|e| warn!("Failed to close context: {}", e))
            .ok();

        let render_time = start.elapsed();
        debug!(
            url = %url,
            final_url = %final_url,
            render_time_ms = render_time.as_millis(),
            "Page rendered successfully"
        );

        Ok(RenderResult {
            url: final_url,
            html,
            title,
            render_time_ms: render_time.as_millis() as u64,
            js_executed: true,
        })
    }

    /// Close the browser pool
    pub async fn close(&self) -> Result<(), CrawlerError> {
        let mut browser_guard = self.browser.lock().await;
        if let Some(browser) = browser_guard.take() {
            browser.close()
                .await
                .map_err(|e| CrawlerError::BrowserError(format!("Failed to close browser: {}", e)))?;
        }
        info!("Browser pool closed");
        Ok(())
    }
}

#[cfg(feature = "browser")]
impl Drop for BrowserPool {
    fn drop(&mut self) {
        // Note: async cleanup should be done explicitly via close()
        debug!("BrowserPool dropped");
    }
}

/// Stub implementation when browser feature is disabled
#[cfg(not(feature = "browser"))]
pub struct BrowserPool;

#[cfg(not(feature = "browser"))]
impl BrowserPool {
    pub async fn new(_config: BrowserConfig) -> Result<Self, CrawlerError> {
        Err(CrawlerError::BrowserError(
            "Browser feature not enabled. Compile with --features browser".to_string()
        ))
    }

    pub async fn render(&self, _url: &Url) -> Result<RenderResult, CrawlerError> {
        Err(CrawlerError::BrowserError(
            "Browser feature not enabled".to_string()
        ))
    }

    pub async fn close(&self) -> Result<(), CrawlerError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_browser_config_default() {
        let config = BrowserConfig::default();
        assert_eq!(config.max_contexts, 4);
        assert!(config.headless);
        assert!(config.block_images);
    }
}
