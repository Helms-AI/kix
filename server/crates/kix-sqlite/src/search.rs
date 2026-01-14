//! Full-text search (FTS5) operations for SQLite.

use crate::error::Result;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

/// FTS search result.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FtsResult {
    /// ID of the matched record
    pub id: String,

    /// Table the result came from
    pub table: String,

    /// Title (if available)
    pub title: Option<String>,

    /// Snippet of matched text
    pub snippet: Option<String>,

    /// BM25 relevance score (lower is better in SQLite FTS5)
    pub rank: f64,
}

/// Search entries using FTS5.
pub async fn search_entries(pool: &SqlitePool, query: &str, limit: usize) -> Result<Vec<FtsResult>> {
    // FTS5 uses bm25() for ranking - lower scores are better
    let results = sqlx::query_as::<_, (String, String, Option<String>, f64)>(
        r#"
        SELECT
            e.id,
            e.title,
            snippet(entries_fts, 2, '<mark>', '</mark>', '...', 32) as snippet,
            bm25(entries_fts) as rank
        FROM entries_fts
        JOIN entries e ON entries_fts.rowid = e.rowid
        WHERE entries_fts MATCH ?
        ORDER BY rank
        LIMIT ?
        "#,
    )
    .bind(query)
    .bind(limit as i64)
    .fetch_all(pool)
    .await?;

    Ok(results
        .into_iter()
        .map(|(id, title, snippet, rank)| FtsResult {
            id,
            table: "entries".to_string(),
            title: Some(title),
            snippet,
            rank,
        })
        .collect())
}

/// Search pages using FTS5.
pub async fn search_pages(pool: &SqlitePool, query: &str, limit: usize) -> Result<Vec<FtsResult>> {
    let results = sqlx::query_as::<_, (String, Option<String>, Option<String>, f64)>(
        r#"
        SELECT
            p.page_id,
            p.title,
            snippet(pages_fts, 1, '<mark>', '</mark>', '...', 32) as snippet,
            bm25(pages_fts) as rank
        FROM pages_fts
        JOIN pages p ON pages_fts.rowid = p.rowid
        WHERE pages_fts MATCH ?
        ORDER BY rank
        LIMIT ?
        "#,
    )
    .bind(query)
    .bind(limit as i64)
    .fetch_all(pool)
    .await?;

    Ok(results
        .into_iter()
        .map(|(id, title, snippet, rank)| FtsResult {
            id,
            table: "pages".to_string(),
            title,
            snippet,
            rank,
        })
        .collect())
}

/// Search issues using FTS5.
pub async fn search_issues(
    pool: &SqlitePool,
    query: &str,
    project_id: Option<&str>,
    limit: usize,
) -> Result<Vec<FtsResult>> {
    let results = match project_id {
        Some(pid) => {
            sqlx::query_as::<_, (String, String, Option<String>, f64)>(
                r#"
                SELECT
                    i.id,
                    i.title,
                    snippet(issues_fts, 1, '<mark>', '</mark>', '...', 32) as snippet,
                    bm25(issues_fts) as rank
                FROM issues_fts
                JOIN issues i ON issues_fts.rowid = i.rowid
                WHERE issues_fts MATCH ? AND i.project_id = ?
                ORDER BY rank
                LIMIT ?
                "#,
            )
            .bind(query)
            .bind(pid)
            .bind(limit as i64)
            .fetch_all(pool)
            .await?
        }
        None => {
            sqlx::query_as::<_, (String, String, Option<String>, f64)>(
                r#"
                SELECT
                    i.id,
                    i.title,
                    snippet(issues_fts, 1, '<mark>', '</mark>', '...', 32) as snippet,
                    bm25(issues_fts) as rank
                FROM issues_fts
                JOIN issues i ON issues_fts.rowid = i.rowid
                WHERE issues_fts MATCH ?
                ORDER BY rank
                LIMIT ?
                "#,
            )
            .bind(query)
            .bind(limit as i64)
            .fetch_all(pool)
            .await?
        }
    };

    Ok(results
        .into_iter()
        .map(|(id, title, snippet, rank)| FtsResult {
            id,
            table: "issues".to_string(),
            title: Some(title),
            snippet,
            rank,
        })
        .collect())
}

