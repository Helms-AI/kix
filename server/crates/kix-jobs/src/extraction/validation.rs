//! Code validation logic for filtering non-code content.

/// Configuration for code validation
#[derive(Debug, Clone)]
pub struct ValidationConfig {
    /// Minimum code length in characters
    pub min_length: usize,

    /// Maximum prose ratio (0.0-1.0)
    pub max_prose_ratio: f32,

    /// Filter placeholder text
    pub filter_placeholders: bool,

    /// Minimum line count for multi-line code
    pub min_lines: usize,
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            min_length: 10,
            max_prose_ratio: 0.6,
            filter_placeholders: true,
            min_lines: 1,
        }
    }
}

/// Validation statistics
#[derive(Debug, Default, Clone)]
pub struct ValidationStats {
    pub passed: usize,
    pub filtered_too_short: usize,
    pub filtered_high_prose: usize,
    pub filtered_placeholder: usize,
    pub filtered_no_structure: usize,
}

impl ValidationStats {
    pub fn total_filtered(&self) -> usize {
        self.filtered_too_short
            + self.filtered_high_prose
            + self.filtered_placeholder
            + self.filtered_no_structure
    }

    pub fn total(&self) -> usize {
        self.passed + self.total_filtered()
    }
}

/// Result of code validation
#[derive(Debug, Clone)]
pub enum ValidationResult {
    Valid,
    TooShort,
    Placeholder,
    NoStructure,
    HighProseRatio(f32),
}

impl ValidationResult {
    pub fn is_valid(&self) -> bool {
        matches!(self, Self::Valid)
    }
}

/// Validate code content
pub fn validate_code(content: &str, config: &ValidationConfig) -> ValidationResult {
    // Length check
    if content.len() < config.min_length {
        return ValidationResult::TooShort;
    }

    // Placeholder check
    if config.filter_placeholders && is_placeholder(content) {
        return ValidationResult::Placeholder;
    }

    // Code structure check
    if !has_code_structure(content) {
        return ValidationResult::NoStructure;
    }

    // Prose ratio check
    let prose_ratio = calculate_prose_ratio(content);
    if prose_ratio > config.max_prose_ratio {
        return ValidationResult::HighProseRatio(prose_ratio);
    }

    ValidationResult::Valid
}

/// Check if content is placeholder text
fn is_placeholder(content: &str) -> bool {
    let lower = content.to_lowercase();
    let trimmed = lower.trim();

    // Check if empty
    if trimmed.is_empty() {
        return true;
    }

    let placeholders = [
        "loading",
        "loading...",
        "please wait",
        "...",
        "copy",
        "copied",
        "copied!",
        "click to copy",
        "copy to clipboard",
        "[object object]",
        "undefined",
        "null",
        "error",
        "n/a",
    ];

    placeholders.iter().any(|p| trimmed == *p)
}

/// Check if content has code-like structure
fn has_code_structure(content: &str) -> bool {
    // Check for code-like characters
    let code_chars = ['{', '}', '[', ']', '(', ')', ';', '=', '<', '>', ':', '/', '\\', '|', '&', '*', '%', '#', '@', '$'];
    let code_char_count = content.chars().filter(|c| code_chars.contains(c)).count();

    // At least 2% code characters
    if !content.is_empty() && code_char_count > content.len() / 50 {
        return true;
    }

    // Or contains common code patterns
    let patterns = [
        // Function/method definitions
        "function ", "def ", "fn ", "func ", "sub ", "method ",
        // Class/type definitions
        "class ", "struct ", "enum ", "interface ", "trait ", "type ",
        // Imports/modules
        "import ", "from ", "require(", "include ", "use ", "using ",
        "module.exports", "export ", "package ",
        // Variable declarations
        "const ", "let ", "var ", "val ", "mut ",
        // Access modifiers
        "pub ", "public ", "private ", "protected ", "static ",
        // Control flow
        "async ", "await ", "return ", "yield ", "throw ",
        "if ", "else ", "for ", "while ", "switch ", "case ",
        "try ", "catch ", "finally ",
        // Common operators and syntax
        "=> ", "-> ", ":: ", "..",
        // Shell commands
        "$ ", "# ", "sudo ", "npm ", "yarn ", "cargo ", "pip ", "apt ",
    ];

    patterns.iter().any(|p| content.contains(p))
}

