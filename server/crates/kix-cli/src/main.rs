//! EIP CLI - Command-line interface for the EIP Knowledge System.

// Use jemalloc as the global allocator for better performance
#[cfg(not(target_env = "msvc"))]
use jemallocator::Jemalloc;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

use anyhow::{Context, Result};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Json;
use clap::{Parser, Subcommand};
use glob::glob;
use indicatif::{ProgressBar, ProgressStyle};
use serde_json::json;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

use kix_api::{create_router, create_indexing_router, AppState, IndexingState};
use kix_crawler::file_handler::FileHandler;
use kix_embeddings::{DocumentChunker, EmbeddingGenerator, ensure_setup, is_setup, model_cache_dir};
use kix_jobs::{JobExecutor, ExecutorConfig, JobQueue, QueueConfig};
use kix_mcp::KixMcpServer;
use kix_parser::{Entry, EntryType, PdfParser, SourceType};
use kix_crawler::ContentExtractor;
use url::Url;
use kix_sse::{ConnectionManager, spawn_cleanup_task};
use kix_store::search::SearchFilters;
use kix_store::{JobStore, KixStore};
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::StreamableHttpService;

#[derive(Parser)]
#[command(name = "eip")]
#[command(about = "Enterprise Integration Patterns Knowledge System", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Path to LanceDB database
    #[arg(long, default_value = "./data/lancedb")]
    db_path: PathBuf,
}

#[derive(Subcommand)]
enum Commands {
    /// Index all content from the source directory
    Index {
        /// Path to content directory
        #[arg(long)]
        content_path: PathBuf,

        /// Rebuild index from scratch
        #[arg(long)]
        rebuild: bool,
    },

    /// Start the MCP server (stdio transport)
    Serve,

    /// Start the MCP server over HTTP (streamable HTTP transport at /mcp)
    ServeHttp {
        /// Port to listen on
        #[arg(short, long, default_value = "3002")]
        port: u16,

        /// Host to bind to (defaults to localhost per MCP spec security requirements)
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
    },

    /// Start the REST API for the dashboard
    Api {
        /// Port to listen on
        #[arg(short, long, default_value = "3001")]
        port: u16,
    },

    /// Start both API and MCP servers in a single process (shared store)
    Run {
        /// API server port
        #[arg(long, default_value = "3001")]
        api_port: u16,

        /// MCP server port
        #[arg(long, default_value = "3002")]
        mcp_port: u16,

        /// Host to bind to
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
    },

    /// Test search from command line
    Search {
        /// Search query
        query: String,

        /// Number of results
        #[arg(short, long, default_value = "5")]
        limit: usize,

        /// Search type: semantic, text, or hybrid
        #[arg(short, long, default_value = "hybrid")]
        search_type: String,

        /// Filter by pattern type (messaging, conversation)
        #[arg(long)]
        pattern_type: Option<String>,
    },

    /// Show indexing statistics
    Stats,

    /// Create search indexes on existing data
    CreateIndexes,

    /// Download and setup embedding models
    Setup {
        /// Model name to download (default: bge-base-en-v1.5)
        #[arg(short, long)]
        model: Option<String>,

        /// Force re-download even if model exists
        #[arg(short, long)]
        force: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("eip=info".parse().unwrap()),
        )
        .init();

    let cli = Cli::parse();
    let db_path_str = cli.db_path.to_string_lossy().to_string();

    match cli.command {
        Commands::Index {
            content_path,
            rebuild,
        } => {
            run_index(&db_path_str, &content_path, rebuild).await?;
        }
        Commands::Serve => {
            run_serve(&db_path_str).await?;
        }
        Commands::ServeHttp { port, host } => {
            run_serve_http(&db_path_str, &host, port).await?;
        }
        Commands::Api { port } => {
            run_api(&db_path_str, port).await?;
        }
        Commands::Run { api_port, mcp_port, host } => {
            run_unified(&db_path_str, &host, api_port, mcp_port).await?;
        }
        Commands::Search {
            query,
            limit,
            search_type,
            pattern_type,
        } => {
            run_search(&db_path_str, &query, limit, &search_type, pattern_type).await?;
        }
        Commands::Stats => {
            run_stats(&db_path_str).await?;
        }
        Commands::CreateIndexes => {
            run_create_indexes(&db_path_str).await?;
        }
        Commands::Setup { model, force } => {
            run_setup(model, force).await?;
        }
    }

