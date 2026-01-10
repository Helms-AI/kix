//! URL crawling and file scanning/upload handling

pub mod browser;
pub mod crawler;
pub mod file_handler;
pub mod frontier;
pub mod rate_limiter;
pub mod robots;

pub use browser::{BrowserConfig, BrowserPool, RenderResult};
pub use crawler::{Crawler, CrawlerConfig, CrawlResult};
pub use file_handler::{FileHandler, FileHandlerConfig, UploadedFile};
pub use frontier::{CrawlFrontier, CrawlTask};
pub use rate_limiter::{DomainRateLimiter, RateLimiterConfig};

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