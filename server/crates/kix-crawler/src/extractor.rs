//! Content Extractor
//!
//! Extracts clean markdown content from HTML pages with optimized
//! configuration for knowledge indexing. Features:
//! - Boilerplate removal (nav, footer, sidebar, ads)
//! - Main content area detection
//! - Code block preservation
//! - DOM-order header extraction
//! - Metadata extraction (title, description)

use readability::extractor::extract;
use scraper::{ElementRef, Html, Selector};
use sha2::{Digest, Sha256};
use url::Url;

/// Extracted content from an HTML page
#[derive(Debug, Clone)]
pub struct ExtractedContent {
    /// Clean markdown content
    pub markdown: String,
    /// Page title
    pub title: String,
    /// Page description (from meta tags)
    pub description: Option<String>,
    /// Headers in DOM order
    pub headers: Vec<ExtractedHeader>,
    /// Code blocks found in the page
    pub code_blocks: Vec<ExtractedCodeBlock>,
    /// Canonical URL if found
    pub canonical_url: Option<String>,
    /// Word count of main content
    pub word_count: usize,
    /// SHA-256 hash of content for change detection
    pub content_hash: String,
    /// Character count for metadata
    pub char_count: usize,
}

impl ExtractedContent {
    /// Create an empty ExtractedContent (used for fallback when extraction fails)
    pub fn empty() -> Self {
        Self {
            markdown: String::new(),
            title: String::new(),
            description: None,
            headers: Vec::new(),
            code_blocks: Vec::new(),
            canonical_url: None,
            word_count: 0,
            content_hash: String::new(),
            char_count: 0,
        }
    }
}

/// A header extracted from the page
#[derive(Debug, Clone)]
pub struct ExtractedHeader {
    /// Header level (1-6)
    pub level: u8,
    /// Header text content
    pub text: String,
    /// Optional ID for linking
    pub id: Option<String>,
}

/// A code block extracted from the page
#[derive(Debug, Clone)]
pub struct ExtractedCodeBlock {
    /// Code content
    pub content: String,
    /// Detected or specified language
    pub language: Option<String>,
    /// Whether this is inline code vs block
    pub is_inline: bool,
}

/// Content source for markdown generation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentSource {
    /// Use raw HTML
    Html,
    /// Use cleaned/processed HTML
    Cleaned,
}

/// Markdown generation configuration
///
/// CRITICAL: `content_source` MUST be `ContentSource::Html` (not `Cleaned`) to preserve
/// code blocks properly. Using raw HTML ensures code blocks and their syntax highlighting
/// information are not stripped during processing.
#[derive(Debug, Clone)]
pub struct MarkdownConfig {
    /// Content source to use - MUST be Html to preserve code blocks
    pub content_source: ContentSource,
    /// Mark code blocks with language hints
    pub mark_code: bool,
    /// Handle code inside pre tags specially
    pub handle_code_in_pre: bool,
    /// Body width for wrapping (0 = no wrapping)
    pub body_width: usize,
    /// Skip internal links (same-domain)
    pub skip_internal_links: bool,
    /// Escape special markdown characters
    pub escape: bool,
}

impl Default for MarkdownConfig {
    fn default() -> Self {
        // CRITICAL: content_source MUST be Html to preserve code blocks
        Self {
            content_source: ContentSource::Html, // Use raw HTML to preserve code blocks
            mark_code: true,
            handle_code_in_pre: true,
            body_width: 0,
            skip_internal_links: true,
            escape: false,
        }
    }
}

/// Text density pruning threshold (20% = element must have 20% text vs tags)
/// This is used as a fallback when Readability fails
const TEXT_DENSITY_THRESHOLD: f32 = 0.20;

/// Minimum content length to consider Readability extraction successful
const MIN_READABILITY_CONTENT_LEN: usize = 100;

/// Content extractor with optimized behavior for knowledge indexing
pub struct ContentExtractor {
    config: MarkdownConfig,
    /// Selectors for boilerplate elements to remove
    boilerplate_selectors: Vec<Selector>,
    /// Selectors for main content areas
    main_content_selectors: Vec<Selector>,
    /// Cached body selector
    body_selector: Selector,
}