    Ok(())
}

/// Index content from the source directory.
async fn run_index(db_path: &str, content_path: &PathBuf, rebuild: bool) -> Result<()> {
    info!("Starting indexing...");
    println!("Indexing content from: {:?}", content_path);
    println!("Database path: {}", db_path);
    println!("Rebuild: {}", rebuild);

    // Auto-setup: download models if not present
    auto_setup()?;

    // Initialize embedder
    println!("\nInitializing embedding model...");
    let mut embedder = EmbeddingGenerator::new().context("Failed to initialize embedding model")?;
    println!("Embedding model initialized.");

    // Initialize store
    println!("\nInitializing database...");
    let mut store = KixStore::new(db_path)
        .await
        .context("Failed to open database")?;

    if rebuild {
        println!("Rebuilding index from scratch...");
        store
            .clear_tables()
            .await
            .context("Failed to clear database")?;
    }

    // Initialize tables
    store
        .init_tables()
        .await
        .context("Failed to initialize tables")?;

    // Find HTML files
    let html_pattern = content_path
        .join("**/patterns/**/*.html")
        .to_string_lossy()
        .to_string();
    let html_files: Vec<PathBuf> = glob(&html_pattern)
        .context("Failed to glob HTML files")?
        .filter_map(|e| e.ok())
        .filter(|p| !p.to_string_lossy().contains("toc.html"))
        .collect();

    // Find PDF files
    let pdf_pattern = content_path
        .join("**/docs/*.pdf")
        .to_string_lossy()
        .to_string();
    let pdf_files: Vec<PathBuf> = glob(&pdf_pattern)
        .context("Failed to glob PDF files")?
        .filter_map(|e| e.ok())
        .collect();

    println!(
        "\nFound {} HTML files and {} PDF files",
        html_files.len(),
        pdf_files.len()
    );

    let chunker = DocumentChunker::with_defaults();

    // Process HTML files
    if !html_files.is_empty() {
        println!("\nProcessing HTML files...");
        let pb = ProgressBar::new(html_files.len() as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})",
                )
                .unwrap()
                .progress_chars("#>-"),
        );

        let content_extractor = ContentExtractor::default();

        for file_path in &html_files {
            pb.set_message(
                file_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
            );

            // Read file content
            let content = match std::fs::read_to_string(file_path) {
                Ok(c) => c,
                Err(e) => {
                    error!("Failed to read {:?}: {}", file_path, e);
                    pb.inc(1);
                    continue;
                }
            };

            let file_path_str = file_path.to_string_lossy().to_string();

            // Use ContentExtractor for consistent HTML processing
            let url = Url::parse(&format!("file://{}", file_path_str))
                .unwrap_or_else(|_| Url::parse("file:///unknown").unwrap());
            let extracted = content_extractor.extract(&content, &url);

            // Create Entry from extracted content
            let document = create_entry_from_extracted(&extracted, &file_path_str);

            // Generate chunks
            let chunks = chunker.chunk(&document);

            if !chunks.is_empty() {
                // Generate embeddings for chunks
                let chunk_texts: Vec<&str> = chunks.iter().map(|c| c.text.as_str()).collect();
                match embedder.embed_texts(&chunk_texts) {
                    Ok(embeddings) => {
                        if let Err(e) = store.insert_chunks(&chunks, &embeddings).await {
                            error!("Failed to store chunks: {}", e);
                        }
                    }
                    Err(e) => {
                        error!("Failed to embed chunks: {}", e);
                    }
                }
            }

            // Store document
            if let Err(e) = store.insert_documents(&[document]).await {
                error!("Failed to store document: {}", e);
            }

            pb.inc(1);
        }

        pb.finish_with_message("HTML files processed");
    }

    // Process PDF files
    if !pdf_files.is_empty() {
        println!("\nProcessing PDF files...");
        let pb = ProgressBar::new(pdf_files.len() as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})",
                )
                .unwrap()
                .progress_chars("#>-"),
        );

        let pdf_parser = PdfParser::new();

        for file_path in &pdf_files {
            pb.set_message(
                file_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
            );

            let file_path_str = file_path.to_string_lossy().to_string();

            match pdf_parser.parse(&file_path_str) {
                Ok(document) => {
                    // Generate chunks
                    let chunks = chunker.chunk(&document);

                    if !chunks.is_empty() {
                        // Generate embeddings for chunks
                        let chunk_texts: Vec<&str> = chunks.iter().map(|c| c.text.as_str()).collect();
                        match embedder.embed_texts(&chunk_texts) {
                            Ok(embeddings) => {
                                if let Err(e) = store.insert_chunks(&chunks, &embeddings).await {
                                    error!("Failed to store chunks: {}", e);
                                }
                            }
                            Err(e) => {
                                error!("Failed to embed chunks: {}", e);
                            }
                        }
                    }

                    // Store document
                    if let Err(e) = store.insert_documents(&[document]).await {
                        error!("Failed to store document: {}", e);
                    }
                }
                Err(e) => {
                    error!("Failed to parse {:?}: {}", file_path, e);
                }
            }

            pb.inc(1);
        }

        pb.finish_with_message("PDF files processed");
    }

    // Create indexes
    println!("\nCreating search indexes...");
    store
        .create_indexes()
        .await
        .context("Failed to create indexes")?;

    println!("\nIndexing complete!");

    Ok(())
}

