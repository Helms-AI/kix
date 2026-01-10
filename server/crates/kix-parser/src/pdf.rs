//! PDF parser for EIP documentation.

use sha2::{Digest, Sha256};
use std::path::Path;

use crate::document::{Entry, EntryType, SourceType};
use crate::error::ParseError;

/// Parser for PDF documents.
pub struct PdfParser;

impl Default for PdfParser {
    fn default() -> Self {
        Self::new()
    }
}

impl PdfParser {
    /// Creates a new PDF parser.
    pub fn new() -> Self {
        Self
    }

    /// Parses a PDF file into an Entry.
    pub fn parse(&self, file_path: &str) -> Result<Entry, ParseError> {
        // Read and extract text from PDF
        let text = pdf_extract::extract_text(file_path)
            .map_err(|e| ParseError::PdfExtraction(e.to_string()))?;

        // Compute hash for change detection
        let source_hash = self.compute_hash(&text);

        // Extract title from filename or first line
        let title = self.extract_title_from_path(file_path);

        // Clean and normalize text
        let content = self.clean_text(&text);

        // Generate description from first paragraph
        let description = self.extract_first_paragraph(&content);

        // Generate entry ID from path
        let id = Entry::generate_id_from_path(file_path);

        // Determine entry type
        let entry_type = self.determine_entry_type(file_path, &content);

        // Generate slug
        let slug = self.generate_slug(file_path);

        let entry = Entry::with_id(id, title, file_path.to_string(), source_hash)
            .with_description(description)
            .with_content(content)
            .with_tags(Vec::new())
            .with_entry_type(entry_type)
            .with_source_type(SourceType::Pdf)
            .with_slug(slug);

        Ok(entry)
    }

    /// Computes SHA-256 hash of the content.
    fn compute_hash(&self, content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Extracts a title from the file path.
    fn extract_title_from_path(&self, path: &str) -> String {
        Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| {
                // Convert filename to readable title
                s.replace('_', " ")
                    .replace('-', " ")
                    // Split CamelCase
                    .chars()
                    .fold(String::new(), |mut acc, c| {
                        if c.is_uppercase() && !acc.is_empty() && !acc.ends_with(' ') {
                            acc.push(' ');
                        }
                        acc.push(c);
                        acc
                    })
                    .trim()
                    .to_string()
            })
            .unwrap_or_else(|| "Unknown Document".to_string())
    }

    /// Cleans and normalizes text content.
    fn clean_text(&self, text: &str) -> String {
        text.lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Extracts the first paragraph as a description.
    fn extract_first_paragraph(&self, content: &str) -> String {
        content
            .lines()
            .take(5)
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .take(300)
            .collect::<String>()
            .trim()
            .to_string()
    }

    /// Generates a slug from the file path.
    fn generate_slug(&self, path: &str) -> String {
        Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| {
                s.to_lowercase()
                    .replace(' ', "-")
                    .replace('_', "-")
            })
            .unwrap_or_else(|| "unknown".to_string())
    }

    /// Determines the entry type based on path and content.
    fn determine_entry_type(&self, path: &str, content: &str) -> EntryType {
        let path_lower = path.to_lowercase();
        let content_lower = content.to_lowercase();

        if path_lower.contains("presentation") || path_lower.contains("slide") {
            EntryType::Article
        } else if content_lower.contains("case study") {
            EntryType::Article
        } else if path_lower.contains("pattern") || content_lower.contains("enterprise integration") {
            EntryType::Document
        } else {
            EntryType::Pdf
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_title_extraction() {
        let parser = PdfParser::new();

        assert_eq!(
            parser.extract_title_from_path("/docs/EnterpriseIntegrationPatterns.pdf"),
            "Enterprise Integration Patterns"
        );

        assert_eq!(
            parser.extract_title_from_path("/docs/my_document_name.pdf"),
            "my document name"
        );
    }

    #[test]
    fn test_slug_generation() {
        let parser = PdfParser::new();

        assert_eq!(
            parser.generate_slug("/docs/My Document.pdf"),
            "my-document"
        );
    }
}