impl Default for ContentExtractor {
    fn default() -> Self {
        Self::new(MarkdownConfig::default())
    }
}

impl ContentExtractor {
    /// Create a new content extractor with the given configuration
    pub fn new(config: MarkdownConfig) -> Self {
        let boilerplate_selectors = Self::compile_boilerplate_selectors();
        let main_content_selectors = Self::compile_main_content_selectors();
        let body_selector = Selector::parse("body").unwrap();

        Self {
            config,
            boilerplate_selectors,
            main_content_selectors,
            body_selector,
        }
    }

    /// Compile CSS selectors for boilerplate elements
    fn compile_boilerplate_selectors() -> Vec<Selector> {
        let patterns = [
            // Navigation
            "nav",
            "header nav",
            "[role='navigation']",
            ".nav",
            ".navigation",
            ".navbar",
            ".menu",
            ".header-menu",
            // Footer
            "footer",
            "[role='contentinfo']",
            ".footer",
            ".site-footer",
            // Sidebar
            "aside",
            "[role='complementary']",
            ".sidebar",
            ".side-bar",
            ".widget",
            ".toc",
            ".table-of-contents",
            // Ads and promotions
            ".advertisement",
            ".ad",
            ".ads",
            ".advert",
            ".sponsored",
            ".promotion",
            "[data-ad]",
            // Cookie/consent banners
            ".cookie-banner",
            ".cookie-notice",
            ".cookie-consent",
            ".gdpr",
            ".consent-banner",
            "#cookie-notice",
            // Comments
            ".comments",
            ".comment-section",
            "#comments",
            "[data-comments]",
            // Social/sharing
            ".social-share",
            ".share-buttons",
            ".social-buttons",
            // Newsletter
            ".newsletter",
            ".subscribe",
            ".email-signup",
            // Scripts and styles
            "script",
            "style",
            "noscript",
            // Hidden elements
            "[hidden]",
            "[aria-hidden='true']",
            ".hidden",
            ".d-none",
            // Skip links
            ".skip-link",
            ".skip-to-content",
        ];

        patterns
            .iter()
            .filter_map(|p| Selector::parse(p).ok())
            .collect()
    }

    /// Compile CSS selectors for main content areas
    fn compile_main_content_selectors() -> Vec<Selector> {
        let patterns = [
            // Semantic elements (priority order)
            "main",
            "article",
            "[role='main']",
            // Common content classes
            ".content",
            ".main-content",
            ".post-content",
            ".article-content",
            ".entry-content",
            ".page-content",
            ".prose",
            ".markdown-body",
            // Documentation-specific
            ".docs-content",
            ".documentation",
            ".doc-content",
            ".api-content",
            // Blog-specific
            ".post",
            ".entry",
            ".blog-post",
            // ID-based
            "#content",
            "#main",
            "#main-content",
            "#article",
        ];

        patterns
            .iter()
            .filter_map(|p| Selector::parse(p).ok())
            .collect()
    }

    /// Extract content from HTML using Mozilla Readability algorithm
    ///
    /// This method uses a two-stage extraction approach:
    /// 1. PRIMARY: Try Mozilla Readability algorithm (same as Firefox Reader Mode)
    /// 2. FALLBACK: If Readability fails, use text density scoring
    ///
    /// Code blocks are extracted separately and preserved regardless of which
    /// extraction method is used.
    pub fn extract(&self, html: &str, page_url: &Url) -> ExtractedContent {
        let document = Html::parse_document(html);

        // Extract metadata first (always works, independent of content extraction)
        let title = self.extract_title(&document);
        let description = self.extract_description(&document);
        let canonical_url = self.extract_canonical(&document);

        // Extract headers in DOM order (always works)
        let headers = self.extract_headers_dom_order(&document);

        // Extract code blocks FIRST (critical for KIX, works on raw HTML)
        let code_blocks = self.extract_code_blocks(&document);

        // PRIMARY: Try Mozilla Readability algorithm
        let main_content = self.extract_with_readability(html, page_url);

        // FALLBACK: If Readability didn't produce good content, use text density
        let main_content = if main_content.is_none()
            || main_content
                .as_ref()
                .map(|c| c.len() < MIN_READABILITY_CONTENT_LEN)
                .unwrap_or(true)
        {
            // Try text density-based extraction as fallback
            let density_content = self.extract_with_density(&document);
            if !density_content.is_empty() {
                density_content
            } else {
                // Ultimate fallback: use old CSS selector approach
                self.extract_markdown_legacy(&document, page_url)
            }
        } else {
            main_content.unwrap()
        };

        // Convert to markdown and clean up
        let markdown = self.clean_markdown(&main_content, page_url);
        let word_count = markdown.split_whitespace().count();
        let char_count = markdown.chars().count();
        let content_hash = Self::compute_hash(&markdown);

        ExtractedContent {
            markdown,
            title,
            description,
            headers,
            code_blocks,
            canonical_url,
            word_count,
            content_hash,
            char_count,
        }
    }

