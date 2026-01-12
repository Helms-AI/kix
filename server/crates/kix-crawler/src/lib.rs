//! URL crawling and file scanning/upload handling
//!
//! This crate provides comprehensive web crawling functionality including:
//! - URL discovery via llms.txt, sitemap.xml, and robots.txt
//! - Browser-based rendering for JavaScript-heavy pages
//! - Rate limiting and robots.txt compliance
//! - SSRF protection for security
//! - Stage-based progress tracking
//! - Cancellation support for long-running crawls

// Core modules
pub mod browser;
pub mod crawler;
pub mod file_handler;
pub mod frontier;
pub mod rate_limiter;
pub mod robots;

// Advanced crawling modules
pub mod cancellation;
pub mod code;
pub mod discovery;
pub mod extractor;
pub mod progress;
pub mod service;
pub mod ssrf;
pub mod strategies;

// Re-exports: Core functionality
pub use browser::{BrowserConfig, BrowserPool, RenderResult};
pub use crawler::{Crawler, CrawlerConfig, CrawlResult};
pub use file_handler::{FileHandler, FileHandlerConfig, UploadedFile};
pub use frontier::{CrawlFrontier, CrawlTask};
pub use rate_limiter::{DomainRateLimiter, RateLimiterConfig};

// Re-exports: Advanced crawling functionality
pub use cancellation::{
    global_registry, run_cancellable, CancellableOperation, CancellationRegistry, JobGuard,
};
pub use code::{CodeExtractionService, CodePattern, ExtractedCode};
pub use discovery::{DiscoveryConfig, DiscoveryMethod, DiscoveryResult, DiscoveryService};
pub use extractor::{
    ContentExtractor, ExtractedCodeBlock, ExtractedContent, ExtractedHeader, MarkdownConfig,
};
pub use progress::{
    new_shared_tracker, CrawlStage, ProgressSnapshot, ProgressTracker, SharedProgressTracker,
};
pub use ssrf::{SsrfError, SsrfValidator};
pub use strategies::{
    BatchStrategy, CrawlPageResult, CrawlStrategy, CrawlStrategyType, RecursiveStrategy,
    SinglePageStrategy, SitemapStrategy, StrategyConfig,
};
pub use service::{
    CrawlJob, CrawlPipelineResult, CrawlerService, CrawlerServiceConfig, ProcessedPage,
    crawl_url, crawl_with_discovery,
};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum CrawlerError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("URL parse error: {0}")]
    UrlParse(#[from] url::ParseError),

    #[error("Rate limit exceeded for domain: {0}")]
    RateLimited(String),

    #[error("Robots.txt disallowed: {0}")]
    RobotsDisallowed(String),

    #[error("Maximum depth exceeded")]
    MaxDepthExceeded,

    #[error("Maximum pages exceeded")]
    MaxPagesExceeded,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Browser error: {0}")]
    BrowserError(String),

    #[error("Cancelled")]
    Cancelled,
}