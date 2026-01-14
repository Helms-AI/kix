//! Indexing services.
//!
//! Provides shared types for indexing operations used by both
//! the REST API and MCP server.
//!
//! Note: Actual job management functions remain in handlers due to
//! tight coupling with infrastructure (job queue, SSE, etc.).

use serde::{Deserialize, Serialize};

use crate::error::{ServiceError, ServiceResult};

// =============================================================================
// CODE EXTRACTION TYPES (Phase 5)
// =============================================================================

/// Code extraction pattern information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternInfo {
    pub name: String,
    pub css_selector: String,
    pub description: String,
    pub example_sites: Vec<String>,
}

/// Supported language information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageInfo {
    pub name: String,
    pub aliases: Vec<String>,
    pub extensions: Vec<String>,
    pub tree_sitter_support: bool,
}

/// Code extraction statistics for a job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeExtractionStats {
    pub job_id: String,
    pub total_pages: usize,
    pub pages_with_code: usize,
    pub total_code_blocks: usize,
    pub languages: Vec<LanguageStats>,
    pub patterns: Vec<PatternStats>,
    pub validation: ValidationSummary,
}

/// Language statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageStats {
    pub language: String,
    pub block_count: usize,
    pub total_lines: usize,
    pub percentage: f32,
}

/// Pattern match statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternStats {
    pub pattern: String,
    pub match_count: usize,
    pub percentage: f32,
}

/// Validation summary for code extraction
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ValidationSummary {
    pub total_extracted: usize,
    pub passed: usize,
    pub pass_rate: f32,
    pub rejection_reasons: Vec<RejectionReason>,
}

/// Code rejection reason
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RejectionReason {
    pub reason: String,
    pub count: usize,
}

/// Code block response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeBlockResponse {
    pub id: String,
    pub content: String,
    pub language: String,
    pub pattern: String,
    pub line_count: usize,
    pub source_url: String,
    pub validated: bool,
}

// =============================================================================
// CODE EXTRACTION FUNCTIONS
// =============================================================================

/// List all supported code extraction patterns
pub fn list_code_patterns() -> Vec<PatternInfo> {
    use kix_jobs::CodePattern;

    CodePattern::all()
        .iter()
        .map(|p| PatternInfo {
            name: p.name().to_string(),
            css_selector: p.selector().to_string(),
            description: p.description().to_string(),
            example_sites: p.example_sites().iter().map(|s| s.to_string()).collect(),
        })
        .collect()
}

/// List all supported programming languages
pub fn list_supported_languages() -> Vec<LanguageInfo> {
    use kix_jobs::Language;
    use kix_parser::treesitter::SourceLanguage;

    Language::all()
        .iter()
        .map(|lang| {
            // Check if tree-sitter supports this language
            let ext = lang.extension();
            let tree_sitter_support = SourceLanguage::from_extension(ext).is_some();

            LanguageInfo {
                name: lang.display_name().to_string(),
                aliases: lang.aliases().iter().map(|s| s.to_string()).collect(),
                extensions: lang.extensions().iter().map(|s| s.to_string()).collect(),
                tree_sitter_support,
            }
        })
        .collect()
}

// =============================================================================
// TYPES
// =============================================================================

/// Job status summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobStatus {
    pub job_id: String,
    pub job_type: String,
    pub state: String,
    pub progress_percent: f32,
    pub progress_stage: String,
    pub items_total: usize,
    pub items_processed: usize,
    pub items_succeeded: usize,
    pub items_failed: usize,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub error: Option<String>,
}

/// Job list with summary information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobList {
    pub jobs: Vec<JobStatus>,
    pub total: usize,
}

/// Target for delete operations.
#[derive(Debug, Clone)]
pub enum DeleteTarget {
    /// Single document by ID
    Single(String),
    /// Multiple documents by IDs
    Multiple(Vec<String>),
    /// Filter by tag
    Tag(String),
    /// Filter by source domain
    Domain(String),
}

/// Result of delete operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteResult {
    pub deleted: usize,
    pub ids: Vec<String>,
    pub dry_run: bool,
}

/// Index statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStats {
    pub total_documents: usize,
    pub total_chunks: usize,
    pub total_pages: usize,
}