    /// Extract content using Mozilla Readability algorithm
    ///
    /// This uses the same algorithm as Firefox Reader Mode to identify
    /// the main content area and remove boilerplate.
    fn extract_with_readability(&self, html: &str, url: &Url) -> Option<String> {
        // Use readability crate to extract main content
        match extract(&mut html.as_bytes(), url) {
            Ok(product) => {
                let content = product.content;

                // Validate the extraction produced meaningful content
                if content.len() >= MIN_READABILITY_CONTENT_LEN
                    && !self.is_mostly_boilerplate(&content)
                {
                    // Convert the extracted HTML to markdown
                    let options = htmd::HtmlToMarkdown::builder()
                        .skip_tags(vec!["script", "style", "noscript"])
                        .build();

                    options.convert(&content).ok()
                } else {
                    None
                }
            }
            Err(_) => None,
        }
    }

    /// Extract content using text density analysis
    ///
    /// This is a fallback method when Readability fails. It scores DOM elements
    /// by their text density (ratio of text characters to total HTML characters)
    /// and selects the element with the best score.
    fn extract_with_density(&self, document: &Html) -> String {
        let body = match document.select(&self.body_selector).next() {
            Some(b) => b,
            None => return String::new(),
        };

        let mut best_element: Option<ElementRef> = None;
        let mut best_score: f32 = 0.0;

        // Traverse all descendants and score them by text density
        for element in body.descendants().filter_map(ElementRef::wrap) {
            let tag_name = element.value().name();

            // Skip non-content elements
            if matches!(
                tag_name,
                "script" | "style" | "noscript" | "nav" | "header" | "footer" | "aside"
            ) {
                continue;
            }

            // Calculate text density
            let density = self.calculate_text_density(&element);
            let text_len = element.text().collect::<String>().len();

            // Score = density * sqrt(text_length)
            // This prefers denser, longer content blocks
            let score = density * (text_len as f32).sqrt();

            // Must meet minimum density threshold
            if density >= TEXT_DENSITY_THRESHOLD && score > best_score {
                best_score = score;
                best_element = Some(element);
            }
        }

        // Extract and convert the best element
        if let Some(element) = best_element {
            let html_content = element.html();

            // Remove boilerplate from the selected element
            let cleaned = self.remove_boilerplate(&html_content);

            // Convert to markdown
            let options = htmd::HtmlToMarkdown::builder()
                .skip_tags(vec!["script", "style", "noscript"])
                .build();

            options.convert(&cleaned).unwrap_or_default()
        } else {
            String::new()
        }
    }

    /// Calculate text density: (text chars) / (total chars including tags)
    fn calculate_text_density(&self, element: &ElementRef) -> f32 {
        let text_len = element.text().collect::<String>().len();
        let html_len = element.html().len();

        if html_len == 0 {
            return 0.0;
        }

        text_len as f32 / html_len as f32
    }

