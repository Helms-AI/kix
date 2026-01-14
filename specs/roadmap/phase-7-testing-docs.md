# Phase 7: Testing & Documentation

**Duration**: 2-3 days
**Dependencies**: All previous phases
**Status**: Not Started

---

## Objective

Comprehensive testing of all new functionality and documentation updates.

---

## Tasks

### 7.1 Integration Tests

**File**: `server/crates/kix-jobs/tests/spider_integration.rs` (NEW)

```rust
//! Integration tests for spider-based crawling

use kix_jobs::crawler::{SpiderCrawler, SpiderConfig, CrawlMode};
use kix_jobs::extraction::{CodeExtractor, ExtractionConfig};
use tokio_stream::StreamExt;

#[tokio::test]
async fn test_spider_crawl_single_page() {
    let config = SpiderConfig::single_page();
    let crawler = SpiderCrawler::new(config);

    let result = crawler.crawl_single("https://example.com").await;

    assert!(result.is_ok());
    let page = result.unwrap();
    assert!(!page.html.is_empty());
    assert!(!page.markdown.is_empty());
    assert_eq!(page.status, 200);
}

#[tokio::test]
async fn test_spider_crawl_documentation_site() {
    let config = SpiderConfig::documentation();
    let crawler = SpiderCrawler::new(config);

    let mut pages = vec![];
    let mut stream = crawler.crawl("https://docs.rs/tokio/latest/tokio/").await;

    // Collect first 5 pages
    while let Some(result) = stream.next().await {
        if let Ok(page) = result {
            pages.push(page);
            if pages.len() >= 5 {
                break;
            }
        }
    }

    assert!(!pages.is_empty());

    // At least one page should have code
    let has_code = pages.iter().any(|p| p.html.contains("<code"));
    assert!(has_code, "Documentation site should have code blocks");
}

#[tokio::test]
async fn test_spider_with_code_extraction() {
    let crawler_config = SpiderConfig::single_page();
    let crawler = SpiderCrawler::new(crawler_config);

    let extractor = CodeExtractor::new(ExtractionConfig::default());

    // Crawl a page with known code blocks
    let page = crawler
        .crawl_single("https://docs.rs/tokio/latest/tokio/")
        .await
        .expect("Should crawl page");

    // Extract code
    let result = extractor.extract(&page.html);

    assert!(result.is_ok());
    let extraction = result.unwrap();

    // Documentation pages typically have code
    if !extraction.blocks.is_empty() {
        // Verify blocks have content
        for block in &extraction.blocks {
            assert!(!block.content.is_empty());
            assert!(block.line_count > 0);
        }
    }
}

#[tokio::test]
async fn test_spider_rate_limiting() {
    use std::time::Instant;

    let config = SpiderConfig {
        rate_limit: Some(std::time::Duration::from_millis(200)),
        max_pages: 3,
        ..SpiderConfig::default()
    };
    let crawler = SpiderCrawler::new(config);

    let start = Instant::now();
    let mut count = 0;

    let mut stream = crawler.crawl("https://httpbin.org/").await;
    while let Some(_) = stream.next().await {
        count += 1;
        if count >= 3 {
            break;
        }
    }

    let elapsed = start.elapsed();

    // With 200ms delay between requests, 3 requests should take at least 400ms
    assert!(elapsed.as_millis() >= 400, "Rate limiting should be enforced");
}
```

---

### 7.2 Code Extraction Tests

**File**: `server/crates/kix-jobs/tests/code_extraction.rs` (NEW)

