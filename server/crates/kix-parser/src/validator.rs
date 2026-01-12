//! Code Validation Service
//!
//! Multi-stage validation for code extraction.
//! Ensures extracted content is actual code rather than prose or documentation.
//!
//! Validation stages:
//! 1. Length check - minimum viable code length
//! 2. Structural validation - code indicators and patterns
//! 3. Language-specific patterns - syntax markers per language
//! 4. Prose filtering - reject high prose ratio content

use regex::Regex;
use std::collections::HashMap;

/// Language-specific pattern configuration
#[derive(Debug, Clone)]
pub struct LanguagePatterns {
    /// Regex pattern for block start (function/class definitions)
    pub block_start: Regex,
    /// Regex pattern for block end
    pub block_end: Regex,
    /// Minimum indicators that should be present
    pub min_indicators: Vec<&'static str>,
}

impl LanguagePatterns {
    fn new(block_start: &str, block_end: &str, min_indicators: Vec<&'static str>) -> Self {
        Self {
            block_start: Regex::new(block_start).unwrap_or_else(|_| Regex::new(".*").unwrap()),
            block_end: Regex::new(block_end).unwrap_or_else(|_| Regex::new(".*").unwrap()),
            min_indicators,
        }
    }
}

/// Configuration for the code validator
#[derive(Debug, Clone)]
pub struct ValidatorConfig {
    /// Minimum code length in characters
    pub min_length: usize,
    /// Maximum code length
    pub max_length: usize,
    /// Maximum allowed prose ratio (0.0 - 1.0)
    pub max_prose_ratio: f32,
    /// Minimum required code indicators
    pub min_code_indicators: usize,
    /// Enable diagram filtering
    pub filter_diagrams: bool,
    /// Enable prose filtering
    pub filter_prose: bool,
    /// Enable language-specific validation
    pub enable_language_patterns: bool,
}

impl Default for ValidatorConfig {
    fn default() -> Self {
        Self {
            min_length: 10,
            max_length: 50000,
            max_prose_ratio: 0.15,
            min_code_indicators: 3,
            filter_diagrams: true,
            filter_prose: true,
            enable_language_patterns: true,
        }
    }
}

/// Result of code validation
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Whether the code passed validation
    pub is_valid: bool,
    /// Reason for failure (if any)
    pub failure_reason: Option<String>,
    /// Number of code indicators found
    pub indicator_count: usize,
    /// Calculated prose ratio
    pub prose_ratio: f32,
    /// Detected language (if any)
    pub detected_language: Option<String>,
}

/// Code validator with multi-stage validation
pub struct CodeValidator {
    config: ValidatorConfig,
    language_patterns: HashMap<String, LanguagePatterns>,
    code_indicators: Vec<(&'static str, Regex)>,
    prose_indicators: Vec<Regex>,
    bad_patterns: Vec<Regex>,
    comment_patterns: Vec<Regex>,
}

impl CodeValidator {
    /// Create a new validator with default configuration
    pub fn new() -> Self {
        Self::with_config(ValidatorConfig::default())
    }

    /// Create a validator with custom configuration
    pub fn with_config(config: ValidatorConfig) -> Self {
        let language_patterns = Self::build_language_patterns();
        let code_indicators = Self::build_code_indicators();
        let prose_indicators = Self::build_prose_indicators();
        let bad_patterns = Self::build_bad_patterns();
        let comment_patterns = Self::build_comment_patterns();

        Self {
            config,
            language_patterns,
            code_indicators,
            prose_indicators,
            bad_patterns,
            comment_patterns,
        }
    }

