//! Code Extraction Service
//!
//! Extracts code blocks from HTML documents using 30+ patterns. Supports:
//! - Standard HTML patterns (pre, code)
//! - Documentation frameworks (Docusaurus, MkDocs, Sphinx, ReadTheDocs)
//! - Syntax highlighters (Prism.js, Highlight.js, Rouge)
//! - Editor components (Monaco, CodeMirror, Ace)
//! - Platform-specific patterns (GitHub, GitLab, Stack Overflow)

use scraper::{ElementRef, Html, Selector};
use std::collections::HashSet;

/// An extracted code block with metadata
#[derive(Debug, Clone)]
pub struct ExtractedCode {
    /// The code content
    pub content: String,
    /// Detected programming language
    pub language: Option<String>,
    /// CSS selector that matched this code
    pub pattern: CodePattern,
    /// Original HTML for debugging
    pub html: String,
    /// Content hash for deduplication
    pub hash: u64,
}

/// Code extraction patterns for documentation sites
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CodePattern {
    // Standard HTML patterns
    PreCode,
    PreOnly,
    CodeOnly,

    // Documentation frameworks
    DocusaurusCodeBlock,
    DocusaurusTabCodeBlock,
    MkDocsCodeBlock,
    SphinxCodeBlock,
    ReadTheDocsCode,
    JekyllHighlight,
    HugoHighlight,

    // Syntax highlighters
    PrismJs,
    HighlightJs,
    SyntaxHighlighter,
    RougeSyntax,
    Shiki,

    // Editor components
    MonacoEditor,
    CodeMirror,
    AceEditor,

    // Platform-specific
    GitHubCode,
    GitLabCode,
    BitbucketCode,
    StackOverflowCode,

    // Framework patterns
    VuePressCode,
    GatsbyCode,
    NextjsRehype,
    AstroCode,

    // Terminal/shell patterns
    TerminalOutput,
    AsciinemaPlayer,

    // Custom/generic
    DataLanguageAttr,
    ClassPrefixCode,
    DataCodeAttr,
}

