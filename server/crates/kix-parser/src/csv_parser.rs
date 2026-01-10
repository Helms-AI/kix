//! CSV parser for EIP Knowledge System
//!
//! Parses CSV files and extracts text content.

use csv::ReaderBuilder;
use sha2::{Digest, Sha256};
use std::path::Path;

use crate::document::{Entry, EntryType, SourceType};
use crate::error::ParseError;

/// Parser for CSV documents
pub struct CsvParser {
    /// Whether to treat the first row as headers
    has_headers: bool,
    /// Maximum rows to parse (0 = all)
    max_rows: usize,
}

impl CsvParser {
    /// Create a new CSV parser with default settings
    pub fn new() -> Self {
        Self {
            has_headers: true,
            max_rows: 10000,
        }
    }

    /// Create parser with custom settings
    pub fn with_config(has_headers: bool, max_rows: usize) -> Self {
        Self {
            has_headers,
            max_rows,
        }
    }

    /// Parse a CSV file
    pub fn parse(&self, file_path: &str) -> Result<Entry, ParseError> {
        // Read file content
        let content = std::fs::read_to_string(file_path)
            .map_err(|e| ParseError::Io(e.to_string()))?;

        self.parse_content(&content, file_path)
    }

    /// Parse CSV content string
    pub fn parse_content(&self, content: &str, source_path: &str) -> Result<Entry, ParseError> {
        let path = Path::new(source_path);

        // Compute hash
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let source_hash = format!("{:x}", hasher.finalize());

        // Extract title from filename
        let title = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| self.format_title(s))
            .unwrap_or_else(|| "Untitled CSV".to_string());

        // Generate entry ID from path
        let id = Entry::generate_id_from_path(source_path);

        // Parse CSV
        let mut reader = ReaderBuilder::new()
            .has_headers(self.has_headers)
            .flexible(true)
            .from_reader(content.as_bytes());

        let mut all_content = String::new();
        let mut headers: Vec<String> = Vec::new();
        let mut row_count = 0;

        // Get headers if available
        if self.has_headers {
            if let Ok(header_record) = reader.headers() {
                headers = header_record.iter().map(|s| s.to_string()).collect();
                all_content.push_str("Headers: ");
                all_content.push_str(&headers.join(" | "));
                all_content.push_str("\n\n");
            }
        }

        // Process rows
        for result in reader.records() {
            if self.max_rows > 0 && row_count >= self.max_rows {
                all_content.push_str(&format!("\n... (truncated at {} rows)\n", self.max_rows));
                break;
            }

            if let Ok(record) = result {
                let row: Vec<&str> = record.iter().collect();

                // Create structured representation if we have headers
                if !headers.is_empty() && headers.len() == row.len() {
                    let structured: Vec<String> = headers
                        .iter()
                        .zip(row.iter())
                        .filter(|(_, v)| !v.is_empty())
                        .map(|(h, v)| format!("{}: {}", h, v))
                        .collect();
                    all_content.push_str(&structured.join(", "));
                } else {
                    all_content.push_str(&row.join(" | "));
                }
                all_content.push('\n');
                row_count += 1;
            }
        }

        // Description from first data row or headers
        let description: String = if !headers.is_empty() {
            format!("CSV data with columns: {}", headers.join(", "))
        } else {
            format!("CSV data with {} rows", row_count)
        };

        let entry = Entry::with_id(id, title, source_path.to_string(), source_hash)
            .with_description(description.chars().take(200).collect())
            .with_content(all_content)
            .with_tags(headers)
            .with_entry_type(EntryType::Other)
            .with_source_type(SourceType::Csv);

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

impl Default for CsvParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_csv() {
        let parser = CsvParser::new();
        let content = "name,age,city\nAlice,30,NYC\nBob,25,LA";
        let entry = parser.parse_content(content, "people.csv").unwrap();

        assert_eq!(entry.title, "People");
        assert!(entry.content.contains("Alice"));
        assert!(entry.tags.contains(&"name".to_string()));
        assert_eq!(entry.source_type, SourceType::Csv);
    }

    #[test]
    fn test_csv_no_headers() {
        let parser = CsvParser::with_config(false, 100);
        let content = "Alice,30,NYC\nBob,25,LA";
        let entry = parser.parse_content(content, "data.csv").unwrap();

        assert!(entry.tags.is_empty());
        assert!(entry.content.contains("Alice | 30 | NYC"));
    }

    #[test]
    fn test_format_title() {
        let parser = CsvParser::new();
        assert_eq!(parser.format_title("user-data-export"), "User Data Export");
    }
}
