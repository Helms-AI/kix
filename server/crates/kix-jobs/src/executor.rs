//! Job executor for running jobs

use std::sync::Arc;

use futures::stream::{self, StreamExt};
use tokio::sync::Semaphore;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use kix_crawler::crawler::{CrawlResult, Crawler, CrawlerConfig};
use kix_sse::event::{Event, EventType, SourceType};
use kix_sse::ConnectionManager;

use crate::job::{Job, JobResult, JobType};
use crate::processor::{ContentProcessor, ProcessorConfig};
use crate::progress::ProgressTracker;
use crate::queue::JobQueue;
use crate::JobError;

/// Configuration for job executor
#[derive(Clone, Debug)]
pub struct ExecutorConfig {
    /// Maximum concurrent jobs
    pub max_concurrent: usize,
    /// Worker count
    pub worker_count: usize,
    /// Maximum memory per job (bytes)
    pub max_memory_per_job: usize,
    /// Database path for LanceDB storage
    pub db_path: String,
    /// Content processor configuration
    pub processor_config: ProcessorConfig,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            max_concurrent: 8,   // Increased from 4 for higher throughput
            worker_count: 8,     // Increased from 4 to match concurrent jobs
            max_memory_per_job: 512 * 1024 * 1024, // 512MB
            db_path: "./data/eip.lance".to_string(),
            processor_config: ProcessorConfig::default(),
        }
    }
}

/// Job executor manages job execution
pub struct JobExecutor {
    queue: Arc<JobQueue>,
    sse_manager: Arc<ConnectionManager>,
    config: ExecutorConfig,
    semaphore: Arc<Semaphore>,
    shutdown_token: CancellationToken,
    workers: Vec<JoinHandle<()>>,
    processor: Option<Arc<ContentProcessor>>,
}

impl JobExecutor {
    /// Create a new job executor
    pub async fn new(
        queue: Arc<JobQueue>,
        sse_manager: Arc<ConnectionManager>,
        config: ExecutorConfig,
    ) -> Result<Self, JobError> {
        let semaphore = Arc::new(Semaphore::new(config.max_concurrent));

        // Initialize content processor
        let processor = ContentProcessor::new(&config.db_path, config.processor_config.clone())
            .await?;

        Ok(Self {
            queue,
            sse_manager,
            config,
            semaphore,
            shutdown_token: CancellationToken::new(),
            workers: Vec::new(),
            processor: Some(Arc::new(processor)),
        })
    }

    /// Create a new job executor without a processor (for testing)
    pub fn new_without_processor(
        queue: Arc<JobQueue>,
        sse_manager: Arc<ConnectionManager>,
        config: ExecutorConfig,
    ) -> Self {
        let semaphore = Arc::new(Semaphore::new(config.max_concurrent));

        Self {
            queue,
            sse_manager,
            config,
            semaphore,
            shutdown_token: CancellationToken::new(),
            workers: Vec::new(),
            processor: None,
        }
    }

    /// Start the executor with worker threads
    pub fn start(&mut self) {
        info!(workers = self.config.worker_count, "Starting job executor");

        for i in 0..self.config.worker_count {
            let queue = self.queue.clone();
            let sse_manager = self.sse_manager.clone();
            let semaphore = self.semaphore.clone();
            let shutdown = self.shutdown_token.clone();
            let processor = self.processor.clone();

            let handle = tokio::spawn(async move {
                Self::worker_loop(i, queue, sse_manager, semaphore, shutdown, processor).await;
            });

            self.workers.push(handle);
        }
    }

    /// Worker loop for processing jobs
    async fn worker_loop(
        worker_id: usize,
        queue: Arc<JobQueue>,
        sse_manager: Arc<ConnectionManager>,
        semaphore: Arc<Semaphore>,
        shutdown: CancellationToken,
        processor: Option<Arc<ContentProcessor>>,
    ) {
        info!(worker_id, "Worker started");

        loop {
            tokio::select! {
                _ = shutdown.cancelled() => {
                    info!(worker_id, "Worker shutting down");
                    break;
                }
                job = queue.wait_for_job() => {
                    // Acquire semaphore permit
                    let _permit = semaphore.acquire().await.unwrap();

                    info!(worker_id, job_id = %job.id, "Processing job");

                    match Self::execute_job(&job, &sse_manager, processor.as_ref()).await {
                        Ok(result) => {
                            queue.complete(job.id, result).await;
                        }
                        Err(e) => {
                            error!(worker_id, job_id = %job.id, error = %e, "Job failed");
                            queue.fail(job.id, e.to_string(), 0).await;
                        }
                    }
                }
            }
        }
    }