/// Start the MCP server with stdio transport.
async fn run_serve(db_path: &str) -> Result<()> {
    info!("Starting MCP server...");

    // Auto-setup: download models if not present
    auto_setup()?;

    // Initialize embedder and store
    let embedder = EmbeddingGenerator::new().context("Failed to initialize embedding model")?;
    let mut store = KixStore::new(db_path)
        .await
        .context("Failed to open database")?;
    store
        .init_tables()
        .await
        .context("Failed to initialize tables")?;

    // Create job queue for async indexing operations
    let job_queue = Arc::new(JobQueue::new(QueueConfig::default()));

    // Create SSE manager for progress tracking (even though stdio won't use SSE directly)
    let sse_manager = Arc::new(ConnectionManager::new(Default::default()));

    // Create and start the job executor to process queued jobs
    let executor_config = ExecutorConfig {
        db_path: db_path.to_string(),
        jobs_db_path: None,
        on_job_complete: None,
        ..Default::default()
    };

    let mut executor = JobExecutor::new(job_queue.clone(), sse_manager, executor_config)
        .await
        .context("Failed to create job executor")?;
    executor.start();
    info!("Job executor started for async indexing");

    // Create MCP server
    let server = KixMcpServer::new(store, embedder, job_queue);

    // Create stdio transport
    let transport = rmcp::transport::io::stdio();

    // Run the server
    info!("MCP server listening on stdio...");
    let running = rmcp::serve_server(server, transport).await?;

    // Wait for the server to finish
    running.waiting().await?;

    Ok(())
}

/// Start the MCP server with HTTP streaming transport at /mcp.
async fn run_serve_http(db_path: &str, host: &str, port: u16) -> Result<()> {
    info!("Starting MCP HTTP server...");

    // Auto-setup: download models if not present
    auto_setup()?;

    // Pre-initialize the embedder and store for MCP server
    let embedder = EmbeddingGenerator::new().context("Failed to initialize embedding model")?;
    let mut store = KixStore::new(db_path)
        .await
        .context("Failed to open database")?;
    store
        .init_tables()
        .await
        .context("Failed to initialize tables")?;

    // Create job queue for async indexing operations
    let job_queue = Arc::new(JobQueue::new(QueueConfig::default()));

    // Create SSE manager for progress tracking
    let sse_manager = Arc::new(ConnectionManager::new(Default::default()));

    // Create and start the job executor
    // Note: JobExecutor creates its own store instance here since KixMcpServer
    // takes ownership of its store. Both use refresh_tables() for data consistency.
    let executor_config = ExecutorConfig {
        db_path: db_path.to_string(),
        jobs_db_path: None,
        on_job_complete: None,
        ..Default::default()
    };

    let mut executor = JobExecutor::new(job_queue.clone(), sse_manager, executor_config)
        .await
        .context("Failed to create job executor")?;
    executor.start();
    info!("Job executor started for async indexing");

    // Create a template server that will be cloned for each session
    // KixMcpServer implements Clone since it uses Arc internally
    let template_server = KixMcpServer::new(store, embedder, job_queue);

    // Create the streamable HTTP service with a factory function
    let service = StreamableHttpService::new(
        move || {
            // Clone the server for each new session
            // This works because KixMcpServer uses Arc<Mutex<>> internally
            let server = template_server.clone();
            Ok(server)
        },
        Arc::new(LocalSessionManager::default()),
        Default::default(),
    );

    // Create Axum router with the MCP service at /mcp
    // Add OAuth stub routes for clients that expect OAuth (returns "no auth required")
    let app = axum::Router::new()
        .route("/.well-known/oauth-authorization-server", get(oauth_metadata_handler))
        .route("/.well-known/oauth-protected-resource", get(oauth_resource_handler))
        .route("/oauth/register", axum::routing::post(oauth_register_handler))
        .route("/oauth/authorize", get(oauth_authorize_handler))
        .route("/oauth/token", axum::routing::post(oauth_token_handler))
        .nest_service("/mcp", service);

    // Start HTTP server
    let addr: SocketAddr = format!("{}:{}", host, port).parse()?;
    println!("MCP HTTP server listening at http://{}/mcp", addr);
    info!("MCP HTTP server started at http://{}/mcp", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c()
                .await
                .expect("Failed to listen for ctrl-c");
            info!("Shutting down MCP HTTP server...");
        })
        .await?;

    Ok(())
}

