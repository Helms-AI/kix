# Phase 4: Tree-sitter Integration

**Duration**: 3-4 days
**Dependencies**: Phase 2
**Status**: Not Started

---

## Objective

Add tree-sitter-based parsing for source code files to enable AST-aware semantic chunking.

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    Tree-sitter Architecture                      │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Source File (e.g., main.rs, app.py)                            │
│         │                                                        │
│         ▼                                                        │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  LanguageRegistry                                        │    │
│  │  ├─ detect_language(path) → Language                    │    │
│  │  └─ get_parser(Language) → Parser                       │    │
│  └─────────────────────────────────────────────────────────┘    │
│                    │                                             │
│                    ▼                                             │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  TreeSitterChunker                                       │    │
│  │  ├─ parse(source) → Tree                                │    │
│  │  ├─ extract_symbols(tree) → Vec<Symbol>                 │    │
│  │  └─ chunk_by_symbols(symbols) → Vec<CodeChunk>          │    │
│  └─────────────────────────────────────────────────────────┘    │
│                    │                                             │
│         ┌─────────┴─────────┐                                   │
│         ▼                   ▼                                    │
│  ┌─────────────┐     ┌─────────────────┐                        │
│  │ Functions   │     │ Classes/Structs │                        │
│  │ Methods     │     │ Modules         │                        │
│  └─────────────┘     └─────────────────┘                        │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## Tasks

### 4.1 Add Dependencies

**File**: `server/Cargo.toml` (workspace)

```toml
[workspace.dependencies]
tree-sitter = "0.22"
tree-sitter-rust = "0.21"
tree-sitter-python = "0.21"
tree-sitter-javascript = "0.21"
tree-sitter-typescript = "0.21"
tree-sitter-go = "0.21"
tree-sitter-java = "0.21"
tree-sitter-c = "0.21"
tree-sitter-cpp = "0.21"
tree-sitter-c-sharp = "0.21"
tree-sitter-ruby = "0.21"
tree-sitter-php = "0.21"
tree-sitter-swift = "0.21"
tree-sitter-kotlin = "0.21"
tree-sitter-scala = "0.21"
tree-sitter-html = "0.21"
tree-sitter-css = "0.21"
tree-sitter-json = "0.21"
tree-sitter-yaml = "0.21"
tree-sitter-toml = "0.21"
tree-sitter-bash = "0.21"
tree-sitter-sql = "0.21"
```

**File**: `server/crates/kix-parser/Cargo.toml`

```toml
[dependencies]
tree-sitter = { workspace = true }
tree-sitter-rust = { workspace = true }
tree-sitter-python = { workspace = true }
tree-sitter-javascript = { workspace = true }
tree-sitter-typescript = { workspace = true }
tree-sitter-go = { workspace = true }
tree-sitter-java = { workspace = true }
tree-sitter-c = { workspace = true }
tree-sitter-cpp = { workspace = true }
tree-sitter-c-sharp = { workspace = true }
tree-sitter-ruby = { workspace = true }
tree-sitter-php = { workspace = true }
tree-sitter-swift = { workspace = true }
tree-sitter-kotlin = { workspace = true }
tree-sitter-scala = { workspace = true }
tree-sitter-html = { workspace = true }
tree-sitter-css = { workspace = true }
tree-sitter-json = { workspace = true }
tree-sitter-yaml = { workspace = true }
tree-sitter-toml = { workspace = true }
tree-sitter-bash = { workspace = true }
tree-sitter-sql = { workspace = true }
```

**Verification**:
```bash
cargo check -p kix-parser
```

---

### 4.2 Create Language Registry

**File**: `server/crates/kix-parser/src/treesitter/registry.rs` (NEW)

