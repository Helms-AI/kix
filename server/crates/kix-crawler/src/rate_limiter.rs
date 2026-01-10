//! Domain-based rate limiting

use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use governor::clock::DefaultClock;
use governor::state::{InMemoryState, NotKeyed};
use governor::{Quota, RateLimiter};
use tracing::debug;

/// Configuration for rate limiter
#[derive(Clone, Debug)]
pub struct RateLimiterConfig {
    /// Default requests per second per domain
    pub default_rps: f64,
    /// Minimum delay between requests to same domain
    pub min_delay: Duration,
    /// Maximum concurrent requests per domain
    pub max_concurrent_per_domain: usize,
}

impl Default for RateLimiterConfig {
    fn default() -> Self {
        Self {
            default_rps: 10.0,                    // Increased from 2.0 for higher throughput
            min_delay: Duration::from_millis(100), // Reduced from 500ms
            max_concurrent_per_domain: 8,         // Increased from 2 for parallel requests
        }
    }
}

type DomainLimiter = RateLimiter<NotKeyed, InMemoryState, DefaultClock>;

/// Domain-based rate limiter
pub struct DomainRateLimiter {
    /// Per-domain limiters
    limiters: DashMap<String, Arc<DomainLimiter>>,
    /// Per-domain delays from robots.txt
    crawl_delays: DashMap<String, Duration>,
    /// Configuration
    config: RateLimiterConfig,
}

impl DomainRateLimiter {
    /// Create a new rate limiter
    pub fn new(config: RateLimiterConfig) -> Self {
        Self {
            limiters: DashMap::new(),
            crawl_delays: DashMap::new(),
            config,
        }
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(RateLimiterConfig::default())
    }

    /// Set crawl delay for a domain (from robots.txt)
    pub fn set_crawl_delay(&self, domain: &str, delay: Duration) {
        self.crawl_delays.insert(domain.to_string(), delay);
    }

    /// Wait for permission to crawl a domain
    pub async fn acquire(&self, domain: &str) {
        let limiter = self.get_or_create_limiter(domain);

        // Wait for rate limit
        limiter.until_ready().await;

        // Apply crawl delay if set
        if let Some(delay) = self.crawl_delays.get(domain) {
            tokio::time::sleep(*delay).await;
        }

        debug!(domain = domain, "Rate limit acquired");
    }

    /// Try to acquire permission without waiting
    pub fn try_acquire(&self, domain: &str) -> bool {
        let limiter = self.get_or_create_limiter(domain);
        limiter.check().is_ok()
    }

    /// Get or create a limiter for a domain
    fn get_or_create_limiter(&self, domain: &str) -> Arc<DomainLimiter> {
        self.limiters
            .entry(domain.to_string())
            .or_insert_with(|| {
                let rps = self.config.default_rps;
                let quota = Quota::per_second(NonZeroU32::new(rps.ceil() as u32).unwrap_or(NonZeroU32::MIN));
                Arc::new(RateLimiter::direct(quota))
            })
            .clone()
    }

    /// Get statistics
    pub fn stats(&self) -> RateLimiterStats {
        RateLimiterStats {
            domains_tracked: self.limiters.len(),
            domains_with_delay: self.crawl_delays.len(),
        }
    }
}

/// Rate limiter statistics
#[derive(Debug, Clone)]
pub struct RateLimiterStats {
    pub domains_tracked: usize,
    pub domains_with_delay: usize,
}

impl Default for DomainRateLimiter {
    fn default() -> Self {
        Self::with_defaults()
    }
}