    /// Execute a single job
    async fn execute_job(
        job: &Job,
        sse_manager: &ConnectionManager,
        processor: Option<&Arc<ContentProcessor>>,
    ) -> Result<JobResult, JobError> {
        let tracker = ProgressTracker::new();

        // Send job started event
        let source_type = match &job.job_type {
            JobType::Url { url, depth, .. } => SourceType::Url {
                url: url.clone(),
                depth: *depth,
            },
            JobType::FileUpload { file_names, .. } => SourceType::File {
                path: file_names.first().cloned().unwrap_or_default(),
            },
            JobType::Reindex { collection_id, .. } => SourceType::Directory {
                path: collection_id.clone(),
                recursive: true,
            },
        };

        let _ = sse_manager.broadcast_to_job(
            job.id,
            Event::new(EventType::JobStarted {
                job_id: job.id,
                source: source_type,
                total_items: None,
                timestamp: chrono::Utc::now(),
            }),
        );

        // Execute based on job type
        let result = match &job.job_type {
            JobType::Url {
                url,
                depth,
                respect_robots,
                render_js,
            } => {
                Self::execute_url_job(job, url, *depth, *respect_robots, *render_js, &tracker, sse_manager, processor)
                    .await?
            }
            JobType::FileUpload {
                file_paths,
                file_names,
                extract_archives,
            } => {
                Self::execute_file_upload_job(
                    job,
                    file_paths,
                    file_names,
                    *extract_archives,
                    &tracker,
                    sse_manager,
                    processor,
                )
                .await?
            }
            JobType::Reindex { collection_id, filters } => {
                Self::execute_reindex_job(job, collection_id, filters.as_ref(), &tracker, sse_manager, processor)
                    .await?
            }
        };

        // Send completion event
        let _ = sse_manager.broadcast_to_job(
            job.id,
            Event::new(EventType::JobCompleted {
                job_id: job.id,
                total_processed: result.items_processed,
                total_chunks: result.chunks_created,
                duration_ms: result.duration_ms,
                errors: result.errors.clone(),
            }),
        );

        Ok(result)
    }

