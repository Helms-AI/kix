//! Plain text parser for EIP Knowledge System
//!
//! Parses plain text files and source code.

use sha2::{Digest, Sha256};
use std::path::Path;

use crate::document::{Entry, EntryType, SourceType};
use crate::error::ParseError;

/// Parser for plain text and source code files
pub struct TextParser;

impl TextParser {
    /// Create a new text parser
    pub fn new() -> Self {
        Self
    }

    /// Parse a text file
    pub fn parse(&self, file_path: &str) -> Result<Entry, ParseError> {
        // Read file content
        let content = std::fs::read_to_string(file_path)
            .map_err(|e| ParseError::Io(e.to_string()))?;

        self.parse_content(&content, file_path)
    }

    /// Parse text content string
    pub fn parse_content(&self, content: &str, source_path: &str) -> Result<Entry, ParseError> {
        let path = Path::new(source_path);

        // Compute hash
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let source_hash = format!("{:x}", hasher.finalize());

        // Get file extension
        let extension = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("txt");

        // Determine source type
        let source_type = SourceType::from_extension(extension);

        // Extract title from filename
        let title = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| self.format_title(s))
            .unwrap_or_else(|| "Untitled".to_string());

        // Generate entry ID from path
        let id = Entry::generate_id_from_path(source_path);

        let entry_type = if source_type == SourceType::SourceCode {
            EntryType::Code
        } else {
            EntryType::Article
        };

        // Extract description (first non-empty line or first N chars)
        let description: String = content
            .lines()
            .filter(|line| !line.trim().is_empty())
            .next()
            .map(|s| s.chars().take(200).collect())
            .unwrap_or_default();

        // Extract tags based on file type
        let tags = if source_type == SourceType::SourceCode {
            self.extract_code_keywords(content, extension)
        } else {
            self.extract_text_keywords(content)
        };

        let entry = Entry::with_id(id, title, source_path.to_string(), source_hash)
            .with_description(description)
            .with_content(content.to_string())
            .with_tags(tags)
            .with_entry_type(entry_type)
            .with_source_type(source_type);

        Ok(entry)
    }

    /// Extract keywords from source code
    fn extract_code_keywords(&self, content: &str, extension: &str) -> Vec<String> {
        let mut keywords = Vec::new();

        // Add language as keyword
        let language = self.extension_to_language(extension);
        keywords.push(language.to_string());

        // Extract function/class definitions based on language
        for line in content.lines() {
            let trimmed = line.trim();

            // Python: def function_name, class ClassName
            if trimmed.starts_with("def ") || trimmed.starts_with("class ") {
                if let Some(name) = self.extract_identifier(trimmed) {
                    keywords.push(name);
                }
            }
            // Rust: fn function_name, struct StructName, impl Trait
            else if trimmed.starts_with("fn ")
                || trimmed.starts_with("struct ")
                || trimmed.starts_with("enum ")
                || trimmed.starts_with("impl ")
            {
                if let Some(name) = self.extract_identifier(trimmed) {
                    keywords.push(name);
                }
            }
            // JavaScript/TypeScript: function name, class Name, const NAME
            else if trimmed.starts_with("function ")
                || trimmed.starts_with("class ")
                || trimmed.starts_with("export ")
            {
                if let Some(name) = self.extract_identifier(trimmed) {
                    keywords.push(name);
                }
            }

            // Limit keywords
            if keywords.len() >= 20 {
                break;
            }
        }

        keywords
    }

    /// Extract identifier from definition line
    fn extract_identifier(&self, line: &str) -> Option<String> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            let name = parts[1]
                .trim_start_matches("async ")
                .trim_end_matches('(')
                .trim_end_matches(':')
                .trim_end_matches('{')
                .trim_end_matches('<')
                .to_string();
            if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                return Some(name);
            }
        }
        None
    }

    /// Extract keywords from plain text
    fn extract_text_keywords(&self, content: &str) -> Vec<String> {
        // Look for potential headings or important lines
        content
            .lines()
            .filter(|line| {
                let trimmed = line.trim();
                !trimmed.is_empty()
                    && trimmed.len() < 80
                    && trimmed.chars().filter(|c| c.is_uppercase()).count() > 1
            })
            .take(10)
            .map(|s| s.trim().to_string())
            .collect()
    }

    /// Map file extension to language name
    fn extension_to_language(&self, ext: &str) -> &str {
        match ext.to_lowercase().as_str() {
            "rs" => "Rust",
            "py" => "Python",
            "js" => "JavaScript",
            "ts" => "TypeScript",
            "jsx" => "React JSX",
            "tsx" => "React TSX",
            "java" => "Java",
            "go" => "Go",
            "c" => "C",
            "cpp" | "cc" | "cxx" => "C++",
            "h" | "hpp" => "C/C++ Header",
            "cs" => "C#",
            "rb" => "Ruby",
            "php" => "PHP",
            "swift" => "Swift",
            "kt" => "Kotlin",
            "scala" => "Scala",
            "sh" | "bash" | "zsh" => "Shell",
            "yaml" | "yml" => "YAML",
            "toml" => "TOML",
            "json" => "JSON",
            "xml" => "XML",
            "sql" => "SQL",
            "html" | "htm" => "HTML",
            "css" | "scss" | "sass" => "CSS",
            _ => "Text",
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

impl Default for TextParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rust_code() {
        let parser = TextParser::new();
        let content = r#"
//! Module documentation

pub fn main() {
    println!("Hello, world!");
}

struct MyStruct {
    field: i32,
}
"#;
        let entry = parser.parse_content(content, "main.rs").unwrap();
        assert_eq!(entry.source_type, SourceType::SourceCode);
        assert!(entry.tags.contains(&"Rust".to_string()));
    }

    #[test]
    fn test_parse_python_code() {
        let parser = TextParser::new();
        let content = r#"
"""Module docstring."""

def hello_world():
    print("Hello, world!")

class MyClass:
    pass
"#;
        let entry = parser.parse_content(content, "main.py").unwrap();
        assert_eq!(entry.source_type, SourceType::SourceCode);
        assert!(entry.tags.contains(&"Python".to_string()));
    }

    #[test]
    fn test_extension_to_language() {
        let parser = TextParser::new();
        assert_eq!(parser.extension_to_language("rs"), "Rust");
        assert_eq!(parser.extension_to_language("py"), "Python");
        assert_eq!(parser.extension_to_language("tsx"), "React TSX");
    }
}
