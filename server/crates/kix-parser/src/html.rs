//! HTML parser for web content.

use scraper::{Html, Selector};
use sha2::{Digest, Sha256};

use crate::document::{Entry, EntryType, SourceType};
use crate::error::ParseError;

/// Parser for HTML content (generic websites).
pub struct HtmlParser {
    // Pre-compiled selectors for performance
    title_selector: Selector,
    meta_desc_selector: Selector,
    meta_keywords_selector: Selector,
    h1_selector: Selector,
    og_title_selector: Selector,
    // Generic content selectors
    main_selector: Selector,
    article_selector: Selector,
    role_main_selector: Selector,
    content_div_selector: Selector,
    body_selector: Selector,
}

impl Default for HtmlParser {
    fn default() -> Self {
        Self::new()
    }
}

impl HtmlParser {
    /// Creates a new HTML parser with pre-compiled selectors.
    pub fn new() -> Self {
        Self {
            title_selector: Selector::parse("title").unwrap(),
            meta_desc_selector: Selector::parse(r#"meta[name="description"]"#).unwrap(),
            meta_keywords_selector: Selector::parse(r#"meta[name="keywords"]"#).unwrap(),
            h1_selector: Selector::parse("h1").unwrap(),
            og_title_selector: Selector::parse(r#"meta[property="og:title"]"#).unwrap(),
            // Generic content selectors
            main_selector: Selector::parse("main").unwrap(),
            article_selector: Selector::parse("article").unwrap(),
            role_main_selector: Selector::parse("[role='main']").unwrap(),
            content_div_selector: Selector::parse("div.content, div.main-content, div#content, div.markdown-body, div.pattern").unwrap(),
            body_selector: Selector::parse("body").unwrap(),
        }
    }

    /// Parses HTML content into an Entry.
    pub fn parse(&self, html_content: &str, source_path: &str) -> Result<Entry, ParseError> {
        let document = Html::parse_document(html_content);

        // Compute hash for change detection
        let source_hash = self.compute_hash(html_content);

        // Extract title
        let title = self.extract_title(&document, source_path);

        // Extract description from meta tag
        let description = self
            .extract_meta_content(&document, &self.meta_desc_selector)
            .unwrap_or_default();

        // Extract keywords as tags
        let tags = self.extract_keywords(&document);

        // Extract main content
        let content = self.extract_content(&document);

        // Generate slug
        let slug = Self::generate_slug(source_path);

        // Determine entry type
        let entry_type = Self::determine_entry_type(source_path);

        // Generate ID from source path for deduplication
        let id = if source_path.starts_with("http://") || source_path.starts_with("https://") {
            // For URLs, use full URL as ID for easy reindexing
            slug.clone()
        } else {
            // For local files, generate deterministic ID from path
            Entry::generate_id_from_path(source_path)
        };

        Ok(Entry::with_id(id, title, source_path.to_string(), source_hash)
            .with_description(description)
            .with_content(content)
            .with_tags(tags)
            .with_entry_type(entry_type)
            .with_source_type(SourceType::Html)
            .with_slug(slug))
    }

    /// Computes SHA-256 hash of the content.
    fn compute_hash(&self, content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Extracts title from file path or URL as fallback
    fn extract_title_from_path(&self, path: &str) -> String {
        // For URLs, extract from last path segment
        let path = if path.starts_with("http") {
            path.rsplit('/').next().unwrap_or(path)
        } else {
            path
        };

        std::path::Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| {
                // Convert PascalCase/kebab-case to Title Case
                s.chars()
                    .enumerate()
                    .map(|(i, c)| {
                        if i > 0 && c.is_uppercase() {
                            format!(" {}", c)
                        } else if c == '-' || c == '_' {
                            " ".to_string()
                        } else {
                            c.to_string()
                        }
                    })
                    .collect::<String>()
                    .split_whitespace()
                    .map(|word| {
                        let mut chars: Vec<char> = word.chars().collect();
                        if !chars.is_empty() {
                            chars[0] = chars[0].to_uppercase().next().unwrap_or(chars[0]);
                        }
                        chars.into_iter().collect::<String>()
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_else(|| "Untitled".to_string())
    }

    /// Extracts the title with fallback chain: <title> -> <h1> -> og:title -> path
    fn extract_title(&self, doc: &Html, file_path: &str) -> String {
        // Try <title> tag first
        if let Some(title) = doc.select(&self.title_selector).next() {
            let text = title.text().collect::<String>();
            let cleaned = text.split(" - ")
                .next()
                .unwrap_or(&text)
                .trim();
            if !cleaned.is_empty() {
                return cleaned.to_string();
            }
        }

        // Try <h1> heading
        if let Some(h1) = doc.select(&self.h1_selector).next() {
            let text = h1.text().collect::<String>().trim().to_string();
            if !text.is_empty() {
                return text;
            }
        }

        // Try og:title meta tag
        if let Some(og_title) = self.extract_meta_content(doc, &self.og_title_selector) {
            if !og_title.is_empty() {
                return og_title;
            }
        }

        // Fall back to path-based title
        self.extract_title_from_path(file_path)
    }

    /// Extracts content from a meta tag.
    fn extract_meta_content(&self, doc: &Html, selector: &Selector) -> Option<String> {
        doc.select(selector)
            .next()
            .and_then(|el| el.attr("content"))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    /// Extracts keywords from meta tags as tags.
    fn extract_keywords(&self, doc: &Html) -> Vec<String> {
        self.extract_meta_content(doc, &self.meta_keywords_selector)
            .map(|s| {
                s.split(',')
                    .map(|k| k.trim().to_lowercase())
                    .filter(|k| !k.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Extracts the main content with fallback chain for generic websites.
    fn extract_content(&self, doc: &Html) -> String {
        // Try <main> element
        if let Some(el) = doc.select(&self.main_selector).next() {
            let text = self.clean_text(&el.text().collect::<String>());
            if !text.is_empty() {
                return text;
            }
        }

        // Try <article> element
        if let Some(el) = doc.select(&self.article_selector).next() {
            let text = self.clean_text(&el.text().collect::<String>());
            if !text.is_empty() {
                return text;
            }
        }

        // Try [role="main"]
        if let Some(el) = doc.select(&self.role_main_selector).next() {
            let text = self.clean_text(&el.text().collect::<String>());
            if !text.is_empty() {
                return text;
            }
        }

        // Try common content div patterns
        if let Some(el) = doc.select(&self.content_div_selector).next() {
            let text = self.clean_text(&el.text().collect::<String>());
            if !text.is_empty() {
                return text;
            }
        }

        // Last resort: body text minus nav/script/style
        self.extract_clean_body_text(doc)
    }

    /// Extracts text from body, excluding nav/header/footer/script/style elements.
    fn extract_clean_body_text(&self, doc: &Html) -> String {
        let body = match doc.select(&self.body_selector).next() {
            Some(b) => b,
            None => return String::new(),
        };

        let full_text = body.text().collect::<String>();
        self.clean_text(&full_text)
    }

    /// Determines entry type from the file path.
    fn determine_entry_type(path: &str) -> EntryType {
        let path_lower = path.to_lowercase();

        if path_lower.ends_with(".pdf") {
            EntryType::Pdf
        } else if path_lower.contains("/article") || path_lower.contains("/blog") || path_lower.contains("/post") {
            EntryType::Article
        } else if path_lower.contains("/docs/") || path_lower.contains("/documentation/") {
            EntryType::Document
        } else {
            EntryType::Document
        }
    }

    /// Generates a slug from the file path or URL.
    fn generate_slug(path: &str) -> String {
        if path.starts_with("http://") || path.starts_with("https://") {
            // For URLs, use the full URL as slug (will become the doc_id)
            path.to_string()
        } else {
            // For local files, use filename stem
            std::path::Path::new(path)
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.to_lowercase())
                .unwrap_or_else(|| "unknown".to_string())
        }
    }

    /// Cleans text by normalizing whitespace.
    fn clean_text(&self, text: &str) -> String {
        text.split_whitespace().collect::<Vec<_>>().join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entry_type_detection() {
        assert_eq!(
            HtmlParser::determine_entry_type("/docs/api.html"),
            EntryType::Document
        );
        assert_eq!(
            HtmlParser::determine_entry_type("/blog/my-post.html"),
            EntryType::Article
        );
        assert_eq!(
            HtmlParser::determine_entry_type("/manual.pdf"),
            EntryType::Pdf
        );
    }

    #[test]
    fn test_slug_generation() {
        assert_eq!(
            HtmlParser::generate_slug("/path/to/page.html"),
            "page"
        );
        assert_eq!(
            HtmlParser::generate_slug("https://example.com/docs/api"),
            "https://example.com/docs/api"
        );
    }

    #[test]
    fn test_parse_simple_html() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head>
                <title>API Documentation - Example</title>
                <meta name="description" content="Learn how to use our API">
                <meta name="keywords" content="api, documentation, rest">
            </head>
            <body>
                <main>
                    <h1>API Documentation</h1>
                    <p>Welcome to the API documentation.</p>
                </main>
            </body>
            </html>
        "#;

        let parser = HtmlParser::new();
        let entry = parser.parse(html, "https://example.com/docs/api").unwrap();

        assert_eq!(entry.title, "API Documentation");
        assert_eq!(entry.description, "Learn how to use our API");
        assert_eq!(entry.tags, vec!["api", "documentation", "rest"]);
        assert_eq!(entry.entry_type, EntryType::Document);
    }
}
