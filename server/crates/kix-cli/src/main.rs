//! EIP CLI - Command-line interface for the EIP Knowledge System.

// Use jemalloc as the global allocator for better performance
#[cfg(not(target_env = "msvc"))]
use jemallocator::Jemalloc;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

use anyhow::{Context, Result};
use axum::routing::get;
use clap::{Parser, Subcommand};
use glob::glob;
use indicatif::{ProgressBar, ProgressStyle};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

use kix_api::{create_router, create_indexing_router, create_project_router, create_explorer_router, AppState, IndexingState, ProjectState, ExplorerState};
use kix_crawler::file_handler::FileHandler;
use kix_embeddings::{DocumentChunker, OllamaEmbedder, EmbeddingConfig};
use kix_jobs::{JobExecutor, ExecutorConfig, JobQueue, QueueConfig};
use kix_mcp::KixMcpServer;
use kix_parser::{Entry, EntryType, PdfParser, SourceType};
use kix_crawler::ContentExtractor;
use url::Url;
use kix_sse::{ConnectionManager, spawn_cleanup_task};
use kix_store::search::SearchFilters;
use kix_store::{JobStore, KixStore, ProjectStore};
use kix_projects::create_event_bus;
use kix_auth::{AuthStore, AuthState, handlers as auth_handlers};
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::StreamableHttpService;

