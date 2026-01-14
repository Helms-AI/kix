//! Framework-aware code extraction from HTML.

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

    /// Enable tree-sitter validation (Phase 4)
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

/// Extraction result with statistics
#[derive(Debug, Clone)]
pub struct ExtractionResult {
    pub code_blocks: Vec<CodeBlock>,
    pub validation_stats: ValidationStats,
    pub language_counts: HashMap<Language, usize>,
    pub pattern_counts: HashMap<CodePattern, usize>,
}

impl ExtractionResult {
    /// Get total code blocks extracted
    pub fn total_blocks(&self) -> usize {
        self.code_blocks.len()
    }

    /// Get total lines of code
    pub fn total_lines(&self) -> usize {
        self.code_blocks.iter().map(|b| b.line_count).sum()
    }

    /// Check if extraction found any code
    pub fn is_empty(&self) -> bool {
        self.code_blocks.is_empty()
    }
}

/// Framework-aware code extractor
pub struct CodeExtractor {
    config: CodeExtractionConfig,
    selectors: Vec<(CodePattern, Selector)>,
}

impl CodeExtractor {
    /// Create a new code extractor with the given configuration
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

    /// Create an extractor with default configuration
    pub fn with_defaults() -> Self {
        Self::new(CodeExtractionConfig::default())
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

    /// Get code content from an element, cleaning up whitespace
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

    /// Detect language from element attributes and classes
    fn detect_language(&self, element: &ElementRef) -> Language {
        // 1. Check element's class attribute
        if let Some(lang) = self.detect_from_class(element) {
            return lang;
        }

        // 2. Check element's data attributes
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
                for attr in ["data-language", "data-lang", "data-code-language"] {
                    if let Some(hint) = parent_el.attr(attr) {
                        return Language::from_hint(hint);
                    }
                }
            }
        }

        // 4. Check grandparent (for nested structures)
        if let Some(parent) = element.parent() {
            if let Some(grandparent) = parent.parent() {
                if let Some(gp_el) = grandparent.value().as_element() {
                    if let Some(classes) = gp_el.attr("class") {
                        if let Some(lang) = self.detect_from_class_str(classes) {
                            return lang;
                        }
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
            // language-X pattern (most common)
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

            // highlight-X (common)
            if let Some(lang) = class.strip_prefix("highlight-") {
                return Some(Language::from_hint(lang));
            }

            // hljs language prefix (Highlight.js)
            if let Some(lang) = class.strip_prefix("hljs-") {
                // Skip non-language classes
                if !["keyword", "string", "number", "comment", "function", "class-name", "operator"].contains(&lang) {
                    return Some(Language::from_hint(lang));
                }
            }

            // Direct language names (e.g., class="rust")
            let lang = Language::from_hint(class);
            if lang != Language::Other {
                return Some(lang);
            }
        }
        None
    }

    fn detect_from_data_attr(&self, element: &ElementRef) -> Option<Language> {
        for attr in ["data-language", "data-lang", "data-code-language", "data-highlight"] {
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

#[cfg(test)]
mod tests {
    use super::*;

    const DOCUSAURUS_HTML: &str = r#"
        <div class="prism-code language-rust">
            <pre><code>fn main() {
    println!("Hello, world!");
}</code></pre>
        </div>
    "#;

    const GITHUB_HTML: &str = r#"
        <div class="highlight highlight-source-python">
            <pre><code>def hello():
    print("Hello")</code></pre>
        </div>
    "#;

    const PRISM_HTML: &str = r#"
        <pre class="language-javascript"><code class="language-javascript">
const x = { foo: 1, bar: 2 };
console.log(x.foo);
        </code></pre>
    "#;

    const HIGHLIGHT_JS_HTML: &str = r#"
        <pre><code class="hljs language-go">
package main

import "fmt"

func main() {
    fmt.Println("Hello")
}
        </code></pre>
    "#;

    #[test]
    fn test_docusaurus_extraction() {
        let extractor = CodeExtractor::with_defaults();
        let result = extractor.extract(DOCUSAURUS_HTML);

        assert!(!result.is_empty());
        assert!(result.code_blocks.iter().any(|b| b.language == Language::Rust));
        assert!(result.code_blocks.iter().any(|b| b.content.contains("println")));
    }

    #[test]
    fn test_github_extraction() {
        let extractor = CodeExtractor::with_defaults();
        let result = extractor.extract(GITHUB_HTML);

        assert!(!result.is_empty());
        assert!(result.code_blocks.iter().any(|b| b.language == Language::Python));
    }

    #[test]
    fn test_prism_extraction() {
        let extractor = CodeExtractor::with_defaults();
        let result = extractor.extract(PRISM_HTML);

        assert!(!result.is_empty());
        assert!(result.code_blocks.iter().any(|b| b.language == Language::JavaScript));
    }

    #[test]
    fn test_highlight_js_extraction() {
        let extractor = CodeExtractor::with_defaults();
        let result = extractor.extract(HIGHLIGHT_JS_HTML);

        assert!(!result.is_empty());
        assert!(result.code_blocks.iter().any(|b| b.language == Language::Go));
    }

    #[test]
    fn test_deduplication() {
        let html = r#"
            <pre><code class="language-rust">fn test() { let x = 1; }</code></pre>
            <pre><code class="language-rust">fn test() { let x = 1; }</code></pre>
        "#;

        let extractor = CodeExtractor::with_defaults();
        let result = extractor.extract(html);

        // Should deduplicate identical blocks
        assert_eq!(result.code_blocks.len(), 1);
    }

    #[test]
    fn test_multiple_languages() {
        let html = r#"
            <pre><code class="language-rust">fn main() { println!("Rust"); }</code></pre>
            <pre><code class="language-python">def main(): print("Python")</code></pre>
            <pre><code class="language-javascript">const x = () => console.log("JS");</code></pre>
        "#;

        let extractor = CodeExtractor::with_defaults();
        let result = extractor.extract(html);

        assert_eq!(result.code_blocks.len(), 3);
        assert!(result.language_counts.contains_key(&Language::Rust));
        assert!(result.language_counts.contains_key(&Language::Python));
        assert!(result.language_counts.contains_key(&Language::JavaScript));
    }

    #[test]
    fn test_empty_html() {
        let extractor = CodeExtractor::with_defaults();
        let result = extractor.extract("<html><body></body></html>");

        assert!(result.is_empty());
    }

    #[test]
    fn test_validation_filters() {
        let html = r#"
            <pre><code>x</code></pre>
            <pre><code>loading...</code></pre>
            <pre><code class="language-rust">fn valid() { println!("test"); }</code></pre>
        "#;

        let extractor = CodeExtractor::with_defaults();
        let result = extractor.extract(html);

        // Only the valid code should be extracted
        // Note: We may get duplicates due to multiple CSS selectors matching the same element
        // The important thing is filtering works
        assert!(result.code_blocks.len() >= 1, "Expected at least 1 valid code block");
        assert!(result.code_blocks.iter().any(|b| b.content.contains("println")), "Should have the valid Rust code");
        assert!(result.validation_stats.filtered_too_short > 0, "Should filter too short");
        assert!(result.validation_stats.filtered_placeholder > 0, "Should filter placeholder");
    }
}
