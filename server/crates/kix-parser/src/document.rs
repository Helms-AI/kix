//! Core types for the Knowledge Indexer.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Represents an indexed knowledge entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    /// Unique identifier (UUID or user-provided)
    pub id: String,

    /// Entry title
    pub title: String,

    /// Short description/summary
    pub description: String,

    /// Full content text
    pub content: String,

    /// Auto-extracted + user-provided tags
    pub tags: Vec<String>,

    /// Collection IDs this entry belongs to
    pub collection_ids: Vec<String>,

    /// Entry type for filtering
    pub entry_type: EntryType,

    /// Source format type
    pub source_type: SourceType,

    /// Original source path/URL
    pub source_path: String,

    /// URL-safe identifier
    pub slug: String,

    /// Content hash for change detection
    pub source_hash: String,

    /// Created timestamp
    pub created_at: DateTime<Utc>,

    /// Updated timestamp
    pub updated_at: DateTime<Utc>,
}

/// Type of entry (simplified from PatternType).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum EntryType {
    /// Document (HTML, Markdown, etc.)
    Document,
    /// PDF file
    Pdf,
    /// Article/Blog post
    Article,
    /// Code/Source file
    Code,
    /// Other/Unknown
    Other,
}

impl EntryType {
    /// Returns the string representation of the entry type.
    pub fn as_str(&self) -> &'static str {
        match self {
            EntryType::Document => "document",
            EntryType::Pdf => "pdf",
            EntryType::Article => "article",
            EntryType::Code => "code",
            EntryType::Other => "other",
        }
    }

    /// Parses a string into an EntryType.
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "document" | "html" | "markdown" => EntryType::Document,
            "pdf" | "pdf_document" => EntryType::Pdf,
            "article" => EntryType::Article,
            "code" | "source_code" => EntryType::Code,
            _ => EntryType::Other,
        }
    }
}

impl std::fmt::Display for EntryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Source type of the document.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SourceType {
    Html,
    Pdf,
    Docx,
    Excel,
    Csv,
    Json,
    Markdown,
    SourceCode,
    PlainText,
    Url,
}

impl std::fmt::Display for SourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SourceType::Html => write!(f, "html"),
            SourceType::Pdf => write!(f, "pdf"),
            SourceType::Docx => write!(f, "docx"),
            SourceType::Excel => write!(f, "excel"),
            SourceType::Csv => write!(f, "csv"),
            SourceType::Json => write!(f, "json"),
            SourceType::Markdown => write!(f, "markdown"),
            SourceType::SourceCode => write!(f, "source_code"),
            SourceType::PlainText => write!(f, "plain_text"),
            SourceType::Url => write!(f, "url"),
        }
    }
}

impl SourceType {
    /// Detect source type from file extension
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "html" | "htm" => SourceType::Html,
            "pdf" => SourceType::Pdf,
            "docx" | "doc" => SourceType::Docx,
            "xlsx" | "xls" => SourceType::Excel,
            "csv" => SourceType::Csv,
            "json" => SourceType::Json,
            "md" | "markdown" => SourceType::Markdown,
            "txt" => SourceType::PlainText,
            // Source code
            "rs" | "py" | "js" | "ts" | "jsx" | "tsx" | "java" | "go" | "c" | "cpp" | "h" |
            "cs" | "rb" | "php" | "swift" | "kt" | "scala" | "sh" | "bash" | "zsh" |
            "yaml" | "yml" | "toml" | "xml" => SourceType::SourceCode,
            _ => SourceType::PlainText,
        }
    }

    /// Check if the source is from a URL
    pub fn from_path(path: &str) -> Self {
        if path.starts_with("http://") || path.starts_with("https://") {
            return SourceType::Url;
        }

        // Get extension from path
        std::path::Path::new(path)
            .extension()
            .and_then(|ext| ext.to_str())
            .map(Self::from_extension)
            .unwrap_or(SourceType::PlainText)
    }
}

/// Represents a chunk of an entry for embedding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryChunk {
    /// Unique chunk identifier ("{entry_id}#{chunk_index}")
    pub chunk_id: String,

    /// Parent entry ID
    pub entry_id: String,

    /// FK to pages table for two-layer storage (context retrieval)
    pub page_id: Option<String>,

    /// Index of this chunk within the entry
    pub chunk_index: u32,

    /// Type of chunk content
    pub chunk_type: ChunkType,

    /// The actual text content of this chunk
    pub text: String,

    /// Metadata about the parent entry (denormalized for filtering)
    pub metadata: ChunkMetadata,
}

