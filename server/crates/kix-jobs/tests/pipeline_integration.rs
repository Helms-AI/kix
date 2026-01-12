//! Integration tests for the full KIX processing pipeline
//!
//! Tests the complete flow from HTML content to searchable chunks in LanceDB,
//! including two-layer storage with page context retrieval.

use std::path::PathBuf;
use std::sync::Once;
use tempfile::TempDir;

use kix_jobs::{ContentProcessor, ProcessorConfig};

/// Ensure Ollama backend is skipped to avoid runtime conflicts in tests.
/// The Ollama backend uses reqwest::blocking which creates its own tokio runtime,
/// conflicting with the test runtime.
static INIT: Once = Once::new();

fn init_test_env() {
    INIT.call_once(|| {
        std::env::set_var("SKIP_OLLAMA_BACKEND", "1");
    });
}

/// Create a temporary directory for test database
fn create_temp_db() -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("test_lancedb");
    (temp_dir, db_path)
}

/// Sample HTML content for testing
const SAMPLE_HTML: &str = r#"
<!DOCTYPE html>
<html>
<head>
    <title>Enterprise Integration Patterns</title>
</head>
<body>
    <article>
        <h1>Message Queue Pattern</h1>
        <p>The Message Queue pattern provides asynchronous communication between components.</p>
        <h2>Problem</h2>
        <p>How can you enable asynchronous processing of messages while decoupling senders from receivers?</p>
        <h2>Solution</h2>
        <p>Use a Message Queue to store messages temporarily until the receiver is ready to process them.</p>
        <pre><code class="language-rust">
use std::collections::VecDeque;

pub struct MessageQueue<T> {
    messages: VecDeque<T>,
}

impl<T> MessageQueue<T> {
    pub fn new() -> Self {
        Self { messages: VecDeque::new() }
    }

    pub fn send(&mut self, message: T) {
        self.messages.push_back(message);
    }

    pub fn receive(&mut self) -> Option<T> {
        self.messages.pop_front()
    }
}
        </code></pre>
        <h2>Benefits</h2>
        <ul>
            <li>Decouples sender and receiver</li>
            <li>Enables asynchronous processing</li>
            <li>Provides buffering during load spikes</li>
        </ul>
    </article>
</body>
</html>
"#;

/// Sample HTML with multiple sections for chunking test
const MULTI_SECTION_HTML: &str = r#"
<!DOCTYPE html>
<html>
<head>
    <title>API Documentation</title>
</head>
<body>
    <main>
        <h1>REST API Guide</h1>
        <p>This guide covers the REST API endpoints and usage patterns.</p>

        <h2>Authentication</h2>
        <p>All API requests require authentication using Bearer tokens.</p>
        <pre><code class="language-bash">
curl -H "Authorization: Bearer YOUR_TOKEN" \
     https://api.example.com/v1/resources
        </code></pre>

        <h2>Endpoints</h2>
        <h3>GET /users</h3>
        <p>Returns a list of users with pagination support.</p>
        <pre><code class="language-json">
{
    "users": [
        {"id": 1, "name": "Alice"},
        {"id": 2, "name": "Bob"}
    ],
    "total": 100,
    "page": 1
}
        </code></pre>

        <h3>POST /users</h3>
        <p>Creates a new user in the system.</p>
        <pre><code class="language-json">
{
    "name": "Charlie",
    "email": "charlie@example.com"
}
        </code></pre>

        <h2>Error Handling</h2>
        <p>All errors return a consistent JSON structure with error codes and messages.</p>
    </main>
