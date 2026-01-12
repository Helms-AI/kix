//! Job executor for running jobs

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use chrono::Utc;
use futures::stream::{self, StreamExt};
use tokio::sync::{RwLock, Semaphore};
use tokio::task::JoinHandle;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use url::Url;

use kix_crawler::crawler::{CrawlResult, Crawler, CrawlerConfig};
use kix_sse::event::{Event, EventType, SourceType};
use kix_sse::ConnectionManager;
use kix_store::{JobItemRecord, JobRecord, JobStats, JobStore};

use crate::job::{Job, JobResult, JobType};
use crate::processor::{ContentProcessor, ProcessorConfig};
use crate::progress::ProgressTracker;
use crate::queue::JobQueue;
use crate::JobError;

/// Normalize URL for display deduplication (strips query params and fragments)
/// This ensures URLs like /login?redirect=/docs and /login?redirect=/api
/// are treated as the same page for UI display purposes.
fn normalize_url_for_display(url: &Url) -> String {
    let mut result = String::new();

    // Scheme
    result.push_str(url.scheme());
    result.push_str("://");

    // Host (lowercase)
    if let Some(host) = url.host_str() {
        result.push_str(&host.to_lowercase());
    }

    // Path only (no query params, no fragment)
    // Remove trailing slash unless root
    let path = url.path();
    if path == "/" {
        result.push('/');
    } else {
        result.push_str(path.trim_end_matches('/'));
    }

    result
}

/// Type alias for cache invalidation callback
pub type CacheInvalidationCallback = Arc<dyn Fn() + Send + Sync>;

/// Configuration for job executor
#[derive(Clone)]
pub struct ExecutorConfig {
    /// Maximum concurrent jobs
    pub max_concurrent: usize,
    /// Worker count
    pub worker_count: usize,
    /// Maximum memory per job (bytes)
    pub max_memory_per_job: usize,
    /// Database path for LanceDB storage
    pub db_path: String,
    /// Path for job history persistence (jobs.lance)
    pub jobs_db_path: Option<String>,
    /// Content processor configuration
    pub processor_config: ProcessorConfig,
    /// Optional callback invoked when a job completes (for cache invalidation)
    pub on_job_complete: Option<CacheInvalidationCallback>,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            max_concurrent: 8,   // Increased from 4 for higher throughput
            worker_count: 8,     // Increased from 4 to match concurrent jobs
            max_memory_per_job: 512 * 1024 * 1024, // 512MB
            db_path: "./data/eip.lance".to_string(),
            jobs_db_path: Some("./data/lancedb/jobs.lance".to_string()),
            processor_config: ProcessorConfig::default(),
            on_job_complete: None,
        }
    }
}

/// Context for tracking items during job execution.
/// Collects data for job history persistence.
#[derive(Clone)]
pub struct JobExecutionContext {
    /// Items keyed by URL/path for O(1) upsert
    pub items: Arc<RwLock<HashMap<String, JobItemRecord>>>,
    /// Job start time for duration calculation
    pub start_time: std::time::Instant,
    /// Actual count of discovered items (may differ from items.len() due to filtering)
    pub items_discovered: Arc<AtomicUsize>,
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
    /// Job history store (optional - for persisting completed jobs)
    job_store: Option<Arc<JobStore>>,
    /// Callback invoked when a job completes (for cache invalidation)
    on_job_complete: Option<CacheInvalidationCallback>,
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

        // Initialize job history store if configured
        let job_store = if let Some(ref jobs_path) = config.jobs_db_path {
            match JobStore::new(jobs_path).await {
                Ok(mut store) => {
                    if let Err(e) = store.init_tables().await {
                        warn!("Failed to initialize job history tables: {}", e);
                        None
                    } else {
                        info!("Job history store initialized at: {}", jobs_path);
                        Some(Arc::new(store))
                    }
                }
                Err(e) => {
                    warn!("Failed to create job history store: {}", e);
                    None
                }
            }
        } else {
            None
        };

        let on_job_complete = config.on_job_complete.clone();

