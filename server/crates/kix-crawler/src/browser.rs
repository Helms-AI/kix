//! Browser-based page rendering using Playwright
//!
//! This module provides JavaScript rendering capabilities for crawling
//! dynamic web pages that require JavaScript execution.

use std::time::Duration;

#[cfg(feature = "browser")]
use std::path::PathBuf;

#[cfg(feature = "browser")]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(feature = "browser")]
use std::sync::Arc;

#[cfg(feature = "browser")]
use tokio::sync::{RwLock, Semaphore};

#[cfg(feature = "browser")]
use tracing::{debug, info, warn};

use url::Url;

use crate::CrawlerError;

/// Find the Chromium executable from the JavaScript Playwright installation
#[cfg(feature = "browser")]
fn find_chromium_executable() -> Option<PathBuf> {
    // Check environment variable first
    if let Ok(path) = std::env::var("PLAYWRIGHT_CHROMIUM_PATH") {
        let path = PathBuf::from(path);
        if path.exists() {
            return Some(path);
        }
    }

    // Look in the standard Playwright cache locations
    let cache_dirs = [
        dirs::cache_dir().map(|d| d.join("ms-playwright")),
        dirs::home_dir().map(|d| d.join("Library/Caches/ms-playwright")),
        Some(PathBuf::from("/app/cache/playwright")), // Docker location
    ];

    for cache_dir in cache_dirs.iter().flatten() {
        if !cache_dir.exists() {
            continue;
        }

        // Look for chromium-* directories
        if let Ok(entries) = std::fs::read_dir(cache_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.starts_with("chromium-") {
                    // macOS path
                    let mac_path = entry.path()
                        .join("chrome-mac-arm64")
                        .join("Google Chrome for Testing.app")
                        .join("Contents/MacOS/Google Chrome for Testing");
                    if mac_path.exists() {
                        return Some(mac_path);
                    }

                    // macOS x64 path
                    let mac_x64_path = entry.path()
                        .join("chrome-mac")
                        .join("Google Chrome for Testing.app")
                        .join("Contents/MacOS/Google Chrome for Testing");
                    if mac_x64_path.exists() {
                        return Some(mac_x64_path);
                    }

                    // Linux path
                    let linux_path = entry.path()
                        .join("chrome-linux")
                        .join("chrome");
                    if linux_path.exists() {
                        return Some(linux_path);
                    }
                }
            }
        }
    }

    None
}

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
    /// Maximum browser regeneration attempts before giving up
    /// Default: 3 - prevents infinite regeneration loops
    pub max_regeneration_attempts: u8,
}