    /// Build language-specific patterns
    fn build_language_patterns() -> HashMap<String, LanguagePatterns> {
        let mut patterns = HashMap::new();

        patterns.insert(
            "typescript".to_string(),
            LanguagePatterns::new(
                r"^\s*(export\s+)?(class|interface|function|const|type|enum)\s+\w+",
                r"^\}(\s*;)?$",
                vec![":", "{", "}", "=>", "function", "class", "interface", "type"],
            ),
        );

        patterns.insert(
            "javascript".to_string(),
            LanguagePatterns::new(
                r"^\s*(export\s+)?(class|function|const|let|var)\s+\w+",
                r"^\}(\s*;)?$",
                vec!["function", "{", "}", "=>", "const", "let", "var"],
            ),
        );

        patterns.insert(
            "python".to_string(),
            LanguagePatterns::new(
                r"^\s*(class|def|async\s+def)\s+\w+",
                r"^\S", // Unindented line
                vec!["def", ":", "return", "self", "import", "class"],
            ),
        );

        patterns.insert(
            "java".to_string(),
            LanguagePatterns::new(
                r"^\s*(public|private|protected)?\s*(class|interface|enum)\s+\w+",
                r"^\}$",
                vec!["class", "public", "private", "{", "}", ";"],
            ),
        );

        patterns.insert(
            "rust".to_string(),
            LanguagePatterns::new(
                r"^\s*(pub\s+)?(fn|struct|impl|trait|enum)\s+\w+",
                r"^\}$",
                vec!["fn", "let", "mut", "impl", "struct", "->"],
            ),
        );

        patterns.insert(
            "go".to_string(),
            LanguagePatterns::new(
                r"^\s*(func|type|struct)\s+\w+",
                r"^\}$",
                vec!["func", "type", "struct", "{", "}", ":="],
            ),
        );

        patterns.insert(
            "cpp".to_string(),
            LanguagePatterns::new(
                r"^\s*(class|struct|void|int|bool|auto)\s+\w+",
                r"^\}(\s*;)?$",
                vec!["class", "public", "private", "::", "{", "}", ";"],
            ),
        );

        patterns.insert(
            "c".to_string(),
            LanguagePatterns::new(
                r"^\s*(void|int|char|struct)\s+\w+",
                r"^\}$",
                vec!["#include", "void", "int", "{", "}", ";"],
            ),
        );

        patterns
    }

    /// Build code indicator patterns
    fn build_code_indicators() -> Vec<(&'static str, Regex)> {
        vec![
            ("function_calls", Regex::new(r"\w+\s*\([^)]*\)").unwrap()),
            ("assignments", Regex::new(r"\w+\s*=\s*.+").unwrap()),
            (
                "control_flow",
                Regex::new(r"\b(if|for|while|switch|case|try|catch|except)\b").unwrap(),
            ),
            (
                "declarations",
                Regex::new(r"\b(var|let|const|def|class|function|interface|type|struct|enum)\b")
                    .unwrap(),
            ),
            (
                "imports",
                Regex::new(r"\b(import|from|require|include|using|use)\b").unwrap(),
            ),
            ("brackets", Regex::new(r"[\{\}\[\]]").unwrap()),
            ("operators", Regex::new(r"[\+\-\*/%&\|\^<>=!]").unwrap()),
            ("method_chains", Regex::new(r"\.\w+").unwrap()),
            ("arrows", Regex::new(r"(=>|->)").unwrap()),
            (
                "keywords",
                Regex::new(r"\b(return|break|continue|yield|await|async)\b").unwrap(),
            ),
        ]
    }

    /// Build prose indicator patterns
    fn build_prose_indicators() -> Vec<Regex> {
        vec![
            Regex::new(r"\b(the|this|that|these|those|is|are|was|were|will|would|should|could|have|has|had)\b").unwrap(),
            Regex::new(r"[.!?]\s+[A-Z]").unwrap(), // Sentence endings
            Regex::new(r"\b(however|therefore|furthermore|moreover|nevertheless|additionally|specifically)\b").unwrap(),
        ]
    }