    /// Execute URL crawling job
    async fn execute_url_job(
        job: &Job,
        url: &str,
        depth: usize,
        respect_robots: bool,
        _render_js: bool,
        tracker: &ProgressTracker,
        sse_manager: &ConnectionManager,
        processor: Option<&Arc<ContentProcessor>>,
    ) -> Result<JobResult, JobError> {
        use url::Url;

        tracker.set_step("Crawling URL").await;
        tracker.set_current_item(Some(url.to_string())).await;

        // Parse the seed URL
        let seed_url = Url::parse(url)
            .map_err(|e| JobError::Processing(format!("Invalid URL: {}", e)))?;

        // Configure and run crawler
        let crawler_config = CrawlerConfig {
            max_depth: depth,
            max_pages: usize::MAX, // Unlimited pages
            respect_robots,
            ..Default::default()
        };

        let crawler = Crawler::new(crawler_config)
            .map_err(|e| JobError::Processing(format!("Failed to create crawler: {}", e)))?;

        // Use a channel to collect results from the callback
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<CrawlResult>();

        // Use callback-based crawling - just collect results
        let stats = crawler
            .crawl(seed_url, move |result| {
                // Send result through channel
                let _ = tx.send(result);
            })
            .await
            .map_err(|e| JobError::Processing(format!("Crawl failed: {}", e)))?;

        // Collect all results from the channel
        let mut pages = Vec::new();
        rx.close();
        while let Some(result) = rx.recv().await {
            pages.push(result);
        }

        tracker.set_total(pages.len());
        info!(url = url, pages = pages.len(), stats = ?stats, "Crawl completed, processing pages in parallel");

        // Process pages in parallel using buffered streams with real-time updates
        let total_pages = pages.len();
        let concurrency = 16; // Process up to 16 pages simultaneously

        // Define the result type for each page processing
        struct PageResult {
            url: String,
            chunks: usize,
            embeddings: usize,
            error: Option<String>,
        }

        // Create parallel processing stream - use streaming iteration for real-time updates
        let job_id = job.id;
        let result_stream = stream::iter(pages)
            .map(|page| {
                let page_url = page.url.to_string();
                let content = page.content.clone();
                let proc = processor.cloned();

                async move {
                    if let Some(proc) = proc {
                        match proc.process_html(&content, &page_url).await {
                            Ok(result) => PageResult {
                                url: page_url,
                                chunks: result.chunks_created,
                                embeddings: result.embeddings_generated,
                                error: None,
                            },
                            Err(e) => {
                                warn!(url = page_url, error = %e, "Failed to process page");
                                PageResult {
                                    url: page_url.clone(),
                                    chunks: 0,
                                    embeddings: 0,
                                    error: Some(format!("Failed to process {}: {}", page_url, e)),
                                }
                            }
                        }
                    } else {
                        PageResult {
                            url: page_url,
                            chunks: 0,
                            embeddings: 0,
                            error: None,
                        }
                    }
                }
            })
            .buffer_unordered(concurrency);

        // Process results as they complete and send real-time updates
        let mut total_chunks = 0;
        let mut total_embeddings = 0;
        let mut errors = vec![];
        let mut processed_count = 0;

        // Use pin_mut! for safe iteration over the stream
        futures::pin_mut!(result_stream);

        while let Some(result) = result_stream.next().await {
            processed_count += 1;
            total_chunks += result.chunks;
            total_embeddings += result.embeddings;

            if let Some(ref err) = result.error {
                errors.push(err.clone());
                // Send error event for processing failure
                let _ = sse_manager.broadcast_to_job(
                    job_id,
                    Event::new(EventType::Error {
                        job_id,
                        item_path: Some(result.url.clone()),
                        error_message: err.clone(),
                        recoverable: true,
                    }),
                );
            } else {
                // Send item processed event immediately
                let _ = sse_manager.broadcast_to_job(
                    job_id,
                    Event::new(EventType::ItemProcessed {
                        job_id,
                        item_path: result.url.clone(),
                        chunks_created: result.chunks,
                        embeddings_generated: result.embeddings,
                        duration_ms: 0,
                    }),
                );
            }

            // Update tracker
            tracker.increment(1).await;
            tracker
                .update_metrics(|m| {
                    m.chunks_created = total_chunks;
                    m.items_discovered = processed_count;
                })
                .await;

            // Send progress event immediately
            let progress = tracker.get_progress().await;
            let _ = sse_manager.broadcast_to_job(
                job_id,
                Event::new(EventType::Progress {
                    job_id,
                    processed: processed_count,
                    total: total_pages,
                    current_item: Some(result.url),
                    rate: progress.rate,
                    percentage: progress.percentage,
                }),
            );
        }

        Ok(JobResult {
            items_processed: total_pages,
            chunks_created: total_chunks,
            embeddings_generated: total_embeddings,
            errors,
            duration_ms: tracker.elapsed().as_millis() as u64,
        })
    }