/// Type of chunk content.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ChunkType {
    /// Title + description summary
    Summary,
    /// General body content
    Content,
    /// Code block
    Code,
    /// Header/section title
    Header,
}

impl ChunkType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ChunkType::Summary => "summary",
            ChunkType::Content => "content",
            ChunkType::Code => "code",
            ChunkType::Header => "header",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "summary" | "title" | "full" => ChunkType::Summary,
            "code" => ChunkType::Code,
            "header" => ChunkType::Header,
            _ => ChunkType::Content,
        }
    }
}

impl std::fmt::Display for ChunkType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Denormalized metadata for chunks (enables filtering without joins).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkMetadata {
    /// Entry title
    pub entry_title: String,

    /// Tags for filtering
    pub tags: Vec<String>,

    /// Entry type as string
    pub entry_type: String,
}

/// Represents a collection/folder for organizing entries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collection {
    /// Unique identifier (UUID)
    pub id: String,

    /// Collection name (e.g., "API Documentation", "docs.example.com")
    pub name: String,

    /// Optional description
    pub description: Option<String>,

    /// Parent collection ID (for hierarchy)
    pub parent_id: Option<String>,

    /// How this collection was created
    pub source: CollectionSource,

    /// Number of entries in this collection
    pub entry_count: usize,

    /// Created timestamp
    pub created_at: DateTime<Utc>,

    /// Updated timestamp
    pub updated_at: DateTime<Utc>,
}

/// How a collection was created.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CollectionSource {
    /// Auto-created from URL domain
    UrlDomain { domain: String },
    /// Auto-created from file path
    FilePath { path: String },
    /// User-created manually
    UserDefined,
    /// System default collection
    System,
}

impl CollectionSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            CollectionSource::UrlDomain { .. } => "url_domain",
            CollectionSource::FilePath { .. } => "file_path",
            CollectionSource::UserDefined => "user_defined",
            CollectionSource::System => "system",
        }
    }
}

/// Represents an explicit relationship between two entries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryRelationship {
    /// Unique relationship ID
    pub id: String,

    /// Source entry ID
    pub source_id: String,

    /// Target entry ID
    pub target_id: String,

    /// Type of relationship
    pub relationship_type: RelationshipType,

    /// Optional description of the relationship
    pub description: Option<String>,

    /// Confidence score (0.0-1.0) for auto-detected relationships
    pub confidence: Option<f32>,

    /// How the relationship was created
    pub source: RelationshipSource,

    /// Created timestamp
    pub created_at: DateTime<Utc>,
}

/// Type of relationship between entries.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RelationshipType {
    /// General related content
    Related,
    /// One entry references another
    References,
    /// One entry is a parent/contains another
    Parent,
    /// One entry follows/continues another
    Follows,
    /// One entry supersedes/replaces another
    Supersedes,
    /// Custom relationship type
    Custom(String),
}

impl RelationshipType {
    pub fn as_str(&self) -> &str {
        match self {
            RelationshipType::Related => "related",
            RelationshipType::References => "references",
            RelationshipType::Parent => "parent",
            RelationshipType::Follows => "follows",
            RelationshipType::Supersedes => "supersedes",
            RelationshipType::Custom(s) => s.as_str(),
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "related" => RelationshipType::Related,
            "references" => RelationshipType::References,
            "parent" => RelationshipType::Parent,
            "follows" => RelationshipType::Follows,
            "supersedes" => RelationshipType::Supersedes,
            other => RelationshipType::Custom(other.to_string()),
        }
    }
}

/// How a relationship was created.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RelationshipSource {
    /// Extracted from content links
    AutoExtracted,
    /// Created by user
    UserDefined,
    /// Inferred from semantic similarity
    SemanticSimilarity,
}

impl RelationshipSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            RelationshipSource::AutoExtracted => "auto_extracted",
            RelationshipSource::UserDefined => "user_defined",
            RelationshipSource::SemanticSimilarity => "semantic_similarity",
        }
    }
}

/// Represents a tag/keyword for an entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedTag {
    /// Tag name (normalized)
    pub text: String,

    /// Relevance score (0.0-1.0)
    pub score: f32,

    /// How this tag was created
    pub source: TagSource,
}

