# Phase 2: CodeExtractor Module

**Duration**: 2-3 days
**Dependencies**: Phase 1 (Spider Integration)
**Status**: Not Started

---

## Objective

Extract the framework-aware code extraction logic from `kix-crawler/src/code.rs` into a standalone `CodeExtractor` module that works with spider's raw HTML output.

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    CodeExtractor Module                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  spider::Page.get_html() ─────────┐                             │
│                                    ▼                             │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  CodeExtractor                                           │    │
│  │  ├─ patterns: Vec<(CodePattern, Selector)>              │    │
│  │  ├─ extract(&html) → Vec<CodeBlock>                     │    │
│  │  ├─ detect_language(&element) → Language                │    │
│  │  └─ validate(&code) → bool                              │    │
│  └─────────────────────────────────────────────────────────┘    │
│            │                                                     │
│            ▼                                                     │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  CodeBlock                                               │    │
│  │  ├─ content: String                                     │    │
│  │  ├─ language: Language                                  │    │
│  │  ├─ pattern: CodePattern                                │    │
│  │  ├─ hash: u64                                           │    │
│  │  └─ line_count: usize                                   │    │
│  └─────────────────────────────────────────────────────────┘    │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## Tasks

### 2.1 Create Module Structure

**Directory**: `server/crates/kix-jobs/src/extraction/`

```
extraction/
├── mod.rs              # Module exports
├── code_extractor.rs   # Main extractor
├── patterns.rs         # CodePattern enum + CSS selectors
├── language.rs         # Language enum + detection
└── validation.rs       # Code validation logic
```

---

### 2.2 Create Language Enum

**File**: `server/crates/kix-jobs/src/extraction/language.rs`

```rust
use std::fmt;

/// Supported programming languages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Go,
    Java,
    CSharp,
    Cpp,
    C,
    Ruby,
    Php,
    Swift,
    Kotlin,
    Sql,
    Shell,
    Yaml,
    Json,
    Toml,
    Markdown,
    Html,
    Css,
    Other,
}

impl Language {
    /// Detect language from a string hint
    pub fn from_hint(hint: &str) -> Self {
        match hint.to_lowercase().as_str() {
            // Rust
            "rust" | "rs" => Self::Rust,

            // Python
            "python" | "py" | "python3" => Self::Python,

            // JavaScript
            "javascript" | "js" | "jsx" | "mjs" => Self::JavaScript,

            // TypeScript
            "typescript" | "ts" | "tsx" => Self::TypeScript,

            // Go
            "go" | "golang" => Self::Go,

            // Java
            "java" => Self::Java,

            // C#
            "csharp" | "cs" | "c#" | "dotnet" => Self::CSharp,

            // C++
            "cpp" | "c++" | "cxx" | "cc" => Self::Cpp,

            // C
            "c" => Self::C,

            // Ruby
            "ruby" | "rb" => Self::Ruby,

            // PHP
            "php" => Self::Php,

            // Swift
            "swift" => Self::Swift,

            // Kotlin
            "kotlin" | "kt" => Self::Kotlin,

            // SQL
            "sql" | "mysql" | "postgresql" | "postgres" | "sqlite" => Self::Sql,

            // Shell
            "shell" | "bash" | "sh" | "zsh" | "fish" | "powershell" | "ps1" => Self::Shell,

            // Data formats
            "yaml" | "yml" => Self::Yaml,
            "json" | "jsonc" => Self::Json,
            "toml" => Self::Toml,

            // Markup
            "markdown" | "md" => Self::Markdown,
            "html" | "htm" | "xhtml" => Self::Html,
            "css" | "scss" | "sass" | "less" => Self::Css,

            // Default
            _ => Self::Other,
        }
    }

    /// Get file extension for this language
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Rust => "rs",
            Self::Python => "py",
            Self::JavaScript => "js",
            Self::TypeScript => "ts",
            Self::Go => "go",
            Self::Java => "java",
            Self::CSharp => "cs",
            Self::Cpp => "cpp",
            Self::C => "c",
            Self::Ruby => "rb",
            Self::Php => "php",
            Self::Swift => "swift",
            Self::Kotlin => "kt",
            Self::Sql => "sql",
            Self::Shell => "sh",
            Self::Yaml => "yaml",
            Self::Json => "json",
            Self::Toml => "toml",
            Self::Markdown => "md",
            Self::Html => "html",
            Self::Css => "css",
            Self::Other => "txt",
        }
    }
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rust => write!(f, "Rust"),
            Self::Python => write!(f, "Python"),
            Self::JavaScript => write!(f, "JavaScript"),
            Self::TypeScript => write!(f, "TypeScript"),
            Self::Go => write!(f, "Go"),
            Self::Java => write!(f, "Java"),
            Self::CSharp => write!(f, "C#"),
            Self::Cpp => write!(f, "C++"),
            Self::C => write!(f, "C"),
            Self::Ruby => write!(f, "Ruby"),
            Self::Php => write!(f, "PHP"),
            Self::Swift => write!(f, "Swift"),
            Self::Kotlin => write!(f, "Kotlin"),
            Self::Sql => write!(f, "SQL"),
            Self::Shell => write!(f, "Shell"),
            Self::Yaml => write!(f, "YAML"),
            Self::Json => write!(f, "JSON"),
            Self::Toml => write!(f, "TOML"),
            Self::Markdown => write!(f, "Markdown"),
            Self::Html => write!(f, "HTML"),
            Self::Css => write!(f, "CSS"),
            Self::Other => write!(f, "Other"),
        }
    }
}
```

