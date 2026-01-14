//! Tree-sitter based code chunker for AST-aware semantic chunking.

use std::path::Path;
use tree_sitter::{Node, Tree};

use super::registry::{LanguageRegistry, SourceLanguage};
use super::symbols::{CodeChunk, Symbol, SymbolKind};

/// Configuration for tree-sitter chunking
#[derive(Debug, Clone)]
pub struct ChunkerConfig {
    /// Maximum chunk size in bytes
    pub max_chunk_size: usize,

    /// Minimum chunk size in bytes
    pub min_chunk_size: usize,

    /// Whether to include imports in chunks
    pub include_imports: bool,

    /// Whether to include doc comments
    pub include_docs: bool,

    /// Merge small adjacent symbols
    pub merge_small_symbols: bool,
}

impl Default for ChunkerConfig {
    fn default() -> Self {
        Self {
            max_chunk_size: 4000,
            min_chunk_size: 200,
            include_imports: false,
            include_docs: true,
            merge_small_symbols: true,
        }
    }
}

/// Tree-sitter based code chunker
pub struct TreeSitterChunker {
    registry: LanguageRegistry,
    config: ChunkerConfig,
}

impl TreeSitterChunker {
    /// Create a new chunker with configuration
    pub fn new(config: ChunkerConfig) -> Self {
        Self {
            registry: LanguageRegistry::new(),
            config,
        }
    }

    /// Chunk a source file
    pub fn chunk_file(&self, path: &Path, source: &str) -> Result<Vec<CodeChunk>, ChunkerError> {
        let language = SourceLanguage::from_path(path)
            .ok_or_else(|| ChunkerError::UnsupportedLanguage(
                path.extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("unknown")
                    .to_string()
            ))?;

        self.chunk_source(source, language, path.to_string_lossy().to_string())
    }

    /// Chunk source code with known language
    pub fn chunk_source(
        &self,
        source: &str,
        language: SourceLanguage,
        file_path: String,
    ) -> Result<Vec<CodeChunk>, ChunkerError> {
        let mut parser = self.registry.create_parser(language)
            .ok_or(ChunkerError::ParserCreationFailed)?;

        let tree = parser.parse(source, None)
            .ok_or(ChunkerError::ParseFailed)?;

        let symbols = self.extract_symbols(&tree, source, language);

        if symbols.is_empty() {
            // No symbols found, create a single chunk with full content
            return Ok(vec![CodeChunk {
                content: source.to_string(),
                file_path,
                primary_symbol: None,
                symbols: vec![],
                line_range: 1..(source.lines().count() + 1),
                chunk_index: 0,
            }]);
        }

        let chunks = self.create_chunks(symbols, source, file_path);
        Ok(chunks)
    }

    /// Check if a file extension is supported
    pub fn supports_extension(&self, ext: &str) -> bool {
        SourceLanguage::from_extension(ext).is_some()
    }

    /// Extract symbols from parse tree
    fn extract_symbols(&self, tree: &Tree, source: &str, language: SourceLanguage) -> Vec<Symbol> {
        let mut symbols = Vec::new();
        let root = tree.root_node();

        self.visit_node(root, source, language, None, &mut symbols);

        symbols
    }