/// How a tag was created.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TagSource {
    /// Auto-extracted during indexing (RAKE)
    Rake,
    /// Auto-extracted during indexing (TF-IDF)
    TfIdf,
    /// From source metadata (HTML keywords, etc.)
    MetaTag,
    /// User-provided
    UserDefined,
}

impl TagSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            TagSource::Rake => "rake",
            TagSource::TfIdf => "tfidf",
            TagSource::MetaTag => "meta_tag",
            TagSource::UserDefined => "user_defined",
        }
    }
}

impl Entry {
    /// Creates a new Entry with all required fields.
    pub fn new(
        title: String,
        source_path: String,
        source_hash: String,
    ) -> Self {
        let id = Uuid::new_v4().to_string();
        let slug = Self::path_to_slug(&source_path);
        let source_type = SourceType::from_path(&source_path);
        let entry_type = Self::infer_entry_type(&source_type);
        let now = Utc::now();

        Self {
            id,
            title,
            description: String::new(),
            content: String::new(),
            tags: Vec::new(),
            collection_ids: Vec::new(),
            entry_type,
            source_type,
            source_path,
            slug,
            source_hash,
            created_at: now,
            updated_at: now,
        }
    }

    /// Creates a new Entry with a specific ID.
    pub fn with_id(
        id: String,
        title: String,
        source_path: String,
        source_hash: String,
    ) -> Self {
        let slug = Self::path_to_slug(&source_path);
        let source_type = SourceType::from_path(&source_path);
        let entry_type = Self::infer_entry_type(&source_type);
        let now = Utc::now();

        Self {
            id,
            title,
            description: String::new(),
            content: String::new(),
            tags: Vec::new(),
            collection_ids: Vec::new(),
            entry_type,
            source_type,
            source_path,
            slug,
            source_hash,
            created_at: now,
            updated_at: now,
        }
    }

    /// Builder-style method to set description.
    pub fn with_description(mut self, description: String) -> Self {
        self.description = description;
        self
    }

    /// Builder-style method to set content.
    pub fn with_content(mut self, content: String) -> Self {
        self.content = content;
        self
    }

    /// Builder-style method to set tags.
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Builder-style method to set entry type.
    pub fn with_entry_type(mut self, entry_type: EntryType) -> Self {
        self.entry_type = entry_type;
        self
    }

    /// Builder-style method to set source type.
    pub fn with_source_type(mut self, source_type: SourceType) -> Self {
        self.source_type = source_type;
        self
    }

    /// Builder-style method to set slug.
    pub fn with_slug(mut self, slug: String) -> Self {
        self.slug = slug;
        self
    }

    /// Backward compatibility: generate_id for pattern-style IDs.
    /// Deprecated: use Entry::new() which generates UUID, or with_id() for custom IDs.
    #[deprecated(note = "Use Entry::new() or Entry::with_id() instead")]
    pub fn generate_id(_entry_type: EntryType, slug: &str) -> String {
        // For backward compatibility, just use the slug as ID
        slug.to_lowercase()
    }

    /// Converts a file path to a slug.
    fn path_to_slug(path: &str) -> String {
        // For URLs, use the path component
        if path.starts_with("http://") || path.starts_with("https://") {
            if let Ok(url) = url::Url::parse(path) {
                return url.path()
                    .trim_start_matches('/')
                    .trim_end_matches('/')
                    .replace('/', "-")
                    .to_lowercase();
            }
        }

        // For file paths, use the filename
        std::path::Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_else(|| "unknown".to_string())
    }

    /// Infers entry type from source type.
    fn infer_entry_type(source_type: &SourceType) -> EntryType {
        match source_type {
            SourceType::Pdf => EntryType::Pdf,
            SourceType::SourceCode => EntryType::Code,
            SourceType::Html | SourceType::Markdown | SourceType::Url => EntryType::Document,
            _ => EntryType::Other,
        }
    }

    /// Generates a deterministic ID from source path (for deduplication).
    pub fn generate_id_from_path(source_path: &str) -> String {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(source_path.as_bytes());
        let result = hasher.finalize();
        format!("{:x}", result)[..32].to_string()
    }
}

impl EntryChunk {
    /// Creates a new EntryChunk.
    pub fn new(
        entry_id: String,
        chunk_index: u32,
        chunk_type: ChunkType,
        text: String,
        metadata: ChunkMetadata,
    ) -> Self {
        let chunk_id = format!("{}#{}", entry_id, chunk_index);
        Self {
            chunk_id,
            entry_id,
            page_id: None,
            chunk_index,
            chunk_type,
            text,
            metadata,
        }
    }