/// URL indexing configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UrlIndexConfig {
    pub url: String,
    pub depth: usize,
    pub respect_robots: bool,
    pub render_js: bool,
    pub timeout_secs: u64,
    pub max_pages: usize,
    pub priority: Option<u8>,
}

impl Default for UrlIndexConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            depth: 1,
            respect_robots: true,
            render_js: true,
            timeout_secs: 30,
            max_pages: 0,
            priority: Some(5),
        }
    }
}

/// File indexing configuration.
#[derive(Debug, Clone)]
pub struct FileIndexConfig {
    pub files: Vec<FileInput>,
    pub tags: Option<Vec<String>>,
}

/// Input file for indexing.
#[derive(Debug, Clone)]
pub struct FileInput {
    pub filename: String,
    pub content: Vec<u8>,
}

/// Sync indexing options.
#[derive(Debug, Clone, Default)]
pub struct SyncIndexOptions {
    pub id: Option<String>,
    pub title: Option<String>,
    pub tags: Option<Vec<String>>,
    pub replace: bool,
}

/// Result of sync indexing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncIndexResult {
    pub document_id: String,
    pub chunks_created: usize,
    pub status: String,
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Parse job ID from string (handles both UUID and string formats).
pub fn parse_job_id(id_str: &str) -> ServiceResult<String> {
    // UUID format validation
    if uuid::Uuid::parse_str(id_str).is_ok() {
        Ok(id_str.to_string())
    } else {
        Err(ServiceError::Validation(format!(
            "Invalid job ID format: {}",
            id_str
        )))
    }
}

/// Check if a job state is terminal (completed, failed, or cancelled).
pub fn is_terminal_state(state: &str) -> bool {
    matches!(state, "completed" | "failed" | "cancelled")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_status() {
        let status = JobStatus {
            job_id: "test-123".to_string(),
            job_type: "url".to_string(),
            state: "running".to_string(),
            progress_percent: 50.0,
            progress_stage: "crawling".to_string(),
            items_total: 10,
            items_processed: 5,
            items_succeeded: 5,
            items_failed: 0,
            started_at: Some("2024-01-01T00:00:00Z".to_string()),
            completed_at: None,
            error: None,
        };
        assert_eq!(status.progress_percent, 50.0);
    }

    #[test]
    fn test_delete_target_variants() {
        let single = DeleteTarget::Single("doc-1".to_string());
        let multiple = DeleteTarget::Multiple(vec!["doc-1".to_string(), "doc-2".to_string()]);
        let tag = DeleteTarget::Tag("my-tag".to_string());
        let domain = DeleteTarget::Domain("example.com".to_string());

        match single {
            DeleteTarget::Single(id) => assert_eq!(id, "doc-1"),
            _ => panic!("Expected Single"),
        }

        match multiple {
            DeleteTarget::Multiple(ids) => assert_eq!(ids.len(), 2),
            _ => panic!("Expected Multiple"),
        }

        match tag {
            DeleteTarget::Tag(t) => assert_eq!(t, "my-tag"),
            _ => panic!("Expected Tag"),
        }

        match domain {
            DeleteTarget::Domain(d) => assert_eq!(d, "example.com"),
            _ => panic!("Expected Domain"),
        }
    }

    #[test]
    fn test_delete_result() {
        let result = DeleteResult {
            deleted: 5,
            ids: vec!["a".to_string(), "b".to_string()],
            dry_run: false,
        };
        assert_eq!(result.deleted, 5);
        assert!(!result.dry_run);
    }

    #[test]
    fn test_url_index_config_default() {
        let config = UrlIndexConfig::default();
        assert_eq!(config.depth, 1);
        assert!(config.respect_robots);
        assert!(config.render_js);
    }

    #[test]
    fn test_parse_job_id() {
        let valid = parse_job_id("550e8400-e29b-41d4-a716-446655440000");
        assert!(valid.is_ok());

        let invalid = parse_job_id("not-a-uuid");
        assert!(invalid.is_err());
    }

    #[test]
    fn test_is_terminal_state() {
        assert!(is_terminal_state("completed"));
        assert!(is_terminal_state("failed"));
        assert!(is_terminal_state("cancelled"));
        assert!(!is_terminal_state("running"));
        assert!(!is_terminal_state("pending"));
    }
}