```rust
//! Tests for code extraction patterns

use kix_jobs::extraction::{CodeExtractor, CodePattern, Language, ExtractionConfig};

/// Test fixture HTML for various frameworks
mod fixtures {
    pub const DOCUSAURUS_HTML: &str = r#"
        <div class="theme-code-block">
            <pre><code class="language-rust">
fn main() {
    println!("Hello, world!");
}
            </code></pre>
        </div>
    "#;

    pub const MKDOCS_HTML: &str = r#"
        <div class="codehilite">
            <pre><code class="language-python">
def hello():
    print("Hello, world!")
            </code></pre>
        </div>
    "#;

    pub const PRISM_HTML: &str = r#"
        <pre class="language-javascript"><code>
const greeting = () => console.log("Hello!");
        </code></pre>
    "#;

    pub const GITHUB_HTML: &str = r#"
        <div class="highlight highlight-source-go">
            <pre>
package main

func main() {
    fmt.Println("Hello!")
}
            </pre>
        </div>
    "#;
}

#[test]
fn test_extract_docusaurus_pattern() {
    let extractor = CodeExtractor::new(ExtractionConfig::default());
    let result = extractor.extract(fixtures::DOCUSAURUS_HTML).unwrap();

    assert_eq!(result.blocks.len(), 1);
    assert_eq!(result.blocks[0].pattern, CodePattern::DocusaurusCodeBlock);
    assert_eq!(result.blocks[0].language, Language::Rust);
    assert!(result.blocks[0].content.contains("println!"));
}

#[test]
fn test_extract_mkdocs_pattern() {
    let extractor = CodeExtractor::new(ExtractionConfig::default());
    let result = extractor.extract(fixtures::MKDOCS_HTML).unwrap();

    assert_eq!(result.blocks.len(), 1);
    assert_eq!(result.blocks[0].pattern, CodePattern::MkDocsCodeHilite);
    assert_eq!(result.blocks[0].language, Language::Python);
    assert!(result.blocks[0].content.contains("def hello"));
}

#[test]
fn test_extract_prism_pattern() {
    let extractor = CodeExtractor::new(ExtractionConfig::default());
    let result = extractor.extract(fixtures::PRISM_HTML).unwrap();

    assert_eq!(result.blocks.len(), 1);
    assert_eq!(result.blocks[0].pattern, CodePattern::PrismJs);
    assert_eq!(result.blocks[0].language, Language::JavaScript);
}

#[test]
fn test_extract_github_pattern() {
    let extractor = CodeExtractor::new(ExtractionConfig::default());
    let result = extractor.extract(fixtures::GITHUB_HTML).unwrap();

    assert_eq!(result.blocks.len(), 1);
    assert_eq!(result.blocks[0].pattern, CodePattern::GitHubHighlight);
    assert_eq!(result.blocks[0].language, Language::Go);
}

#[test]
fn test_language_detection_from_class() {
    let cases = [
        ("language-rust", Language::Rust),
        ("language-python", Language::Python),
        ("language-javascript", Language::JavaScript),
        ("language-typescript", Language::TypeScript),
        ("language-go", Language::Go),
        ("language-java", Language::Java),
        ("lang-rs", Language::Rust),
        ("lang-py", Language::Python),
        ("lang-js", Language::JavaScript),
    ];

    for (class, expected) in cases {
        let detected = Language::from_class(class);
        assert_eq!(
            detected,
            Some(expected),
            "Failed for class: {}",
            class
        );
    }
}

#[test]
fn test_language_detection_from_hint() {
    let cases = [
        ("rust", Language::Rust),
        ("rs", Language::Rust),
        ("python", Language::Python),
        ("py", Language::Python),
        ("javascript", Language::JavaScript),
        ("js", Language::JavaScript),
        ("typescript", Language::TypeScript),
        ("ts", Language::TypeScript),
    ];

    for (hint, expected) in cases {
        let detected = Language::from_hint(hint);
        assert_eq!(
            detected,
            Some(expected),
            "Failed for hint: {}",
            hint
        );
    }
}

#[test]
fn test_validation_rejects_short_blocks() {
    let config = ExtractionConfig {
        min_length: 50,
        ..Default::default()
    };
    let extractor = CodeExtractor::new(config);

    let html = r#"<pre><code class="language-rust">x</code></pre>"#;
    let result = extractor.extract(html).unwrap();

    assert!(result.blocks.is_empty());
    assert_eq!(result.stats.rejected_too_short, 1);
}

#[test]
fn test_validation_rejects_prose() {
    let config = ExtractionConfig {
        max_prose_ratio: 0.5,
        ..Default::default()
    };
    let extractor = CodeExtractor::new(config);

    let html = r#"<pre><code class="language-rust">
This is just a paragraph of text that happens to be in a code block.
It doesn't contain any actual code, just prose content that should be rejected.
There are no function definitions, variable declarations, or any programming constructs.
    </code></pre>"#;

    let result = extractor.extract(html).unwrap();

    assert!(result.blocks.is_empty() || result.stats.rejected_prose > 0);
}

#[test]
fn test_deduplication() {
    let extractor = CodeExtractor::new(ExtractionConfig::default());

    let html = r#"
        <pre><code class="language-rust">fn hello() {}</code></pre>
        <pre><code class="language-rust">fn hello() {}</code></pre>
        <pre><code class="language-rust">fn hello() {}</code></pre>
    "#;

    let result = extractor.extract(html).unwrap();

    // Should only keep one unique block
    assert_eq!(result.blocks.len(), 1);
    assert!(result.stats.rejected_duplicates >= 2);
}

#[test]
fn test_all_patterns_have_selectors() {
    for pattern in CodePattern::all() {
        let selector = pattern.css_selector();
        assert!(!selector.is_empty(), "Pattern {:?} missing selector", pattern);
    }
}

#[test]
fn test_all_languages_have_display_names() {
    for lang in Language::all() {
        let name = lang.display_name();
        assert!(!name.is_empty(), "Language {:?} missing display name", lang);
    }
}
```