#[derive(Parser)]
#[command(name = "eip")]
#[command(about = "Enterprise Integration Patterns Knowledge System", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Path to data directory
    #[arg(long, default_value = "./data")]
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

    /// Start the MCP server (stdio transport for IDE integration)
    Serve,

    /// Start the unified server (API + MCP with shared stores)
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

        /// Enable MCP stdio transport (for IDE integration)
        #[arg(long, default_value = "false")]
        stdio: bool,
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

    /// Migrate from old data format to SQLite architecture
    Migrate {
        /// Source data directory (old format)
        #[arg(long)]
        from: PathBuf,

        /// Destination data directory for SQLite store
        #[arg(long)]
        to: PathBuf,

        /// Skip confirmation prompt
        #[arg(long)]
        yes: bool,
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
        Commands::Run { api_port, mcp_port, host, stdio } => {
            run(&db_path_str, &host, api_port, mcp_port, stdio).await?;
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
        Commands::Migrate { from, to, yes } => {
            run_migrate(&from, &to, yes).await?;
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

    // Initialize embedder (connects to Ollama server)
    println!("\nInitializing Ollama embedder...");
    let config = EmbeddingConfig::default();
    let embedder = OllamaEmbedder::new(config).context("Failed to initialize Ollama embedder")?;
    embedder.health_check().await.context("Ollama server not reachable")?;
    println!("Ollama embedder initialized.");

    // Initialize store
    println!("\nInitializing database...");
    let mut store = KixStore::new(Path::new(db_path))
        .await
        .context("Failed to open database")?;

    if rebuild {
        println!("Rebuilding index from scratch...");
        store
            .clear_all()
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
                // Generate embeddings for chunks (async)
                let chunk_texts: Vec<String> = chunks.iter().map(|c| c.text.clone()).collect();
                match embedder.embed_batch(&chunk_texts).await {
                    Ok(embeddings) => {
                        if let Err(e) = store.insert_chunks(&chunks, &embeddings) {
                            error!("Failed to store chunks: {}", e);
                        }
                    }
                    Err(e) => {
                        error!("Failed to embed chunks: {}", e);
                    }
                }
            }

            // Store document
            if let Err(e) = store.insert_documents_from_entries(&[document]).await {
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
                        // Generate embeddings for chunks (async)
                        let chunk_texts: Vec<String> = chunks.iter().map(|c| c.text.clone()).collect();
                        match embedder.embed_batch(&chunk_texts).await {
                            Ok(embeddings) => {
                                if let Err(e) = store.insert_chunks(&chunks, &embeddings) {
                                    error!("Failed to store chunks: {}", e);
                                }
                            }
                            Err(e) => {
                                error!("Failed to embed chunks: {}", e);
                            }
                        }
                    }

                    // Store document
                    if let Err(e) = store.insert_documents_from_entries(&[document]).await {
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

    // Indexes are created automatically by the unified SQLite architecture
    println!("\nIndexing complete!");

    Ok(())
}

/// Start the MCP server with stdio transport.
async fn run_serve(db_path: &str) -> Result<()> {
    info!("Starting MCP server...");

    // Initialize embedder (connects to Ollama server)
    let config = EmbeddingConfig::default();
    let embedder = OllamaEmbedder::new(config).context("Failed to initialize Ollama embedder")?;
    embedder.health_check().await.context("Ollama server not reachable")?;

    // Initialize store
    let mut store = KixStore::new(Path::new(db_path))
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

/// Start both API and MCP servers in a single process with shared stores.
/// This is the recommended mode for single-node deployments as it ensures
/// data indexed via MCP is immediately visible to API searches.
/// Optionally enables stdio transport for IDE integration.
async fn run(db_path: &str, host: &str, api_port: u16, mcp_port: u16, enable_stdio: bool) -> Result<()> {
    info!("Starting unified server (API + MCP with shared store)...");

    // Initialize store - SINGLE SHARED INSTANCE
    let mut store = KixStore::new(Path::new(db_path))
        .await
        .context("Failed to open database")?;
    store
        .init_tables()
        .await
        .context("Failed to initialize tables")?;

    // Create the single shared store
    let shared_store = Arc::new(RwLock::new(store));
    info!("Created single shared KixStore instance for API and MCP servers");

    // Create single shared embedder for both API and MCP (connects to Ollama server)
    let config = EmbeddingConfig::default();
    let embedder = OllamaEmbedder::new(config).context("Failed to initialize Ollama embedder")?;
    embedder.health_check().await.context("Ollama server not reachable")?;
    let shared_embedder = Arc::new(embedder);
    info!("Created single shared OllamaEmbedder for API and MCP servers");

    // Create shared components
    let job_queue = Arc::new(JobQueue::new(QueueConfig::default()));
    let sse_manager = Arc::new(ConnectionManager::new(Default::default()));

    // Start SSE cleanup background task
    spawn_cleanup_task(sse_manager.clone());

    let file_handler = Arc::new(FileHandler::with_defaults());
    file_handler.init().await?;

    // Initialize job history store (uses shared SQLite database)
    let jobs_db_path = PathBuf::from(db_path).join("sqlite/kix.db");
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

    // Initialize auth store for OAuth persistence
    let auth_db_path = PathBuf::from(db_path).join("auth.db");
    let auth_store = AuthStore::new(&auth_db_path)
        .await
        .context("Failed to create auth store")?;
    auth_store
        .init_schema()
        .await
        .context("Failed to initialize auth schema")?;
    info!("Auth store initialized at {}", auth_db_path.display());

    let auth_state = AuthState::new(Arc::new(auth_store));

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

    // Create and start the job executor with shared stores
    let executor_config = ExecutorConfig {
        db_path: db_path.to_string(),
        jobs_db_path: None, // Not needed when using shared_job_store
        on_job_complete: None,
        shared_store: Some(shared_store.clone()), // Pass shared KixStore to executor
        shared_job_store: job_store.clone(),      // Pass shared JobStore to executor
        ..Default::default()
    };

    let mut executor = JobExecutor::new(job_queue.clone(), sse_manager.clone(), executor_config)
        .await
        .context("Failed to create job executor")?;
    executor.start();
    info!("Job executor started with shared KixStore and JobStore");

    // Create project store and event bus for project management (uses shared SQLite database)
    let project_db_path = format!("{}/sqlite/kix.db", db_path);
    let mut project_store = ProjectStore::new(&project_db_path, 384) // 384 is fastembed dimension
        .await
        .context("Failed to create project store")?;
    project_store
        .init_tables()
        .await
        .context("Failed to initialize project tables")?;
    let shared_project_store = Arc::new(RwLock::new(project_store));

    let event_bus = create_event_bus();
    let project_state = ProjectState::new(shared_project_store.clone(), event_bus.clone());
    info!("Project store initialized at {}", project_db_path);

    // Create explorer state for data explorer
    let explorer_state = ExplorerState::new(PathBuf::from(db_path));
    info!("Explorer state initialized for data directory: {}", db_path);

    // Create API router
    let main_router = create_router(api_state);
    let indexing_router = create_indexing_router(indexing_state);
    let project_router = create_project_router(project_state);
    let explorer_router = create_explorer_router(explorer_state);
    let api_app = main_router.merge(indexing_router).merge(project_router).merge(explorer_router);

    // Create stdio MCP server BEFORE HTTP closure moves the Arcs (if enabled)
    let stdio_mcp_server = if enable_stdio {
        Some(KixMcpServer::with_shared(
            shared_store.clone(),
            shared_embedder.clone(),
            job_queue.clone(),
        )
        .with_project_store(shared_project_store.clone())
        .with_event_bus(event_bus.clone()))
    } else {
        None
    };

    // Create MCP server using the SAME shared store, embedder, and project store
    let mcp_server = KixMcpServer::with_shared(shared_store.clone(), shared_embedder, job_queue)
        .with_project_store(shared_project_store)
        .with_event_bus(event_bus);
    info!("MCP server initialized with project management enabled");

    // Spawn stdio transport task if enabled
    let stdio_handle = stdio_mcp_server.map(|server| {
        tokio::spawn(async move {
            info!("Starting MCP stdio transport...");
            let transport = rmcp::transport::io::stdio();
            let running = rmcp::serve_server(server, transport).await
                .context("Failed to start stdio transport")?;
            running.waiting().await.context("stdio transport error")?;
            info!("MCP stdio transport closed");
            Ok::<(), anyhow::Error>(())
        })
    });

    // Create the MCP streamable HTTP service
    let mcp_service = StreamableHttpService::new(
        move || {
            let server = mcp_server.clone();
            Ok(server)
        },
        Arc::new(LocalSessionManager::default()),
        Default::default(),
    );

    // Create MCP router with persistent OAuth handlers
    let mcp_app = axum::Router::new()
        .route("/.well-known/oauth-authorization-server", get(auth_handlers::oauth_metadata))
        .route("/.well-known/oauth-protected-resource", get(auth_handlers::oauth_resource_metadata))
        .route("/oauth/register", axum::routing::post(auth_handlers::oauth_register))
        .route("/oauth/authorize", get(auth_handlers::oauth_authorize))
        .route("/oauth/token", axum::routing::post(auth_handlers::oauth_token))
        .route("/oauth/revoke", axum::routing::post(auth_handlers::oauth_revoke))
        .nest_service("/mcp", mcp_service)
        .with_state(auth_state.clone());

    // Bind to addresses
    let api_addr: SocketAddr = format!("{}:{}", host, api_port).parse()?;
    let mcp_addr: SocketAddr = format!("{}:{}", host, mcp_port).parse()?;

    let api_listener = tokio::net::TcpListener::bind(api_addr).await?;
    let mcp_listener = tokio::net::TcpListener::bind(mcp_addr).await?;

    println!("=== Unified Server (Shared Store) ===");
    println!("  REST API:    http://{}", api_addr);
    println!("  MCP HTTP:    http://{}/mcp", mcp_addr);
    if enable_stdio {
        println!("  MCP stdio:   enabled (reading from stdin)");
    }
    println!();
    println!("All servers share a single KixStore instance.");
    println!("Data indexed via any transport is immediately searchable.");
    info!("Unified server started - API: {}, MCP HTTP: {}, stdio: {}", api_addr, mcp_addr, enable_stdio);

    // Create shutdown signal
    let shutdown = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to listen for ctrl-c");
        info!("Shutting down unified server...");
    };

    // Run all servers concurrently
    tokio::select! {
        result = axum::serve(api_listener, api_app.into_make_service_with_connect_info::<SocketAddr>()) => {
            if let Err(e) = result {
                error!("API server error: {}", e);
            }
        }
        result = axum::serve(mcp_listener, mcp_app) => {
            if let Err(e) = result {
                error!("MCP HTTP server error: {}", e);
            }
        }
        result = async {
            match stdio_handle {
                Some(handle) => handle.await,
                None => std::future::pending().await,
            }
        } => {
            match result {
                Ok(Ok(())) => info!("MCP stdio transport closed normally"),
                Ok(Err(e)) => error!("MCP stdio transport error: {}", e),
                Err(e) => error!("MCP stdio task panicked: {}", e),
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

    // Initialize embedder (connects to Ollama server)
    let config = EmbeddingConfig::default();
    let embedder = OllamaEmbedder::new(config).context("Failed to initialize Ollama embedder")?;
    embedder.health_check().await.context("Ollama server not reachable")?;

    // Initialize store
    let mut store = KixStore::new(Path::new(db_path))
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

    // Perform search based on type (all modes use async embedding)
    let results = match search_type {
        "semantic" | "vector" => {
            let embedding = embedder.embed_one(query).await?;
            store.vector_search(&embedding, limit, &filters).await?
        }
        "text" | "fts" => {
            // FTS-only search: Use hybrid search with zero-vector for pure text matching
            let embedding = embedder.embed_one(query).await?;
            store.hybrid_search(query, &embedding, limit, &filters).await?
        }
        _ => {
            // Default to hybrid
            let embedding = embedder.embed_one(query).await?;
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

    let mut store = KixStore::new(Path::new(db_path))
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

    let mut store = KixStore::new(Path::new(db_path))
        .await
        .context("Failed to open database")?;
    store
        .init_tables()
        .await
        .context("Failed to initialize tables")?;

    // Note: In the unified SQLite architecture, indexes are created automatically
    // during store initialization. This command is kept for backward compatibility.
    println!("Search indexes are created automatically during store initialization.");
    println!("Store initialized successfully!");

    Ok(())
}

/// Verify Ollama connection and embedding model availability.
async fn run_setup(model: Option<String>, _force: bool) -> Result<()> {
    println!("\n=== Ollama Embedding Setup ===\n");

    // Get model name
    let model_name = model.unwrap_or_else(|| {
        std::env::var("OLLAMA_MODEL")
            .unwrap_or_else(|_| "nomic-embed-text".to_string())
    });
    println!("Model: {}", model_name);

    // Get Ollama URL
    let ollama_url = std::env::var("OLLAMA_URL")
        .unwrap_or_else(|_| "http://localhost:11434".to_string());
    println!("Ollama URL: {}", ollama_url);

    // Create embedder config with model
    let config = EmbeddingConfig {
        model: model_name.clone(),
        ollama_url: ollama_url.clone(),
        ..Default::default()
    };

    println!("\nConnecting to Ollama...");
    let embedder = OllamaEmbedder::new(config).context("Failed to initialize Ollama embedder")?;

    // Test connection and model availability
    match embedder.health_check().await {
        Ok(()) => {
            println!("✓ Ollama server is reachable");
        }
        Err(e) => {
            println!("✗ Failed to connect to Ollama: {}", e);
            println!("\nMake sure Ollama is running:");
            println!("  ollama serve");
            println!("\nAnd the embedding model is pulled:");
            println!("  ollama pull {}", model_name);
            return Err(anyhow::anyhow!("Ollama not reachable: {}", e));
        }
    }

    // Test embedding generation
    println!("\nTesting embedding generation...");
    match embedder.embed_one("test embedding").await {
        Ok(embedding) => {
            println!("✓ Embedding generated successfully ({} dimensions)", embedding.len());
        }
        Err(e) => {
            println!("✗ Failed to generate embedding: {}", e);
            println!("\nMake sure the model is pulled:");
            println!("  ollama pull {}", model_name);
            return Err(anyhow::anyhow!("Embedding generation failed: {}", e));
        }
    }

    println!("\nSetup complete!");
    println!("  Ollama URL: {}", ollama_url);
    println!("  Model: {}", model_name);

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

// OAuth handlers are now provided by the kix-auth crate with SQLite persistence.
// See: kix-auth/src/handlers.rs

/// Migrate from old data format to unified SQLite architecture.
///
/// This migrates:
/// - Entries, pages, projects, issues, tokens, jobs → SQLite (kix.db)
/// - Chunks (with vector embeddings) → SQLite + sqlite-vec (vectors.db)
async fn run_migrate(from: &PathBuf, to: &PathBuf, skip_confirm: bool) -> Result<()> {
    use std::io::{self, Write};

    println!("╔════════════════════════════════════════════════════════════════╗");
    println!("║       KIX Migration: Old Format → Unified SQLite               ║");
    println!("╚════════════════════════════════════════════════════════════════╝");
    println!();
    println!("Source (old format):      {}", from.display());
    println!("Destination (SQLite):     {}", to.display());
    println!("  └─ SQLite:              {}/sqlite/kix.db", to.display());
    println!("  └─ Vectors:             {}/sqlite/vectors.db", to.display());
    println!();

    // Check source exists
    if !from.exists() {
        anyhow::bail!("Source directory does not exist: {}", from.display());
    }

    // Check destination
    let sqlite_path = to.join("sqlite/kix.db");
    let vectors_path = to.join("sqlite/vectors.db");

    if sqlite_path.exists() || vectors_path.exists() {
        println!("⚠️  Warning: Destination already contains data!");
        if !skip_confirm {
            print!("Continue and overwrite? [y/N]: ");
            io::stdout().flush()?;
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            if !input.trim().eq_ignore_ascii_case("y") {
                println!("Migration cancelled.");
                return Ok(());
            }
        }
    }

    if !skip_confirm {
        print!("Start migration? [y/N]: ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Migration cancelled.");
            return Ok(());
        }
    }

    println!();
    println!("Starting migration...");
    println!();

    // For now, just print instructions since migration from old formats is complex
    // and requires re-indexing anyway
    println!("ℹ️  Migration from old formats requires re-indexing.");
    println!();
    println!("To migrate your data:");
    println!("  1. Create a new data directory: mkdir -p {}/sqlite", to.display());
    println!("  2. Start the API server: kix api --db-path {}", to.display());
    println!("  3. Re-index your content through the dashboard or CLI");
    println!();
    println!("Projects and issues will need to be recreated via the UI or MCP tools.");
    println!();
    println!("New data directory structure:");
    println!("   {}/sqlite/kix.db      <- All metadata (entries, pages, projects, etc.)", to.display());
    println!("   {}/sqlite/vectors.db  <- Vector embeddings", to.display());

    Ok(())
}
