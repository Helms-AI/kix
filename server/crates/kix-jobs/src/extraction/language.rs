//! Programming language detection and representation.

use std::fmt;

/// Supported programming languages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Language {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Go,
    Java,
    CSharp,
    Cpp,
    C,
    Ruby,
    Php,
    Swift,
    Kotlin,
    Sql,
    Shell,
    Yaml,
    Json,
    Toml,
    Markdown,
    Html,
    Css,
    #[default]
    Other,
}

impl Language {
    /// Detect language from a string hint (class name, attribute, etc.)
    pub fn from_hint(hint: &str) -> Self {
        match hint.to_lowercase().as_str() {
            // Rust
            "rust" | "rs" => Self::Rust,

            // Python
            "python" | "py" | "python3" | "py3" => Self::Python,

            // JavaScript
            "javascript" | "js" | "jsx" | "mjs" | "cjs" => Self::JavaScript,

            // TypeScript
            "typescript" | "ts" | "tsx" => Self::TypeScript,

            // Go
            "go" | "golang" => Self::Go,

            // Java
            "java" => Self::Java,

            // C#
            "csharp" | "cs" | "c#" | "dotnet" => Self::CSharp,

            // C++
            "cpp" | "c++" | "cxx" | "cc" | "cplusplus" => Self::Cpp,

            // C
            "c" => Self::C,

            // Ruby
            "ruby" | "rb" => Self::Ruby,

            // PHP
            "php" => Self::Php,

            // Swift
            "swift" => Self::Swift,

            // Kotlin
            "kotlin" | "kt" | "kts" => Self::Kotlin,

            // SQL
            "sql" | "mysql" | "postgresql" | "postgres" | "sqlite" | "plsql" => Self::Sql,

            // Shell
            "shell" | "bash" | "sh" | "zsh" | "fish" | "powershell" | "ps1" | "console" | "terminal" => Self::Shell,

            // Data formats
            "yaml" | "yml" => Self::Yaml,
            "json" | "jsonc" | "json5" => Self::Json,
            "toml" => Self::Toml,

            // Markup
            "markdown" | "md" => Self::Markdown,
            "html" | "htm" | "xhtml" => Self::Html,
            "css" | "scss" | "sass" | "less" | "stylus" => Self::Css,

            // Default
            _ => Self::Other,
        }
    }