impl CodePattern {
    /// Get the CSS selector for this pattern
    pub fn selector(&self) -> &'static str {
        match self {
            // Standard HTML
            Self::PreCode => "pre code",
            Self::PreOnly => "pre:not(:has(code))",
            Self::CodeOnly => "code:not(pre code)",

            // Documentation frameworks
            Self::DocusaurusCodeBlock => ".prism-code, [class*='codeBlockContent']",
            Self::DocusaurusTabCodeBlock => ".tabs-container pre code",
            Self::MkDocsCodeBlock => ".highlight pre, .codehilite pre",
            Self::SphinxCodeBlock => ".highlight-python pre, .highlight-default pre",
            Self::ReadTheDocsCode => ".rst-content pre",
            Self::JekyllHighlight => ".highlighter-rouge pre, .highlight pre.highlight",
            Self::HugoHighlight => ".highlight pre, .chroma pre",

            // Syntax highlighters
            Self::PrismJs => "[class*='language-'] code, pre[class*='language-']",
            Self::HighlightJs => ".hljs, pre code.hljs",
            Self::SyntaxHighlighter => ".syntaxhighlighter",
            Self::RougeSyntax => ".rouge pre, .rouge-code",
            Self::Shiki => ".shiki code, pre.shiki",

            // Editor components
            Self::MonacoEditor => ".monaco-editor .view-lines",
            Self::CodeMirror => ".CodeMirror-code, .cm-content",
            Self::AceEditor => ".ace_editor .ace_content",

            // Platform-specific
            Self::GitHubCode => ".blob-code-content, .highlight pre, .js-file-line",
            Self::GitLabCode => ".blob-content pre, .code pre",
            Self::BitbucketCode => ".code-container pre",
            Self::StackOverflowCode => ".s-prose pre, .s-code-block, .post-text pre",

            // Framework patterns
            Self::VuePressCode => "div[class*='language-'] pre",
            Self::GatsbyCode => ".gatsby-highlight pre",
            Self::NextjsRehype => "[data-rehype-pretty-code] code",
            Self::AstroCode => ".astro-code pre",

            // Terminal/shell
            Self::TerminalOutput => ".terminal pre, .console pre, .shell pre",
            Self::AsciinemaPlayer => ".asciinema-player pre",

            // Custom/generic
            Self::DataLanguageAttr => "[data-language] code, [data-lang] code",
            Self::ClassPrefixCode => "[class*='code-'] pre, [class*='snippet'] pre",
            Self::DataCodeAttr => "[data-code]",
        }
    }

    /// Get all patterns in recommended order
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
            // Syntax highlighters
            Self::PrismJs,
            Self::HighlightJs,
            Self::SyntaxHighlighter,
            Self::RougeSyntax,
            Self::Shiki,
            // Editor components
            Self::MonacoEditor,
            Self::CodeMirror,
            Self::AceEditor,
            // Framework patterns
            Self::VuePressCode,
            Self::GatsbyCode,
            Self::NextjsRehype,
            Self::AstroCode,
            // Terminal
            Self::TerminalOutput,
            Self::AsciinemaPlayer,
            // Generic attributes
            Self::DataLanguageAttr,
            Self::ClassPrefixCode,
            Self::DataCodeAttr,
            // Standard HTML (fallback)
            Self::PreCode,
            Self::PreOnly,
            Self::CodeOnly,
        ]
    }

    /// Get a description of this pattern
    pub fn description(&self) -> &'static str {
        match self {
            Self::PreCode => "Standard pre > code",
            Self::PreOnly => "Pre without code tag",
            Self::CodeOnly => "Standalone code element",
            Self::DocusaurusCodeBlock => "Docusaurus code block",
            Self::DocusaurusTabCodeBlock => "Docusaurus tabbed code",
            Self::MkDocsCodeBlock => "MkDocs code block",
            Self::SphinxCodeBlock => "Sphinx documentation",
            Self::ReadTheDocsCode => "ReadTheDocs content",
            Self::JekyllHighlight => "Jekyll highlight",
            Self::HugoHighlight => "Hugo highlight",
            Self::PrismJs => "Prism.js syntax highlighting",
            Self::HighlightJs => "Highlight.js syntax highlighting",
            Self::SyntaxHighlighter => "SyntaxHighlighter library",
            Self::RougeSyntax => "Rouge syntax highlighting",
            Self::Shiki => "Shiki syntax highlighting",
            Self::MonacoEditor => "Monaco Editor",
            Self::CodeMirror => "CodeMirror Editor",
            Self::AceEditor => "Ace Editor",
            Self::GitHubCode => "GitHub code view",
            Self::GitLabCode => "GitLab code view",
            Self::BitbucketCode => "Bitbucket code view",
            Self::StackOverflowCode => "Stack Overflow code block",
            Self::VuePressCode => "VuePress code block",
            Self::GatsbyCode => "Gatsby highlight",
            Self::NextjsRehype => "Next.js rehype code",
            Self::AstroCode => "Astro code block",
            Self::TerminalOutput => "Terminal output",
            Self::AsciinemaPlayer => "Asciinema recording",
            Self::DataLanguageAttr => "data-language attribute",
            Self::ClassPrefixCode => "Code/snippet class prefix",
            Self::DataCodeAttr => "data-code attribute",
        }
    }
}

/// Code extraction service
pub struct CodeExtractionService {
    /// Compiled selectors (cached) - patterns are embedded in the tuple
    selectors: Vec<(CodePattern, Option<Selector>)>,
    /// Minimum code length to consider
    min_length: usize,
    /// Maximum prose ratio (to filter non-code content)
    max_prose_ratio: f32,
}

impl Default for CodeExtractionService {
    fn default() -> Self {
        Self::new()
    }
}

impl CodeExtractionService {
    /// Create a new code extraction service with all patterns
    pub fn new() -> Self {
        let patterns = CodePattern::all();
        let selectors = patterns
            .iter()
            .map(|p| (*p, Selector::parse(p.selector()).ok()))
            .collect();

        Self {
            selectors,
            min_length: 10,
            max_prose_ratio: 0.6,
        }
    }

    /// Create with custom patterns
    pub fn with_patterns(patterns: Vec<CodePattern>) -> Self {
        let selectors = patterns
            .iter()
            .map(|p| (*p, Selector::parse(p.selector()).ok()))
            .collect();

        Self {
            selectors,
            min_length: 10,
            max_prose_ratio: 0.6,
        }
    }

    /// Set minimum code length
    pub fn min_length(mut self, len: usize) -> Self {
        self.min_length = len;
        self
    }

    /// Set maximum prose ratio
    pub fn max_prose_ratio(mut self, ratio: f32) -> Self {
        self.max_prose_ratio = ratio;
        self
    }

    /// Extract all code blocks from a document
    pub fn extract_all(&self, document: &Html) -> Vec<ExtractedCode> {
        let mut code_blocks = Vec::new();
        let mut seen_hashes = HashSet::new();

        for (pattern, selector_opt) in &self.selectors {
            let selector = match selector_opt {
                Some(s) => s,
                None => continue,
            };

            for element in document.select(selector) {
                if let Some(code) = self.extract_from_element(&element, *pattern) {
                    // Skip duplicates
                    if seen_hashes.contains(&code.hash) {
                        continue;
                    }

                    // Validate the code
                    if self.validate(&code) {
                        seen_hashes.insert(code.hash);
                        code_blocks.push(code);
                    }
                }
            }
        }

        code_blocks
    }