---

### 7.3 Embedding Tests

**File**: `server/crates/kix-embeddings/tests/ollama_integration.rs` (NEW)

```rust
//! Integration tests for Ollama embeddings
//! Run with: cargo test -p kix-embeddings --test ollama_integration -- --ignored

use kix_embeddings::{OllamaEmbedder, EmbeddingConfig};

#[tokio::test]
#[ignore] // Requires running Ollama
async fn test_ollama_connection() {
    let embedder = OllamaEmbedder::default_config()
        .expect("Should create embedder");

    let result = embedder.health_check().await;
    assert!(result.is_ok(), "Ollama should be reachable");
}

#[tokio::test]
#[ignore]
async fn test_embedding_dimensions() {
    let embedder = OllamaEmbedder::default_config().unwrap();

    let embedding = embedder.embed_one("Test text").await.unwrap();

    assert_eq!(embedding.len(), 768, "nomic-embed-text should produce 768 dims");
}

#[tokio::test]
#[ignore]
async fn test_batch_embeddings() {
    let embedder = OllamaEmbedder::default_config().unwrap();

    let texts = vec![
        "First document about Rust programming".to_string(),
        "Second document about Python".to_string(),
        "Third document about JavaScript".to_string(),
    ];

    let embeddings = embedder.embed_batch(&texts).await.unwrap();

    assert_eq!(embeddings.len(), 3);
    for emb in &embeddings {
        assert_eq!(emb.len(), 768);
    }
}

#[tokio::test]
#[ignore]
async fn test_semantic_similarity() {
    let embedder = OllamaEmbedder::default_config().unwrap();

    // Similar texts
    let rust_1 = embedder.embed_one("Rust programming language").await.unwrap();
    let rust_2 = embedder.embed_one("The Rust language for systems programming").await.unwrap();

    // Different text
    let cooking = embedder.embed_one("How to make chocolate cake").await.unwrap();

    let sim_rust = cosine_similarity(&rust_1, &rust_2);
    let sim_diff = cosine_similarity(&rust_1, &cooking);

    assert!(
        sim_rust > sim_diff,
        "Similar texts should have higher similarity"
    );
    assert!(sim_rust > 0.7, "Similar texts should have high similarity");
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    dot / (mag_a * mag_b)
}
```

---

### 7.4 Tree-sitter Tests

**File**: `server/crates/kix-parser/tests/treesitter_integration.rs` (NEW)