/// Start the REST API server.
async fn run_api(db_path: &str, port: u16) -> Result<()> {
    info!("Starting REST API server...");

    // Auto-setup: download models if not present
    auto_setup()?;

    // Initialize embedder and store
    let embedder = EmbeddingGenerator::new().context("Failed to initialize embedding model")?;
    let mut store = KixStore::new(db_path)
        .await
        .context("Failed to open database")?;
    store
        .init_tables()
        .await
        .context("Failed to initialize tables")?;

    // Create shared store - single instance shared between API and JobExecutor
    let shared_store = Arc::new(RwLock::new(store));
    info!("Created shared KixStore instance for API and JobExecutor");

    // Create app state using the shared store
    let state = AppState::new_with_store(shared_store.clone(), embedder);

    // Create indexing components
    let job_queue = Arc::new(JobQueue::new(QueueConfig::default()));
    let sse_manager = Arc::new(ConnectionManager::new(Default::default()));

    // Start SSE cleanup background task to prevent stale connections
    spawn_cleanup_task(sse_manager.clone());

    let file_handler = Arc::new(FileHandler::with_defaults());

    // Initialize file handler upload directory
    file_handler.init().await?;

    // Initialize job history store
    let jobs_db_path = PathBuf::from(db_path).join("jobs.lance");
    let job_store = match JobStore::new(jobs_db_path.to_string_lossy().as_ref()).await {
        Ok(mut store) => {
            if let Err(e) = store.init_tables().await {
                error!("Failed to initialize job history tables: {}", e);
                None
            } else {
                info!("Job history store initialized at {}", jobs_db_path.display());
                Some(Arc::new(store))
            }
        }
        Err(e) => {
            error!("Failed to create job history store: {}", e);
            None
        }
    };

    // Create indexing state
    let mut indexing_state = IndexingState::new(
        state.clone(),
        job_queue.clone(),
        sse_manager.clone(),
        file_handler,
    );

    // Attach job store if initialized
    if let Some(store) = job_store.clone() {
        indexing_state = indexing_state.with_job_store(store);
    }

    // Create and start the job executor with shared store
    let executor_config = ExecutorConfig {
        db_path: db_path.to_string(),
        jobs_db_path: job_store.as_ref().map(|_| jobs_db_path.to_string_lossy().to_string()),
        on_job_complete: None,
        shared_store: Some(shared_store),  // Pass shared store to executor
        ..Default::default()
    };

    let mut executor = JobExecutor::new(job_queue.clone(), sse_manager.clone(), executor_config)
        .await
        .context("Failed to create job executor")?;
    executor.start();

    // Create routers
    let main_router = create_router(state);
    let indexing_router = create_indexing_router(indexing_state);

    // Merge routers
    let app = main_router.merge(indexing_router);

    // Start server
    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse()?;
    println!("REST API server listening on http://{}", addr);
    println!("Indexing API available at http://{}/api/indexing/*", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;

    // Use into_make_service_with_connect_info to provide SocketAddr to handlers
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to listen for ctrl-c");
        info!("Shutting down API server...");
        executor.shutdown().await;
    })
    .await?;

    Ok(())
}