    /// Check if content appears to be mostly boilerplate
    ///
    /// Detects common navigation patterns that indicate the content
    /// is not the main article content.
    fn is_mostly_boilerplate(&self, content: &str) -> bool {
        let lower = content.to_lowercase();

        // Common navigation phrase patterns
        let nav_patterns = [
            "log in",
            "sign in",
            "sign up",
            "sign out",
            "log out",
            "search",
            "menu",
            "navigation",
            "skip to content",
            "cookie",
            "privacy policy",
            "terms of service",
            "all rights reserved",
        ];

        // Count how many navigation patterns appear
        let nav_count = nav_patterns
            .iter()
            .filter(|p| lower.contains(*p))
            .count();

        // If more than 3 nav patterns in short content, likely boilerplate
        let word_count = content.split_whitespace().count();
        if word_count < 100 && nav_count >= 3 {
            return true;
        }

        // Check if content is mostly links (common in navs)
        let link_ratio = content.matches("](").count() as f32 / word_count.max(1) as f32;
        if link_ratio > 0.3 {
            return true;
        }

        false
    }

    /// Compute SHA-256 hash of content for change detection
    pub fn compute_hash(content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Legacy extraction method using CSS selectors (ultimate fallback)
    fn extract_markdown_legacy(&self, document: &Html, _page_url: &Url) -> String {
        // Try to find main content area using CSS selectors
        let main_content = self.find_main_content(document);

        let html_content = if let Some(main) = main_content {
            main.html()
        } else {
            // Fall back to body
            if let Some(body) = document.select(&self.body_selector).next() {
                body.html()
            } else {
                return String::new();
            }
        };

        // Remove boilerplate from the content
        let cleaned_html = self.remove_boilerplate(&html_content);

        // Convert to markdown using htmd
        let options = htmd::HtmlToMarkdown::builder()
            .skip_tags(vec!["script", "style", "noscript"])
            .build();

        options.convert(&cleaned_html).unwrap_or_default()
    }

    /// Extract the page title
    fn extract_title(&self, document: &Html) -> String {
        // Try og:title first
        if let Some(og_title) = self.get_meta_content(document, "og:title") {
            return og_title;
        }

        // Try title tag
        let title_selector = Selector::parse("title").unwrap();
        if let Some(title) = document.select(&title_selector).next() {
            let text = title.text().collect::<String>();
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }

        // Try h1
        let h1_selector = Selector::parse("h1").unwrap();
        if let Some(h1) = document.select(&h1_selector).next() {
            return h1.text().collect::<String>().trim().to_string();
        }

        "Untitled".to_string()
    }

    /// Extract the page description
    fn extract_description(&self, document: &Html) -> Option<String> {
        // Try og:description
        if let Some(desc) = self.get_meta_content(document, "og:description") {
            return Some(desc);
        }

        // Try meta description
        self.get_meta_content(document, "description")
    }

    /// Extract canonical URL
    fn extract_canonical(&self, document: &Html) -> Option<String> {
        let selector = Selector::parse("link[rel='canonical']").unwrap();
        document.select(&selector).next().and_then(|el| {
            el.value()
                .attr("href")
                .map(|s| s.to_string())
        })
    }

    /// Get meta tag content by name or property
    fn get_meta_content(&self, document: &Html, name: &str) -> Option<String> {
        // Try property (for og: tags)
        let prop_selector = Selector::parse(&format!("meta[property='{}']", name)).ok()?;
        if let Some(el) = document.select(&prop_selector).next() {
            if let Some(content) = el.value().attr("content") {
                let trimmed = content.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }

        // Try name attribute
        let name_selector = Selector::parse(&format!("meta[name='{}']", name)).ok()?;
        if let Some(el) = document.select(&name_selector).next() {
            if let Some(content) = el.value().attr("content") {
                let trimmed = content.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }

        None
    }

    /// Extract headers in DOM order (not by level)
    fn extract_headers_dom_order(&self, document: &Html) -> Vec<ExtractedHeader> {
        let mut headers = Vec::new();
        let body_selector = Selector::parse("body").unwrap();

        if let Some(body) = document.select(&body_selector).next() {
            // Traverse DOM in document order
            for node in body.descendants() {
                if let Some(element) = node.value().as_element() {
                    let name = element.name();
                    let level = match name {
                        "h1" => Some(1),
                        "h2" => Some(2),
                        "h3" => Some(3),
                        "h4" => Some(4),
                        "h5" => Some(5),
                        "h6" => Some(6),
                        _ => None,
                    };

                    if let Some(level) = level {
                        if let Some(el_ref) = ElementRef::wrap(node) {
                            let text = el_ref.text().collect::<String>();
                            let text = text.trim().to_string();
                            if !text.is_empty() {
                                let id = element.attr("id").map(|s| s.to_string());
                                headers.push(ExtractedHeader { level, text, id });
                            }
                        }
                    }
                }
            }
        }

        headers
    }

    /// Extract code blocks from the document
    fn extract_code_blocks(&self, document: &Html) -> Vec<ExtractedCodeBlock> {
        let mut code_blocks = Vec::new();
        let pre_selector = Selector::parse("pre").unwrap();
        let code_selector = Selector::parse("code").unwrap();

        // Extract pre > code blocks
        for pre in document.select(&pre_selector) {
            if let Some(code) = pre.select(&code_selector).next() {
                let content = code.text().collect::<String>();
                let language = self.detect_code_language(&code);
                code_blocks.push(ExtractedCodeBlock {
                    content,
                    language,
                    is_inline: false,
                });
            } else {
                // Pre without code tag
                let content = pre.text().collect::<String>();
                code_blocks.push(ExtractedCodeBlock {
                    content,
                    language: None,
                    is_inline: false,
                });
            }
        }

        // Extract standalone code elements (not inside pre)
        for code in document.select(&code_selector) {
            // Skip if inside a pre
            let mut parent = code.parent();
            let mut inside_pre = false;
            while let Some(p) = parent {
                if p.value().as_element().map(|e| e.name()) == Some("pre") {
                    inside_pre = true;
                    break;
                }
                parent = p.parent();
            }

            if !inside_pre {
                let content = code.text().collect::<String>();
                if content.len() > 50 {
                    // Treat longer code as block
                    code_blocks.push(ExtractedCodeBlock {
                        content,
                        language: self.detect_code_language(&code),
                        is_inline: false,
                    });
                }
            }
        }

        code_blocks
    }

    /// Detect code language from element classes or attributes
    fn detect_code_language(&self, element: &ElementRef) -> Option<String> {
        let classes = element.value().attr("class").unwrap_or("");

        // Check for language- prefix (common pattern)
        for class in classes.split_whitespace() {
            if let Some(lang) = class.strip_prefix("language-") {
                return Some(lang.to_string());
            }
            if let Some(lang) = class.strip_prefix("lang-") {
                return Some(lang.to_string());
            }
            if class.starts_with("hljs-") {
                continue; // This is a highlight class, not language
            }

            // Common language class names
            let known_langs = [
                "javascript",
                "typescript",
                "python",
                "rust",
                "go",
                "java",
                "c",
                "cpp",
                "csharp",
                "ruby",
                "php",
                "swift",
                "kotlin",
                "scala",
                "html",
                "css",
                "scss",
                "sql",
                "shell",
                "bash",
                "powershell",
                "yaml",
                "json",
                "xml",
                "markdown",
                "toml",
            ];

            if known_langs.contains(&class) {
                return Some(class.to_string());
            }
        }

        // Check data-language attribute
        if let Some(lang) = element.value().attr("data-language") {
            return Some(lang.to_string());
        }

        // Check parent pre element
        if let Some(parent) = element.parent() {
            if let Some(parent_el) = parent.value().as_element() {
                if parent_el.name() == "pre" {
                    if let Some(lang) = parent_el.attr("data-language") {
                        return Some(lang.to_string());
                    }
                }
            }
        }

        None
    }

    /// Find the main content area
    fn find_main_content<'a>(&self, document: &'a Html) -> Option<ElementRef<'a>> {
        for selector in &self.main_content_selectors {
            if let Some(element) = document.select(selector).next() {
                return Some(element);
            }
        }
        None
    }

    /// Remove boilerplate elements from HTML string
    fn remove_boilerplate(&self, html: &str) -> String {
        let document = Html::parse_fragment(html);
        let mut html_output = html.to_string();

        // Remove elements matching boilerplate selectors
        for selector in &self.boilerplate_selectors {
            for element in document.select(selector) {
                let element_html = element.html();
                html_output = html_output.replace(&element_html, "");
            }
        }

        html_output
    }

    /// Clean up the markdown output
    fn clean_markdown(&self, markdown: &str, page_url: &Url) -> String {
        let mut result = String::with_capacity(markdown.len());
        let mut prev_was_blank = false;
        let base_domain = page_url.host_str().unwrap_or("");

        for line in markdown.lines() {
            let trimmed = line.trim();

            // Skip empty lines that would create too much whitespace
            if trimmed.is_empty() {
                if !prev_was_blank {
                    result.push('\n');
                    prev_was_blank = true;
                }
                continue;
            }
            prev_was_blank = false;

            // Process the line
            let mut processed = line.to_string();

            // Skip internal links if configured
            if self.config.skip_internal_links {
                processed = self.skip_internal_links(&processed, base_domain);
            }

            result.push_str(&processed);
            result.push('\n');
        }

        // Trim trailing whitespace but keep one newline
        let trimmed = result.trim_end();
        if !trimmed.is_empty() {
            format!("{}\n", trimmed)
        } else {
            String::new()
        }
    }

    /// Convert internal links to plain text
    fn skip_internal_links(&self, line: &str, base_domain: &str) -> String {
        // Simple regex-free approach for common link patterns
        let mut result = line.to_string();

        // Pattern: [text](url) - convert internal links to just text
        let mut start = 0;
        while let Some(bracket_start) = result[start..].find('[') {
            let abs_bracket_start = start + bracket_start;
            if let Some(bracket_end) = result[abs_bracket_start..].find("](") {
                let abs_bracket_end = abs_bracket_start + bracket_end;
                if let Some(paren_end) = result[abs_bracket_end + 2..].find(')') {
                    let abs_paren_end = abs_bracket_end + 2 + paren_end;
                    let url = &result[abs_bracket_end + 2..abs_paren_end];

                    // Check if internal link
                    let is_internal = url.starts_with('/')
                        || url.starts_with('#')
                        || url.contains(base_domain);

                    if is_internal {
                        let text = &result[abs_bracket_start + 1..abs_bracket_end];
                        let full_link = &result[abs_bracket_start..=abs_paren_end];
                        result = result.replace(full_link, text);
                        // Don't advance start, check from same position
                        continue;
                    }
                }
            }
            start = abs_bracket_start + 1;
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_title() {
        let extractor = ContentExtractor::default();
        let html = r#"
            <html>
            <head><title>Test Page - Site Name</title></head>
            <body><h1>Welcome</h1></body>
            </html>
        "#;
        let document = Html::parse_document(html);
        let title = extractor.extract_title(&document);
        assert_eq!(title, "Test Page - Site Name");
    }

    #[test]
    fn test_extract_headers_dom_order() {
        let extractor = ContentExtractor::default();
        let html = r#"
            <html><body>
                <h1>First Header</h1>
                <h3>Third Level</h3>
                <h2>Second Level</h2>
                <h2>Another Second</h2>
            </body></html>
        "#;
        let document = Html::parse_document(html);
        let headers = extractor.extract_headers_dom_order(&document);

        assert_eq!(headers.len(), 4);
        assert_eq!(headers[0].text, "First Header");
        assert_eq!(headers[0].level, 1);
        assert_eq!(headers[1].text, "Third Level");
        assert_eq!(headers[1].level, 3);
        assert_eq!(headers[2].text, "Second Level");
        assert_eq!(headers[2].level, 2);
    }

    #[test]
    fn test_detect_code_language() {
        let extractor = ContentExtractor::default();
        let html = r#"<code class="language-rust">fn main() {}</code>"#;
        let document = Html::parse_fragment(html);
        let code_selector = Selector::parse("code").unwrap();
        let code = document.select(&code_selector).next().unwrap();
        let lang = extractor.detect_code_language(&code);
        assert_eq!(lang, Some("rust".to_string()));
    }

    #[test]
    fn test_skip_internal_links() {
        let extractor = ContentExtractor::default();
        let line = "Check out [the docs](/docs) and [external](https://example.com).";
        let result = extractor.skip_internal_links(line, "mysite.com");
        assert!(result.contains("the docs"));
        assert!(result.contains("[external](https://example.com)"));
        assert!(!result.contains("[the docs]"));
    }

    // ========================================================================
    // Content Source Validation Tests
    // ========================================================================

    #[test]
    fn test_content_source_is_html() {
        // CRITICAL: Verify default config uses raw HTML to preserve code blocks
        let config = MarkdownConfig::default();
        assert_eq!(
            config.content_source,
            ContentSource::Html,
            "CRITICAL: Must use raw HTML to preserve code blocks"
        );
    }

    #[test]
    fn test_code_block_preservation_integration() {
        // Verify code blocks are extracted and preserved completely
        let extractor = ContentExtractor::default();
        let html = r#"
            <html><body>
                <p>Some intro text.</p>
                <pre><code class="language-python">def hello():
    print("Hello, World!")
    return True</code></pre>
                <p>Some outro text.</p>
            </body></html>
        "#;
        let url = Url::parse("https://example.com/").unwrap();
        let extracted = extractor.extract(html, &url);

        // Code block should be extracted
        assert!(!extracted.code_blocks.is_empty(), "Code block must be extracted");

        // Code content should be preserved completely
        let code_block = &extracted.code_blocks[0];
        assert!(
            code_block.content.contains("def hello()"),
            "Code block must preserve function definition"
        );
        assert!(
            code_block.content.contains("return True"),
            "Code block must preserve complete code"
        );

        // Language should be detected
        assert_eq!(
            code_block.language,
            Some("python".to_string()),
            "Language must be detected"
        );
    }

    // ========================================================================
    // Readability and Boilerplate Removal Tests
    // ========================================================================

    #[test]
    fn test_readability_extracts_article_content() {
        let html = r#"
            <html>
            <head><title>Test Article</title></head>
            <body>
                <nav>Home | About | Contact</nav>
                <header>Site Header</header>
                <article>
                    <h1>Main Article Title</h1>
                    <p>This is the main content of the article. It contains
                    substantial text that should be extracted by Readability.
                    We need enough content here to pass the minimum threshold.</p>
                    <p>Another paragraph with more meaningful content that adds
                    to the overall word count and helps demonstrate the extraction.</p>
                    <p>A third paragraph to ensure we have sufficient content
                    for the Readability algorithm to work properly.</p>
                </article>
                <aside>Related Articles</aside>
                <footer>Copyright 2024</footer>
            </body>
            </html>
        "#;

        let extractor = ContentExtractor::default();
        let url = Url::parse("https://example.com/article").unwrap();
        let extracted = extractor.extract(html, &url);

        // Should extract article content
        assert!(
            extracted.markdown.contains("Main Article Title")
                || extracted.markdown.contains("main content"),
            "Should extract article content"
        );

        // Should NOT contain boilerplate
        assert!(
            !extracted.markdown.contains("Home | About | Contact"),
            "Should not contain navigation"
        );
    }

    #[test]
    fn test_navigation_content_not_extracted() {
        // This is the specific problem case from the bug report
        let html = r#"
            <html>
            <head><title>Developer Docs</title></head>
            <body>
                <nav>Developer Guide API Reference MCP Resources Release Notes English Log in Search...</nav>
                <main>
                    <h1>Getting Started</h1>
                    <p>Welcome to the documentation. This guide will help you
                    get started with our platform. We'll cover installation,
                    configuration, and basic usage patterns.</p>
                    <p>First, you'll need to install the required dependencies.
                    Then configure your environment variables and API keys.</p>
                </main>
            </body>
            </html>
        "#;

        let extractor = ContentExtractor::default();
        let url = Url::parse("https://example.com/docs").unwrap();
        let extracted = extractor.extract(html, &url);

        // Should have "Getting Started" content
        assert!(
            extracted.markdown.contains("Getting Started")
                || extracted.markdown.contains("documentation"),
            "Should extract main content"
        );

        // Should NOT have navigation spam
        assert!(
            !extracted.markdown.contains("Developer Guide API Reference MCP"),
            "Should not contain navigation spam"
        );
        assert!(
            !extracted.markdown.contains("Log in Search"),
            "Should not contain login/search navigation"
        );
    }

    #[test]
    fn test_code_blocks_preserved_with_readability() {
        let html = r#"
            <html><body>
                <nav>Navigation</nav>
                <article>
                    <h1>Code Tutorial</h1>
                    <p>Here is some example code for the tutorial. This paragraph
                    provides context for the code example that follows.</p>
                    <pre><code class="language-rust">fn main() {
    println!("Hello");
}</code></pre>
                    <p>After running the code above, you should see output.</p>
                </article>
                <footer>Footer</footer>
            </body></html>
        "#;

        let extractor = ContentExtractor::default();
        let url = Url::parse("https://example.com").unwrap();
        let extracted = extractor.extract(html, &url);

        // Code blocks should be extracted
        assert!(!extracted.code_blocks.is_empty(), "Code blocks must be extracted");
        assert!(
            extracted.code_blocks[0].content.contains("fn main()"),
            "Code block must preserve function definition"
        );
        assert_eq!(
            extracted.code_blocks[0].language,
            Some("rust".to_string()),
            "Language must be detected"
        );
    }

    #[test]
    fn test_content_hash_computed() {
        let html = r#"
            <html><body>
                <article>
                    <p>Test content for hash computation. This needs enough text
                    to pass the minimum content threshold for extraction.</p>
                </article>
            </body></html>
        "#;
        let extractor = ContentExtractor::default();
        let url = Url::parse("https://example.com").unwrap();
        let extracted = extractor.extract(html, &url);

        // Hash should be present and non-empty
        assert!(!extracted.content_hash.is_empty(), "Hash should be computed");
        assert_eq!(
            extracted.content_hash.len(),
            64,
            "SHA-256 hex should be 64 chars"
        );
    }

    #[test]
    fn test_char_count_computed() {
        let html = r#"
            <html><body>
                <article>
                    <p>Test content for counting characters.</p>
                </article>
            </body></html>
        "#;
        let extractor = ContentExtractor::default();
        let url = Url::parse("https://example.com").unwrap();
        let extracted = extractor.extract(html, &url);

        // Char count should be positive
        assert!(extracted.char_count > 0, "Char count should be computed");
    }

    #[test]
    fn test_is_mostly_boilerplate_detects_nav() {
        let extractor = ContentExtractor::default();

        // Navigation-heavy content
        let nav_content = "Home Log in Sign up Search Menu Navigation";
        assert!(
            extractor.is_mostly_boilerplate(nav_content),
            "Should detect navigation as boilerplate"
        );

        // Real article content
        let article_content = "This is a comprehensive guide to building web applications. \
            We will cover frontend development, backend APIs, and deployment strategies. \
            The tutorial includes code examples and best practices.";
        assert!(
            !extractor.is_mostly_boilerplate(article_content),
            "Should not mark article content as boilerplate"
        );
    }

    #[test]
    fn test_text_density_calculation() {
        let extractor = ContentExtractor::default();

        // High density content (mostly text)
        let html = "<p>This is plain text content without many tags</p>";
        let doc = Html::parse_fragment(html);
        let p_selector = Selector::parse("p").unwrap();
        let p = doc.select(&p_selector).next().unwrap();
        let density = extractor.calculate_text_density(&p);
        assert!(
            density > 0.5,
            "Simple paragraph should have high text density"
        );

        // Low density content (many tags)
        let html_low = r##"<div><span><a href="#"><b><i>X</i></b></a></span></div>"##;
        let doc_low = Html::parse_fragment(html_low);
        let div_selector = Selector::parse("div").unwrap();
        let div = doc_low.select(&div_selector).next().unwrap();
        let density_low = extractor.calculate_text_density(&div);
        assert!(
            density_low < 0.2,
            "Heavily nested tags should have low text density"
        );
    }

    #[test]
    fn test_compute_hash_deterministic() {
        let content1 = "Test content for hashing";
        let content2 = "Test content for hashing";
        let content3 = "Different content";

        let hash1 = ContentExtractor::compute_hash(content1);
        let hash2 = ContentExtractor::compute_hash(content2);
        let hash3 = ContentExtractor::compute_hash(content3);

        assert_eq!(hash1, hash2, "Same content should produce same hash");
        assert_ne!(hash1, hash3, "Different content should produce different hash");
    }
}