```rust
//! Integration tests for tree-sitter chunking

use kix_parser::treesitter::{TreeSitterChunker, SourceLanguage, ChunkerConfig};
use std::path::PathBuf;

#[test]
fn test_rust_file_chunking() {
    let chunker = TreeSitterChunker::default();

    let source = include_str!("fixtures/sample.rs");
    let path = PathBuf::from("sample.rs");

    let chunks = chunker.chunk_file(&path, source).unwrap();

    assert!(!chunks.is_empty());

    // Should extract functions and structs
    let symbols: Vec<_> = chunks.iter().flat_map(|c| &c.symbols).collect();
    assert!(
        symbols.iter().any(|s| s.kind == SymbolKind::Function),
        "Should find functions"
    );
}

#[test]
fn test_python_file_chunking() {
    let chunker = TreeSitterChunker::default();

    let source = include_str!("fixtures/sample.py");
    let path = PathBuf::from("sample.py");

    let chunks = chunker.chunk_file(&path, source).unwrap();

    assert!(!chunks.is_empty());

    let symbols: Vec<_> = chunks.iter().flat_map(|c| &c.symbols).collect();
    assert!(
        symbols.iter().any(|s| s.kind == SymbolKind::Class),
        "Should find classes"
    );
}

#[test]
fn test_javascript_file_chunking() {
    let chunker = TreeSitterChunker::default();

    let source = r#"
function greet(name) {
    console.log(`Hello, ${name}!`);
}

class Person {
    constructor(name) {
        this.name = name;
    }

    sayHello() {
        greet(this.name);
    }
}

const helper = () => {
    return 42;
};
"#;

    let chunks = chunker
        .chunk_source(source, SourceLanguage::JavaScript, "test.js".to_string())
        .unwrap();

    assert!(!chunks.is_empty());
}

#[test]
fn test_unsupported_extension() {
    let chunker = TreeSitterChunker::default();
    let path = PathBuf::from("file.xyz");

    let result = chunker.chunk_file(&path, "content");
    assert!(result.is_err());
}

#[test]
fn test_chunk_size_limits() {
    let config = ChunkerConfig {
        max_chunk_size: 200,
        min_chunk_size: 50,
        ..Default::default()
    };
    let chunker = TreeSitterChunker::new(config);

    let source = r#"
fn func1() { println!("1"); }
fn func2() { println!("2"); }
fn func3() { println!("3"); }
fn func4() { println!("4"); }
fn func5() { println!("5"); }
fn func6() { println!("6"); }
fn func7() { println!("7"); }
fn func8() { println!("8"); }
"#;

    let chunks = chunker
        .chunk_source(source, SourceLanguage::Rust, "test.rs".to_string())
        .unwrap();

    // With small max size, should have multiple chunks
    assert!(chunks.len() > 1);
}
```

---

### 7.5 End-to-End Pipeline Test

**File**: `server/crates/kix-jobs/tests/pipeline_e2e.rs` (NEW)

```rust
//! End-to-end pipeline tests

use kix_jobs::ContentProcessor;
use kix_store::KixStore;
use tempfile::tempdir;

#[tokio::test]
#[ignore] // Requires Ollama
async fn test_full_indexing_pipeline() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let store = KixStore::new(&db_path).await.unwrap();
    let processor = ContentProcessor::new(store.clone());

    // Index a known documentation page
    let result = processor
        .process_url("https://docs.rs/tokio/latest/tokio/", Default::default())
        .await;

    assert!(result.is_ok());
    let summary = result.unwrap();

    // Verify results
    assert!(summary.pages_crawled > 0);
    assert!(summary.chunks_created > 0);

    // Verify search works
    let search_results = store
        .hybrid_search("async runtime", 5)
        .await
        .unwrap();

    assert!(!search_results.is_empty());
}

#[tokio::test]
#[ignore]
async fn test_code_extraction_in_pipeline() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let store = KixStore::new(&db_path).await.unwrap();
    let processor = ContentProcessor::new(store.clone());

    let result = processor
        .process_url("https://doc.rust-lang.org/book/ch01-02-hello-world.html", Default::default())
        .await
        .unwrap();

    // The Rust book should have code examples
    assert!(result.code_blocks_extracted > 0);

    // Verify code blocks are searchable
    let code_results = store
        .hybrid_search("fn main println", 5)
        .await
        .unwrap();

    assert!(!code_results.is_empty());
}
```

---

### 7.6 API Tests

**File**: `server/crates/kix-api/tests/api_integration.rs` (NEW)