---

### 2.3 Create CodePattern Enum

**File**: `server/crates/kix-jobs/src/extraction/patterns.rs`

```rust
/// Code extraction patterns (30+)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CodePattern {
    // Documentation frameworks
    DocusaurusCodeBlock,
    DocusaurusTabCodeBlock,
    MkDocsCodeBlock,
    SphinxCodeBlock,
    ReadTheDocsCode,
    JekyllHighlight,
    HugoHighlight,
    VuePressCode,
    GatsbyCode,
    NextjsRehype,
    AstroCode,

    // Syntax highlighters
    PrismJs,
    HighlightJs,
    SyntaxHighlighter,
    RougeSyntax,
    Shiki,

    // Platforms
    GitHubCode,
    GitLabCode,
    BitbucketCode,
    StackOverflowCode,

    // Editors
    MonacoEditor,
    CodeMirror,
    AceEditor,

    // Terminal
    TerminalOutput,
    AsciinemaPlayer,

    // Generic
    DataLanguageAttr,
    ClassPrefixCode,
    DataCodeAttr,
    PreCode,
    PreOnly,
    CodeOnly,
}

impl CodePattern {
    /// Get CSS selector for this pattern
    pub fn selector(&self) -> &'static str {
        match self {
            // Documentation frameworks
            Self::DocusaurusCodeBlock => ".prism-code, [class*='codeBlockContent']",
            Self::DocusaurusTabCodeBlock => ".tabs-container pre code",
            Self::MkDocsCodeBlock => ".highlight pre, .codehilite pre",
            Self::SphinxCodeBlock => ".highlight-python pre, .highlight-default pre, .highlight pre",
            Self::ReadTheDocsCode => ".rst-content pre",
            Self::JekyllHighlight => ".highlighter-rouge pre, .highlight pre.highlight",
            Self::HugoHighlight => ".highlight pre, .chroma pre",
            Self::VuePressCode => "div[class*='language-'] pre",
            Self::GatsbyCode => ".gatsby-highlight pre",
            Self::NextjsRehype => "[data-rehype-pretty-code] code",
            Self::AstroCode => ".astro-code pre",

            // Syntax highlighters
            Self::PrismJs => "[class*='language-'] code, pre[class*='language-']",
            Self::HighlightJs => ".hljs, pre code.hljs",
            Self::SyntaxHighlighter => ".syntaxhighlighter",
            Self::RougeSyntax => ".rouge pre, .rouge-code",
            Self::Shiki => ".shiki code, pre.shiki",

            // Platforms
            Self::GitHubCode => ".blob-code-content, .highlight pre, .js-file-line",
            Self::GitLabCode => ".blob-content pre, .code pre",
            Self::BitbucketCode => ".code-container pre",
            Self::StackOverflowCode => ".s-prose pre, .s-code-block, .post-text pre",

            // Editors
            Self::MonacoEditor => ".monaco-editor .view-lines",
            Self::CodeMirror => ".CodeMirror-code, .cm-content",
            Self::AceEditor => ".ace_editor .ace_content",

            // Terminal
            Self::TerminalOutput => ".terminal pre, .console pre, .shell pre",
            Self::AsciinemaPlayer => ".asciinema-player pre",

            // Generic
            Self::DataLanguageAttr => "[data-language] code, [data-lang] code",
            Self::ClassPrefixCode => "[class*='code-'] pre, [class*='snippet'] pre",
            Self::DataCodeAttr => "[data-code]",
            Self::PreCode => "pre code",
            Self::PreOnly => "pre:not(:has(code))",
            Self::CodeOnly => "code:not(pre code)",
        }
    }

    /// Get all patterns in priority order
    pub fn all() -> &'static [CodePattern] {
        &[
            // Platform-specific first (most specific)
            Self::GitHubCode,
            Self::GitLabCode,
            Self::BitbucketCode,
            Self::StackOverflowCode,

            // Documentation frameworks
            Self::DocusaurusCodeBlock,
            Self::DocusaurusTabCodeBlock,
            Self::MkDocsCodeBlock,
            Self::SphinxCodeBlock,
            Self::ReadTheDocsCode,
            Self::JekyllHighlight,
            Self::HugoHighlight,
            Self::VuePressCode,
            Self::GatsbyCode,
            Self::NextjsRehype,
            Self::AstroCode,

            // Syntax highlighters
            Self::PrismJs,
            Self::HighlightJs,
            Self::SyntaxHighlighter,
            Self::RougeSyntax,
            Self::Shiki,

            // Editors
            Self::MonacoEditor,
            Self::CodeMirror,
            Self::AceEditor,

            // Terminal
            Self::TerminalOutput,
            Self::AsciinemaPlayer,

            // Generic (fallback)
            Self::DataLanguageAttr,
            Self::ClassPrefixCode,
            Self::DataCodeAttr,
            Self::PreCode,
            Self::PreOnly,
            Self::CodeOnly,
        ]
    }

    /// Get description of this pattern
    pub fn description(&self) -> &'static str {
        match self {
            Self::DocusaurusCodeBlock => "Docusaurus code block",
            Self::DocusaurusTabCodeBlock => "Docusaurus tabbed code",
            Self::MkDocsCodeBlock => "MkDocs code block",
            Self::SphinxCodeBlock => "Sphinx documentation",
            Self::ReadTheDocsCode => "ReadTheDocs content",
            Self::JekyllHighlight => "Jekyll highlight",
            Self::HugoHighlight => "Hugo highlight",
            Self::VuePressCode => "VuePress code block",
            Self::GatsbyCode => "Gatsby highlight",
            Self::NextjsRehype => "Next.js rehype code",
            Self::AstroCode => "Astro code block",
            Self::PrismJs => "Prism.js",
            Self::HighlightJs => "Highlight.js",
            Self::SyntaxHighlighter => "SyntaxHighlighter",
            Self::RougeSyntax => "Rouge syntax",
            Self::Shiki => "Shiki",
            Self::GitHubCode => "GitHub code view",
            Self::GitLabCode => "GitLab code view",
            Self::BitbucketCode => "Bitbucket code view",
            Self::StackOverflowCode => "Stack Overflow",
            Self::MonacoEditor => "Monaco Editor",
            Self::CodeMirror => "CodeMirror",
            Self::AceEditor => "Ace Editor",
            Self::TerminalOutput => "Terminal output",
            Self::AsciinemaPlayer => "Asciinema",
            Self::DataLanguageAttr => "data-language attribute",
            Self::ClassPrefixCode => "code class prefix",
            Self::DataCodeAttr => "data-code attribute",
            Self::PreCode => "pre > code",
            Self::PreOnly => "pre only",
            Self::CodeOnly => "code only",
        }
    }
}
```