    /// Build bad pattern indicators (poor extraction)
    fn build_bad_patterns() -> Vec<Regex> {
        vec![
            // Concatenated keywords without spaces (but allow camelCase)
            // Match keyword immediately followed by lowercase letter (e.g., "defmain" instead of "def main")
            Regex::new(r"\b(from|import|def|class|if|for|while|return)[a-z]").unwrap(),
            // HTML entities that weren't decoded
            Regex::new(r"&[lg]t;|&amp;|&quot;|&#\d+;").unwrap(),
            // Excessive HTML tags
            Regex::new(r"<[^>]{50,}>").unwrap(),
            // Multiple spans in a row (indicates poor extraction)
            Regex::new(r"(<span[^>]*>){5,}").unwrap(),
            // Very long unbroken strings
            Regex::new(r"[^\s]{200,}").unwrap(),
        ]
    }

    /// Build comment pattern indicators
    fn build_comment_patterns() -> Vec<Regex> {
        vec![
            Regex::new(r"^\s*(//|#|/\*|\*|<!--)").unwrap(),
            Regex::new(r#"^\s*""""#).unwrap(),
            Regex::new(r"^\s*'''").unwrap(),
            Regex::new(r"^\s*\*\s").unwrap(),
        ]
    }

    /// Validate code content using multi-stage validation
    pub fn validate(&self, code: &str, language: Option<&str>) -> ValidationResult {
        // Stage 1: Basic length check
        let trimmed = code.trim();
        if trimmed.len() < self.config.min_length {
            return ValidationResult {
                is_valid: false,
                failure_reason: Some(format!(
                    "Code too short: {} < {} chars",
                    trimmed.len(),
                    self.config.min_length
                )),
                indicator_count: 0,
                prose_ratio: 0.0,
                detected_language: None,
            };
        }

        if trimmed.len() > self.config.max_length {
            return ValidationResult {
                is_valid: false,
                failure_reason: Some(format!(
                    "Code too long: {} > {} chars",
                    trimmed.len(),
                    self.config.max_length
                )),
                indicator_count: 0,
                prose_ratio: 0.0,
                detected_language: None,
            };
        }

        // Stage 2: Check for diagram languages (skip if enabled)
        if self.config.filter_diagrams {
            if let Some(lang) = language {
                let lang_lower = lang.to_lowercase();
                if ["mermaid", "plantuml", "graphviz", "dot", "diagram"]
                    .contains(&lang_lower.as_str())
                {
                    return ValidationResult {
                        is_valid: false,
                        failure_reason: Some(format!("Diagram language filtered: {}", lang)),
                        indicator_count: 0,
                        prose_ratio: 0.0,
                        detected_language: Some(lang.to_string()),
                    };
                }
            }
        }

        // Stage 3: Check for bad extraction patterns
        for pattern in &self.bad_patterns {
            if pattern.is_match(code) {
                return ValidationResult {
                    is_valid: false,
                    failure_reason: Some("Code contains bad extraction patterns".to_string()),
                    indicator_count: 0,
                    prose_ratio: 0.0,
                    detected_language: language.map(|s| s.to_string()),
                };
            }
        }

        // Stage 4: Count code indicators
        let mut indicator_count = 0;
        let mut found_indicators = Vec::new();
        for (name, pattern) in &self.code_indicators {
            if pattern.is_match(code) {
                indicator_count += 1;
                found_indicators.push(*name);
            }
        }

        if indicator_count < self.config.min_code_indicators {
            return ValidationResult {
                is_valid: false,
                failure_reason: Some(format!(
                    "Insufficient code indicators: {} found ({}), need {}",
                    indicator_count,
                    found_indicators.join(", "),
                    self.config.min_code_indicators
                )),
                indicator_count,
                prose_ratio: 0.0,
                detected_language: language.map(|s| s.to_string()),
            };
        }

        // Stage 5: Check comment ratio
        let lines: Vec<&str> = code.lines().collect();
        let non_empty_lines: Vec<&str> = lines.iter().filter(|l| !l.trim().is_empty()).copied().collect();

        if non_empty_lines.is_empty() {
            return ValidationResult {
                is_valid: false,
                failure_reason: Some("Code has no non-empty lines".to_string()),
                indicator_count,
                prose_ratio: 0.0,
                detected_language: language.map(|s| s.to_string()),
            };
        }

        // Count comment lines
        let mut comment_lines = 0;
        for line in &lines {
            for pattern in &self.comment_patterns {
                if pattern.is_match(line.trim()) {
                    comment_lines += 1;
                    break;
                }
            }
        }

        // Allow up to 70% comments
        if non_empty_lines.len() > 0 && (comment_lines as f32 / non_empty_lines.len() as f32) > 0.7
        {
            return ValidationResult {
                is_valid: false,
                failure_reason: Some(format!(
                    "Code is mostly comments: {}/{} lines",
                    comment_lines,
                    non_empty_lines.len()
                )),
                indicator_count,
                prose_ratio: 0.0,
                detected_language: language.map(|s| s.to_string()),
            };
        }

        // Stage 6: Language-specific validation
        if self.config.enable_language_patterns {
            if let Some(lang) = language {
                if let Some(lang_patterns) = self.language_patterns.get(&lang.to_lowercase()) {
                    let found_lang_indicators: usize = lang_patterns
                        .min_indicators
                        .iter()
                        .filter(|indicator| code.to_lowercase().contains(*indicator))
                        .count();

                    if found_lang_indicators < 2 {
                        return ValidationResult {
                            is_valid: false,
                            failure_reason: Some(format!(
                                "Code lacks {} indicators: only {} found",
                                lang, found_lang_indicators
                            )),
                            indicator_count,
                            prose_ratio: 0.0,
                            detected_language: Some(lang.to_string()),
                        };
                    }
                }
            }
        }

        // Stage 7: Check for reasonable structure
        if non_empty_lines.len() < 3 {
            return ValidationResult {
                is_valid: false,
                failure_reason: Some(format!(
                    "Code has too few non-empty lines: {}",
                    non_empty_lines.len()
                )),
                indicator_count,
                prose_ratio: 0.0,
                detected_language: language.map(|s| s.to_string()),
            };
        }

        // Check for reasonable line lengths
        let very_long_lines = lines.iter().filter(|l| l.len() > 300).count();
        if !lines.is_empty() && very_long_lines > lines.len() / 2 {
            return ValidationResult {
                is_valid: false,
                failure_reason: Some("Code has too many very long lines".to_string()),
                indicator_count,
                prose_ratio: 0.0,
                detected_language: language.map(|s| s.to_string()),
            };
        }

        // Stage 8: Prose filtering
        let prose_ratio = if self.config.filter_prose {
            self.calculate_prose_ratio(code)
        } else {
            0.0
        };

        if prose_ratio > self.config.max_prose_ratio {
            return ValidationResult {
                is_valid: false,
                failure_reason: Some(format!(
                    "Code appears to be prose: ratio {:.2} > {:.2}",
                    prose_ratio, self.config.max_prose_ratio
                )),
                indicator_count,
                prose_ratio,
                detected_language: language.map(|s| s.to_string()),
            };
        }

        // Passed all checks
        ValidationResult {
            is_valid: true,
            failure_reason: None,
            indicator_count,
            prose_ratio,
            detected_language: language.map(|s| s.to_string()),
        }
    }

    /// Calculate the prose ratio of the code
    fn calculate_prose_ratio(&self, code: &str) -> f32 {
        let word_count = code.split_whitespace().count();
        if word_count == 0 {
            return 0.0;
        }

        let mut prose_score = 0;
        for pattern in &self.prose_indicators {
            prose_score += pattern.find_iter(code).count();
        }

        prose_score as f32 / word_count as f32
    }

    /// Detect language from code content
    pub fn detect_language(&self, code: &str) -> Option<String> {
        let mut best_match: Option<(String, usize)> = None;

        for (lang, patterns) in &self.language_patterns {
            let indicator_count = patterns
                .min_indicators
                .iter()
                .filter(|indicator| code.contains(*indicator))
                .count();

            if indicator_count >= 2 {
                match &best_match {
                    Some((_, count)) if indicator_count > *count => {
                        best_match = Some((lang.clone(), indicator_count));
                    }
                    None => {
                        best_match = Some((lang.clone(), indicator_count));
                    }
                    _ => {}
                }
            }
        }

        best_match.map(|(lang, _)| lang)
    }

    /// Quick validation check (for filtering before full validation)
    pub fn quick_check(&self, code: &str) -> bool {
        let trimmed = code.trim();

        // Basic length check
        if trimmed.len() < self.config.min_length {
            return false;
        }

        // Must have some brackets or operators
        let has_brackets = trimmed.contains('{')
            || trimmed.contains('}')
            || trimmed.contains('[')
            || trimmed.contains(']');
        let has_operators =
            trimmed.contains('=') || trimmed.contains('+') || trimmed.contains('-');

        has_brackets || has_operators
    }
}

impl Default for CodeValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_rust_code() {
        let validator = CodeValidator::new();
        let code = r#"
fn main() {
    let x = 5;
    let y = 10;
    println!("Sum: {}", x + y);
}
        "#;

