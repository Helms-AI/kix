//! Markdown parser for EIP Knowledge System
//!
//! Parses Markdown files and extracts structured content.

use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use sha2::{Digest, Sha256};

use crate::document::{Entry, EntryType, SourceType};
use crate::error::ParseError;

/// Parser for Markdown documents
pub struct MarkdownParser;

impl MarkdownParser {
    /// Create a new Markdown parser
    pub fn new() -> Self {
        Self
    }

    /// Parse a Markdown file
    pub fn parse(&self, content: &str, source_path: &str) -> Result<Entry, ParseError> {
        // Compute hash of source content
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let source_hash = format!("{:x}", hasher.finalize());

        // Extract title from first heading or filename
        let title = self.extract_title(content)
            .unwrap_or_else(|| self.extract_title_from_path(source_path));

        // Generate entry ID from path
        let id = Entry::generate_id_from_path(source_path);

        // Extract plain text content
        let text_content = self.extract_text(content);

        // Extract description (first paragraph)
        let description = self.extract_description(content);

        // Extract headings as tags
        let tags = self.extract_headings(content);

        let entry = Entry::with_id(id, title, source_path.to_string(), source_hash)
            .with_description(description)
            .with_content(text_content)
            .with_tags(tags)
            .with_entry_type(EntryType::Article)
            .with_source_type(SourceType::Markdown);

        Ok(entry)
    }

    /// Extract title from first H1 heading
    fn extract_title(&self, content: &str) -> Option<String> {
        let parser = Parser::new(content);
        let mut in_heading = false;
        let mut title = String::new();

        for event in parser {
            match event {
                Event::Start(Tag::Heading { level, .. }) if level == pulldown_cmark::HeadingLevel::H1 => {
                    in_heading = true;
                }
                Event::End(TagEnd::Heading(level)) if level == pulldown_cmark::HeadingLevel::H1 => {
                    if !title.is_empty() {
                        return Some(title.trim().to_string());
                    }
                    in_heading = false;
                }
                Event::Text(text) if in_heading => {
                    title.push_str(&text);
                }
                _ => {}
            }
        }

        None
    }

    /// Extract title from file path
    fn extract_title_from_path(&self, path: &str) -> String {
        std::path::Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| {
                // Convert kebab-case or snake_case to Title Case
                s.replace('-', " ")
                    .replace('_', " ")
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

    /// Extract plain text from markdown
    fn extract_text(&self, content: &str) -> String {
        let parser = Parser::new(content);
        let mut text = String::new();

        for event in parser {
            match event {
                Event::Text(t) | Event::Code(t) => {
                    text.push_str(&t);
                    text.push(' ');
                }
                Event::SoftBreak | Event::HardBreak => {
                    text.push('\n');
                }
                Event::End(TagEnd::Paragraph) => {
                    text.push_str("\n\n");
                }
                _ => {}
            }
        }

        text.trim().to_string()
    }

    /// Extract first paragraph as description
    fn extract_description(&self, content: &str) -> String {
        let parser = Parser::new(content);
        let mut in_paragraph = false;
        let mut description = String::new();
        let mut found_first = false;

        for event in parser {
            match event {
                Event::Start(Tag::Paragraph) => {
                    in_paragraph = true;
                }
                Event::End(TagEnd::Paragraph) => {
                    if !description.is_empty() && !found_first {
                        found_first = true;
                        // Skip if it's just a heading continuation
                        if description.len() > 20 {
                            return description.trim().to_string();
                        }
                        description.clear();
                    }
                    in_paragraph = false;
                }
                Event::Text(text) if in_paragraph => {
                    description.push_str(&text);
                }
                _ => {}
            }
        }

        description.chars().take(200).collect::<String>().trim().to_string()
    }

    /// Extract headings as keywords
    fn extract_headings(&self, content: &str) -> Vec<String> {
        let parser = Parser::new(content);
        let mut headings = Vec::new();
        let mut in_heading = false;
        let mut current_heading = String::new();

        for event in parser {
            match event {
                Event::Start(Tag::Heading { .. }) => {
                    in_heading = true;
                    current_heading.clear();
                }
                Event::End(TagEnd::Heading(_)) => {
                    if !current_heading.is_empty() {
                        headings.push(current_heading.trim().to_string());
                    }
                    in_heading = false;
                }
                Event::Text(text) if in_heading => {
                    current_heading.push_str(&text);
                }
                _ => {}
            }
        }

        headings
    }
}

impl Default for MarkdownParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_markdown() {
        let parser = MarkdownParser::new();
        let content = r#"# Hello World

This is a test document.

## Section 1

Some content here.
"#;

        let entry = parser.parse(content, "test.md").unwrap();
        assert_eq!(entry.title, "Hello World");
        assert!(entry.content.contains("test document"));
        assert!(entry.tags.contains(&"Section 1".to_string()));
    }

    #[test]
    fn test_extract_title_from_path() {
        let parser = MarkdownParser::new();
        assert_eq!(
            parser.extract_title_from_path("my-awesome-doc.md"),
            "My Awesome Doc"
        );
        assert_eq!(
            parser.extract_title_from_path("snake_case_file.md"),
            "Snake Case File"
        );
    }
}
