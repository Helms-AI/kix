//! File upload handling with ZIP extraction support

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio::fs;
use tracing::info;
use zip::ZipArchive;

use crate::CrawlerError;

/// Configuration for file handler
#[derive(Clone, Debug)]
pub struct FileHandlerConfig {
    /// Upload directory for temporary files
    pub upload_dir: PathBuf,
    /// Maximum file size (bytes)
    pub max_file_size: u64,
    /// Maximum total upload size (bytes)
    pub max_total_size: u64,
    /// Allowed file extensions
    pub allowed_extensions: Vec<String>,
    /// Auto-extract ZIP files
    pub auto_extract_archives: bool,
    /// Maximum files to extract from archive
    pub max_archive_files: usize,
}

impl Default for FileHandlerConfig {
    fn default() -> Self {
        Self {
            upload_dir: std::env::temp_dir().join("eip-uploads"),
            max_file_size: 100 * 1024 * 1024,     // 100MB per file
            max_total_size: 500 * 1024 * 1024,    // 500MB total
            allowed_extensions: vec![
                // Documents
                "html".to_string(),
                "htm".to_string(),
                "pdf".to_string(),
                "docx".to_string(),
                "doc".to_string(),
                "txt".to_string(),
                "md".to_string(),
                "markdown".to_string(),
                // Data
                "json".to_string(),
                "xml".to_string(),
                "csv".to_string(),
                "xlsx".to_string(),
                "xls".to_string(),
                // Source code
                "py".to_string(),
                "js".to_string(),
                "ts".to_string(),
                "jsx".to_string(),
                "tsx".to_string(),
                "rs".to_string(),
                "java".to_string(),
                "go".to_string(),
                "cpp".to_string(),
                "c".to_string(),
                "h".to_string(),
                // Archives
                "zip".to_string(),
            ],
            auto_extract_archives: true,
            max_archive_files: 1000,
        }
    }
}

/// Information about an uploaded file
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UploadedFile {
    /// Original filename
    pub original_name: String,
    /// Path on disk
    pub path: PathBuf,
    /// File size in bytes
    pub size: u64,
    /// MIME type
    pub mime_type: String,
    /// Whether this was extracted from an archive
    pub from_archive: bool,
    /// Source archive name if extracted
    pub source_archive: Option<String>,
}

/// Handles file uploads and extraction
pub struct FileHandler {
    config: FileHandlerConfig,
}

impl FileHandler {
    /// Create a new file handler
    pub fn new(config: FileHandlerConfig) -> Self {
        Self { config }
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(FileHandlerConfig::default())
    }

    /// Initialize the upload directory
    pub async fn init(&self) -> Result<(), CrawlerError> {
        fs::create_dir_all(&self.config.upload_dir).await?;
        Ok(())
    }

