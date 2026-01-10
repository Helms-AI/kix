//! Excel parser for EIP Knowledge System
//!
//! Parses Excel files (XLSX, XLS) and extracts text content.

use calamine::{open_workbook_auto, Data, Reader};
use sha2::{Digest, Sha256};
use std::path::Path;

use crate::document::{Entry, EntryType, SourceType};
use crate::error::ParseError;

/// Parser for Excel documents
pub struct ExcelParser;

impl ExcelParser {
    /// Create a new Excel parser
    pub fn new() -> Self {
        Self
    }

    /// Parse an Excel file
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
            .unwrap_or_else(|| "Untitled Spreadsheet".to_string());

        // Generate entry ID from path
        let id = Entry::generate_id_from_path(file_path);

        // Open workbook
        let mut workbook = open_workbook_auto(file_path)
            .map_err(|e| ParseError::Parse(format!("Failed to open Excel file: {}", e)))?;

        // Extract content from all sheets
        let mut all_content = String::new();
        let mut description = String::new();
        let sheet_names: Vec<String> = workbook.sheet_names().to_vec();

        for sheet_name in &sheet_names {
            if let Ok(range) = workbook.worksheet_range(sheet_name) {
                all_content.push_str(&format!("\n=== Sheet: {} ===\n", sheet_name));

                for row in range.rows() {
                    let row_text: Vec<String> = row
                        .iter()
                        .map(|cell| self.cell_to_string(cell))
                        .filter(|s| !s.is_empty())
                        .collect();

                    if !row_text.is_empty() {
                        all_content.push_str(&row_text.join(" | "));
                        all_content.push('\n');

                        // Use first non-empty row as description
                        if description.is_empty() && row_text.len() > 1 {
                            description = row_text.join(", ");
                        }
                    }
                }
            }
        }

        let entry = Entry::with_id(id, title, file_path.to_string(), source_hash)
            .with_description(description.chars().take(200).collect())
            .with_content(all_content)
            .with_tags(sheet_names)
            .with_entry_type(EntryType::Other)
            .with_source_type(SourceType::Excel);

        Ok(entry)
    }

    /// Convert cell data to string
    fn cell_to_string(&self, cell: &Data) -> String {
        match cell {
            Data::Int(i) => i.to_string(),
            Data::Float(f) => {
                if f.fract() == 0.0 {
                    format!("{:.0}", f)
                } else {
                    format!("{:.2}", f)
                }
            }
            Data::String(s) => s.clone(),
            Data::Bool(b) => if *b { "Yes" } else { "No" }.to_string(),
            Data::DateTime(dt) => format!("{:.2}", dt),
            Data::DateTimeIso(s) => s.clone(),
            Data::DurationIso(s) => s.clone(),
            Data::Error(e) => format!("Error: {:?}", e),
            Data::Empty => String::new(),
        }
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

impl Default for ExcelParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_title() {
        let parser = ExcelParser::new();
        assert_eq!(parser.format_title("sales-report-2024"), "Sales Report 2024");
        assert_eq!(parser.format_title("user_data"), "User Data");
    }

    #[test]
    fn test_cell_to_string() {
        let parser = ExcelParser::new();
        assert_eq!(parser.cell_to_string(&Data::Int(42)), "42");
        assert_eq!(parser.cell_to_string(&Data::Float(3.14)), "3.14");
        assert_eq!(parser.cell_to_string(&Data::String("test".to_string())), "test");
        assert_eq!(parser.cell_to_string(&Data::Bool(true)), "Yes");
        assert_eq!(parser.cell_to_string(&Data::Empty), "");
    }
}
