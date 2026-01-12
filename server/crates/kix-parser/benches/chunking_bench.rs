//! Performance benchmarks for the smart chunking algorithm
//!
//! Run with: cargo bench -p kix-parser

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use kix_parser::{SmartChunker, ChunkerConfig};

/// Generate sample content of varying sizes
fn generate_content(size: usize) -> String {
    let paragraph = "This is a sample paragraph of text that contains multiple sentences. \
        Each sentence has different content to simulate real documentation. \
        The algorithm should handle this efficiently.\n\n";

    let code_block = "```rust\n\
        fn example_function() {\n\
            let x = 5;\n\
            let y = 10;\n\
            println!(\"Sum: {}\", x + y);\n\
        }\n\
        ```\n\n";

    let mut content = String::with_capacity(size);
    let mut current_size = 0;
    let mut use_code = false;

    while current_size < size {
        if use_code {
            content.push_str(code_block);
            current_size += code_block.len();
        } else {
            content.push_str(paragraph);
            current_size += paragraph.len();
        }
        use_code = !use_code;
    }

    content
}

fn bench_smart_chunking(c: &mut Criterion) {
    let mut group = c.benchmark_group("smart_chunking");

    // Test different document sizes
    for size in [1_000, 5_000, 10_000, 50_000, 100_000].iter() {
        let content = generate_content(*size);
        let chunker = SmartChunker::new();

        group.bench_with_input(
            BenchmarkId::new("chunk", size),
            &content,
            |b, content| {
                b.iter(|| {
                    chunker.chunk(black_box(content))
                })
            },
        );
    }

    group.finish();
}

fn bench_chunker_configs(c: &mut Criterion) {
    let mut group = c.benchmark_group("chunker_configs");
    let content = generate_content(50_000);

    // Default config (5000 char chunks)
    let default_chunker = SmartChunker::new();
    group.bench_function("default_5000", |b| {
        b.iter(|| default_chunker.chunk(black_box(&content)))
    });

    // Smaller chunks (1000 chars)
    let small_chunker = SmartChunker::with_config(ChunkerConfig {
        chunk_size: 1000,
        min_chunk_size: 100,
        break_threshold: 0.3,
        preserve_code_blocks: true,
    });
    group.bench_function("small_1000", |b| {
        b.iter(|| small_chunker.chunk(black_box(&content)))
    });

    // Larger chunks (10000 chars)
    let large_chunker = SmartChunker::with_config(ChunkerConfig {
        chunk_size: 10000,
        min_chunk_size: 500,
        break_threshold: 0.3,
        preserve_code_blocks: true,
    });
    group.bench_function("large_10000", |b| {
        b.iter(|| large_chunker.chunk(black_box(&content)))
    });

    group.finish();
}

fn bench_code_heavy_content(c: &mut Criterion) {
    let mut group = c.benchmark_group("code_heavy");

    // Generate code-heavy content
    let code_content = r#"
# API Documentation

```python
def calculate_metrics(data):
    total = sum(data)
    average = total / len(data)
    return {
        'total': total,
        'average': average,
        'count': len(data)
    }
```

## Usage Examples

```javascript
const fetchData = async (url) => {
    const response = await fetch(url);
    const data = await response.json();
    return data.map(item => item.value);
};
```

```rust
impl<T: Clone> MessageQueue<T> {
    pub fn new() -> Self {
        Self { messages: VecDeque::new() }
    }

    pub fn send(&mut self, message: T) {
        self.messages.push_back(message);
    }

    pub fn receive(&mut self) -> Option<T> {
        self.messages.pop_front()
    }

    pub fn len(&self) -> usize {
        self.messages.len()
    }
}
```
"#.repeat(10);

    let chunker = SmartChunker::new();

    group.bench_function("code_heavy_content", |b| {
        b.iter(|| chunker.chunk(black_box(&code_content)))
    });

    group.finish();
}

criterion_group!(benches, bench_smart_chunking, bench_chunker_configs, bench_code_heavy_content);
criterion_main!(benches);