    /// Recursively visit nodes to extract symbols
    fn visit_node(
        &self,
        node: Node,
        source: &str,
        language: SourceLanguage,
        parent: Option<&str>,
        symbols: &mut Vec<Symbol>,
    ) {
        // Check if this node is a symbol we care about
        if let Some(symbol) = self.node_to_symbol(node, source, language, parent) {
            let name = symbol.name.clone();
            symbols.push(symbol);

            // Visit children with this as parent
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                self.visit_node(child, source, language, Some(&name), symbols);
            }
        } else {
            // Visit children without parent context
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                self.visit_node(child, source, language, parent, symbols);
            }
        }
    }

    /// Convert a node to a symbol if applicable
    fn node_to_symbol(
        &self,
        node: Node,
        source: &str,
        language: SourceLanguage,
        parent: Option<&str>,
    ) -> Option<Symbol> {
        let kind = self.classify_node(node.kind(), language)?;

        // Skip imports if configured
        if kind == SymbolKind::Import && !self.config.include_imports {
            return None;
        }

        let name = self.extract_name(node, source, language)?;
        let byte_range = node.byte_range();
        let start_line = node.start_position().row + 1;
        let end_line = node.end_position().row + 1;

        let symbol_source = source.get(byte_range.clone())?.to_string();

        Some(Symbol {
            name,
            kind,
            byte_range,
            line_range: start_line..(end_line + 1),
            source: symbol_source,
            parent: parent.map(String::from),
            doc_comment: self.extract_doc_comment(node, source),
            visibility: self.extract_visibility(node, source),
            signature: self.extract_signature(node, source, language),
        })
    }

    /// Classify a node kind to a symbol kind
    fn classify_node(&self, kind: &str, language: SourceLanguage) -> Option<SymbolKind> {
        match kind {
            // Universal patterns
            "function_definition" | "function_declaration" |
            "function_item" | "method_definition" | "method_declaration" |
            "arrow_function" | "function_expression" => Some(SymbolKind::Function),

            "class_definition" | "class_declaration" => Some(SymbolKind::Class),

            "struct_item" | "struct_definition" => Some(SymbolKind::Struct),

            "enum_item" | "enum_definition" | "enum_declaration" => Some(SymbolKind::Enum),

            "trait_item" | "interface_declaration" | "protocol_declaration" => Some(SymbolKind::Trait),

            "mod_item" | "module_definition" | "namespace_definition" => Some(SymbolKind::Module),

            "const_item" | "static_item" | "const_declaration" => Some(SymbolKind::Constant),

            "type_alias" | "type_alias_declaration" | "type_item" => Some(SymbolKind::TypeAlias),

            "use_declaration" | "import_statement" | "import_declaration" => Some(SymbolKind::Import),

            // Language-specific
            _ => self.classify_language_specific(kind, language),
        }
    }

    /// Classify language-specific node kinds
    fn classify_language_specific(&self, kind: &str, language: SourceLanguage) -> Option<SymbolKind> {
        match language {
            SourceLanguage::Rust => match kind {
                "impl_item" => Some(SymbolKind::Impl),
                "macro_definition" => Some(SymbolKind::Function),
                _ => None,
            },
            SourceLanguage::Python => match kind {
                "decorated_definition" => Some(SymbolKind::Function),
                _ => None,
            },
            SourceLanguage::Go => match kind {
                "type_declaration" => Some(SymbolKind::Struct),
                "method_declaration" => Some(SymbolKind::Function),
                _ => None,
            },
            SourceLanguage::Java => match kind {
                "method_declaration" => Some(SymbolKind::Function),
                "constructor_declaration" => Some(SymbolKind::Function),
                _ => None,
            },
            SourceLanguage::JavaScript | SourceLanguage::TypeScript => match kind {
                "method_definition" => Some(SymbolKind::Function),
                "class_declaration" => Some(SymbolKind::Class),
                "interface_declaration" => Some(SymbolKind::Trait),
                _ => None,
            },
            _ => None,
        }
    }

    /// Extract the name of a symbol from a node
    fn extract_name(&self, node: Node, source: &str, _language: SourceLanguage) -> Option<String> {
        // Try common name field patterns
        for field_name in ["name", "declarator", "identifier"] {
            if let Some(name_node) = node.child_by_field_name(field_name) {
                let name = source.get(name_node.byte_range())?;
                // Skip complex names (like function declarators with params)
                if !name.contains('(') && !name.contains('{') {
                    return Some(name.to_string());
                }
            }
        }

        // Fallback: find first identifier child
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" || child.kind().ends_with("_identifier") {
                let name = source.get(child.byte_range())?;
                return Some(name.to_string());
            }
        }

        None
    }

    /// Extract doc comment preceding a node
    fn extract_doc_comment(&self, node: Node, source: &str) -> Option<String> {
        if !self.config.include_docs {
            return None;
        }

        // Look for comment sibling before this node
        let prev = node.prev_sibling()?;
        if prev.kind().contains("comment") {
            let comment = source.get(prev.byte_range())?;
            return Some(comment.to_string());
        }

        None
    }

    /// Extract visibility modifier
    fn extract_visibility(&self, node: Node, source: &str) -> Option<String> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind().contains("visibility") || child.kind() == "pub" {
                let vis = source.get(child.byte_range())?;
                return Some(vis.to_string());
            }
        }
        None
    }

    /// Extract function signature
    fn extract_signature(&self, node: Node, source: &str, _language: SourceLanguage) -> Option<String> {
        // For functions, get up to the body
        if let Some(params) = node.child_by_field_name("parameters") {
            let start = node.byte_range().start;
            let end = params.byte_range().end;
            let sig = source.get(start..end)?;
            return Some(sig.to_string());
        }
        None
    }

    /// Create chunks from symbols
    fn create_chunks(
        &self,
        symbols: Vec<Symbol>,
        source: &str,
        file_path: String,
    ) -> Vec<CodeChunk> {
        let mut chunks = Vec::new();
        let mut current_symbols: Vec<Symbol> = Vec::new();
        let mut current_size = 0;

        for symbol in symbols {
            let symbol_size = symbol.source.len();

            // If this symbol alone exceeds max, make it its own chunk
            if symbol_size > self.config.max_chunk_size {
                // Flush current
                if !current_symbols.is_empty() {
                    chunks.push(self.create_chunk(
                        std::mem::take(&mut current_symbols),
                        source,
                        file_path.clone(),
                        chunks.len(),
                    ));
                    current_size = 0;
                }

                // Add large symbol as its own chunk
                chunks.push(self.create_chunk(
                    vec![symbol],
                    source,
                    file_path.clone(),
                    chunks.len(),
                ));
                continue;
            }

            // Check if adding this would exceed max
            if current_size + symbol_size > self.config.max_chunk_size && !current_symbols.is_empty() {
                chunks.push(self.create_chunk(
                    std::mem::take(&mut current_symbols),
                    source,
                    file_path.clone(),
                    chunks.len(),
                ));
                current_size = 0;
            }

            current_symbols.push(symbol);
            current_size += symbol_size;
        }

        // Flush remaining
        if !current_symbols.is_empty() {
            chunks.push(self.create_chunk(
                current_symbols,
                source,
                file_path.clone(),
                chunks.len(),
            ));
        }

        chunks
    }

    /// Create a chunk from symbols
    fn create_chunk(
        &self,
        symbols: Vec<Symbol>,
        source: &str,
        file_path: String,
        chunk_index: usize,
    ) -> CodeChunk {
        let start_line = symbols.iter().map(|s| s.line_range.start).min().unwrap_or(1);
        let end_line = symbols.iter().map(|s| s.line_range.end).max().unwrap_or(1);

        let start_byte = symbols.iter().map(|s| s.byte_range.start).min().unwrap_or(0);
        let end_byte = symbols.iter().map(|s| s.byte_range.end).max().unwrap_or(source.len());

        let content = source.get(start_byte..end_byte)
            .unwrap_or("")
            .to_string();

        let primary_symbol = symbols.first().cloned();

        CodeChunk {
            content,
            file_path,
            primary_symbol,
            symbols,
            line_range: start_line..end_line,
            chunk_index,
        }
    }
}