        let result = validator.validate(code, Some("rust"));
        assert!(result.is_valid, "Rust code should be valid: {:?}", result.failure_reason);
    }

    #[test]
    fn test_valid_python_code() {
        let validator = CodeValidator::new();
        let code = r#"
def calculate_sum(a, b):
    return a + b

class Calculator:
    def __init__(self):
        self.result = 0

    def add(self, value):
        self.result += value
        return self.result
        "#;

        let result = validator.validate(code, Some("python"));
        assert!(result.is_valid, "Python code should be valid: {:?}", result.failure_reason);
    }

    #[test]
    fn test_reject_prose() {
        let validator = CodeValidator::new();
        let prose = r#"
This is a paragraph of text that describes something.
It has sentences that end with periods and start with capital letters.
The text continues with more explanations and descriptions.
However, there are no code-like elements in this content.
Therefore, it should be rejected by the validator.
        "#;

        let result = validator.validate(prose, None);
        assert!(
            !result.is_valid,
            "Prose should be rejected"
        );
    }

    #[test]
    fn test_reject_too_short() {
        let validator = CodeValidator::new();
        let code = "x = 1";

        let result = validator.validate(code, None);
        assert!(!result.is_valid);
        assert!(result.failure_reason.unwrap().contains("too short"));
    }