/// Search across all tables using FTS5.
///
/// Combines results from entries, pages, and issues.
pub async fn search_all(pool: &SqlitePool, query: &str, limit: usize) -> Result<Vec<FtsResult>> {
    // Search each table and combine
    let per_table_limit = (limit / 3).max(1);

    let mut results = Vec::new();

    // Search entries
    if let Ok(entries) = search_entries(pool, query, per_table_limit).await {
        results.extend(entries);
    }

    // Search pages
    if let Ok(pages) = search_pages(pool, query, per_table_limit).await {
        results.extend(pages);
    }

    // Search issues
    if let Ok(issues) = search_issues(pool, query, None, per_table_limit).await {
        results.extend(issues);
    }

    // Sort by rank (lower is better in BM25)
    results.sort_by(|a, b| a.rank.partial_cmp(&b.rank).unwrap_or(std::cmp::Ordering::Equal));

    // Take top results
    results.truncate(limit);

    Ok(results)
}

/// Search with phrase matching.
///
/// Uses FTS5 phrase syntax for exact phrase matching.
pub async fn search_phrase(
    pool: &SqlitePool,
    phrase: &str,
    table: &str,
    limit: usize,
) -> Result<Vec<FtsResult>> {
    // Wrap in quotes for phrase matching
    let phrase_query = format!("\"{}\"", phrase.replace('"', ""));

    match table {
        "entries" => search_entries(pool, &phrase_query, limit).await,
        "pages" => search_pages(pool, &phrase_query, limit).await,
        "issues" => search_issues(pool, &phrase_query, None, limit).await,
        _ => Ok(vec![]),
    }
}