    /// Get file extension for this language
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Rust => "rs",
            Self::Python => "py",
            Self::JavaScript => "js",
            Self::TypeScript => "ts",
            Self::Go => "go",
            Self::Java => "java",
            Self::CSharp => "cs",
            Self::Cpp => "cpp",
            Self::C => "c",
            Self::Ruby => "rb",
            Self::Php => "php",
            Self::Swift => "swift",
            Self::Kotlin => "kt",
            Self::Sql => "sql",
            Self::Shell => "sh",
            Self::Yaml => "yaml",
            Self::Json => "json",
            Self::Toml => "toml",
            Self::Markdown => "md",
            Self::Html => "html",
            Self::Css => "css",
            Self::Other => "txt",
        }
    }

    /// Check if this is a common programming language (not markup/data)
    pub fn is_programming_language(&self) -> bool {
        matches!(
            self,
            Self::Rust
                | Self::Python
                | Self::JavaScript
                | Self::TypeScript
                | Self::Go
                | Self::Java
                | Self::CSharp
                | Self::Cpp
                | Self::C
                | Self::Ruby
                | Self::Php
                | Self::Swift
                | Self::Kotlin
                | Self::Shell
        )
    }

    /// Check if this is a data format
    pub fn is_data_format(&self) -> bool {
        matches!(self, Self::Yaml | Self::Json | Self::Toml | Self::Sql)
    }

    /// Get all languages
    pub fn all() -> &'static [Self] {
        &[
            Self::Rust, Self::Python, Self::JavaScript, Self::TypeScript,
            Self::Go, Self::Java, Self::CSharp, Self::Cpp, Self::C,
            Self::Ruby, Self::Php, Self::Swift, Self::Kotlin, Self::Sql,
            Self::Shell, Self::Yaml, Self::Json, Self::Toml,
            Self::Markdown, Self::Html, Self::Css, Self::Other,
        ]
    }

    /// Get display name
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Rust => "Rust",
            Self::Python => "Python",
            Self::JavaScript => "JavaScript",
            Self::TypeScript => "TypeScript",
            Self::Go => "Go",
            Self::Java => "Java",
            Self::CSharp => "C#",
            Self::Cpp => "C++",
            Self::C => "C",
            Self::Ruby => "Ruby",
            Self::Php => "PHP",
            Self::Swift => "Swift",
            Self::Kotlin => "Kotlin",
            Self::Sql => "SQL",
            Self::Shell => "Shell",
            Self::Yaml => "YAML",
            Self::Json => "JSON",
            Self::Toml => "TOML",
            Self::Markdown => "Markdown",
            Self::Html => "HTML",
            Self::Css => "CSS",
            Self::Other => "Other",
        }
    }

    /// Get language aliases (for hint detection)
    pub fn aliases(&self) -> &'static [&'static str] {
        match self {
            Self::Rust => &["rust", "rs"],
            Self::Python => &["python", "py", "python3", "py3"],
            Self::JavaScript => &["javascript", "js", "jsx", "mjs", "cjs"],
            Self::TypeScript => &["typescript", "ts", "tsx"],
            Self::Go => &["go", "golang"],
            Self::Java => &["java"],
            Self::CSharp => &["csharp", "cs", "c#", "dotnet"],
            Self::Cpp => &["cpp", "c++", "cxx", "cc", "cplusplus"],
            Self::C => &["c"],
            Self::Ruby => &["ruby", "rb"],
            Self::Php => &["php"],
            Self::Swift => &["swift"],
            Self::Kotlin => &["kotlin", "kt", "kts"],
            Self::Sql => &["sql", "mysql", "postgresql", "postgres", "sqlite"],
            Self::Shell => &["shell", "bash", "sh", "zsh", "fish"],
            Self::Yaml => &["yaml", "yml"],
            Self::Json => &["json", "jsonc", "json5"],
            Self::Toml => &["toml"],
            Self::Markdown => &["markdown", "md"],
            Self::Html => &["html", "htm", "xhtml"],
            Self::Css => &["css", "scss", "sass", "less"],
            Self::Other => &["other", "unknown"],
        }
    }

    /// Get file extensions for this language
    pub fn extensions(&self) -> &'static [&'static str] {
        match self {
            Self::Rust => &["rs"],
            Self::Python => &["py", "pyw", "pyi"],
            Self::JavaScript => &["js", "mjs", "cjs", "jsx"],
            Self::TypeScript => &["ts", "tsx"],
            Self::Go => &["go"],
            Self::Java => &["java"],
            Self::CSharp => &["cs"],
            Self::Cpp => &["cpp", "cc", "cxx", "hpp", "hxx"],
            Self::C => &["c", "h"],
            Self::Ruby => &["rb", "rake"],
            Self::Php => &["php", "phtml"],
            Self::Swift => &["swift"],
            Self::Kotlin => &["kt", "kts"],
            Self::Sql => &["sql"],
            Self::Shell => &["sh", "bash", "zsh"],
            Self::Yaml => &["yaml", "yml"],
            Self::Json => &["json"],
            Self::Toml => &["toml"],
            Self::Markdown => &["md", "markdown"],
            Self::Html => &["html", "htm"],
            Self::Css => &["css", "scss", "sass", "less"],
            Self::Other => &["txt"],
        }
    }
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rust => write!(f, "Rust"),
            Self::Python => write!(f, "Python"),
            Self::JavaScript => write!(f, "JavaScript"),
            Self::TypeScript => write!(f, "TypeScript"),
            Self::Go => write!(f, "Go"),
            Self::Java => write!(f, "Java"),
            Self::CSharp => write!(f, "C#"),
            Self::Cpp => write!(f, "C++"),
            Self::C => write!(f, "C"),
            Self::Ruby => write!(f, "Ruby"),
            Self::Php => write!(f, "PHP"),
            Self::Swift => write!(f, "Swift"),
            Self::Kotlin => write!(f, "Kotlin"),
            Self::Sql => write!(f, "SQL"),
            Self::Shell => write!(f, "Shell"),
            Self::Yaml => write!(f, "YAML"),
            Self::Json => write!(f, "JSON"),
            Self::Toml => write!(f, "TOML"),
            Self::Markdown => write!(f, "Markdown"),
            Self::Html => write!(f, "HTML"),
            Self::Css => write!(f, "CSS"),
            Self::Other => write!(f, "Other"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_from_hint() {
        assert_eq!(Language::from_hint("rust"), Language::Rust);
        assert_eq!(Language::from_hint("rs"), Language::Rust);
        assert_eq!(Language::from_hint("RUST"), Language::Rust);
        assert_eq!(Language::from_hint("python"), Language::Python);
        assert_eq!(Language::from_hint("py"), Language::Python);
        assert_eq!(Language::from_hint("javascript"), Language::JavaScript);
        assert_eq!(Language::from_hint("js"), Language::JavaScript);
        assert_eq!(Language::from_hint("typescript"), Language::TypeScript);
        assert_eq!(Language::from_hint("ts"), Language::TypeScript);
        assert_eq!(Language::from_hint("go"), Language::Go);
        assert_eq!(Language::from_hint("golang"), Language::Go);
        assert_eq!(Language::from_hint("bash"), Language::Shell);
        assert_eq!(Language::from_hint("sh"), Language::Shell);
        assert_eq!(Language::from_hint("unknown"), Language::Other);
    }

    #[test]
    fn test_language_extension() {
        assert_eq!(Language::Rust.extension(), "rs");
        assert_eq!(Language::Python.extension(), "py");
        assert_eq!(Language::JavaScript.extension(), "js");
        assert_eq!(Language::Other.extension(), "txt");
    }

    #[test]
    fn test_is_programming_language() {
        assert!(Language::Rust.is_programming_language());
        assert!(Language::Python.is_programming_language());
        assert!(!Language::Yaml.is_programming_language());
        assert!(!Language::Json.is_programming_language());
        assert!(!Language::Html.is_programming_language());
    }

    #[test]
    fn test_is_data_format() {
        assert!(Language::Yaml.is_data_format());
        assert!(Language::Json.is_data_format());
        assert!(Language::Toml.is_data_format());
        assert!(Language::Sql.is_data_format());
        assert!(!Language::Rust.is_data_format());
    }
}
