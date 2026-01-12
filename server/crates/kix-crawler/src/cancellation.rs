//! Cancellation Registry for Crawl Jobs
//!
//! Implements a global cancellation registry pattern for
//! graceful cancellation of in-flight crawl operations.

use dashmap::DashMap;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

/// A registry for managing cancellation tokens across multiple crawl jobs
#[derive(Debug, Clone)]
pub struct CancellationRegistry {
    /// Map of job IDs to their cancellation tokens
    active_jobs: Arc<DashMap<Uuid, CancellationToken>>,
}

impl Default for CancellationRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl CancellationRegistry {
    /// Create a new cancellation registry
    pub fn new() -> Self {
        Self {
            active_jobs: Arc::new(DashMap::new()),
        }
    }

    /// Register a new job and get its cancellation token
    ///
    /// Returns a CancellationToken that can be used to:
    /// - Check if cancellation was requested via `is_cancelled()`
    /// - Wait for cancellation via `cancelled().await`
    /// - Create child tokens for subtasks
    pub fn register(&self, job_id: Uuid) -> CancellationToken {
        let token = CancellationToken::new();
        self.active_jobs.insert(job_id, token.clone());
        tracing::debug!("Registered cancellation token for job {}", job_id);
        token
    }

    /// Cancel a job by its ID
    ///
    /// Returns true if the job was found and cancelled,
    /// false if the job was not found (already completed or never registered)
    pub fn cancel(&self, job_id: &Uuid) -> bool {
        if let Some((_, token)) = self.active_jobs.remove(job_id) {
            token.cancel();
            tracing::info!("Cancelled job {}", job_id);
            true
        } else {
            tracing::debug!("Job {} not found for cancellation", job_id);
            false
        }
    }

    /// Check if a job has been cancelled
    pub fn is_cancelled(&self, job_id: &Uuid) -> bool {
        self.active_jobs
            .get(job_id)
            .map(|token| token.is_cancelled())
            .unwrap_or(true) // If not found, consider it cancelled
    }

    /// Unregister a completed job without cancelling it
    ///
    /// Call this when a job completes normally to clean up the registry
    pub fn complete(&self, job_id: &Uuid) {
        if self.active_jobs.remove(job_id).is_some() {
            tracing::debug!("Completed job {}", job_id);
        }
    }

    /// Get the number of active jobs
    pub fn active_count(&self) -> usize {
        self.active_jobs.len()
    }

    /// Get a list of all active job IDs
    pub fn active_jobs(&self) -> Vec<Uuid> {
        self.active_jobs.iter().map(|entry| *entry.key()).collect()
    }

    /// Cancel all active jobs
    ///
    /// Returns the number of jobs cancelled
    pub fn cancel_all(&self) -> usize {
        let jobs: Vec<_> = self.active_jobs.iter().map(|e| *e.key()).collect();
        let count = jobs.len();

        for job_id in jobs {
            self.cancel(&job_id);
        }

        tracing::info!("Cancelled {} active jobs", count);
        count
    }
}

/// Extension trait for using cancellation tokens in async code
pub trait CancellableOperation {
    /// Run an async operation with cancellation support
    ///
    /// Returns `Err(CrawlerError::Cancelled)` if the token is cancelled
    /// before or during the operation
    fn with_cancellation<F, T>(
        &self,
        token: &CancellationToken,
        operation: F,
    ) -> impl std::future::Future<Output = Result<T, super::CrawlerError>> + Send
    where
        F: std::future::Future<Output = Result<T, super::CrawlerError>> + Send,
        T: Send;
}

impl CancellableOperation for () {
    async fn with_cancellation<F, T>(
        &self,
        token: &CancellationToken,
        operation: F,
    ) -> Result<T, super::CrawlerError>
    where
        F: std::future::Future<Output = Result<T, super::CrawlerError>> + Send,
        T: Send,
    {
        tokio::select! {
            biased;
            _ = token.cancelled() => {
                Err(super::CrawlerError::Cancelled)
            }
            result = operation => {
                result
            }
        }
    }
}

/// Helper function to run an operation with cancellation support
pub async fn run_cancellable<F, T>(
    token: &CancellationToken,
    operation: F,
) -> Result<T, super::CrawlerError>
where
    F: std::future::Future<Output = Result<T, super::CrawlerError>> + Send,
    T: Send,
{
    tokio::select! {
        biased;
        _ = token.cancelled() => {
            Err(super::CrawlerError::Cancelled)
        }
        result = operation => {
            result
        }
    }
}

/// A guard that automatically unregisters a job when dropped
pub struct JobGuard {
    registry: CancellationRegistry,
    job_id: Uuid,
}

impl JobGuard {
    /// Create a new job guard
    pub fn new(registry: CancellationRegistry, job_id: Uuid) -> Self {
        Self { registry, job_id }
    }

    /// Get the job ID
    pub fn job_id(&self) -> Uuid {
        self.job_id
    }
}

impl Drop for JobGuard {
    fn drop(&mut self) {
        self.registry.complete(&self.job_id);
    }
}

/// Create a new global cancellation registry
///
/// This is typically called once at application startup
pub fn global_registry() -> CancellationRegistry {
    CancellationRegistry::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_cancel() {
        let registry = CancellationRegistry::new();
        let job_id = Uuid::new_v4();

        let token = registry.register(job_id);
        assert!(!token.is_cancelled());
        assert_eq!(registry.active_count(), 1);

        let cancelled = registry.cancel(&job_id);
        assert!(cancelled);
        assert!(token.is_cancelled());
        assert_eq!(registry.active_count(), 0);
    }

    #[test]
    fn test_cancel_nonexistent() {
        let registry = CancellationRegistry::new();
        let job_id = Uuid::new_v4();

        let cancelled = registry.cancel(&job_id);
        assert!(!cancelled);
    }

    #[test]
    fn test_complete() {
        let registry = CancellationRegistry::new();
        let job_id = Uuid::new_v4();

        let token = registry.register(job_id);
        assert_eq!(registry.active_count(), 1);

        registry.complete(&job_id);
        assert_eq!(registry.active_count(), 0);
        // Token should still be usable but not in registry
        assert!(!token.is_cancelled());
    }

    #[test]
    fn test_cancel_all() {
        let registry = CancellationRegistry::new();
        let tokens: Vec<_> = (0..5)
            .map(|_| {
                let job_id = Uuid::new_v4();
                registry.register(job_id)
            })
            .collect();

        assert_eq!(registry.active_count(), 5);

        let count = registry.cancel_all();
        assert_eq!(count, 5);
        assert_eq!(registry.active_count(), 0);

        for token in tokens {
            assert!(token.is_cancelled());
        }
    }

    #[tokio::test]
    async fn test_run_cancellable() {
        let token = CancellationToken::new();

        // Test successful operation
        let result: Result<i32, super::super::CrawlerError> =
            run_cancellable(&token, async { Ok(42) }).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);

        // Test cancelled operation
        token.cancel();
        let result: Result<i32, super::super::CrawlerError> =
            run_cancellable(&token, async { Ok(42) }).await;
        assert!(matches!(result, Err(super::super::CrawlerError::Cancelled)));
    }

    #[test]
    fn test_job_guard() {
        let registry = CancellationRegistry::new();
        let job_id = Uuid::new_v4();

        registry.register(job_id);
        assert_eq!(registry.active_count(), 1);

        {
            let _guard = JobGuard::new(registry.clone(), job_id);
            assert_eq!(registry.active_count(), 1);
        } // guard dropped here

        assert_eq!(registry.active_count(), 0);
    }
}