    /// Creates a new EntryChunk with a page_id (two-layer storage).
    pub fn with_page_id(mut self, page_id: String) -> Self {
        self.page_id = Some(page_id);
        self
    }
}

impl Collection {
    /// Creates a new user-defined collection.
    pub fn new_user_defined(name: String, description: Option<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            description,
            parent_id: None,
            source: CollectionSource::UserDefined,
            entry_count: 0,
            created_at: now,
            updated_at: now,
        }
    }

    /// Creates a collection from a URL domain.
    pub fn from_url(url_str: &str) -> Option<Self> {
        let url = url::Url::parse(url_str).ok()?;
        let domain = url.host_str()?.to_string();
        let now = Utc::now();

        Some(Self {
            id: Uuid::new_v4().to_string(),
            name: domain.clone(),
            description: Some(format!("Content from {}", domain)),
            parent_id: None,
            source: CollectionSource::UrlDomain { domain },
            entry_count: 0,
            created_at: now,
            updated_at: now,
        })
    }

    /// Creates a collection from a file path.
    pub fn from_file_path(path: &str) -> Self {
        let parent = std::path::Path::new(path)
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("uploads")
            .to_string();

        let now = Utc::now();

        Self {
            id: Uuid::new_v4().to_string(),
            name: parent.clone(),
            description: Some(format!("Files from {}", parent)),
            parent_id: None,
            source: CollectionSource::FilePath { path: path.to_string() },
            entry_count: 0,
            created_at: now,
            updated_at: now,
        }
    }
}

impl EntryRelationship {
    /// Creates a new relationship.
    pub fn new(
        source_id: String,
        target_id: String,
        relationship_type: RelationshipType,
        source: RelationshipSource,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            source_id,
            target_id,
            relationship_type,
            description: None,
            confidence: None,
            source,
            created_at: Utc::now(),
        }
    }

    /// Creates a user-defined relationship.
    pub fn user_defined(
        source_id: String,
        target_id: String,
        relationship_type: RelationshipType,
        description: Option<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            source_id,
            target_id,
            relationship_type,
            description,
            confidence: None,
            source: RelationshipSource::UserDefined,
            created_at: Utc::now(),
        }
    }
}

// ============================================================================
// Backward compatibility aliases (to be removed in future versions)
// ============================================================================

/// Alias for backward compatibility - use Entry instead.
pub type Document = Entry;

/// Alias for backward compatibility - use EntryChunk instead.
pub type DocumentChunk = EntryChunk;

/// Alias for backward compatibility - use EntryType instead.
pub type PatternType = EntryType;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entry_creation() {
        let entry = Entry::new(
            "Test Entry".to_string(),
            "/path/to/file.html".to_string(),
            "abc123".to_string(),
        );
        assert_eq!(entry.title, "Test Entry");
        assert_eq!(entry.slug, "file");
        assert_eq!(entry.entry_type, EntryType::Document);
    }

    #[test]
    fn test_entry_type_display() {
        assert_eq!(EntryType::Document.as_str(), "document");
        assert_eq!(EntryType::Pdf.as_str(), "pdf");
        assert_eq!(EntryType::Code.as_str(), "code");
    }

    #[test]
    fn test_chunk_id_format() {
        let chunk = EntryChunk::new(
            "entry-123".to_string(),
            0,
            ChunkType::Summary,
            "test".to_string(),
            ChunkMetadata {
                entry_title: "Test".to_string(),
                tags: vec!["tag1".to_string()],
                entry_type: "document".to_string(),
            },
        );
        assert_eq!(chunk.chunk_id, "entry-123#0");
    }

    #[test]
    fn test_collection_from_url() {
        let collection = Collection::from_url("https://docs.example.com/api/v1");
        assert!(collection.is_some());
        let c = collection.unwrap();
        assert_eq!(c.name, "docs.example.com");
        assert!(matches!(c.source, CollectionSource::UrlDomain { .. }));
    }

    #[test]
    fn test_source_type_from_path() {
        assert_eq!(SourceType::from_path("https://example.com/doc"), SourceType::Url);
        assert_eq!(SourceType::from_path("/path/to/file.pdf"), SourceType::Pdf);
        assert_eq!(SourceType::from_path("/path/to/code.rs"), SourceType::SourceCode);
    }
}