    /// Extract code from a single element
    fn extract_from_element(&self, element: &ElementRef, pattern: CodePattern) -> Option<ExtractedCode> {
        let content = self.get_code_content(element);
        if content.is_empty() {
            return None;
        }

        let language = self.detect_language(element);
        let html = element.html();
        let hash = self.hash_content(&content);

        Some(ExtractedCode {
            content,
            language,
            pattern,
            html,
            hash,
        })
    }

    /// Get the text content of a code element
    fn get_code_content(&self, element: &ElementRef) -> String {
        // For most elements, just get the text content
        let content = element.text().collect::<String>();

        // Clean up the content
        content
            .lines()
            .map(|line| line.trim_end())
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_string()
    }

    /// Detect the programming language
    fn detect_language(&self, element: &ElementRef) -> Option<String> {
        // Check class attribute for language hints
        if let Some(classes) = element.value().attr("class") {
            for class in classes.split_whitespace() {
                // language-X pattern
                if let Some(lang) = class.strip_prefix("language-") {
                    return Some(self.normalize_language(lang));
                }
                if let Some(lang) = class.strip_prefix("lang-") {
                    return Some(self.normalize_language(lang));
                }

                // hljs language pattern
                if class.starts_with("hljs") && !class.contains('-') {
                    continue; // Just the hljs class, not a language
                }

                // Direct language names as classes
                if self.is_known_language(class) {
                    return Some(self.normalize_language(class));
                }
            }
        }

        // Check data attributes
        for attr in ["data-language", "data-lang", "data-code-language"] {
            if let Some(lang) = element.value().attr(attr) {
                return Some(self.normalize_language(lang));
            }
        }

        // Check parent element
        if let Some(parent) = element.parent() {
            if let Some(parent_el) = parent.value().as_element() {
                if let Some(classes) = parent_el.attr("class") {
                    for class in classes.split_whitespace() {
                        if let Some(lang) = class.strip_prefix("language-") {
                            return Some(self.normalize_language(lang));
                        }
                    }
                }
                for attr in ["data-language", "data-lang"] {
                    if let Some(lang) = parent_el.attr(attr) {
                        return Some(self.normalize_language(lang));
                    }
                }
            }
        }

        None
    }

    /// Check if a string is a known programming language
    fn is_known_language(&self, s: &str) -> bool {
        let known = [
            "javascript", "typescript", "python", "rust", "go", "java", "c",
            "cpp", "csharp", "ruby", "php", "swift", "kotlin", "scala", "html",
            "css", "scss", "sass", "less", "sql", "shell", "bash", "zsh",
            "powershell", "yaml", "yml", "json", "xml", "markdown", "md",
            "toml", "ini", "dockerfile", "makefile", "cmake", "lua", "perl",
            "r", "matlab", "julia", "haskell", "elixir", "erlang", "clojure",
            "groovy", "dart", "objectivec", "objc", "assembly", "asm", "wasm",
            "graphql", "protobuf", "proto", "terraform", "hcl", "nix",
        ];
        known.contains(&s.to_lowercase().as_str())
    }

    /// Normalize language name
    fn normalize_language(&self, lang: &str) -> String {
        let lang = lang.to_lowercase();

        // Common normalizations
        match lang.as_str() {
            "js" => "javascript".to_string(),
            "ts" => "typescript".to_string(),
            "py" => "python".to_string(),
            "rb" => "ruby".to_string(),
            "rs" => "rust".to_string(),
            "cs" => "csharp".to_string(),
            "c++" | "cxx" => "cpp".to_string(),
            "sh" | "zsh" => "bash".to_string(),
            "yml" => "yaml".to_string(),
            "md" => "markdown".to_string(),
            "objc" | "objective-c" => "objectivec".to_string(),
            "asm" => "assembly".to_string(),
            "proto" => "protobuf".to_string(),
            _ => lang,
        }
    }

    /// Validate extracted code
    fn validate(&self, code: &ExtractedCode) -> bool {
        // Length check
        if code.content.len() < self.min_length {
            return false;
        }

        // Skip common non-code content
        let content_lower = code.content.to_lowercase();
        if content_lower.starts_with("loading")
            || content_lower.starts_with("please wait")
            || content_lower == "..."
            || content_lower == "copy"
        {
            return false;
        }

        // Check for code structure
        if !self.has_code_structure(&code.content) {
            return false;
        }

        // Check prose ratio (if too much English prose, probably not code)
        if self.calculate_prose_ratio(&code.content) > self.max_prose_ratio {
            // But allow if it has a detected language
            if code.language.is_none() {
                return false;
            }
        }

        true
    }