    #[test]
    fn test_detect_language() {
        let validator = CodeValidator::new();

        // Rust-specific: fn, let, mut, impl, struct, ->
        let rust_code = "fn main() { let mut x = 5; impl Foo -> Bar { struct Data {} } }";
        assert_eq!(validator.detect_language(rust_code), Some("rust".to_string()));

        // Python-specific: def, self, return, import, class, :
        let python_code = "def foo(): return self.value; import sys; class Bar: pass";
        assert_eq!(validator.detect_language(python_code), Some("python".to_string()));
    }

    #[test]
    fn test_reject_diagram_language() {
        let validator = CodeValidator::new();
        let mermaid = r#"
graph TD
    A[Start] --> B{Decision}
    B -->|Yes| C[Action 1]
    B -->|No| D[Action 2]
        "#;

        let result = validator.validate(mermaid, Some("mermaid"));
        assert!(!result.is_valid);
        assert!(result.failure_reason.unwrap().contains("Diagram"));
    }

    #[test]
    fn test_quick_check() {
        let validator = CodeValidator::new();

        assert!(validator.quick_check("function foo() { return x + y; }"));
        assert!(validator.quick_check("let x = [1, 2, 3];"));
        assert!(!validator.quick_check("short"));
        assert!(!validator.quick_check("This is just plain text"));
    }
}