/// Start both API and MCP servers in a single process with a shared store.
/// This is the recommended mode for single-node deployments as it ensures
/// data indexed via MCP is immediately visible to API searches.
async fn run_unified(db_path: &str, host: &str, api_port: u16, mcp_port: u16) -> Result<()> {
    info!("Starting unified server (API + MCP with shared store)...");

    // Auto-setup: download models if not present
    auto_setup()?;

    // Initialize store - SINGLE SHARED INSTANCE
    let mut store = KixStore::new(db_path)
        .await
        .context("Failed to open database")?;
    store
        .init_tables()
        .await
        .context("Failed to initialize tables")?;

    // Create the single shared store
    let shared_store = Arc::new(RwLock::new(store));
    info!("Created single shared KixStore instance for API and MCP servers");

    // Create single shared embedder for both API and MCP
    let embedder = EmbeddingGenerator::new().context("Failed to initialize embedding model")?;
    let shared_embedder = Arc::new(RwLock::new(embedder));
    info!("Created single shared EmbeddingGenerator for API and MCP servers");

    // Create shared components
    let job_queue = Arc::new(JobQueue::new(QueueConfig::default()));
    let sse_manager = Arc::new(ConnectionManager::new(Default::default()));

    // Start SSE cleanup background task
    spawn_cleanup_task(sse_manager.clone());

    let file_handler = Arc::new(FileHandler::with_defaults());
    file_handler.init().await?;

    // Initialize job history store
    let jobs_db_path = PathBuf::from(db_path).join("jobs.lance");
    let job_store = match JobStore::new(jobs_db_path.to_string_lossy().as_ref()).await {
        Ok(mut store) => {
            if let Err(e) = store.init_tables().await {
                error!("Failed to initialize job history tables: {}", e);
                None
            } else {
                info!("Job history store initialized at {}", jobs_db_path.display());
                Some(Arc::new(store))
            }
        }
        Err(e) => {
            error!("Failed to create job history store: {}", e);
            None
        }
    };

    // Create API state using the shared store and shared embedder
    let api_state = AppState::with_shared(shared_store.clone(), shared_embedder.clone());

    // Create indexing state for API
    let mut indexing_state = IndexingState::new(
        api_state.clone(),
        job_queue.clone(),
        sse_manager.clone(),
        file_handler,
    );
    if let Some(store) = job_store.clone() {
        indexing_state = indexing_state.with_job_store(store);
    }

    // Create and start the job executor with shared store
    let executor_config = ExecutorConfig {
        db_path: db_path.to_string(),
        jobs_db_path: job_store.as_ref().map(|_| jobs_db_path.to_string_lossy().to_string()),
        on_job_complete: None,
        shared_store: Some(shared_store.clone()), // Pass shared store to executor
        ..Default::default()
    };

    let mut executor = JobExecutor::new(job_queue.clone(), sse_manager.clone(), executor_config)
        .await
        .context("Failed to create job executor")?;
    executor.start();
    info!("Job executor started with shared store");

    // Create API router
    let main_router = create_router(api_state);
    let indexing_router = create_indexing_router(indexing_state);
    let api_app = main_router.merge(indexing_router);

    // Create MCP server using the SAME shared store and shared embedder
    let mcp_server = KixMcpServer::with_shared(shared_store.clone(), shared_embedder, job_queue);

    // Create the MCP streamable HTTP service
    let mcp_service = StreamableHttpService::new(
        move || {
            let server = mcp_server.clone();
            Ok(server)
        },
        Arc::new(LocalSessionManager::default()),
        Default::default(),
    );

    // Create MCP router with OAuth stubs
    let mcp_app = axum::Router::new()
        .route("/.well-known/oauth-authorization-server", get(oauth_metadata_handler))
        .route("/.well-known/oauth-protected-resource", get(oauth_resource_handler))
        .route("/oauth/register", axum::routing::post(oauth_register_handler))
        .route("/oauth/authorize", get(oauth_authorize_handler))
        .route("/oauth/token", axum::routing::post(oauth_token_handler))
        .nest_service("/mcp", mcp_service);

    // Bind to addresses
    let api_addr: SocketAddr = format!("{}:{}", host, api_port).parse()?;
    let mcp_addr: SocketAddr = format!("{}:{}", host, mcp_port).parse()?;

    let api_listener = tokio::net::TcpListener::bind(api_addr).await?;
    let mcp_listener = tokio::net::TcpListener::bind(mcp_addr).await?;

    println!("=== Unified Server (Shared Store) ===");
    println!("  REST API:  http://{}", api_addr);
    println!("  MCP:       http://{}/mcp", mcp_addr);
    println!("");
    println!("Both servers share a single KixStore instance.");
    println!("Data indexed via MCP is immediately searchable via API.");
    info!("Unified server started - API on {}, MCP on {}", api_addr, mcp_addr);

    // Create shutdown signal
    let shutdown = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to listen for ctrl-c");
        info!("Shutting down unified server...");
    };

    // Run both servers concurrently
    tokio::select! {
        result = axum::serve(api_listener, api_app.into_make_service_with_connect_info::<SocketAddr>()) => {
            if let Err(e) = result {
                error!("API server error: {}", e);
            }
        }
        result = axum::serve(mcp_listener, mcp_app) => {
            if let Err(e) = result {
                error!("MCP server error: {}", e);
            }
        }
        _ = shutdown => {
            info!("Shutdown signal received");
        }
    }

    executor.shutdown().await;
    info!("Unified server stopped");

    Ok(())
}