/// Calculate ratio of prose-like words
fn calculate_prose_ratio(content: &str) -> f32 {
    let words: Vec<&str> = content.split_whitespace().collect();
    if words.is_empty() {
        return 0.0;
    }

    // Common English prose words
    let prose_words = [
        // Articles
        "the", "a", "an",
        // Be verbs
        "is", "are", "was", "were", "be", "been", "being",
        // Pronouns
        "this", "that", "these", "those", "it", "its",
        "you", "your", "we", "our", "they", "their", "i", "my",
        "he", "she", "him", "her", "his", "hers",
        // Modal verbs
        "can", "will", "would", "could", "should", "may", "might", "must",
        // Prepositions
        "to", "of", "in", "for", "on", "with", "at", "by", "from", "into",
        "through", "during", "before", "after", "above", "below",
        // Conjunctions
        "and", "or", "but", "not", "so", "if", "then", "else", "when", "while",
        // Common verbs
        "have", "has", "had", "do", "does", "did", "make", "made",
        // Other common words
        "here", "there", "where", "how", "what", "which", "who", "why",
        "more", "most", "some", "any", "all", "each", "every", "both",
        "also", "just", "only", "even", "still", "now", "then",
    ];

    let prose_count = words.iter()
        .filter(|w| prose_words.contains(&w.to_lowercase().as_str()))
        .count();

    prose_count as f32 / words.len() as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_too_short() {
        let config = ValidationConfig::default();
        let result = validate_code("x", &config);
        assert!(matches!(result, ValidationResult::TooShort));
    }

    #[test]
    fn test_validate_placeholder() {
        let config = ValidationConfig::default();
        let result = validate_code("Loading...", &config);
        assert!(matches!(result, ValidationResult::Placeholder));

        let result = validate_code("click to copy", &config);
        assert!(matches!(result, ValidationResult::Placeholder));
    }

    #[test]
    fn test_validate_valid_code() {
        let config = ValidationConfig::default();

        let result = validate_code("fn main() { println!(\"hello\"); }", &config);
        assert!(result.is_valid());

        let result = validate_code("const x = { a: 1, b: 2 };", &config);
        assert!(result.is_valid());

        let result = validate_code("import React from 'react';", &config);
        assert!(result.is_valid());
    }

    #[test]
    fn test_has_code_structure() {
        assert!(has_code_structure("fn main() {}"));
        assert!(has_code_structure("const x = 1;"));
        assert!(has_code_structure("import foo from 'bar';"));
        assert!(has_code_structure("$ npm install"));
        assert!(!has_code_structure("This is just prose text with no code."));
    }

    #[test]
    fn test_prose_ratio() {
        // High prose content - has many common English words
        let prose = "The quick brown fox jumps over the lazy dog and then runs away from the house";
        let ratio = calculate_prose_ratio(prose);
        assert!(ratio > 0.3, "Expected high prose ratio but got {}", ratio);

        // Code content (low prose)
        let code = "fn main() { let x = 1; println!(x); }";
        let ratio = calculate_prose_ratio(code);
        assert!(ratio < 0.4, "Expected low prose ratio but got {}", ratio);
    }

    #[test]
    fn test_validation_stats() {
        let mut stats = ValidationStats::default();
        stats.passed = 10;
        stats.filtered_too_short = 2;
        stats.filtered_high_prose = 3;

        assert_eq!(stats.total_filtered(), 5);
        assert_eq!(stats.total(), 15);
    }
}