```rust
use std::collections::HashMap;
use std::path::Path;
use tree_sitter::Language;

/// Supported languages for tree-sitter parsing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceLanguage {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Go,
    Java,
    C,
    Cpp,
    CSharp,
    Ruby,
    Php,
    Swift,
    Kotlin,
    Scala,
    Html,
    Css,
    Json,
    Yaml,
    Toml,
    Bash,
    Sql,
}

impl SourceLanguage {
    /// Detect language from file extension
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "rs" => Some(Self::Rust),
            "py" | "pyw" | "pyi" => Some(Self::Python),
            "js" | "mjs" | "cjs" => Some(Self::JavaScript),
            "ts" | "tsx" => Some(Self::TypeScript),
            "jsx" => Some(Self::JavaScript),
            "go" => Some(Self::Go),
            "java" => Some(Self::Java),
            "c" | "h" => Some(Self::C),
            "cpp" | "cc" | "cxx" | "hpp" | "hxx" => Some(Self::Cpp),
            "cs" => Some(Self::CSharp),
            "rb" | "rake" => Some(Self::Ruby),
            "php" | "phtml" => Some(Self::Php),
            "swift" => Some(Self::Swift),
            "kt" | "kts" => Some(Self::Kotlin),
            "scala" | "sc" => Some(Self::Scala),
            "html" | "htm" => Some(Self::Html),
            "css" | "scss" | "sass" => Some(Self::Css),
            "json" => Some(Self::Json),
            "yaml" | "yml" => Some(Self::Yaml),
            "toml" => Some(Self::Toml),
            "sh" | "bash" | "zsh" => Some(Self::Bash),
            "sql" => Some(Self::Sql),
            _ => None,
        }
    }

    /// Detect language from file path
    pub fn from_path(path: &Path) -> Option<Self> {
        path.extension()
            .and_then(|ext| ext.to_str())
            .and_then(Self::from_extension)
    }

    /// Get display name
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Rust => "Rust",
            Self::Python => "Python",
            Self::JavaScript => "JavaScript",
            Self::TypeScript => "TypeScript",
            Self::Go => "Go",
            Self::Java => "Java",
            Self::C => "C",
            Self::Cpp => "C++",
            Self::CSharp => "C#",
            Self::Ruby => "Ruby",
            Self::Php => "PHP",
            Self::Swift => "Swift",
            Self::Kotlin => "Kotlin",
            Self::Scala => "Scala",
            Self::Html => "HTML",
            Self::Css => "CSS",
            Self::Json => "JSON",
            Self::Yaml => "YAML",
            Self::Toml => "TOML",
            Self::Bash => "Bash",
            Self::Sql => "SQL",
        }
    }
}

/// Registry for tree-sitter languages
pub struct LanguageRegistry {
    languages: HashMap<SourceLanguage, Language>,
}

impl LanguageRegistry {
    /// Create a new registry with all supported languages
    pub fn new() -> Self {
        let mut languages = HashMap::new();

        languages.insert(SourceLanguage::Rust, tree_sitter_rust::language());
        languages.insert(SourceLanguage::Python, tree_sitter_python::language());
        languages.insert(SourceLanguage::JavaScript, tree_sitter_javascript::language());
        languages.insert(SourceLanguage::TypeScript, tree_sitter_typescript::language_typescript());
        languages.insert(SourceLanguage::Go, tree_sitter_go::language());
        languages.insert(SourceLanguage::Java, tree_sitter_java::language());
        languages.insert(SourceLanguage::C, tree_sitter_c::language());
        languages.insert(SourceLanguage::Cpp, tree_sitter_cpp::language());
        languages.insert(SourceLanguage::CSharp, tree_sitter_c_sharp::language());
        languages.insert(SourceLanguage::Ruby, tree_sitter_ruby::language());
        languages.insert(SourceLanguage::Php, tree_sitter_php::language_php());
        languages.insert(SourceLanguage::Swift, tree_sitter_swift::language());
        languages.insert(SourceLanguage::Kotlin, tree_sitter_kotlin::language());
        languages.insert(SourceLanguage::Scala, tree_sitter_scala::language());
        languages.insert(SourceLanguage::Html, tree_sitter_html::language());
        languages.insert(SourceLanguage::Css, tree_sitter_css::language());
        languages.insert(SourceLanguage::Json, tree_sitter_json::language());
        languages.insert(SourceLanguage::Yaml, tree_sitter_yaml::language());
        languages.insert(SourceLanguage::Toml, tree_sitter_toml::language());
        languages.insert(SourceLanguage::Bash, tree_sitter_bash::language());
        languages.insert(SourceLanguage::Sql, tree_sitter_sql::language());

        Self { languages }
    }

    /// Get the tree-sitter language for a source language
    pub fn get(&self, lang: SourceLanguage) -> Option<&Language> {
        self.languages.get(&lang)
    }

    /// Create a parser for the given language
    pub fn create_parser(&self, lang: SourceLanguage) -> Option<tree_sitter::Parser> {
        let language = self.get(lang)?;
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(language).ok()?;
        Some(parser)
    }
}

impl Default for LanguageRegistry {
    fn default() -> Self {
        Self::new()
    }
}
```