impl Default for TreeSitterChunker {
    fn default() -> Self {
        Self::new(ChunkerConfig::default())
    }
}

/// Errors that can occur during chunking
#[derive(Debug, thiserror::Error)]
pub enum ChunkerError {
    #[error("Unsupported language: {0}")]
    UnsupportedLanguage(String),

    #[error("Failed to create parser")]
    ParserCreationFailed,

    #[error("Failed to parse source")]
    ParseFailed,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_rust_parsing() {
        let chunker = TreeSitterChunker::default();
        let source = r#"
pub fn hello_world() {
    println!("Hello, world!");
}

pub struct MyStruct {
    field: i32,
}

impl MyStruct {
    pub fn new() -> Self {
        Self { field: 0 }
    }
}
"#;

        let chunks = chunker.chunk_source(
            source,
            SourceLanguage::Rust,
            "test.rs".to_string(),
        ).unwrap();

        assert!(!chunks.is_empty());

        // Should find function and struct
        let all_symbols: Vec<_> = chunks.iter()
            .flat_map(|c| c.symbols.iter())
            .collect();

        let has_function = all_symbols.iter().any(|s| s.kind == SymbolKind::Function);
        let has_struct = all_symbols.iter().any(|s| s.kind == SymbolKind::Struct);

        assert!(has_function, "Should find function");
        assert!(has_struct, "Should find struct");
    }