```rust
//! API integration tests

use axum::http::StatusCode;
use axum_test::TestServer;
use serde_json::json;

mod test_helpers;

#[tokio::test]
async fn test_patterns_endpoint() {
    let server = test_helpers::create_test_server().await;

    let response = server.get("/api/indexing/patterns").await;

    assert_eq!(response.status_code(), StatusCode::OK);

    let patterns: Vec<serde_json::Value> = response.json();
    assert!(!patterns.is_empty());

    // Verify pattern structure
    let first = &patterns[0];
    assert!(first.get("name").is_some());
    assert!(first.get("cssSelector").is_some());
    assert!(first.get("description").is_some());
}

#[tokio::test]
async fn test_languages_endpoint() {
    let server = test_helpers::create_test_server().await;

    let response = server.get("/api/indexing/languages").await;

    assert_eq!(response.status_code(), StatusCode::OK);

    let languages: Vec<serde_json::Value> = response.json();
    assert!(!languages.is_empty());

    // Should include common languages
    let names: Vec<_> = languages
        .iter()
        .map(|l| l["name"].as_str().unwrap())
        .collect();

    assert!(names.contains(&"Rust"));
    assert!(names.contains(&"Python"));
    assert!(names.contains(&"JavaScript"));
}

#[tokio::test]
async fn test_create_job() {
    let server = test_helpers::create_test_server().await;

    let response = server
        .post("/api/indexing/jobs")
        .json(&json!({
            "url": "https://example.com",
            "options": {
                "maxDepth": 1
            }
        }))
        .await;

    assert_eq!(response.status_code(), StatusCode::CREATED);

    let job: serde_json::Value = response.json();
    assert!(job.get("id").is_some());
    assert_eq!(job["status"], "pending");
}

#[tokio::test]
async fn test_sse_connection() {
    let server = test_helpers::create_test_server().await;

    let response = server
        .get("/api/indexing/events")
        .add_header("Accept", "text/event-stream")
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);
}
```

---

### 7.7 Update CLAUDE.md

**File**: `CLAUDE.md` (MODIFY)

Add documentation for new features:

```markdown
## New Features (Spider Migration)

### Spider-based Crawling
KIX now uses the spider crate for web crawling with:
- Smart mode: HTTP first, JS fallback
- HTTP caching (ETag/Last-Modified)
- Sitemap and robots.txt support
- Rate limiting

### Code Extraction (30+ Patterns)
Framework-aware code extraction from HTML:
- Docusaurus, MkDocs, Sphinx, Hugo
- PrismJS, Highlight.js, Shiki
- GitHub, GitLab, Bitbucket
- ReadTheDocs, Gitbook, VuePress

### Language Detection
20+ languages with aliases:
- rust/rs → Rust
- python/py → Python
- javascript/js → JavaScript
- etc.

### Tree-sitter Chunking
AST-aware chunking for source files:
- 21 supported languages
- Symbol extraction (functions, classes, structs)
- Semantic boundaries respected

### Ollama Embeddings
- Model: nomic-embed-text
- Dimensions: 768
- Max tokens: 8192
- GPU auto-detection

### New API Endpoints
- GET /api/indexing/patterns - List extraction patterns
- GET /api/indexing/languages - List supported languages
- GET /api/indexing/jobs/:id/code-stats - Code extraction stats
- GET /api/indexing/jobs/:id/code-blocks - Browse code blocks

### New SSE Events
- code_extraction: Per-page code extraction results
- Enhanced job_completed with code metrics
```

---

### 7.8 Create Test Fixtures

**Directory**: `server/crates/kix-parser/tests/fixtures/`

**File**: `sample.rs`
```rust
//! Sample Rust file for testing

/// A greeting function
pub fn greet(name: &str) {
    println!("Hello, {}!", name);
}

/// A person struct
pub struct Person {
    name: String,
    age: u32,
}

impl Person {
    /// Create a new person
    pub fn new(name: &str, age: u32) -> Self {
        Self {
            name: name.to_string(),
            age,
        }
    }

    /// Get the person's name
    pub fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_greet() {
        greet("World");
    }
}
```