/// Test search from command line.
async fn run_search(
    db_path: &str,
    query: &str,
    limit: usize,
    search_type: &str,
    pattern_type: Option<String>,
) -> Result<()> {
    info!("Running search...");
    println!("Query: {}", query);
    println!("Search type: {}", search_type);

    // Auto-setup: download models if not present
    auto_setup()?;

    // Initialize embedder and store
    let mut embedder = EmbeddingGenerator::new().context("Failed to initialize embedding model")?;
    let mut store = KixStore::new(db_path)
        .await
        .context("Failed to open database")?;
    store
        .init_tables()
        .await
        .context("Failed to initialize tables")?;

    let filters = SearchFilters {
        entry_type: pattern_type,
        chunk_type: None,
        tag: None,
        source_domain: None,
    };

    // Perform search based on type
    let results = match search_type {
        "semantic" | "vector" => {
            let embedding = embedder.embed_query(query)?;
            store.vector_search(&embedding, limit, &filters).await?
        }
        "text" | "fts" => store.text_search(query, limit, &filters).await?,
        _ => {
            // Default to hybrid
            let embedding = embedder.embed_query(query)?;
            store
                .hybrid_search(query, &embedding, limit, &filters)
                .await?
        }
    };

    println!("\n=== Search Results ===\n");

    if results.is_empty() {
        println!("No results found.");
    } else {
        for (i, result) in results.iter().enumerate() {
            println!(
                "{}. {} (Score: {:.4})",
                i + 1,
                result.entry_title,
                result.score
            );
            println!(
                "   Type: {} | Tags: {}",
                result.entry_type,
                result.tags.join(", ")
            );
            println!("   {}", truncate(&result.text, 200));
            println!();
        }
    }

    Ok(())
}

/// Show indexing statistics.
async fn run_stats(db_path: &str) -> Result<()> {
    info!("Getting statistics...");

    let mut store = KixStore::new(db_path)
        .await
        .context("Failed to open database")?;
    store
        .init_tables()
        .await
        .context("Failed to initialize tables")?;

    // Get all patterns
    let patterns = store.list_all_patterns().await?;

    let document = patterns
        .iter()
        .filter(|p| p.entry_type == "document")
        .count();
    let article = patterns
        .iter()
        .filter(|p| p.entry_type == "article")
        .count();
    let pdf = patterns.iter().filter(|p| p.entry_type == "pdf").count();

    println!("\n=== EIP Knowledge System Statistics ===\n");
    println!("Total entries indexed: {}", patterns.len());
    println!("  - Documents: {}", document);
    println!("  - Articles: {}", article);
    println!("  - PDF documents: {}", pdf);

    // Count tags
    let mut tags_count: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for pattern in &patterns {
        for tag in &pattern.tags {
            *tags_count.entry(tag.clone()).or_insert(0) += 1;
        }
    }

    println!("\nTags:");
    let mut sorted_tags: Vec<_> = tags_count.into_iter().collect();
    sorted_tags.sort_by(|a, b| b.1.cmp(&a.1));
    for (tag, count) in sorted_tags.iter().take(10) {
        println!("  - {}: {} entries", tag, count);
    }

    println!("\nDatabase path: {}", db_path);

    Ok(())
}