/// Search with prefix matching.
///
/// Uses FTS5 prefix syntax for prefix matching.
pub async fn search_prefix(
    pool: &SqlitePool,
    prefix: &str,
    table: &str,
    limit: usize,
) -> Result<Vec<FtsResult>> {
    // Add asterisk for prefix matching
    let prefix_query = format!("{}*", prefix);

    match table {
        "entries" => search_entries(pool, &prefix_query, limit).await,
        "pages" => search_pages(pool, &prefix_query, limit).await,
        "issues" => search_issues(pool, &prefix_query, None, limit).await,
        _ => Ok(vec![]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entries::{insert_entry, EntryRecord};
    use crate::issues::{insert_issue, IssueRecord};
    use crate::pages::{insert_page, PageRecord};
    use crate::pool::{create_pool, run_migrations};
    use crate::projects::{insert_project, ProjectRecord};
    use tempfile::TempDir;

    async fn setup_test_db() -> (SqlitePool, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let pool = create_pool(&db_path).await.unwrap();
        run_migrations(&pool).await.unwrap();
        (pool, temp_dir)
    }

    #[tokio::test]
    async fn test_search_entries() {
        let (pool, _temp_dir) = setup_test_db().await;

        // Insert test entries
        let mut entry1 = EntryRecord::new("e1", "Rust Programming Guide", "document", "url", "https://example.com/rust", "h1");
        entry1.content = Some("Learn Rust programming language with examples".to_string());
        insert_entry(&pool, &entry1).await.unwrap();

        let mut entry2 = EntryRecord::new("e2", "Python Tutorial", "document", "url", "https://example.com/python", "h2");
        entry2.content = Some("Python is a great language for beginners".to_string());
        insert_entry(&pool, &entry2).await.unwrap();

        // Search for "Rust"
        let results = search_entries(&pool, "Rust", 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "e1");

        // Search for "programming"
        let results = search_entries(&pool, "programming", 10).await.unwrap();
        assert_eq!(results.len(), 1); // Only Rust entry has "programming"
    }

    #[tokio::test]
    async fn test_search_pages() {
        let (pool, _temp_dir) = setup_test_db().await;

        // Create parent entry
        let entry = EntryRecord::new("entry-fts", "Parent", "document", "url", "https://example.com", "hash");
        insert_entry(&pool, &entry).await.unwrap();

        // Insert test pages
        let page1 = PageRecord::new("entry-fts", "https://example.com/api", "# API Documentation\n\nThis is the API reference.")
            .with_title("API Docs");
        insert_page(&pool, &page1).await.unwrap();

        let page2 = PageRecord::new("entry-fts", "https://example.com/guide", "# User Guide\n\nGetting started with the system.")
            .with_title("User Guide");
        insert_page(&pool, &page2).await.unwrap();

        // Search for "API"
        let results = search_pages(&pool, "API", 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].title.as_ref().unwrap().contains("API"));
    }

    #[tokio::test]
    async fn test_search_issues() {
        let (pool, _temp_dir) = setup_test_db().await;

        // Create project
        let mut project = ProjectRecord::new_local("Test Project");
        project.id = "proj-fts".to_string();
        insert_project(&pool, &project).await.unwrap();

        // Insert issues
        let issue1 = IssueRecord::new("proj-fts", 1, "Bug in authentication").with_body("Users cannot login");
        insert_issue(&pool, &issue1).await.unwrap();

        let issue2 = IssueRecord::new("proj-fts", 2, "Feature request: dark mode").with_body("Add dark theme support");
        insert_issue(&pool, &issue2).await.unwrap();

        // Search for "authentication"
        let results = search_issues(&pool, "authentication", None, 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].title.as_ref().unwrap().contains("authentication"));

        // Search within project
        let results = search_issues(&pool, "dark", Some("proj-fts"), 10).await.unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_search_all() {
        let (pool, _temp_dir) = setup_test_db().await;

        // Create entry
        let mut entry = EntryRecord::new("e-all", "Integration Testing", "document", "url", "https://test.com", "h");
        entry.content = Some("Testing integration patterns".to_string());
        insert_entry(&pool, &entry).await.unwrap();

        // Create page
        let page = PageRecord::new("e-all", "https://test.com/patterns", "# Integration Patterns\n\nCommon patterns.");
        insert_page(&pool, &page).await.unwrap();

        // Create project and issue
        let mut project = ProjectRecord::new_local("Project");
        project.id = "proj-all".to_string();
        insert_project(&pool, &project).await.unwrap();

        let issue = IssueRecord::new("proj-all", 1, "Integration test failure");
        insert_issue(&pool, &issue).await.unwrap();

        // Search across all
        let results = search_all(&pool, "integration", 10).await.unwrap();
        assert!(results.len() >= 1); // Should find results from multiple tables
    }

    #[tokio::test]
    async fn test_phrase_search() {
        let (pool, _temp_dir) = setup_test_db().await;

        let mut entry = EntryRecord::new("phrase", "The Quick Brown Fox", "document", "url", "https://example.com", "h");
        entry.content = Some("The quick brown fox jumps over the lazy dog".to_string());
        insert_entry(&pool, &entry).await.unwrap();

        // Exact phrase
        let results = search_phrase(&pool, "quick brown", "entries", 10).await.unwrap();
        assert_eq!(results.len(), 1);

        // Non-matching phrase
        let results = search_phrase(&pool, "brown quick", "entries", 10).await.unwrap();
        assert_eq!(results.len(), 0);
    }

    #[tokio::test]
    async fn test_prefix_search() {
        let (pool, _temp_dir) = setup_test_db().await;

        let entry = EntryRecord::new("prefix", "Documentation", "document", "url", "https://example.com", "h");
        insert_entry(&pool, &entry).await.unwrap();

        // Prefix match
        let results = search_prefix(&pool, "Doc", "entries", 10).await.unwrap();
        assert_eq!(results.len(), 1);
    }
}
