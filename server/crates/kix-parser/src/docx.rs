//! DOCX parser for EIP Knowledge System
//!
//! Parses Word documents (DOCX) and extracts text content.

use dotext::{Docx, MsDoc};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;

use crate::document::{Entry, EntryType, SourceType};
use crate::error::ParseError;

/// Parser for DOCX documents
pub struct DocxParser;

impl DocxParser {
    /// Create a new DOCX parser
    pub fn new() -> Self {
        Self
    }

    /// Parse a DOCX file
    pub fn parse(&self, file_path: &str) -> Result<Entry, ParseError> {
        let path = Path::new(file_path);

        // Read file content for hashing
        let file_content = std::fs::read(file_path)
            .map_err(|e| ParseError::Io(e.to_string()))?;

        let mut hasher = Sha256::new();
        hasher.update(&file_content);
        let source_hash = format!("{:x}", hasher.finalize());

        // Extract title from filename
        let title = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| self.format_title(s))
            .unwrap_or_else(|| "Untitled Document".to_string());

        // Generate entry ID from path
        let id = Entry::generate_id_from_path(file_path);

        // Open and extract text from DOCX using dotext
        let mut docx = Docx::open(file_path)
            .map_err(|e| ParseError::Parse(format!("Failed to open DOCX: {:?}", e)))?;

        let mut content = String::new();
        docx.read_to_string(&mut content)
            .map_err(|e| ParseError::Parse(format!("Failed to extract text: {:?}", e)))?;

        // Extract first paragraph as description
        let description: String = content
            .lines()
            .filter(|line: &&str| !line.trim().is_empty())
            .next()
            .map(|s: &str| s.chars().take(200).collect())
            .unwrap_or_default();

        // Extract potential headings (lines that are short and might be headings)
        let tags: Vec<String> = content
            .lines()
            .filter(|line: &&str| {
                let trimmed = line.trim();
                !trimmed.is_empty()
                    && trimmed.len() < 100
                    && !trimmed.contains('.')
                    && trimmed.chars().filter(|c: &char| c.is_uppercase()).count() > 0
            })
            .take(10)
            .map(|s: &str| s.trim().to_string())
            .collect();

        let entry = Entry::with_id(id, title, file_path.to_string(), source_hash)
            .with_description(description)
            .with_content(content)
            .with_tags(tags)
            .with_entry_type(EntryType::Article)
            .with_source_type(SourceType::Docx);

        Ok(entry)
    }

    /// Format title from filename
    fn format_title(&self, name: &str) -> String {
        name.replace('-', " ")
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
    }
}

impl Default for DocxParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_title() {
        let parser = DocxParser::new();
        assert_eq!(parser.format_title("my-document"), "My Document");
        assert_eq!(parser.format_title("project_report_2024"), "Project Report 2024");
    }
}