/// Create search indexes on existing data.
async fn run_create_indexes(db_path: &str) -> Result<()> {
    info!("Creating search indexes...");

    let mut store = KixStore::new(db_path)
        .await
        .context("Failed to open database")?;
    store
        .init_tables()
        .await
        .context("Failed to initialize tables")?;

    println!("Creating search indexes...");
    store
        .create_indexes()
        .await
        .context("Failed to create indexes")?;

    println!("Search indexes created successfully!");

    Ok(())
}

/// Download and setup embedding models.
async fn run_setup(model: Option<String>, force: bool) -> Result<()> {
    println!("\n=== Embedding Model Setup ===\n");

    // Set model via environment variable if specified
    if let Some(ref model_name) = model {
        std::env::set_var("KIX_EMBEDDING_MODEL", model_name);
        println!("Model: {}", model_name);
    } else {
        let default_model = std::env::var("KIX_EMBEDDING_MODEL")
            .unwrap_or_else(|_| "bge-base-en-v1.5".to_string());
        println!("Model: {} (default)", default_model);
    }

    let cache_dir = model_cache_dir();
    println!("Cache directory: {:?}", cache_dir);

    // Check if already set up
    if !force && is_setup() {
        println!("\nModel is already downloaded and ready!");
        println!("Use --force to re-download.");
        return Ok(());
    }

    if force {
        println!("\nForce re-download requested...");
        // Remove existing files
        let model_name = model.unwrap_or_else(|| {
            std::env::var("KIX_EMBEDDING_MODEL")
                .unwrap_or_else(|_| "bge-base-en-v1.5".to_string())
        });
        let safe_name = model_name.replace("-", "_").replace("/", "_");
        let model_path = cache_dir.join(format!("{}.onnx", safe_name));
        let tokenizer_path = cache_dir.join(format!("{}_tokenizer.json", safe_name));
        let _ = std::fs::remove_file(&model_path);
        let _ = std::fs::remove_file(&tokenizer_path);
    }

    println!("\nDownloading model...");
    let setup_info = ensure_setup().context("Failed to setup embedding model")?;

    if setup_info.downloaded {
        println!("\nModel downloaded successfully!");
    } else {
        println!("\nModel was already present.");
    }

    println!("\nSetup complete!");
    println!("  Model: {}", setup_info.model_name);
    println!("  Cache: {:?}", setup_info.cache_dir);

    Ok(())
}

/// Ensure models are set up before starting the server.
fn auto_setup() -> Result<()> {
    if !is_setup() {
        println!("First-time setup: downloading embedding model...");
        match ensure_setup() {
            Ok(info) => {
                if info.downloaded {
                    println!("Model downloaded: {}", info.model_name);
                }
            }
            Err(e) => {
                error!("Warning: Failed to auto-download model: {}", e);
                error!("You may need to run 'eip setup' manually.");
            }
        }
    }
    Ok(())
}

/// Truncate text to a maximum length.
fn truncate(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        text.to_string()
    } else {
        format!("{}...", &text[..max_len])
    }
}

/// Create an Entry from ContentExtractor output.
///
/// This helper function converts extracted content to an Entry
/// for consistent indexing.
fn create_entry_from_extracted(
    extracted: &kix_crawler::ExtractedContent,
    source_path: &str,
) -> Entry {
    // Generate slug/ID from path
    let slug = source_path.to_string();
    let id = Entry::generate_id_from_path(source_path);

    // Use extracted description or derive from markdown
    let description = extracted
        .description
        .clone()
        .unwrap_or_else(|| extracted.markdown.chars().take(300).collect());

    // Determine entry type from path
    let entry_type = if source_path.contains("/blog/") || source_path.contains("/article/") {
        EntryType::Article
    } else if source_path.contains("/docs/") || source_path.contains("/documentation/") {
        EntryType::Document
    } else {
        EntryType::Document
    };

    Entry::with_id(
        id,
        extracted.title.clone(),
        source_path.to_string(),
        extracted.content_hash.clone(),
    )
    .with_description(description)
    .with_content(extracted.markdown.clone())
    .with_tags(vec![])
    .with_entry_type(entry_type)
    .with_source_type(SourceType::Html)
    .with_slug(slug)
}