    /// Execute file upload job
    async fn execute_file_upload_job(
        job: &Job,
        file_paths: &[std::path::PathBuf],
        file_names: &[String],
        _extract_archives: bool,
        tracker: &ProgressTracker,
        sse_manager: &ConnectionManager,
        processor: Option<&Arc<ContentProcessor>>,
    ) -> Result<JobResult, JobError> {
        tracker.set_step("Processing uploaded files in parallel").await;
        tracker.set_total(file_paths.len());

        let total_files = file_paths.len();
        let concurrency = 8; // Process up to 8 files simultaneously

        // Define result type for file processing
        struct FileResult {
            name: String,
            chunks: usize,
            embeddings: usize,
            error: Option<String>,
        }

        // Create owned pairs of (path, name) for parallel processing
        let file_pairs: Vec<_> = file_paths
            .iter()
            .cloned()
            .zip(file_names.iter().cloned())
            .collect();

        let job_id = job.id;
        let result_stream = stream::iter(file_pairs)
            .map(|(path, name)| {
                let proc = processor.cloned();

                async move {
                    if let Some(proc) = proc {
                        match proc.process_file(&path, &name).await {
                            Ok(result) => FileResult {
                                name,
                                chunks: result.chunks_created,
                                embeddings: result.embeddings_generated,
                                error: None,
                            },
                            Err(e) => {
                                warn!(file = name, error = %e, "Failed to process file");
                                FileResult {
                                    name: name.clone(),
                                    chunks: 0,
                                    embeddings: 0,
                                    error: Some(format!("Failed to process {}: {}", name, e)),
                                }
                            }
                        }
                    } else {
                        FileResult {
                            name,
                            chunks: 0,
                            embeddings: 0,
                            error: None,
                        }
                    }
                }
            })
            .buffer_unordered(concurrency);

        // Process results as they complete and send real-time updates
        let mut total_chunks = 0;
        let mut total_embeddings = 0;
        let mut errors = vec![];
        let mut processed_count = 0;

        // Use pin_mut! for safe iteration over the stream
        futures::pin_mut!(result_stream);

        while let Some(result) = result_stream.next().await {
            processed_count += 1;
            total_chunks += result.chunks;
            total_embeddings += result.embeddings;

            if let Some(ref err) = result.error {
                errors.push(err.clone());
                // Send error event for processing failure
                let _ = sse_manager.broadcast_to_job(
                    job_id,
                    Event::new(EventType::Error {
                        job_id,
                        item_path: Some(result.name.clone()),
                        error_message: err.clone(),
                        recoverable: true,
                    }),
                );
            } else {
                // Send item processed event immediately
                let _ = sse_manager.broadcast_to_job(
                    job_id,
                    Event::new(EventType::ItemProcessed {
                        job_id,
                        item_path: result.name.clone(),
                        chunks_created: result.chunks,
                        embeddings_generated: result.embeddings,
                        duration_ms: 0,
                    }),
                );
            }

            // Update tracker
            tracker.increment(1).await;
            tracker
                .update_metrics(|m| {
                    m.chunks_created = total_chunks;
                    m.items_discovered = processed_count;
                })
                .await;

            // Send progress event immediately
            let progress = tracker.get_progress().await;
            let _ = sse_manager.broadcast_to_job(
                job_id,
                Event::new(EventType::Progress {
                    job_id,
                    processed: processed_count,
                    total: total_files,
                    current_item: Some(result.name.clone()),
                    rate: progress.rate,
                    percentage: progress.percentage,
                }),
            );
        }

        Ok(JobResult {
            items_processed: total_files,
            chunks_created: total_chunks,
            embeddings_generated: total_embeddings,
            errors,
            duration_ms: tracker.elapsed().as_millis() as u64,
        })
    }

    /// Execute reindex job
    async fn execute_reindex_job(
        _job: &Job,
        collection_id: &str,
        _filters: Option<&crate::job::ReindexFilters>,
        tracker: &ProgressTracker,
        _sse_manager: &ConnectionManager,
        _processor: Option<&Arc<ContentProcessor>>,
    ) -> Result<JobResult, JobError> {
        tracker.set_step("Re-indexing collection").await;
        info!(collection_id = collection_id, "Re-indexing not yet implemented");

        // TODO: Implement actual reindexing with eip-store
        // This would involve:
        // 1. Fetching all documents in the collection
        // 2. Re-parsing and re-chunking them
        // 3. Re-generating embeddings
        // 4. Updating the vector store

        Ok(JobResult {
            items_processed: 0,
            chunks_created: 0,
            embeddings_generated: 0,
            errors: vec![],
            duration_ms: tracker.elapsed().as_millis() as u64,
        })
    }

    /// Shutdown the executor gracefully
    pub async fn shutdown(&self) {
        info!("Shutting down job executor");
        self.shutdown_token.cancel();

        // Wait for workers to finish current jobs
        for handle in &self.workers {
            handle.abort();
        }
    }

    /// Get executor status
    pub fn status(&self) -> ExecutorStatus {
        ExecutorStatus {
            running_jobs: self.queue.running_count(),
            available_permits: self.semaphore.available_permits(),
            is_shutdown: self.shutdown_token.is_cancelled(),
        }
    }
}

/// Executor status information
#[derive(Debug, Clone)]
pub struct ExecutorStatus {
    pub running_jobs: usize,
    pub available_permits: usize,
    pub is_shutdown: bool,
}