        Ok(Self {
            queue,
            sse_manager,
            config,
            semaphore,
            shutdown_token: CancellationToken::new(),
            workers: Vec::new(),
            processor: Some(Arc::new(processor)),
            job_store,
            on_job_complete,
        })
    }

    /// Create a new job executor without a processor (for testing)
    pub fn new_without_processor(
        queue: Arc<JobQueue>,
        sse_manager: Arc<ConnectionManager>,
        config: ExecutorConfig,
    ) -> Self {
        let semaphore = Arc::new(Semaphore::new(config.max_concurrent));
        let on_job_complete = config.on_job_complete.clone();

        Self {
            queue,
            sse_manager,
            config,
            semaphore,
            shutdown_token: CancellationToken::new(),
            workers: Vec::new(),
            processor: None,
            job_store: None,
            on_job_complete,
        }
    }

    /// Get a reference to the job store (for API access)
    pub fn job_store(&self) -> Option<&Arc<JobStore>> {
        self.job_store.as_ref()
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
            let job_store = self.job_store.clone();
            let on_job_complete = self.on_job_complete.clone();

            let handle = tokio::spawn(async move {
                Self::worker_loop(i, queue, sse_manager, semaphore, shutdown, processor, job_store, on_job_complete).await;
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
        job_store: Option<Arc<JobStore>>,
        on_job_complete: Option<CacheInvalidationCallback>,
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

                    // Create execution context for item tracking
                    let ctx = JobExecutionContext {
                        items: Arc::new(RwLock::new(HashMap::new())),
                        start_time: std::time::Instant::now(),
                        items_discovered: Arc::new(AtomicUsize::new(0)),
                    };

                    match Self::execute_job(&job, &sse_manager, processor.as_ref(), &ctx).await {
                        Ok(result) => {
                            // Persist job history before marking complete
                            if let Some(ref store) = job_store {
                                if let Err(e) = Self::persist_job(&job, &result, "completed", &ctx, store).await {
                                    warn!(job_id = %job.id, error = %e, "Failed to persist job history");
                                }
                            }
                            queue.complete(job.id, result).await;

                            // Invalidate caches after successful job completion
                            if let Some(ref callback) = on_job_complete {
                                callback();
                                info!(job_id = %job.id, "Cache invalidation triggered after job completion");
                            }
                        }
                        Err(e) => {
                            error!(worker_id, job_id = %job.id, error = %e, "Job failed");
                            // Persist failed job
                            if let Some(ref store) = job_store {
                                let failed_result = JobResult {
                                    items_processed: 0,
                                    chunks_created: 0,
                                    embeddings_generated: 0,
                                    errors: vec![e.to_string()],
                                    duration_ms: ctx.start_time.elapsed().as_millis() as u64,
                                };
                                if let Err(pe) = Self::persist_job(&job, &failed_result, "failed", &ctx, store).await {
                                    warn!(job_id = %job.id, error = %pe, "Failed to persist failed job history");
                                }
                            }
                            queue.fail(job.id, e.to_string(), 0).await;
                        }
                    }
                }
            }
        }
    }

    /// Persist job and its items to the job history store
    async fn persist_job(
        job: &Job,
        result: &JobResult,
        status: &str,
        ctx: &JobExecutionContext,
        store: &JobStore,
    ) -> Result<(), kix_store::StoreError> {
        // Extract source info from job type
        let (source_url, source_domain) = match &job.job_type {
            JobType::Url { url, .. } => {
                let domain = Url::parse(url)
                    .ok()
                    .and_then(|u| u.domain().map(|d| d.to_string()));
                (Some(url.clone()), domain)
            }
            JobType::FileUpload { file_names, .. } => {
                (file_names.first().cloned(), None)
            }
            JobType::Reindex { collection_id, .. } => {
                (Some(collection_id.clone()), None)
            }
        };

        // Build job type string
        let job_type_str = match &job.job_type {
            JobType::Url { .. } => "url",
            JobType::FileUpload { .. } => "file_upload",
            JobType::Reindex { .. } => "reindex",
        };

        // Serialize job config
        let config = serde_json::to_value(&job.config).unwrap_or(serde_json::json!({}));

        // Calculate rate
        let duration_secs = result.duration_ms as f64 / 1000.0;
        let rate = if duration_secs > 0.0 {
            result.items_processed as f64 / duration_secs
        } else {
            0.0
        };

        // Get actual discovered count from context
        let items_discovered = ctx.items_discovered.load(Ordering::Relaxed) as u32;

        let job_record = JobRecord {
            job_id: job.id.to_string(),
            job_type: job_type_str.to_string(),
            status: status.to_string(),
            created_at: job.created_at,
            started_at: Some(Utc::now() - chrono::Duration::milliseconds(result.duration_ms as i64)),
            completed_at: Utc::now(),
            source_url,
            source_domain,
            config,
            stats: JobStats {
                items_processed: result.items_processed as u32,
                items_discovered: items_discovered.max(result.items_processed as u32),
                chunks_created: result.chunks_created as u32,
                embeddings_generated: result.embeddings_generated as u32,
                error_count: result.errors.len() as u32,
                duration_ms: result.duration_ms,
                processing_rate: rate,
            },
            errors: result.errors.clone(),
        };

        // Get collected items as Vec
        let items_map = ctx.items.read().await;
        let items: Vec<JobItemRecord> = items_map.values().cloned().collect();

        info!(
            job_id = %job.id,
            items_count = items.len(),
            items_discovered = items_discovered,
            "Persisting job with items"
        );

        store.save_job(&job_record, &items).await
    }

    /// Execute a single job
    async fn execute_job(
        job: &Job,
        sse_manager: &ConnectionManager,
        processor: Option<&Arc<ContentProcessor>>,
        ctx: &JobExecutionContext,
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
                timeout_secs,
                max_pages,
            } => {
                Self::execute_url_job(
                    job,
                    url,
                    *depth,
                    *max_pages,
                    *respect_robots,
                    *render_js,
                    *timeout_secs,
                    &tracker,
                    sse_manager,
                    processor,
                    ctx,
                )
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
                    ctx,
                )
                .await?
            }
            JobType::Reindex { collection_id, filters } => {
                Self::execute_reindex_job(job, collection_id, filters.as_ref(), &tracker, sse_manager, processor, ctx)
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

    /// Execute URL crawling job with STREAMING processing
    ///
    /// This processes pages AS THEY ARE CRAWLED rather than collecting all first.
    /// This provides immediate feedback, lower memory usage, and early exit capability.
    async fn execute_url_job(
        job: &Job,
        url: &str,
        depth: usize,
        max_pages: usize,
        respect_robots: bool,
        render_js: bool,
        timeout_secs: u64,
        tracker: &ProgressTracker,
        sse_manager: &ConnectionManager,
        processor: Option<&Arc<ContentProcessor>>,
        ctx: &JobExecutionContext,
    ) -> Result<JobResult, JobError> {
        use std::time::Duration;
        use url::Url;

        tracker.set_step("Crawling and processing URL").await;
        tracker.set_current_item(Some(url.to_string())).await;

        // Parse the seed URL
        let seed_url = Url::parse(url)
            .map_err(|e| JobError::Processing(format!("Invalid URL: {}", e)))?;

        // Configure crawler
        let effective_max_pages = if max_pages == 0 { usize::MAX } else { max_pages };
        let crawler_config = CrawlerConfig {
            max_depth: depth,
            max_pages: effective_max_pages,
            respect_robots,
            render_js,
            render_timeout: Duration::from_secs(timeout_secs),
            ..Default::default()
        };

        let crawler = Arc::new(
            Crawler::new(crawler_config)
                .await
                .map_err(|e| JobError::Processing(format!("Failed to create crawler: {}", e)))?
        );

        // Emit ItemDiscovered for seed URL (path-only normalization to prevent duplicates)
        let normalized_seed = normalize_url_for_display(&seed_url);
        let seed_discovered_at = Utc::now();
        let _ = sse_manager.broadcast_to_job(
            job.id,
            Event::new(EventType::ItemDiscovered {
                job_id: job.id,
                item_path: normalized_seed.clone(),
                discovered_at: seed_discovered_at,
                parent_url: None,
                depth: 0,
            }),
        );

        // Track seed URL as an item for persistence
        {
            let mut items = ctx.items.write().await;
            items.insert(normalized_seed.clone(), JobItemRecord::new_page(job.id.to_string(), normalized_seed.clone()));
        }
        ctx.items_discovered.fetch_add(1, Ordering::Relaxed);

        // Stats for streaming processing (using atomics for thread-safe access)
        let processed_count = Arc::new(AtomicUsize::new(0));
        let mut total_chunks = 0;
        let mut total_embeddings = 0;
        let mut errors = vec![];
        let job_id = job.id;
        let start = std::time::Instant::now();
        let concurrency = 8; // Process up to 8 pages concurrently

        // Track URLs we've already emitted ItemDiscovered for to prevent duplicates
        // The seed URL is already emitted above, so add it (using normalized form)
        let emitted_urls: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
        emitted_urls.lock().unwrap().insert(normalized_seed.clone());

        // Use UNBOUNDED channel - send() is synchronous and never drops messages
        // This ensures all crawled pages are processed, even if processing is slower than crawling
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<CrawlResult>();

        // Spawn crawler task - runs independently
        let crawler_handle = tokio::spawn(async move {
            let result = crawler
                .crawl(seed_url, move |crawl_result| {
                    // Unbounded send - synchronous, never blocks, never drops
                    let _ = tx.send(crawl_result);
                })
                .await;

            // Shutdown crawler to release browser resources
            if let Err(e) = crawler.shutdown().await {
                warn!("Failed to shutdown crawler: {}", e);
            }

            result
        });

        // Define result type for parallel processing
        struct PageResult {
            url: String,
            chunks: usize,
            embeddings: usize,
            error: Option<String>,
        }

        // Convert receiver to stream for parallel processing
        let result_stream = UnboundedReceiverStream::new(rx);
        let processed_count_clone = processed_count.clone();

        // Reference SSE manager for use in processing closure
        let sse_for_processing = sse_manager;
        // Clone emitted_urls for use in processing closure (to track discovered pages)
        let emitted_urls_for_processing = emitted_urls.clone();

        // Process pages in PARALLEL using buffer_unordered (like file uploads)
        info!(url = url, max_pages = effective_max_pages, concurrency = concurrency, "Starting parallel crawl and process");

        let processing_stream = result_stream
            .take_while(move |_| {
                // Stop accepting new pages if we've reached max_pages
                let count = processed_count_clone.load(Ordering::Relaxed);
                futures::future::ready(count < effective_max_pages)
            })
            .map(|result| {
                let proc = processor.cloned();
                let sse = sse_for_processing;
                let jid = job_id;
                let emitted = emitted_urls_for_processing.clone();
                async move {
                    // Keep raw URL for content processing (storage needs original URL)
                    let raw_url = result.url.to_string();
                    // Normalize URL for SSE events (must match ItemDiscovered format)
                    let display_url = normalize_url_for_display(&result.url);

                    // Emit ItemDiscovered when page enters processing (only if not already emitted)
                    // This ensures discovered count = processable pages (not all links found)
                    {
                        let mut emitted_set = emitted.lock().unwrap();
                        if emitted_set.insert(display_url.clone()) {
                            // New URL - emit ItemDiscovered
                            let _ = sse.broadcast_to_job(
                                jid,
                                Event::new(EventType::ItemDiscovered {
                                    job_id: jid,
                                    item_path: display_url.clone(),
                                    discovered_at: chrono::Utc::now(),
                                    parent_url: None,
                                    depth: 0, // We don't track depth per-page anymore
                                }),
                            );
                        }
                    }

                    // Emit ItemStarted when processing begins
                    let _ = sse.broadcast_to_job(
                        jid,
                        Event::new(EventType::ItemStarted {
                            job_id: jid,
                            item_path: display_url.clone(),
                            started_at: chrono::Utc::now(),
                        }),
                    );

                    if let Some(ref p) = proc {
                        // Use raw URL for content processing with two-layer storage
                        match p.process_html_with_page(&result.content, &raw_url, Some(result.fetch_time_ms)).await {
                            Ok(res) => PageResult {
                                url: display_url,
                                chunks: res.chunks_created,
                                embeddings: res.embeddings_generated,
                                error: None,
                            },
                            Err(e) => {
                                let error_msg = format!("Failed to process {}: {}", raw_url, e);
                                warn!("{}", error_msg);
                                PageResult {
                                    url: display_url,
                                    chunks: 0,
                                    embeddings: 0,
                                    error: Some(error_msg),
                                }
                            }
                        }
                    } else {
                        PageResult {
                            url: display_url,
                            chunks: 0,
                            embeddings: 0,
                            error: None,
                        }
                    }
                }
            })
            .buffer_unordered(concurrency); // Process up to 8 pages concurrently

        // Pin the stream for iteration
        futures::pin_mut!(processing_stream);

        // Collect results as they complete and send real-time updates
        while let Some(result) = processing_stream.next().await {
            let count = processed_count.fetch_add(1, Ordering::Relaxed) + 1;
            total_chunks += result.chunks;
            total_embeddings += result.embeddings;

            // Emit ItemProcessed or Error for this page
            if let Some(ref err) = result.error {
                errors.push(err.clone());
                let _ = sse_manager.broadcast_to_job(
                    job_id,
                    Event::new(EventType::Error {
                        job_id,
                        item_path: Some(result.url.clone()),
                        error_message: err.clone(),
                        recoverable: true,
                    }),
                );
                // Update or insert item for persistence (upsert)
                {
                    let mut items = ctx.items.write().await;
                    let item = items.entry(result.url.clone()).or_insert_with(|| {
                        JobItemRecord::new_page(job_id.to_string(), result.url.clone())
                    });
                    item.mark_error(err.clone());
                }
            } else {
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
                // Update or insert item for persistence (upsert)
                {
                    let mut items = ctx.items.write().await;
                    let item = items.entry(result.url.clone()).or_insert_with(|| {
                        JobItemRecord::new_page(job_id.to_string(), result.url.clone())
                    });
                    item.mark_completed(result.chunks as u32, result.embeddings as u32, 0);
                }
            }

            // Get discovered count from our tracking (this is the real-time total)
            // Discovered = pages that entered processing (emitted in map closure above)
            let discovered = emitted_urls.lock().unwrap().len();

            // Update tracker
            tracker.increment(1).await;
            tracker
                .update_metrics(|m| {
                    m.chunks_created = total_chunks;
                    m.items_discovered = discovered;
                })
                .await;

            // Send progress event - total is simply the discovered URLs count
            let progress = tracker.get_progress().await;
            let percentage = if discovered > 0 {
                let raw = (count as f32 / discovered as f32) * 100.0;
                // Cap at 99% during processing - final 100% sent after crawler completes
                raw.min(99.0)
            } else {
                0.0
            };

            let _ = sse_manager.broadcast_to_job(
                job_id,
                Event::new(EventType::Progress {
                    job_id,
                    processed: count,
                    total: discovered,
                    current_item: Some(result.url),
                    rate: progress.rate,
                    percentage,
                }),
            );
        }

        // Wait for crawler to finish (it may have more pages but we stopped processing)
        let final_count = processed_count.load(Ordering::Relaxed);
        match crawler_handle.await {
            Ok(Ok(stats)) => {
                info!(
                    "Crawl completed: {} pages crawled, {} processed, {} bytes, {} errors",
                    stats.pages_crawled, final_count, stats.bytes_downloaded, stats.errors
                );
            }
            Ok(Err(e)) => {
                // Crawler error - but we may have processed some pages
                warn!("Crawler error: {}", e);
                if final_count == 0 {
                    return Err(JobError::Processing(format!("Crawl failed: {}", e)));
                }
                errors.push(format!("Crawler error: {}", e));
            }
            Err(e) => {
                warn!("Crawler task panic: {}", e);
                errors.push(format!("Crawler task error: {}", e));
            }
        }

        // Send final progress event with accurate counts after crawler completes
        let final_discovered = emitted_urls.lock().unwrap().len();
        let final_processed = processed_count.load(Ordering::Relaxed);
        let final_percentage = if final_discovered > 0 {
            (final_processed as f32 / final_discovered as f32) * 100.0
        } else {
            100.0
        };

        let _ = sse_manager.broadcast_to_job(
            job_id,
            Event::new(EventType::Progress {
                job_id,
                processed: final_processed,
                total: final_discovered,
                current_item: None,
                rate: 0.0,
                percentage: final_percentage,
            }),
        );

        // Log completion status
        if final_processed != final_discovered {
            if final_processed >= effective_max_pages && effective_max_pages < usize::MAX {
                info!(
                    job_id = %job_id,
                    processed = final_processed,
                    discovered = final_discovered,
                    max_pages = effective_max_pages,
                    "Job completed: reached max_pages limit"
                );
            } else {
                warn!(
                    job_id = %job_id,
                    processed = final_processed,
                    discovered = final_discovered,
                    "Job completed with item count mismatch"
                );
            }
        } else {
            info!(
                job_id = %job_id,
                processed = final_processed,
                discovered = final_discovered,
                "Job completed: all discovered items processed"
            );
        }

        Ok(JobResult {
            items_processed: final_count,
            chunks_created: total_chunks,
            embeddings_generated: total_embeddings,
            errors,
            duration_ms: start.elapsed().as_millis() as u64,
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
        ctx: &JobExecutionContext,
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
                        // Use two-layer storage for file uploads
                        match proc.process_file_with_page(&path, &name).await {
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

        // Add all files as items upfront for persistence tracking
        {
            let mut items = ctx.items.write().await;
            for name in file_names {
                items.insert(name.clone(), JobItemRecord::new_file(job_id.to_string(), name.clone()));
            }
        }
        ctx.items_discovered.store(file_names.len(), Ordering::Relaxed);

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
                // Update item status for persistence
                {
                    let mut items = ctx.items.write().await;
                    if let Some(item) = items.get_mut(&result.name) {
                        item.mark_error(err.clone());
                    }
                }
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
                // Update item status for persistence
                {
                    let mut items = ctx.items.write().await;
                    if let Some(item) = items.get_mut(&result.name) {
                        item.mark_completed(result.chunks as u32, result.embeddings as u32, 0);
                    }
                }
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
        _ctx: &JobExecutionContext,
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