// =============================================================================
// OAuth Stub Handlers - Allow MCP clients to connect without real authentication
// =============================================================================

/// Helper to get the effective host from X-Forwarded-Host or Host header
fn get_effective_host(headers: &axum::http::HeaderMap, host: &str) -> String {
    // Check for X-Forwarded-Host first (set by reverse proxies)
    if let Some(forwarded_host) = headers.get("x-forwarded-host") {
        if let Ok(fh) = forwarded_host.to_str() {
            let proto = headers
                .get("x-forwarded-proto")
                .and_then(|p| p.to_str().ok())
                .unwrap_or("http");
            return format!("{}://{}", proto, fh);
        }
    }
    format!("http://{}", host)
}

/// OAuth Authorization Server Metadata (RFC 8414)
/// Returns metadata indicating this server supports OAuth but doesn't require it
async fn oauth_metadata_handler(
    headers: axum::http::HeaderMap,
    axum::extract::Host(host): axum::extract::Host,
) -> impl IntoResponse {
    let issuer = get_effective_host(&headers, &host);
    Json(json!({
        "issuer": issuer,
        "authorization_endpoint": format!("{}/oauth/authorize", issuer),
        "token_endpoint": format!("{}/oauth/token", issuer),
        "registration_endpoint": format!("{}/oauth/register", issuer),
        "response_types_supported": ["code"],
        "grant_types_supported": ["authorization_code", "refresh_token"],
        "code_challenge_methods_supported": ["S256"],
        "token_endpoint_auth_methods_supported": ["none"],
        "service_documentation": "https://github.com/anthropics/claude-code"
    }))
}

/// OAuth Protected Resource Metadata
async fn oauth_resource_handler(
    headers: axum::http::HeaderMap,
    axum::extract::Host(host): axum::extract::Host,
) -> impl IntoResponse {
    let issuer = get_effective_host(&headers, &host);
    Json(json!({
        "resource": format!("{}/mcp", issuer),
        "authorization_servers": [issuer],
        "bearer_methods_supported": ["header"]
    }))
}

/// OAuth Dynamic Client Registration (RFC 7591)
/// Returns a client_id for any registration request
async fn oauth_register_handler(
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    let client_name = payload
        .get("client_name")
        .and_then(|v| v.as_str())
        .unwrap_or("mcp-client");

    // Generate a simple client_id based on the client name
    let client_id = format!("kix-{}-{}", client_name, uuid::Uuid::new_v4().simple());

    Json(json!({
        "client_id": client_id,
        "client_name": client_name,
        "redirect_uris": payload.get("redirect_uris").cloned().unwrap_or(json!([])),
        "grant_types": ["authorization_code", "refresh_token"],
        "response_types": ["code"],
        "token_endpoint_auth_method": "none"
    }))
}

/// OAuth Authorization Endpoint
/// Redirects back to the client with an authorization code
async fn oauth_authorize_handler(
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let redirect_uri = params.get("redirect_uri").cloned().unwrap_or_default();
    let state = params.get("state").cloned().unwrap_or_default();

    // Generate a dummy authorization code
    let code = format!("kix-code-{}", uuid::Uuid::new_v4().simple());

    // Build redirect URL with code
    let redirect_url = if redirect_uri.contains('?') {
        format!("{}&code={}&state={}", redirect_uri, code, state)
    } else {
        format!("{}?code={}&state={}", redirect_uri, code, state)
    };

    axum::response::Redirect::temporary(&redirect_url)
}

/// OAuth Token Endpoint
/// Returns an access token for any valid-looking request
async fn oauth_token_handler(
    axum::extract::Form(params): axum::extract::Form<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    // Generate a dummy access token
    let access_token = format!("kix-token-{}", uuid::Uuid::new_v4().simple());
    let refresh_token = format!("kix-refresh-{}", uuid::Uuid::new_v4().simple());

    Json(json!({
        "access_token": access_token,
        "token_type": "Bearer",
        "expires_in": 3600,
        "refresh_token": refresh_token,
        "scope": params.get("scope").cloned().unwrap_or_else(|| "mcp".to_string())
    }))
}