---

### 2.4 Create Validation Module

**File**: `server/crates/kix-jobs/src/extraction/validation.rs`

```rust
use super::CodeBlock;

/// Configuration for code validation
#[derive(Debug, Clone)]
pub struct ValidationConfig {
    /// Minimum code length in characters
    pub min_length: usize,

    /// Maximum prose ratio (0.0-1.0)
    pub max_prose_ratio: f32,

    /// Filter placeholder text
    pub filter_placeholders: bool,
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            min_length: 10,
            max_prose_ratio: 0.6,
            filter_placeholders: true,
        }
    }
}

/// Validation statistics
#[derive(Debug, Default, Clone)]
pub struct ValidationStats {
    pub passed: usize,
    pub filtered_too_short: usize,
    pub filtered_high_prose: usize,
    pub filtered_placeholder: usize,
    pub filtered_no_structure: usize,
}

impl ValidationStats {
    pub fn total_filtered(&self) -> usize {
        self.filtered_too_short
            + self.filtered_high_prose
            + self.filtered_placeholder
            + self.filtered_no_structure
    }
}

/// Validate code content
pub fn validate_code(content: &str, config: &ValidationConfig) -> ValidationResult {
    // Length check
    if content.len() < config.min_length {
        return ValidationResult::TooShort;
    }

    // Placeholder check
    if config.filter_placeholders && is_placeholder(content) {
        return ValidationResult::Placeholder;
    }

    // Code structure check
    if !has_code_structure(content) {
        return ValidationResult::NoStructure;
    }

    // Prose ratio check
    let prose_ratio = calculate_prose_ratio(content);
    if prose_ratio > config.max_prose_ratio {
        return ValidationResult::HighProseRatio(prose_ratio);
    }

    ValidationResult::Valid
}

#[derive(Debug, Clone)]
pub enum ValidationResult {
    Valid,
    TooShort,
    Placeholder,
    NoStructure,
    HighProseRatio(f32),
}

impl ValidationResult {
    pub fn is_valid(&self) -> bool {
        matches!(self, Self::Valid)
    }
}

fn is_placeholder(content: &str) -> bool {
    let lower = content.to_lowercase();
    let placeholders = [
        "loading",
        "please wait",
        "...",
        "copy",
        "copied",
        "click to copy",
    ];
    placeholders.iter().any(|p| lower.trim() == *p)
}

fn has_code_structure(content: &str) -> bool {
    // Check for code-like characters
    let code_chars = ['{', '}', '[', ']', '(', ')', ';', '=', '<', '>', ':'];
    let code_char_count = content.chars().filter(|c| code_chars.contains(c)).count();

    // At least 2% code characters
    if !content.is_empty() && code_char_count > content.len() / 50 {
        return true;
    }

    // Or contains common code patterns
    let patterns = [
        "function ", "def ", "class ", "const ", "let ", "var ",
        "fn ", "pub ", "impl ", "struct ", "enum ", "use ",
        "import ", "from ", "require(", "module.exports",
        "async ", "await ", "return ", "if ", "for ", "while ",
        "package ", "public ", "private ", "protected ",
        "func ", "type ", "interface ",
    ];

    patterns.iter().any(|p| content.contains(p))
}

fn calculate_prose_ratio(content: &str) -> f32 {
    let words: Vec<&str> = content.split_whitespace().collect();
    if words.is_empty() {
        return 0.0;
    }

    // Count "prose-like" words (common English words)
    let prose_words = [
        "the", "a", "an", "is", "are", "was", "were", "be", "been",
        "this", "that", "these", "those", "it", "its",
        "you", "your", "we", "our", "they", "their",
        "can", "will", "would", "could", "should", "may", "might",
        "to", "of", "in", "for", "on", "with", "at", "by", "from",
        "and", "or", "but", "not", "so", "if", "then", "else",
    ];

    let prose_count = words.iter()
        .filter(|w| prose_words.contains(&w.to_lowercase().as_str()))
        .count();

    prose_count as f32 / words.len() as f32
}
```

