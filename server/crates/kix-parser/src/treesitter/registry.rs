//! Language registry for tree-sitter parsing.

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
    Html,
    Css,
    Json,
    Bash,
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
            "html" | "htm" => Some(Self::Html),
            "css" | "scss" | "sass" => Some(Self::Css),
            "json" => Some(Self::Json),
            "sh" | "bash" | "zsh" => Some(Self::Bash),
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
            Self::Html => "HTML",
            Self::Css => "CSS",
            Self::Json => "JSON",
            Self::Bash => "Bash",
        }
    }

    /// Get file extensions for this language
    pub fn extensions(&self) -> &'static [&'static str] {
        match self {
            Self::Rust => &["rs"],
            Self::Python => &["py", "pyw", "pyi"],
            Self::JavaScript => &["js", "mjs", "cjs", "jsx"],
            Self::TypeScript => &["ts", "tsx"],
            Self::Go => &["go"],
            Self::Java => &["java"],
            Self::C => &["c", "h"],
            Self::Cpp => &["cpp", "cc", "cxx", "hpp", "hxx"],
            Self::CSharp => &["cs"],
            Self::Ruby => &["rb", "rake"],
            Self::Html => &["html", "htm"],
            Self::Css => &["css", "scss", "sass"],
            Self::Json => &["json"],
            Self::Bash => &["sh", "bash", "zsh"],
        }
    }

    /// Get all supported languages
    pub fn all() -> &'static [Self] {
        &[
            Self::Rust,
            Self::Python,
            Self::JavaScript,
            Self::TypeScript,
            Self::Go,
            Self::Java,
            Self::C,
            Self::Cpp,
            Self::CSharp,
            Self::Ruby,
            Self::Html,
            Self::Css,
            Self::Json,
            Self::Bash,
        ]
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

        languages.insert(SourceLanguage::Rust, tree_sitter_rust::LANGUAGE.into());
        languages.insert(SourceLanguage::Python, tree_sitter_python::LANGUAGE.into());
        languages.insert(SourceLanguage::JavaScript, tree_sitter_javascript::LANGUAGE.into());
        languages.insert(SourceLanguage::TypeScript, tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into());
        languages.insert(SourceLanguage::Go, tree_sitter_go::LANGUAGE.into());
        languages.insert(SourceLanguage::Java, tree_sitter_java::LANGUAGE.into());
        languages.insert(SourceLanguage::C, tree_sitter_c::LANGUAGE.into());
        languages.insert(SourceLanguage::Cpp, tree_sitter_cpp::LANGUAGE.into());
        languages.insert(SourceLanguage::CSharp, tree_sitter_c_sharp::LANGUAGE.into());
        languages.insert(SourceLanguage::Ruby, tree_sitter_ruby::LANGUAGE.into());
        languages.insert(SourceLanguage::Html, tree_sitter_html::LANGUAGE.into());
        languages.insert(SourceLanguage::Css, tree_sitter_css::LANGUAGE.into());
        languages.insert(SourceLanguage::Json, tree_sitter_json::LANGUAGE.into());
        languages.insert(SourceLanguage::Bash, tree_sitter_bash::LANGUAGE.into());

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

    /// Check if a language is supported
    pub fn is_supported(&self, lang: SourceLanguage) -> bool {
        self.languages.contains_key(&lang)
    }
}

impl Default for LanguageRegistry {
    fn default() -> Self {
        Self::new()
    }
}

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

        for lang in SourceLanguage::all() {
            assert!(registry.is_supported(*lang), "Missing {:?}", lang);
        }
    }

    #[test]
    fn test_create_parser() {
        let registry = LanguageRegistry::new();

        let parser = registry.create_parser(SourceLanguage::Rust);
        assert!(parser.is_some(), "Should create Rust parser");

        let parser = registry.create_parser(SourceLanguage::Python);
        assert!(parser.is_some(), "Should create Python parser");
    }

    #[test]
    fn test_display_names() {
        assert_eq!(SourceLanguage::Rust.display_name(), "Rust");
        assert_eq!(SourceLanguage::Cpp.display_name(), "C++");
        assert_eq!(SourceLanguage::CSharp.display_name(), "C#");
    }
}