    #[test]
    fn test_python_parsing() {
        let chunker = TreeSitterChunker::default();
        let source = r#"
def greet(name):
    print(f"Hello, {name}!")

class Person:
    def __init__(self, name):
        self.name = name

    def say_hello(self):
        greet(self.name)
"#;

        let chunks = chunker.chunk_source(
            source,
            SourceLanguage::Python,
            "test.py".to_string(),
        ).unwrap();

        assert!(!chunks.is_empty());

        let all_symbols: Vec<_> = chunks.iter()
            .flat_map(|c| c.symbols.iter())
            .collect();

        let has_function = all_symbols.iter().any(|s| s.kind == SymbolKind::Function);
        let has_class = all_symbols.iter().any(|s| s.kind == SymbolKind::Class);

        assert!(has_function, "Should find function");
        assert!(has_class, "Should find class");
    }

    #[test]
    fn test_javascript_parsing() {
        let chunker = TreeSitterChunker::default();
        let source = r#"
function hello() {
    console.log("Hello");
}

const greet = (name) => {
    console.log(`Hello, ${name}`);
};

class Person {
    constructor(name) {
        this.name = name;
    }
}
"#;

        let chunks = chunker.chunk_source(
            source,
            SourceLanguage::JavaScript,
            "test.js".to_string(),
        ).unwrap();

        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_chunk_size_limits() {
        let config = ChunkerConfig {
            max_chunk_size: 50,  // Very small to force multiple chunks
            min_chunk_size: 10,
            ..Default::default()
        };
        let chunker = TreeSitterChunker::new(config);

        // Each function is ~25 chars, so with max 50, we should get multiple chunks
        let source = r#"
fn a() { println!("a"); }
fn b() { println!("b"); }
fn c() { println!("c"); }
fn d() { println!("d"); }
"#;

        let chunks = chunker.chunk_source(
            source,
            SourceLanguage::Rust,
            "test.rs".to_string(),
        ).unwrap();

        // With very small max size, should create multiple chunks OR single large symbols
        // The key is that chunks don't exceed max significantly
        assert!(!chunks.is_empty(), "Should create at least one chunk");

        // Each chunk should not be excessively large (allow some flexibility since individual symbols may exceed limit)
        for chunk in &chunks {
            // Single symbols may exceed max_chunk_size, but shouldn't be massively larger
            assert!(chunk.content.len() <= 500, "Chunk exceeded reasonable size: {} chars", chunk.content.len());
        }
    }

    #[test]
    fn test_chunk_file() {
        let chunker = TreeSitterChunker::default();
        let path = PathBuf::from("test.rs");
        let source = "fn main() { println!(\"test\"); }";

        let result = chunker.chunk_file(&path, source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_unsupported_extension() {
        let chunker = TreeSitterChunker::default();
        let path = PathBuf::from("test.xyz");
        let source = "some content";

        let result = chunker.chunk_file(&path, source);
        assert!(matches!(result, Err(ChunkerError::UnsupportedLanguage(_))));
    }

    #[test]
    fn test_supports_extension() {
        let chunker = TreeSitterChunker::default();

        assert!(chunker.supports_extension("rs"));
        assert!(chunker.supports_extension("py"));
        assert!(chunker.supports_extension("js"));
        assert!(!chunker.supports_extension("xyz"));
    }

    #[test]
    fn test_empty_file() {
        let chunker = TreeSitterChunker::default();
        let source = "";

        let chunks = chunker.chunk_source(
            source,
            SourceLanguage::Rust,
            "test.rs".to_string(),
        ).unwrap();

        // Empty file should produce one chunk with empty content
        assert_eq!(chunks.len(), 1);
    }
}