---

### 2.5 Create Main CodeExtractor

**File**: `server/crates/kix-jobs/src/extraction/code_extractor.rs`

```rust
use scraper::{Html, Selector, ElementRef};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

use super::language::Language;
use super::patterns::CodePattern;
use super::validation::{validate_code, ValidationConfig, ValidationResult, ValidationStats};

/// Extracted code block
#[derive(Debug, Clone)]
pub struct CodeBlock {
    /// Code content
    pub content: String,

    /// Detected language
    pub language: Language,

    /// Pattern that matched
    pub pattern: CodePattern,

    /// Content hash for deduplication
    pub hash: u64,

    /// Line count
    pub line_count: usize,
}

/// Code extraction configuration
#[derive(Debug, Clone)]
pub struct CodeExtractionConfig {
    /// Validation configuration
    pub validation: ValidationConfig,

    /// Patterns to use (empty = all)
    pub patterns: Vec<CodePattern>,

    /// Enable tree-sitter validation
    pub validate_syntax: bool,
}

impl Default for CodeExtractionConfig {
    fn default() -> Self {
        Self {
            validation: ValidationConfig::default(),
            patterns: vec![],
            validate_syntax: false,
        }
    }
}

/// Extraction result
#[derive(Debug, Clone)]
pub struct ExtractionResult {
    pub code_blocks: Vec<CodeBlock>,
    pub validation_stats: ValidationStats,
    pub language_counts: HashMap<Language, usize>,
    pub pattern_counts: HashMap<CodePattern, usize>,
}

/// Framework-aware code extractor
pub struct CodeExtractor {
    config: CodeExtractionConfig,
    selectors: Vec<(CodePattern, Selector)>,
}

impl CodeExtractor {
    pub fn new(config: CodeExtractionConfig) -> Self {
        let patterns = if config.patterns.is_empty() {
            CodePattern::all().to_vec()
        } else {
            config.patterns.clone()
        };

        let selectors = patterns
            .iter()
            .filter_map(|p| {
                Selector::parse(p.selector()).ok().map(|s| (*p, s))
            })
            .collect();

        Self { config, selectors }
    }

    /// Extract code blocks from HTML
    pub fn extract(&self, html: &str) -> ExtractionResult {
        let document = Html::parse_document(html);
        let mut code_blocks = Vec::new();
        let mut seen_hashes = HashSet::new();
        let mut stats = ValidationStats::default();
        let mut language_counts = HashMap::new();
        let mut pattern_counts = HashMap::new();

        for (pattern, selector) in &self.selectors {
            for element in document.select(selector) {
                let content = self.get_code_content(&element);
                if content.is_empty() {
                    continue;
                }

                // Validate
                let validation = validate_code(&content, &self.config.validation);
                match validation {
                    ValidationResult::Valid => stats.passed += 1,
                    ValidationResult::TooShort => {
                        stats.filtered_too_short += 1;
                        continue;
                    }
                    ValidationResult::Placeholder => {
                        stats.filtered_placeholder += 1;
                        continue;
                    }
                    ValidationResult::NoStructure => {
                        stats.filtered_no_structure += 1;
                        continue;
                    }
                    ValidationResult::HighProseRatio(_) => {
                        stats.filtered_high_prose += 1;
                        continue;
                    }
                }

                // Deduplicate
                let hash = self.hash_content(&content);
                if seen_hashes.contains(&hash) {
                    continue;
                }
                seen_hashes.insert(hash);

                // Detect language
                let language = self.detect_language(&element);

                // Track counts
                *language_counts.entry(language).or_insert(0) += 1;
                *pattern_counts.entry(*pattern).or_insert(0) += 1;

                // Create block
                let block = CodeBlock {
                    content: content.clone(),
                    language,
                    pattern: *pattern,
                    hash,
                    line_count: content.lines().count(),
                };

                code_blocks.push(block);
            }
        }

        ExtractionResult {
            code_blocks,
            validation_stats: stats,
            language_counts,
            pattern_counts,
        }
    }

    fn get_code_content(&self, element: &ElementRef) -> String {
        element.text()
            .collect::<String>()
            .lines()
            .map(|line| line.trim_end())
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_string()
    }

    fn detect_language(&self, element: &ElementRef) -> Language {
        // 1. Check class attribute
        if let Some(lang) = self.detect_from_class(element) {
            return lang;
        }

        // 2. Check data attributes
        if let Some(lang) = self.detect_from_data_attr(element) {
            return lang;
        }

        // 3. Check parent element
        if let Some(parent) = element.parent() {
            if let Some(parent_el) = parent.value().as_element() {
                // Check parent's class
                if let Some(classes) = parent_el.attr("class") {
                    if let Some(lang) = self.detect_from_class_str(classes) {
                        return lang;
                    }
                }

                // Check parent's data attributes
                for attr in ["data-language", "data-lang"] {
                    if let Some(hint) = parent_el.attr(attr) {
                        return Language::from_hint(hint);
                    }
                }
            }
        }

        Language::Other
    }

    fn detect_from_class(&self, element: &ElementRef) -> Option<Language> {
        element.value()
            .attr("class")
            .and_then(|classes| self.detect_from_class_str(classes))
    }

    fn detect_from_class_str(&self, classes: &str) -> Option<Language> {
        for class in classes.split_whitespace() {
            // language-X pattern
            if let Some(lang) = class.strip_prefix("language-") {
                return Some(Language::from_hint(lang));
            }
            if let Some(lang) = class.strip_prefix("lang-") {
                return Some(Language::from_hint(lang));
            }

            // highlight-source-X (GitHub)
            if let Some(lang) = class.strip_prefix("highlight-source-") {
                return Some(Language::from_hint(lang));
            }

            // Direct language names
            let lang = Language::from_hint(class);
            if lang != Language::Other {
                return Some(lang);
            }
        }
        None
    }

    fn detect_from_data_attr(&self, element: &ElementRef) -> Option<Language> {
        for attr in ["data-language", "data-lang", "data-code-language"] {
            if let Some(hint) = element.value().attr(attr) {
                return Some(Language::from_hint(hint));
            }
        }
        None
    }

    fn hash_content(&self, content: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        hasher.finish()
    }
}
```