**File**: `sample.py`
```python
"""Sample Python file for testing"""

def greet(name: str) -> None:
    """Greet someone by name."""
    print(f"Hello, {name}!")


class Person:
    """A person class."""

    def __init__(self, name: str, age: int):
        """Initialize a person."""
        self.name = name
        self.age = age

    def say_hello(self) -> None:
        """Make the person say hello."""
        greet(self.name)


if __name__ == "__main__":
    person = Person("Alice", 30)
    person.say_hello()
```

---

### 7.9 Performance Benchmarks

**File**: `server/crates/kix-jobs/benches/extraction_bench.rs` (NEW)

```rust
//! Benchmarks for code extraction

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use kix_jobs::extraction::{CodeExtractor, ExtractionConfig};

fn bench_code_extraction(c: &mut Criterion) {
    let extractor = CodeExtractor::new(ExtractionConfig::default());

    // Large HTML with multiple code blocks
    let html = include_str!("../tests/fixtures/large_doc_page.html");

    c.bench_function("extract_code_blocks", |b| {
        b.iter(|| {
            let result = extractor.extract(black_box(html));
            black_box(result)
        })
    });
}

fn bench_language_detection(c: &mut Criterion) {
    use kix_jobs::extraction::Language;

    let hints = vec![
        "rust", "python", "javascript", "typescript", "go",
        "java", "c++", "c#", "ruby", "php",
    ];

    c.bench_function("detect_language", |b| {
        b.iter(|| {
            for hint in &hints {
                let _ = Language::from_hint(black_box(hint));
            }
        })
    });
}

criterion_group!(benches, bench_code_extraction, bench_language_detection);
criterion_main!(benches);
```

---

## Deliverables

| Deliverable | File | Description |
|-------------|------|-------------|
| Spider tests | `kix-jobs/tests/spider_integration.rs` | Crawling tests |
| Extraction tests | `kix-jobs/tests/code_extraction.rs` | Pattern tests |
| Embedding tests | `kix-embeddings/tests/ollama_integration.rs` | Ollama tests |
| Tree-sitter tests | `kix-parser/tests/treesitter_integration.rs` | Chunking tests |
| E2E tests | `kix-jobs/tests/pipeline_e2e.rs` | Full pipeline |
| API tests | `kix-api/tests/api_integration.rs` | Endpoint tests |
| CLAUDE.md | `CLAUDE.md` | Updated docs |
| Fixtures | `tests/fixtures/*` | Test files |
| Benchmarks | `benches/*.rs` | Performance tests |

---

## Test Commands

```bash
# Run all tests
cargo test --release --workspace

# Run specific test suites
cargo test -p kix-jobs --test spider_integration --release
cargo test -p kix-jobs --test code_extraction --release
cargo test -p kix-parser --test treesitter_integration --release
cargo test -p kix-api --test api_integration --release

# Run tests requiring Ollama
cargo test --release --workspace -- --ignored

# Run benchmarks
cargo bench -p kix-jobs

# Client tests
cd client && npm test

# Full E2E test
./scripts/e2e-test.sh
```

---

## Exit Criteria

- [ ] All unit tests pass
- [ ] All integration tests pass
- [ ] E2E pipeline test works
- [ ] API tests pass
- [ ] Client builds without errors
- [ ] CLAUDE.md updated with new features
- [ ] Performance benchmarks run successfully
- [ ] No regressions in existing functionality

---

## Final Checklist

Before marking migration complete:

- [ ] Spider crawls documentation sites
- [ ] 30+ code extraction patterns work
- [ ] Language detection accurate
- [ ] Tree-sitter parses source files
- [ ] Ollama embeddings working
- [ ] UI shows code metrics
- [ ] MCP tools functional
- [ ] All tests pass
- [ ] Documentation current
- [ ] Performance acceptable

---

## Migration Complete

Congratulations! The spider-based architecture migration is complete.

**Summary of Changes**:
1. Replaced custom crawler with spider crate
2. Added CodeExtractor with 30+ patterns
3. Migrated embeddings to Ollama
4. Added tree-sitter for source file parsing
5. Enhanced API with code extraction endpoints
6. Updated UI with code extraction visibility
7. Comprehensive test coverage

**Next Steps**:
- Monitor production performance
- Add more extraction patterns as needed
- Tune validation thresholds based on feedback
- Consider caching optimizations