---

### 4.3 Create Symbol Types

**File**: `server/crates/kix-parser/src/treesitter/symbols.rs` (NEW)

```rust
use std::ops::Range;

/// Type of code symbol
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    /// Function or method
    Function,
    /// Class definition
    Class,
    /// Struct definition
    Struct,
    /// Enum definition
    Enum,
    /// Trait or interface
    Trait,
    /// Module or namespace
    Module,
    /// Constant or static
    Constant,
    /// Type alias
    TypeAlias,
    /// Import statement
    Import,
    /// Comment block (doc comments)
    DocComment,
}

/// A symbol extracted from source code
#[derive(Debug, Clone)]
pub struct Symbol {
    /// Symbol name
    pub name: String,

    /// Kind of symbol
    pub kind: SymbolKind,

    /// Byte range in source
    pub byte_range: Range<usize>,

    /// Line range (1-indexed)
    pub line_range: Range<usize>,

    /// Full source text of the symbol
    pub source: String,

    /// Parent symbol name (for nested items)
    pub parent: Option<String>,

    /// Documentation comment if present
    pub doc_comment: Option<String>,

    /// Visibility (pub, private, etc.)
    pub visibility: Option<String>,

    /// Signature (for functions/methods)
    pub signature: Option<String>,
}

impl Symbol {
    /// Get the symbol's qualified name
    pub fn qualified_name(&self) -> String {
        match &self.parent {
            Some(parent) => format!("{}::{}", parent, self.name),
            None => self.name.clone(),
        }
    }

    /// Get line count
    pub fn line_count(&self) -> usize {
        self.line_range.end - self.line_range.start
    }
}

/// A chunk of code based on symbols
#[derive(Debug, Clone)]
pub struct CodeChunk {
    /// The chunk content
    pub content: String,

    /// Source file path
    pub file_path: String,

    /// Primary symbol in this chunk
    pub primary_symbol: Option<Symbol>,

    /// All symbols in this chunk
    pub symbols: Vec<Symbol>,

    /// Line range in original file
    pub line_range: Range<usize>,

    /// Chunk index within file
    pub chunk_index: usize,
}

impl CodeChunk {
    /// Get a descriptive title for the chunk
    pub fn title(&self) -> String {
        match &self.primary_symbol {
            Some(sym) => format!("{} ({})", sym.qualified_name(), sym.kind.as_str()),
            None => format!("{}:{}-{}", self.file_path, self.line_range.start, self.line_range.end),
        }
    }
}

impl SymbolKind {
    /// Get string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Class => "class",
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::Trait => "trait",
            Self::Module => "module",
            Self::Constant => "constant",
            Self::TypeAlias => "type",
            Self::Import => "import",
            Self::DocComment => "doc",
        }
    }
}
```

---

### 4.4 Create TreeSitterChunker

**File**: `server/crates/kix-parser/src/treesitter/chunker.rs` (NEW)

