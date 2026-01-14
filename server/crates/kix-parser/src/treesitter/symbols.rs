//! Symbol types for tree-sitter parsing.

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
    /// Implementation block
    Impl,
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
            Self::Impl => "impl",
        }
    }
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
        self.line_range.end.saturating_sub(self.line_range.start)
    }

    /// Get byte count
    pub fn byte_count(&self) -> usize {
        self.byte_range.end.saturating_sub(self.byte_range.start)
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

    /// Get total line count
    pub fn line_count(&self) -> usize {
        self.line_range.end.saturating_sub(self.line_range.start)
    }

    /// Check if chunk contains any functions
    pub fn has_functions(&self) -> bool {
        self.symbols.iter().any(|s| s.kind == SymbolKind::Function)
    }

    /// Check if chunk contains any types (struct, class, enum)
    pub fn has_types(&self) -> bool {
        self.symbols.iter().any(|s| matches!(s.kind, SymbolKind::Struct | SymbolKind::Class | SymbolKind::Enum))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_symbol_kind_str() {
        assert_eq!(SymbolKind::Function.as_str(), "function");
        assert_eq!(SymbolKind::Class.as_str(), "class");
        assert_eq!(SymbolKind::Struct.as_str(), "struct");
        assert_eq!(SymbolKind::Impl.as_str(), "impl");
    }

    #[test]
    fn test_code_chunk_title() {
        let chunk = CodeChunk {
            content: "fn test() {}".to_string(),
            file_path: "src/lib.rs".to_string(),
            primary_symbol: Some(Symbol {
                name: "test".to_string(),
                kind: SymbolKind::Function,
                byte_range: 0..12,
                line_range: 1..1,
                source: "fn test() {}".to_string(),
                parent: None,
                doc_comment: None,
                visibility: None,
                signature: None,
            }),
            symbols: vec![],
            line_range: 1..2,
            chunk_index: 0,
        };

        assert_eq!(chunk.title(), "test (function)");
    }

    #[test]
    fn test_code_chunk_no_symbol() {
        let chunk = CodeChunk {
            content: "// comment".to_string(),
            file_path: "src/lib.rs".to_string(),
            primary_symbol: None,
            symbols: vec![],
            line_range: 10..15,
            chunk_index: 0,
        };

        assert_eq!(chunk.title(), "src/lib.rs:10-15");
    }
}