</body>
</html>
"#;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_basic_html_processing() {
    init_test_env();
    let (_temp_dir, db_path) = create_temp_db();

    // Create processor
    let config = ProcessorConfig {
        enable_linking: false, // Disable for faster tests
        ..Default::default()
    };

    let processor = ContentProcessor::new(db_path.to_str().unwrap(), config)
        .await
        .expect("Failed to create processor");

    // Process HTML
    let result = processor
        .process_html(SAMPLE_HTML, "https://example.com/patterns/message-queue")
        .await
        .expect("Failed to process HTML");

    // Verify results
    assert!(!result.document_id.is_empty(), "Document ID should not be empty");
    assert!(result.chunks_created > 0, "Should have created chunks");
    assert_eq!(result.chunks_created, result.embeddings_generated, "Chunks and embeddings should match");

    // Verify document count
    let doc_count = processor.document_count().await.expect("Failed to get doc count");
    assert_eq!(doc_count, 1, "Should have exactly one document");

    // Verify chunk count
    let chunk_count = processor.chunk_count().await.expect("Failed to get chunk count");
    assert_eq!(chunk_count, result.chunks_created, "Chunk count should match");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_two_layer_storage() {
    init_test_env();
    let (_temp_dir, db_path) = create_temp_db();

    // Create processor
    let config = ProcessorConfig {
        enable_linking: false,
        ..Default::default()
    };

    let processor = ContentProcessor::new(db_path.to_str().unwrap(), config)
        .await
        .expect("Failed to create processor");

    // Process HTML with two-layer storage
    let result = processor
        .process_html_with_page(SAMPLE_HTML, "https://example.com/patterns/message-queue", Some(150))
        .await
        .expect("Failed to process HTML with page");

    // Verify results
    assert!(!result.document_id.is_empty(), "Document ID should not be empty");
    assert!(result.page_id.is_some(), "Page ID should be present");
    assert!(result.chunks_created > 0, "Should have created chunks");

    // Verify page count
    let page_count = processor.page_count().await.expect("Failed to get page count");
    assert_eq!(page_count, 1, "Should have exactly one page");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_page_context_retrieval() {
    init_test_env();
    let (_temp_dir, db_path) = create_temp_db();

    // Create processor
    let config = ProcessorConfig {
        enable_linking: false,
        ..Default::default()
    };

    let processor = ContentProcessor::new(db_path.to_str().unwrap(), config)
        .await
        .expect("Failed to create processor");

    // Process HTML with two-layer storage
    let result = processor
        .process_html_with_page(SAMPLE_HTML, "https://example.com/patterns/message-queue", None)
        .await
        .expect("Failed to process HTML with page");

    let page_id = result.page_id.expect("Page ID should be present");

    // Retrieve page context
    let page_context = processor
        .get_page_context(&page_id)
        .await
        .expect("Failed to get page context");

    assert!(page_context.is_some(), "Page context should be found");

    let page = page_context.unwrap();
    assert_eq!(page.page_id, page_id, "Page ID should match");
    assert!(!page.full_content.is_empty(), "Full content should not be empty");
    assert!(page.full_content.contains("Message Queue"), "Content should contain title text");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_multi_section_chunking() {
    init_test_env();
    let (_temp_dir, db_path) = create_temp_db();

    // Create processor with smaller chunk size to force multiple chunks
    let config = ProcessorConfig {
        chunk_size: 500,
        chunk_overlap: 50,
        enable_linking: false,
        ..Default::default()
    };

    let processor = ContentProcessor::new(db_path.to_str().unwrap(), config)
        .await
        .expect("Failed to create processor");

    // Process multi-section HTML
    let result = processor
        .process_html(MULTI_SECTION_HTML, "https://example.com/docs/api")
        .await
        .expect("Failed to process HTML");

    // With smaller chunk size, should have multiple chunks
    assert!(result.chunks_created >= 2, "Should have created multiple chunks, got {}", result.chunks_created);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_document_update_replaces_existing() {
    init_test_env();
    let (_temp_dir, db_path) = create_temp_db();

    let config = ProcessorConfig {
        enable_linking: false,
        ..Default::default()
    };

    let processor = ContentProcessor::new(db_path.to_str().unwrap(), config)
        .await
        .expect("Failed to create processor");

    // Process original HTML
    let result1 = processor
        .process_html(SAMPLE_HTML, "https://example.com/patterns/message-queue")
        .await
        .expect("Failed to process HTML");

    // Process same URL with updated content
    let updated_html = SAMPLE_HTML.replace("Message Queue Pattern", "Updated Message Queue Pattern");
    let result2 = processor
        .process_html(&updated_html, "https://example.com/patterns/message-queue")
        .await
        .expect("Failed to process updated HTML");

    // Document ID should be the same (same URL)
    assert_eq!(result1.document_id, result2.document_id, "Document ID should remain the same");

    // Should still have exactly one document
    let doc_count = processor.document_count().await.expect("Failed to get doc count");
    assert_eq!(doc_count, 1, "Should have exactly one document after update");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_multiple_documents() {
    init_test_env();
    let (_temp_dir, db_path) = create_temp_db();

    let config = ProcessorConfig {
        enable_linking: false,
        ..Default::default()
    };

    let processor = ContentProcessor::new(db_path.to_str().unwrap(), config)
        .await
        .expect("Failed to create processor");

    // Process first document
    let _result1 = processor
        .process_html(SAMPLE_HTML, "https://example.com/patterns/message-queue")
        .await
        .expect("Failed to process first HTML");

    // Process second document
    let _result2 = processor
        .process_html(MULTI_SECTION_HTML, "https://example.com/docs/api")
        .await
        .expect("Failed to process second HTML");

    // Should have two documents
    let doc_count = processor.document_count().await.expect("Failed to get doc count");
    assert_eq!(doc_count, 2, "Should have two documents");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_code_block_preservation() {
    init_test_env();
    let (_temp_dir, db_path) = create_temp_db();

    let config = ProcessorConfig {
        enable_linking: false,
        ..Default::default()
    };

    let processor = ContentProcessor::new(db_path.to_str().unwrap(), config)
        .await
        .expect("Failed to create processor");

    // Process HTML with code blocks
    let result = processor
        .process_html_with_page(SAMPLE_HTML, "https://example.com/patterns/message-queue", None)
        .await
        .expect("Failed to process HTML");

    let page_id = result.page_id.expect("Page ID should be present");

    // Retrieve page context and verify code is preserved
    let page = processor
        .get_page_context(&page_id)
        .await
        .expect("Failed to get page context")
        .expect("Page should exist");

    // Verify code blocks are in the content
    assert!(page.full_content.contains("MessageQueue"), "Code should contain struct name");
    assert!(page.full_content.contains("pub fn new"), "Code should contain method");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_empty_content_handling() {
    init_test_env();
    let (_temp_dir, db_path) = create_temp_db();

    let config = ProcessorConfig {
        enable_linking: false,
        ..Default::default()
    };

    let processor = ContentProcessor::new(db_path.to_str().unwrap(), config)
        .await
        .expect("Failed to create processor");

    // Process minimal HTML
    let minimal_html = "<html><head><title>Empty</title></head><body></body></html>";
    let result = processor
        .process_html(minimal_html, "https://example.com/empty")
        .await
        .expect("Failed to process minimal HTML");

    // Should handle empty content gracefully
    assert!(!result.document_id.is_empty(), "Document ID should still be generated");
}