    /// Process uploaded file data
    pub async fn process_upload(
        &self,
        filename: &str,
        data: &[u8],
    ) -> Result<Vec<UploadedFile>, CrawlerError> {
        // Validate file size
        if data.len() as u64 > self.config.max_file_size {
            return Err(CrawlerError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "File too large: {} bytes (max: {} bytes)",
                    data.len(),
                    self.config.max_file_size
                ),
            )));
        }

        // Validate extension
        let extension = Path::new(filename)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        if !self.config.allowed_extensions.contains(&extension) {
            return Err(CrawlerError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("File type not allowed: {}", extension),
            )));
        }

        // Create unique upload path
        let upload_id = uuid::Uuid::new_v4();
        let upload_subdir = self.config.upload_dir.join(upload_id.to_string());
        fs::create_dir_all(&upload_subdir).await?;

        // Check if it's a ZIP file
        if extension == "zip" && self.config.auto_extract_archives {
            info!(filename = filename, "Processing ZIP archive");
            return self.extract_archive(filename, data, &upload_subdir).await;
        }

        // Save regular file
        let file_path = upload_subdir.join(filename);
        fs::write(&file_path, data).await?;

        let mime_type = self.detect_mime_type(&extension);

        Ok(vec![UploadedFile {
            original_name: filename.to_string(),
            path: file_path,
            size: data.len() as u64,
            mime_type,
            from_archive: false,
            source_archive: None,
        }])
    }

    /// Extract files from a ZIP archive
    async fn extract_archive(
        &self,
        archive_name: &str,
        data: &[u8],
        output_dir: &Path,
    ) -> Result<Vec<UploadedFile>, CrawlerError> {
        // Clone necessary data for the blocking task
        let data = data.to_vec();
        let output_dir = output_dir.to_path_buf();
        let archive_name = archive_name.to_string();
        let archive_name_for_log = archive_name.clone();
        let allowed_extensions = self.config.allowed_extensions.clone();
        let max_archive_files = self.config.max_archive_files;
        let max_file_size = self.config.max_file_size;

        // Use spawn_blocking since ZipFile is not Send
        let extracted_files = tokio::task::spawn_blocking(move || {
            let cursor = std::io::Cursor::new(data);
            let mut archive = ZipArchive::new(cursor).map_err(|e| {
                CrawlerError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Invalid ZIP archive: {}", e),
                ))
            })?;

            let mut extracted_files = Vec::new();
            let file_count = archive.len();

            if file_count > max_archive_files {
                return Err(CrawlerError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "Archive contains too many files: {} (max: {})",
                        file_count, max_archive_files
                    ),
                )));
            }

            for i in 0..file_count {
                let mut file = archive.by_index(i).map_err(|e| {
                    CrawlerError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("Failed to read archive entry: {}", e),
                    ))
                })?;

                // Skip directories
                if file.is_dir() {
                    continue;
                }

                let file_name = file.name().to_string();

                // Security: prevent path traversal
                if file_name.contains("..") || file_name.starts_with('/') {
                    continue;
                }

                // Get extension
                let extension = Path::new(&file_name)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();

                // Skip disallowed file types
                if !allowed_extensions.contains(&extension) {
                    continue;
                }

                // Create output path (flatten directory structure)
                let safe_name = Path::new(&file_name)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&file_name)
                    .to_string();

                let output_path = output_dir.join(&safe_name);

                // Ensure unique filename (synchronous version)
                let output_path = ensure_unique_path_sync(&output_path);

                // Read file contents
                let mut contents = Vec::new();
                file.read_to_end(&mut contents).map_err(|e| {
                    CrawlerError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("Failed to read file from archive: {}", e),
                    ))
                })?;

                // Check file size
                if contents.len() as u64 > max_file_size {
                    continue;
                }

                // Write to disk
                let mut output_file = std::fs::File::create(&output_path)?;
                output_file.write_all(&contents)?;

                let mime_type = detect_mime_type_static(&extension);

                extracted_files.push(UploadedFile {
                    original_name: safe_name,
                    path: output_path,
                    size: contents.len() as u64,
                    mime_type,
                    from_archive: true,
                    source_archive: Some(archive_name.clone()),
                });
            }

            Ok::<_, CrawlerError>(extracted_files)
        })
        .await
        .map_err(|e| {
            CrawlerError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Task join error: {}", e),
            ))
        })??;

        info!(
            archive = archive_name_for_log,
            extracted = extracted_files.len(),
            "Archive extraction complete"
        );

        Ok(extracted_files)
    }

    /// Ensure a file path is unique by adding a suffix if needed
    #[allow(dead_code)]
    async fn ensure_unique_path(&self, path: &Path) -> PathBuf {
        if !path.exists() {
            return path.to_path_buf();
        }

        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("file");
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let parent = path.parent().unwrap_or(Path::new("."));

        let mut counter = 1;
        loop {
            let new_name = if extension.is_empty() {
                format!("{}_{}", stem, counter)
            } else {
                format!("{}_{}.{}", stem, counter, extension)
            };

            let new_path = parent.join(new_name);
            if !new_path.exists() {
                return new_path;
            }
            counter += 1;
        }
    }

    /// Detect MIME type from extension
    fn detect_mime_type(&self, extension: &str) -> String {
        match extension {
            "html" | "htm" => "text/html",
            "pdf" => "application/pdf",
            "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            "doc" => "application/msword",
            "txt" => "text/plain",
            "md" | "markdown" => "text/markdown",
            "json" => "application/json",
            "xml" => "application/xml",
            "csv" => "text/csv",
            "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            "xls" => "application/vnd.ms-excel",
            "py" => "text/x-python",
            "js" | "jsx" => "text/javascript",
            "ts" | "tsx" => "text/typescript",
            "rs" => "text/x-rust",
            "java" => "text/x-java",
            "go" => "text/x-go",
            "cpp" | "cc" => "text/x-c++",
            "c" | "h" => "text/x-c",
            "zip" => "application/zip",
            _ => "application/octet-stream",
        }
        .to_string()
    }

    /// Cleanup uploaded files
    pub async fn cleanup(&self, upload_id: &str) -> Result<(), CrawlerError> {
        let upload_dir = self.config.upload_dir.join(upload_id);
        if upload_dir.exists() {
            fs::remove_dir_all(&upload_dir).await?;
        }
        Ok(())
    }

    /// Get total size of uploaded files
    pub fn total_size(files: &[UploadedFile]) -> u64 {
        files.iter().map(|f| f.size).sum()
    }
}

impl Default for FileHandler {
    fn default() -> Self {
        Self::with_defaults()
    }
}

/// Ensure a file path is unique by adding a suffix if needed (synchronous version)
fn ensure_unique_path_sync(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }

    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file");

    let extension = path.extension().and_then(|e| e.to_str());

    let parent = path.parent().unwrap_or(Path::new("."));

    let mut counter = 1;
    loop {
        let new_name = if let Some(ext) = extension {
            format!("{}_{}.{}", stem, counter, ext)
        } else {
            format!("{}_{}", stem, counter)
        };

        let new_path = parent.join(new_name);
        if !new_path.exists() {
            return new_path;
        }
        counter += 1;
    }
}

/// Detect MIME type from extension (static version for use in spawn_blocking)
fn detect_mime_type_static(extension: &str) -> String {
    match extension {
        "html" | "htm" => "text/html",
        "pdf" => "application/pdf",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "doc" => "application/msword",
        "txt" => "text/plain",
        "md" | "markdown" => "text/markdown",
        "json" => "application/json",
        "xml" => "application/xml",
        "csv" => "text/csv",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "xls" => "application/vnd.ms-excel",
        "py" => "text/x-python",
        "js" | "jsx" => "text/javascript",
        "ts" | "tsx" => "text/typescript",
        "rs" => "text/x-rust",
        "java" => "text/x-java",
        "go" => "text/x-go",
        "cpp" | "cc" => "text/x-c++",
        "c" | "h" => "text/x-c",
        "zip" => "application/zip",
        _ => "application/octet-stream",
    }
    .to_string()
}