---

### 2.6 Create Module Exports

**File**: `server/crates/kix-jobs/src/extraction/mod.rs`

```rust
mod code_extractor;
mod language;
mod patterns;
mod validation;

pub use code_extractor::{
    CodeExtractor,
    CodeExtractionConfig,
    CodeBlock,
    ExtractionResult,
};
pub use language::Language;
pub use patterns::CodePattern;
pub use validation::{ValidationConfig, ValidationStats, ValidationResult};
```

---

### 2.7 Integrate with Spider Crawler

**File**: `server/crates/kix-jobs/src/crawler/spider_adapter.rs` (MODIFY)

```rust
use crate::extraction::{CodeExtractor, CodeExtractionConfig, ExtractionResult};

pub struct CrawledPage {
    pub url: String,
    pub html: String,
    pub markdown: String,
    pub status: u16,
    pub title: Option<String>,
    pub code_extraction: Option<ExtractionResult>,  // NEW
}

impl SpiderCrawler {
    fn process_page_sync(&self, page: &Page) -> Result<CrawledPage, CrawlError> {
        // ... existing code ...

        // Extract code blocks (NEW)
        let extractor = CodeExtractor::new(CodeExtractionConfig::default());
        let code_extraction = Some(extractor.extract(&html));

        Ok(CrawledPage {
            url,
            html,
            markdown,
            status,
            title,
            code_extraction,  // NEW
        })
    }
}
```

