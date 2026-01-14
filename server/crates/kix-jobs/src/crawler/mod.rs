//! Spider-based web crawler module.
//!
//! This module provides a high-performance web crawler built on the spider crate,
//! designed for documentation sites and knowledge base indexing.
//!
//! # Features
//!
//! - **Raw HTML access**: Preserves raw HTML for code extraction
//! - **Markdown conversion**: Automatic HTML → Markdown via spider_transformations
//! - **Smart crawling**: HTTP-first with optional JavaScript rendering
//! - **Configurable**: Rate limiting, depth control, URL filtering
//! - **Streaming**: Process pages as they're crawled
//!
//! # Example
//!
//! ```ignore
//! use kix_jobs::crawler::{SpiderCrawler, SpiderConfig};
//! use futures::StreamExt;
//!
//! let config = SpiderConfig::documentation();
//! let crawler = SpiderCrawler::new(config);
//!
//! let mut stream = crawler.crawl("https://docs.example.com").await;
//! while let Some(result) = stream.next().await {
//!     match result {
//!         Ok(page) => {
//!             println!("Crawled: {}", page.url);
//!             println!("HTML length: {}", page.html.len());
//!             println!("Markdown length: {}", page.markdown.len());
//!         }
//!         Err(e) => eprintln!("Error: {}", e),
//!     }
//! }
//! ```

mod config;
mod spider_adapter;

pub use config::{SpiderConfig, CrawlMode};
pub use spider_adapter::{SpiderCrawler, CrawledPage, CrawlError};