```rust
use std::path::Path;
use tree_sitter::{Node, Parser, Tree};

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
        let chunks = self.create_chunks(symbols, source, file_path);

        Ok(chunks)
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
            for child in node.children(&mut node.walk()) {
                self.visit_node(child, source, language, Some(&name), symbols);
            }
        } else {
            // Visit children without parent context
            for child in node.children(&mut node.walk()) {
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
            line_range: start_line..end_line,
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

            "type_alias" | "type_alias_declaration" => Some(SymbolKind::TypeAlias),

            "use_declaration" | "import_statement" | "import_declaration" => Some(SymbolKind::Import),

            // Language-specific
            _ => self.classify_language_specific(kind, language),
        }
    }

    /// Classify language-specific node kinds
    fn classify_language_specific(&self, kind: &str, language: SourceLanguage) -> Option<SymbolKind> {
        match language {
            SourceLanguage::Rust => match kind {
                "impl_item" => Some(SymbolKind::Trait),
                "macro_definition" => Some(SymbolKind::Function),
                _ => None,
            },
            SourceLanguage::Python => match kind {
                "decorated_definition" => Some(SymbolKind::Function),
                _ => None,
            },
            SourceLanguage::Go => match kind {
                "type_declaration" => Some(SymbolKind::Struct),
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
                return Some(name.to_string());
            }
        }

        // Fallback: find first identifier child
        for child in node.children(&mut node.walk()) {
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
        for child in node.children(&mut node.walk()) {
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

#[derive(Debug, thiserror::Error)]
pub enum ChunkerError {
    #[error("Unsupported language: {0}")]
    UnsupportedLanguage(String),

    #[error("Failed to create parser")]
    ParserCreationFailed,

    #[error("Failed to parse source")]
    ParseFailed,
}
```

---

### 4.5 Create Module Structure

**File**: `server/crates/kix-parser/src/treesitter/mod.rs` (NEW)

```rust
mod registry;
mod symbols;
mod chunker;

pub use registry::{LanguageRegistry, SourceLanguage};
pub use symbols::{Symbol, SymbolKind, CodeChunk};
pub use chunker::{TreeSitterChunker, ChunkerConfig, ChunkerError};
```

**Update**: `server/crates/kix-parser/src/lib.rs`

```rust
pub mod treesitter;

// Re-export for convenience
pub use treesitter::{TreeSitterChunker, SourceLanguage, CodeChunk};
```

---

### 4.6 Integration with Processor

**File**: `server/crates/kix-jobs/src/processor.rs` (MODIFY)

Add tree-sitter chunking for source files:

```rust
use kix_parser::treesitter::{TreeSitterChunker, SourceLanguage, ChunkerConfig};

impl ContentProcessor {
    /// Process a source code file with tree-sitter
    pub async fn process_source_file(
        &self,
        path: &Path,
        content: &str,
    ) -> Result<Vec<EntryChunk>, ProcessorError> {
        let chunker = TreeSitterChunker::new(ChunkerConfig::default());

        let code_chunks = chunker.chunk_file(path, content)
            .map_err(|e| ProcessorError::ChunkingError(e.to_string()))?;

        // Convert CodeChunks to EntryChunks
        let entry_chunks: Vec<EntryChunk> = code_chunks
            .into_iter()
            .map(|chunk| EntryChunk {
                content: chunk.content,
                chunk_index: chunk.chunk_index,
                metadata: ChunkMetadata {
                    line_range: Some(chunk.line_range),
                    primary_symbol: chunk.primary_symbol.map(|s| s.qualified_name()),
                    symbol_count: chunk.symbols.len(),
                },
            })
            .collect();

        Ok(entry_chunks)
    }

    /// Determine if a file should use tree-sitter chunking
    pub fn should_use_treesitter(&self, path: &Path) -> bool {
        SourceLanguage::from_path(path).is_some()
    }
}
```

---

### 4.7 Write Tests