    /// Check if content has code-like structure
    fn has_code_structure(&self, content: &str) -> bool {
        let code_chars = ['{', '}', '[', ']', '(', ')', ';', '=', '<', '>', ':'];
        let code_char_count = content.chars().filter(|c| code_chars.contains(c)).count();

        // At least 2% code characters
        if content.len() > 0 && code_char_count > content.len() / 50 {
            return true;
        }

        // Or contains common code patterns
        let code_patterns = [
            "function ",
            "def ",
            "class ",
            "const ",
            "let ",
            "var ",
            "import ",
            "from ",
            "fn ",
            "pub ",
            "impl ",
            "struct ",
            "enum ",
            "if (",
            "for (",
            "while (",
            "return ",
            "->",
            "=>",
            "$ ",
            "# ",
            "// ",
            "/* ",
            "```",
        ];

        code_patterns.iter().any(|p| content.contains(p))
    }

    /// Calculate ratio of English prose words
    fn calculate_prose_ratio(&self, content: &str) -> f32 {
        let common_words = [
            "the", "a", "an", "and", "or", "but", "in", "on", "at", "to", "for",
            "of", "with", "by", "from", "is", "are", "was", "were", "be", "been",
            "being", "have", "has", "had", "do", "does", "did", "will", "would",
            "could", "should", "may", "might", "must", "shall", "can", "need",
            "this", "that", "these", "those", "it", "its", "i", "you", "we",
            "they", "he", "she", "my", "your", "our", "their", "his", "her",
        ];

        let words: Vec<&str> = content
            .split_whitespace()
            .map(|w| w.trim_matches(|c: char| !c.is_alphabetic()))
            .filter(|w| !w.is_empty())
            .collect();

        if words.is_empty() {
            return 0.0;
        }

        let prose_count = words
            .iter()
            .filter(|w| common_words.contains(&w.to_lowercase().as_str()))
            .count();

        prose_count as f32 / words.len() as f32
    }

    /// Hash content for deduplication
    fn hash_content(&self, content: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        hasher.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_patterns_have_valid_selectors() {
        for pattern in CodePattern::all() {
            let selector = Selector::parse(pattern.selector());
            assert!(
                selector.is_ok(),
                "Invalid selector for {:?}: {}",
                pattern,
                pattern.selector()
            );
        }
    }

    #[test]
    fn test_pattern_count() {
        // Verify we have 30+ patterns as specified
        assert!(
            CodePattern::all().len() >= 30,
            "Expected 30+ patterns, got {}",
            CodePattern::all().len()
        );
    }

    #[test]
    fn test_extract_pre_code() {
        let service = CodeExtractionService::new();
        let html = r#"
            <html><body>
                <pre><code class="language-rust">fn main() { println!("Hello"); }</code></pre>
            </body></html>
        "#;
        let document = Html::parse_document(html);
        let codes = service.extract_all(&document);

        assert!(!codes.is_empty());
        assert!(codes[0].content.contains("fn main"));
        assert_eq!(codes[0].language, Some("rust".to_string()));
    }

    #[test]
    fn test_language_normalization() {
        let service = CodeExtractionService::new();
        assert_eq!(service.normalize_language("js"), "javascript");
        assert_eq!(service.normalize_language("ts"), "typescript");
        assert_eq!(service.normalize_language("py"), "python");
        assert_eq!(service.normalize_language("c++"), "cpp");
    }

    #[test]
    fn test_prose_ratio() {
        let service = CodeExtractionService::new();

        // Code should have low prose ratio
        let code = "fn main() { let x = 5; println!(\"{}\", x); }";
        assert!(service.calculate_prose_ratio(code) < 0.3);

        // English text with many common words should have higher prose ratio
        // "This is a test and it has been written by the user for their own use"
        // Common words: this, is, a, and, it, has, been, by, the, for, their
        let text = "This is a test and it has been written by the user for their own use";
        let ratio = service.calculate_prose_ratio(text);
        // Should have at least 0.4 ratio (11 common words out of ~15)
        assert!(ratio > 0.4, "Expected prose ratio > 0.4, got {}", ratio);
    }

    #[test]
    fn test_deduplication() {
        let service = CodeExtractionService::new();
        let html = r#"
            <html><body>
                <pre><code>fn test() {}</code></pre>
                <pre><code>fn test() {}</code></pre>
            </body></html>
        "#;
        let document = Html::parse_document(html);
        let codes = service.extract_all(&document);

        // Should deduplicate identical code
        assert_eq!(codes.len(), 1);
    }
}