---

### 2.8 Write Tests

**File**: `server/crates/kix-jobs/src/extraction/tests.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const DOCUSAURUS_HTML: &str = r#"
        <div class="prism-code language-rust">
            <pre><code>fn main() {
                println!("Hello");
            }</code></pre>
        </div>
    "#;

    const GITHUB_HTML: &str = r#"
        <div class="highlight highlight-source-python">
            <pre><code>def hello():
                print("Hello")</code></pre>
        </div>
    "#;

    #[test]
    fn test_docusaurus_extraction() {
        let extractor = CodeExtractor::new(CodeExtractionConfig::default());
        let result = extractor.extract(DOCUSAURUS_HTML);

        assert_eq!(result.code_blocks.len(), 1);
        assert_eq!(result.code_blocks[0].language, Language::Rust);
        assert!(result.code_blocks[0].content.contains("println"));
    }

    #[test]
    fn test_github_extraction() {
        let extractor = CodeExtractor::new(CodeExtractionConfig::default());
        let result = extractor.extract(GITHUB_HTML);

        assert_eq!(result.code_blocks.len(), 1);
        assert_eq!(result.code_blocks[0].language, Language::Python);
    }

    #[test]
    fn test_language_detection() {
        assert_eq!(Language::from_hint("rust"), Language::Rust);
        assert_eq!(Language::from_hint("rs"), Language::Rust);
        assert_eq!(Language::from_hint("python"), Language::Python);
        assert_eq!(Language::from_hint("py"), Language::Python);
        assert_eq!(Language::from_hint("javascript"), Language::JavaScript);
        assert_eq!(Language::from_hint("js"), Language::JavaScript);
    }

    #[test]
    fn test_validation() {
        let config = ValidationConfig::default();

        // Too short
        let result = validate_code("x", &config);
        assert!(!result.is_valid());

        // Placeholder
        let result = validate_code("Loading...", &config);
        assert!(!result.is_valid());

        // Valid code
        let result = validate_code("fn main() { println!(\"hello\"); }", &config);
        assert!(result.is_valid());
    }

    #[test]
    fn test_deduplication() {
        let html = r#"
            <pre><code class="language-rust">fn test() {}</code></pre>
            <pre><code class="language-rust">fn test() {}</code></pre>
        "#;

        let extractor = CodeExtractor::new(CodeExtractionConfig::default());
        let result = extractor.extract(html);

        // Should deduplicate identical blocks
        assert_eq!(result.code_blocks.len(), 1);
    }
}
```

---

## Deliverables

| Deliverable | File | Description |
|-------------|------|-------------|
| Language enum | `extraction/language.rs` | 20+ languages with detection |
| CodePattern enum | `extraction/patterns.rs` | 30+ CSS patterns |
| Validation | `extraction/validation.rs` | Code validation logic |
| CodeExtractor | `extraction/code_extractor.rs` | Main extractor |
| Module exports | `extraction/mod.rs` | Public API |
| Tests | `extraction/tests.rs` | Comprehensive tests |

---

## Exit Criteria

- [ ] All 30+ patterns implemented
- [ ] Language detection works for all languages
- [ ] Validation filters non-code content
- [ ] Deduplication prevents duplicates
- [ ] Integrated with SpiderCrawler
- [ ] Tests cover all patterns
- [ ] Existing tests still pass

---

## Next Phase

Upon completion, proceed to [Phase 3: Embedding Migration](./phase-3-embedding-migration.md) or [Phase 4: Tree-sitter Integration](./phase-4-tree-sitter.md).