**File**: `server/crates/kix-parser/src/treesitter/tests.rs` (NEW)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_language_detection() {
        assert_eq!(SourceLanguage::from_extension("rs"), Some(SourceLanguage::Rust));
        assert_eq!(SourceLanguage::from_extension("py"), Some(SourceLanguage::Python));
        assert_eq!(SourceLanguage::from_extension("js"), Some(SourceLanguage::JavaScript));
        assert_eq!(SourceLanguage::from_extension("ts"), Some(SourceLanguage::TypeScript));
        assert_eq!(SourceLanguage::from_extension("go"), Some(SourceLanguage::Go));
        assert_eq!(SourceLanguage::from_extension("unknown"), None);
    }

    #[test]
    fn test_language_from_path() {
        let path = PathBuf::from("src/main.rs");
        assert_eq!(SourceLanguage::from_path(&path), Some(SourceLanguage::Rust));

        let path = PathBuf::from("app.py");
        assert_eq!(SourceLanguage::from_path(&path), Some(SourceLanguage::Python));
    }

    #[test]
    fn test_registry_has_all_languages() {
        let registry = LanguageRegistry::new();

        for lang in [
            SourceLanguage::Rust,
            SourceLanguage::Python,
            SourceLanguage::JavaScript,
            SourceLanguage::TypeScript,
            SourceLanguage::Go,
        ] {
            assert!(registry.get(lang).is_some(), "Missing {:?}", lang);
        }
    }

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
    fn test_chunk_size_limits() {
        let config = ChunkerConfig {
            max_chunk_size: 100,
            min_chunk_size: 10,
            ..Default::default()
        };
        let chunker = TreeSitterChunker::new(config);

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

        // With small max size, should create multiple chunks
        assert!(chunks.len() > 1, "Should create multiple chunks with small max size");

        // Each chunk should respect max size (approximately)
        for chunk in &chunks {
            assert!(chunk.content.len() <= 200, "Chunk exceeded reasonable size");
        }
    }

    #[test]
    fn test_symbol_qualified_names() {
        let symbol = Symbol {
            name: "method".to_string(),
            kind: SymbolKind::Function,
            byte_range: 0..10,
            line_range: 1..5,
            source: "fn method() {}".to_string(),
            parent: Some("MyClass".to_string()),
            doc_comment: None,
            visibility: None,
            signature: None,
        };

        assert_eq!(symbol.qualified_name(), "MyClass::method");

        let top_level = Symbol {
            name: "standalone".to_string(),
            parent: None,
            ..symbol.clone()
        };

        assert_eq!(top_level.qualified_name(), "standalone");
    }
}
```

---

## Deliverables

| Deliverable | File | Description |
|-------------|------|-------------|
| LanguageRegistry | `treesitter/registry.rs` | Language detection and parser creation |
| Symbol types | `treesitter/symbols.rs` | Symbol and CodeChunk types |
| TreeSitterChunker | `treesitter/chunker.rs` | AST-aware chunking |
| Module exports | `treesitter/mod.rs` | Public API |
| Integration | `processor.rs` | Connected to job processor |
| Tests | `treesitter/tests.rs` | Unit tests |

---

## Exit Criteria

- [ ] `cargo check -p kix-parser` passes
- [ ] Language detection works for all 21 languages
- [ ] Rust source files parse correctly
- [ ] Python source files parse correctly
- [ ] JavaScript/TypeScript parse correctly
- [ ] Symbols extracted with names and kinds
- [ ] Chunking respects size limits
- [ ] Processor can use tree-sitter for source files
- [ ] All existing tests still pass

---

## Testing Commands

```bash
# Run tree-sitter tests
cargo test -p kix-parser treesitter --release

# Test specific language
cargo test -p kix-parser treesitter::tests::test_rust_parsing --release

# Manual verification with real files
cargo run --release -p kix-cli -- \
  index file ./src/main.rs \
  --chunker treesitter
```

---

## Supported Languages

| Language | Extension(s) | tree-sitter crate |
|----------|--------------|-------------------|
| Rust | .rs | tree-sitter-rust |
| Python | .py, .pyw, .pyi | tree-sitter-python |
| JavaScript | .js, .mjs, .cjs, .jsx | tree-sitter-javascript |
| TypeScript | .ts, .tsx | tree-sitter-typescript |
| Go | .go | tree-sitter-go |
| Java | .java | tree-sitter-java |
| C | .c, .h | tree-sitter-c |
| C++ | .cpp, .cc, .hpp | tree-sitter-cpp |
| C# | .cs | tree-sitter-c-sharp |
| Ruby | .rb, .rake | tree-sitter-ruby |
| PHP | .php, .phtml | tree-sitter-php |
| Swift | .swift | tree-sitter-swift |
| Kotlin | .kt, .kts | tree-sitter-kotlin |
| Scala | .scala, .sc | tree-sitter-scala |
| HTML | .html, .htm | tree-sitter-html |
| CSS | .css, .scss | tree-sitter-css |
| JSON | .json | tree-sitter-json |
| YAML | .yaml, .yml | tree-sitter-yaml |
| TOML | .toml | tree-sitter-toml |
| Bash | .sh, .bash, .zsh | tree-sitter-bash |
| SQL | .sql | tree-sitter-sql |

---

## Next Phase

Upon completion, proceed to [Phase 5: API & SSE Updates](./phase-5-api-sse-updates.md).