impl Default for BrowserConfig {
    fn default() -> Self {
        Self {
            max_contexts: 8,  // Increased for better parallelism
            timeout: Duration::from_secs(30),
            wait_for_idle: true,
            idle_wait: Duration::from_secs(2),
            headless: true,
            block_images: true,
            block_fonts: true,
            user_agent: None,
            viewport_width: 1920,
            viewport_height: 1080,
            max_regeneration_attempts: 3, // Prevents infinite regeneration loops
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

/// Browser pool for managing browser instances with auto-regeneration
///
/// The browser is initialized eagerly on pool creation to enable true parallel
/// rendering. Multiple browser contexts can be created concurrently, controlled
/// by the semaphore.
///
/// Includes corruption detection AND auto-regeneration: if the browser becomes
/// corrupted (e.g., due to a panic or connection loss), the pool will automatically
/// regenerate the browser on the next render() call. Regeneration is limited to
/// prevent infinite loops.
#[cfg(feature = "browser")]
pub struct BrowserPool {
    config: BrowserConfig,
    playwright: playwright::Playwright,  // Owned, keeps Playwright alive
    /// Browser wrapped in RwLock for interior mutability - enables regeneration
    browser: RwLock<Option<playwright::api::Browser>>,
    semaphore: Arc<Semaphore>,
    /// Flag indicating if the browser connection is corrupted
    /// When true, regenerate() will be called on next render()
    is_corrupted: AtomicBool,
    /// Path to Chromium executable (cached for regeneration)
    chromium_path: Option<PathBuf>,
    /// Counter for regeneration attempts (prevents infinite loops)
    regeneration_attempts: std::sync::atomic::AtomicU8,
}

#[cfg(feature = "browser")]
impl BrowserPool {
    /// Create a new browser pool with eager browser initialization
    ///
    /// The browser is launched immediately to enable true parallel rendering.
    /// Multiple render() calls can create contexts concurrently, limited only
    /// by the semaphore (max_contexts).
    pub async fn new(config: BrowserConfig) -> Result<Self, CrawlerError> {
        let semaphore = Arc::new(Semaphore::new(config.max_contexts));

        // Initialize Playwright eagerly
        info!("Initializing Playwright browser");
        let playwright = playwright::Playwright::initialize()
            .await
            .map_err(|e| CrawlerError::BrowserError(format!("Failed to initialize Playwright: {}", e)))?;

        // Find the Chromium executable from JavaScript Playwright installation
        let chromium_path = find_chromium_executable();
        if let Some(ref path) = chromium_path {
            info!("Using Chromium at: {}", path.display());
        }

        // Launch browser eagerly
        let browser = if let Some(ref path) = chromium_path {
            playwright.chromium()
                .launcher()
                .headless(config.headless)
                .executable(path.as_path())
                .launch()
                .await
                .map_err(|e| CrawlerError::BrowserError(format!(
                    "Failed to launch browser with executable {}: {}",
                    path.display(), e
                )))?
        } else {
            // No pre-installed Chromium found, try default launch
            match playwright.chromium()
                .launcher()
                .headless(config.headless)
                .launch()
                .await
            {
                Ok(browser) => browser,
                Err(e) => {
                    warn!("Browser launch failed, attempting to install Chromium: {}", e);

                    // Install Chromium browser
                    playwright.install_chromium()
                        .map_err(|e| CrawlerError::BrowserError(format!(
                            "Failed to install Chromium: {}. \
                             Try running 'npx playwright install chromium' manually.",
                            e
                        )))?;
                    info!("Chromium installed successfully");

                    // Try launching again
                    playwright.chromium()
                        .launcher()
                        .headless(config.headless)
                        .launch()
                        .await
                        .map_err(|e| CrawlerError::BrowserError(format!(
                            "Failed to launch browser after install: {}", e
                        )))?
                }
            }
        };

        info!("Browser launched successfully (max_contexts: {})", config.max_contexts);

        Ok(Self {
            config,
            playwright,
            browser: RwLock::new(Some(browser)),
            semaphore,
            is_corrupted: AtomicBool::new(false),
            chromium_path,
            regeneration_attempts: std::sync::atomic::AtomicU8::new(0),
        })
    }

    /// Check if the browser pool is corrupted and needs to be recreated
    pub fn is_corrupted(&self) -> bool {
        self.is_corrupted.load(Ordering::SeqCst)
    }

    /// Mark the browser pool as corrupted
    ///
    /// This is called when we detect errors that indicate the browser connection
    /// is broken (e.g., "Object not found", "Target closed").
    fn mark_corrupted(&self) {
        if !self.is_corrupted.swap(true, Ordering::SeqCst) {
            warn!("Browser pool marked as corrupted - should be recreated");
        }
    }

    /// Check if an error message indicates browser corruption or unresponsiveness
    ///
    /// Includes timeout detection - if the browser is timing out on navigation,
    /// it may be hung and should be regenerated.
    fn is_corruption_error(error_msg: &str) -> bool {
        error_msg.contains("Object not found")
            || error_msg.contains("Target closed")
            || error_msg.contains("Connection closed")
            || error_msg.contains("Target page, context or browser has been closed")
            // Timeout patterns indicate browser may be hung or unresponsive
            || error_msg.contains("TimeoutError")
            || (error_msg.contains("Timeout") && error_msg.contains("exceeded"))
    }

    /// Regenerate the browser after corruption
    ///
    /// This method closes the existing browser (if any) and launches a new one.
    /// Called automatically by render() when corruption is detected.
    ///
    /// Includes:
    /// - Retry limit to prevent infinite regeneration loops
    /// - Exponential backoff between attempts
    /// - Counter reset on successful regeneration
    async fn regenerate(&self) -> Result<(), CrawlerError> {
        // Check regeneration attempt limit
        let attempts = self.regeneration_attempts.fetch_add(1, Ordering::SeqCst);
        if attempts >= self.config.max_regeneration_attempts {
            return Err(CrawlerError::BrowserError(format!(
                "Max regeneration attempts ({}) exceeded - browser pool is unusable",
                self.config.max_regeneration_attempts
            )));
        }

        // Exponential backoff before attempting regeneration
        if attempts > 0 {
            let delay = Duration::from_millis(500 * 2u64.pow(attempts as u32));
            warn!(
                attempt = attempts + 1,
                max = self.config.max_regeneration_attempts,
                delay_ms = delay.as_millis(),
                "Regeneration attempt with exponential backoff"
            );
            tokio::time::sleep(delay).await;
        }

        // Acquire write lock with timeout (could block on readers)
        let mut browser_guard = match tokio::time::timeout(
            Duration::from_secs(10),
            self.browser.write()
        ).await {
            Ok(g) => g,
            Err(_) => {
                warn!("Timeout acquiring write lock for browser regeneration");
                return Err(CrawlerError::BrowserError("Timeout acquiring browser write lock".to_string()));
            }
        };

        // Close existing browser if any (with timeout, ignore errors)
        if let Some(ref browser) = *browser_guard {
            let _ = tokio::time::timeout(Duration::from_secs(5), browser.close()).await;
        }
        *browser_guard = None;

        info!(
            attempt = attempts + 1,
            max = self.config.max_regeneration_attempts,
            "Regenerating browser after corruption"
        );

        // Launch new browser using cached chromium path (with timeout)
        let new_browser = if let Some(ref path) = self.chromium_path {
            match tokio::time::timeout(
                Duration::from_secs(20),
                self.playwright.chromium()
                    .launcher()
                    .headless(self.config.headless)
                    .executable(path.as_path())
                    .launch()
            ).await {
                Ok(Ok(b)) => b,
                Ok(Err(e)) => {
                    return Err(CrawlerError::BrowserError(format!(
                        "Failed to regenerate browser (attempt {}): {}",
                        attempts + 1, e
                    )));
                }
                Err(_) => {
                    warn!(attempt = attempts + 1, "Timeout launching browser during regeneration");
                    return Err(CrawlerError::BrowserError(format!(
                        "Timeout launching browser (attempt {})",
                        attempts + 1
                    )));
                }
            }
        } else {
            match tokio::time::timeout(
                Duration::from_secs(20),
                self.playwright.chromium()
                    .launcher()
                    .headless(self.config.headless)
                    .launch()
            ).await {
                Ok(Ok(b)) => b,
                Ok(Err(e)) => {
                    return Err(CrawlerError::BrowserError(format!(
                        "Failed to regenerate browser (attempt {}): {}",
                        attempts + 1, e
                    )));
                }
                Err(_) => {
                    warn!(attempt = attempts + 1, "Timeout launching browser during regeneration");
                    return Err(CrawlerError::BrowserError(format!(
                        "Timeout launching browser (attempt {})",
                        attempts + 1
                    )));
                }
            }
        };

        *browser_guard = Some(new_browser);
        self.is_corrupted.store(false, Ordering::SeqCst);
        // Reset counter on successful regeneration
        self.regeneration_attempts.store(0, Ordering::SeqCst);

        info!("Browser regenerated successfully");
        Ok(())
    }

    /// Render a page and return the HTML content
    ///
    /// Multiple render() calls can execute in parallel, limited by the semaphore.
    /// Each call creates its own browser context, enabling true parallel rendering.
    ///
    /// If the browser is corrupted, it will be automatically regenerated before
    /// attempting to render the page.
    pub async fn render(&self, url: &Url) -> Result<RenderResult, CrawlerError> {
        // Auto-regenerate if corrupted (with timeout to prevent hanging)
        if self.is_corrupted() {
            match tokio::time::timeout(Duration::from_secs(30), self.regenerate()).await {
                Ok(Ok(())) => {}
                Ok(Err(e)) => return Err(e),
                Err(_) => {
                    warn!(url = %url, "Timeout during browser regeneration");
                    return Err(CrawlerError::BrowserError("Timeout during browser regeneration".to_string()));
                }
            }
        }

        let start = std::time::Instant::now();

        // Acquire semaphore permit - this controls concurrency (with timeout)
        let _permit = match tokio::time::timeout(
            Duration::from_secs(30),
            self.semaphore.acquire()
        ).await {
            Ok(Ok(p)) => p,
            Ok(Err(_)) => return Err(CrawlerError::BrowserError("Browser pool closed".to_string())),
            Err(_) => {
                warn!(url = %url, "Timeout waiting for browser semaphore");
                return Err(CrawlerError::BrowserError("Timeout waiting for browser semaphore".to_string()));
            }
        };

        // Get read access to browser (with timeout to prevent deadlock during regeneration)
        let browser_guard = match tokio::time::timeout(
            Duration::from_secs(30),
            self.browser.read()
        ).await {
            Ok(g) => g,
            Err(_) => {
                warn!(url = %url, "Timeout waiting for browser read lock");
                return Err(CrawlerError::BrowserError("Timeout waiting for browser read lock".to_string()));
            }
        };
        let browser = browser_guard.as_ref().ok_or_else(|| {
            CrawlerError::BrowserError("Browser not initialized".to_string())
        })?;

        // Create new context with timeout
        let viewport = playwright::api::Viewport {
            width: self.config.viewport_width as i32,
            height: self.config.viewport_height as i32,
        };
        let context = match tokio::time::timeout(
            Duration::from_secs(15),
            browser.context_builder()
                .viewport(Some(viewport))
                .build()
        ).await {
            Ok(Ok(ctx)) => ctx,
            Ok(Err(e)) => {
                let msg = e.to_string();
                if Self::is_corruption_error(&msg) {
                    self.mark_corrupted();
                }
                return Err(CrawlerError::BrowserError(format!("Failed to create context: {}", e)));
            }
            Err(_) => {
                warn!(url = %url, "Timeout creating browser context");
                self.mark_corrupted();
                return Err(CrawlerError::BrowserError("Timeout creating browser context".to_string()));
            }
        };

        // Note: User agent should be set via context_builder().user_agent() if needed
        // The current playwright crate version doesn't expose this nicely

        // Create new page with timeout
        let page = match tokio::time::timeout(
            Duration::from_secs(10),
            context.new_page()
        ).await {
            Ok(Ok(p)) => p,
            Ok(Err(e)) => {
                let msg = e.to_string();
                if Self::is_corruption_error(&msg) {
                    self.mark_corrupted();
                }
                // Close context before returning error to prevent leak (with timeout)
                let _ = tokio::time::timeout(Duration::from_secs(2), context.close()).await;
                return Err(CrawlerError::BrowserError(format!("Failed to create page: {}", e)));
            }
            Err(_) => {
                warn!(url = %url, "Timeout creating new page");
                self.mark_corrupted();
                let _ = tokio::time::timeout(Duration::from_secs(2), context.close()).await;
                return Err(CrawlerError::BrowserError("Timeout creating new page".to_string()));
            }
        };

        // Block resources if configured
        if self.config.block_images || self.config.block_fonts {
            // Resource blocking would be set up via route interception
            debug!("Resource blocking enabled");
        }

        // Navigate to URL
        debug!(url = %url, "Navigating to page");

        if let Err(e) = page.goto_builder(url.as_str())
            .timeout(self.config.timeout.as_millis() as f64)
            .goto()
            .await
        {
            let msg = e.to_string();
            if Self::is_corruption_error(&msg) {
                self.mark_corrupted();
            }
            // Close context before returning error to prevent leak (with timeout)
            let _ = tokio::time::timeout(Duration::from_secs(2), context.close()).await;
            return Err(CrawlerError::BrowserError(format!("Navigation failed: {}", e)));
        }

        // Wait for network idle if configured
        if self.config.wait_for_idle {
            tokio::time::sleep(self.config.idle_wait).await;
        }

        // Get the final URL (after redirects)
        let final_url_str = match page.url() {
            Ok(u) => u,
            Err(e) => {
                let msg = e.to_string();
                if Self::is_corruption_error(&msg) {
                    self.mark_corrupted();
                }
                // Close context before returning error to prevent leak (with timeout)
                let _ = tokio::time::timeout(Duration::from_secs(2), context.close()).await;
                return Err(CrawlerError::BrowserError(format!("Failed to get URL: {}", e)));
            }
        };

        let final_url = Url::parse(&final_url_str)
            .unwrap_or_else(|_| url.clone());

        // Get page title with timeout to prevent hanging
        let title = match tokio::time::timeout(
            Duration::from_secs(10),
            page.title()
        ).await {
            Ok(Ok(t)) => Some(t),
            Ok(Err(e)) => {
                warn!(url = %url, error = %e, "Failed to get page title");
                None
            }
            Err(_) => {
                warn!(url = %url, "Timeout getting page title");
                self.mark_corrupted();
                None
            }
        };

        // Get rendered HTML with timeout to prevent hanging
        let html = match tokio::time::timeout(
            Duration::from_secs(10),
            page.content()
        ).await {
            Ok(Ok(h)) => h,
            Ok(Err(e)) => {
                let msg = e.to_string();
                if Self::is_corruption_error(&msg) {
                    self.mark_corrupted();
                }
                // Close context before returning error to prevent leak (with timeout)
                let _ = tokio::time::timeout(Duration::from_secs(2), context.close()).await;
                return Err(CrawlerError::BrowserError(format!("Failed to get content: {}", e)));
            }
            Err(_) => {
                warn!(url = %url, "Timeout getting page content");
                self.mark_corrupted();
                // Close context before returning error to prevent leak (with timeout)
                let _ = tokio::time::timeout(Duration::from_secs(2), context.close()).await;
                return Err(CrawlerError::BrowserError("Timeout getting page content".to_string()));
            }
        };

        // Close context with timeout to prevent hanging during cleanup
        if let Err(_) = tokio::time::timeout(
            Duration::from_secs(5),
            context.close()
        ).await {
            warn!(url = %url, "Timeout closing browser context");
            // Don't mark corrupted for cleanup timeout, just continue
        }

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
        let mut browser_guard = self.browser.write().await;
        if let Some(ref browser) = *browser_guard {
            browser.close()
                .await
                .map_err(|e| CrawlerError::BrowserError(format!("Failed to close browser: {}", e)))?;
        }
        *browser_guard = None;
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
        assert_eq!(config.max_contexts, 8);  // Increased for parallel rendering
        assert!(config.headless);
        assert!(config.block_images);
    }
}
