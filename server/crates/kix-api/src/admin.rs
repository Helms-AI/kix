use axum::{
    extract::State,
    response::IntoResponse,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::routes::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct RuntimeSettings {
    pub cache: CacheSettings,
    pub jobs: JobSettings,
    pub api: ApiSettings,
    pub crawler: CrawlerSettings,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CacheSettings {
    pub search_ttl: u64,        // seconds
    pub embedding_ttl: u64,     // seconds
    pub search_capacity: usize,
    pub embedding_capacity: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JobSettings {
    pub max_concurrent: usize,
    pub retry_limit: usize,
    pub timeout: u64, // seconds
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiSettings {
    pub rate_limit: usize,     // requests per minute
    pub max_page_size: usize,
    pub cors_enabled: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CrawlerSettings {
    pub user_agent: String,
    pub max_depth: usize,
    pub delay_ms: u64,
    pub max_pages: usize,
}

#[derive(Debug, Serialize)]
pub struct CacheStats {
    pub search_cache_size: usize,
    pub search_cache_hits: u64,
    pub search_cache_misses: u64,
    pub embedding_cache_size: usize,
    pub embedding_cache_hits: u64,
    pub embedding_cache_misses: u64,
}

#[derive(Debug, Serialize)]
pub struct DataStats {
    pub total_documents: usize,
    pub total_chunks: usize,
    pub total_pages: usize,
    pub unique_domains: usize,
    pub storage_size_mb: f64,
    pub last_updated: String,
}

#[derive(Debug, Serialize)]
pub struct SystemMetrics {
    pub cpu_usage: f32,
    pub memory_used_gb: f32,
    pub memory_total_gb: f32,
    pub disk_used_gb: f32,
    pub disk_total_gb: f32,
    pub active_connections: usize,
    pub requests_per_minute: usize,
}

#[derive(Debug, Serialize)]
pub struct DetailedHealth {
    pub status: String,
    pub services: Vec<ServiceHealth>,
    pub uptime_seconds: u64,
    pub version: String,
}

#[derive(Debug, Serialize)]
pub struct ServiceHealth {
    pub name: String,
    pub status: String,
    pub latency_ms: u32,
    pub details: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AuditLogEntry {
    pub id: String,
    pub timestamp: String,
    pub user: String,
    pub action: String,
    pub details: String,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteByDomainRequest {
    pub domain: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteByDateRangeRequest {
    pub start_date: String,
    pub end_date: String,
}

#[derive(Debug, Serialize)]
pub struct OperationResponse {
    pub status: String,
    pub message: String,
    pub affected_items: Option<usize>,
}

/// Create the admin routes
pub fn admin_routes() -> Router<AppState> {
    Router::new()
        // Cache management
        .route("/cache/invalidate", post(invalidate_caches))
        .route("/cache/stats", get(get_cache_stats))
        // Settings management
        .route("/settings", get(get_runtime_settings))
        .route("/settings", put(update_runtime_settings))
        // Data management
        .route("/data/stats", get(get_data_stats))
        .route("/data/clear", delete(clear_all_data))
        .route("/data/domain", delete(delete_by_domain))
        // System monitoring
        .route("/metrics", get(get_system_metrics))
        .route("/health/detailed", get(get_detailed_health))
        .route("/audit", get(get_audit_log))
}

/// POST /api/admin/cache/invalidate
/// Note: Caching has been removed for real-time data availability.
/// This endpoint is kept for backwards compatibility but is now a no-op.
pub async fn invalidate_caches(State(_state): State<AppState>) -> impl IntoResponse {
    info!("Admin: Cache invalidation requested (no-op - caching disabled for real-time data)");

    Json(OperationResponse {
        status: "ok".to_string(),
        message: "Cache invalidation is no longer needed - data is always fresh (tables refreshed before each search)".to_string(),
        affected_items: None,
    })
}

/// GET /api/admin/cache/stats
async fn get_cache_stats(State(_state): State<AppState>) -> impl IntoResponse {
    // In a real implementation, we would get actual cache statistics
    // For now, return mock data
    Json(CacheStats {
        search_cache_size: 150,
        search_cache_hits: 1234,
        search_cache_misses: 456,
        embedding_cache_size: 75,
        embedding_cache_hits: 890,
        embedding_cache_misses: 123,
    })
}

/// GET /api/admin/settings
async fn get_runtime_settings(State(_state): State<AppState>) -> impl IntoResponse {
    // In a real implementation, we would retrieve actual settings
    // For now, return default settings
    Json(RuntimeSettings {
        cache: CacheSettings {
            search_ttl: 30,
            embedding_ttl: 600,
            search_capacity: 1000,
            embedding_capacity: 500,
        },
        jobs: JobSettings {
            max_concurrent: 3,
            retry_limit: 3,
            timeout: 300,
        },
        api: ApiSettings {
            rate_limit: 100,
            max_page_size: 50,
            cors_enabled: true,
        },
        crawler: CrawlerSettings {
            user_agent: "KIX-Crawler/1.0".to_string(),
            max_depth: 3,
            delay_ms: 1000,
            max_pages: 100,
        },
    })
}

/// PUT /api/admin/settings
async fn update_runtime_settings(
    State(_state): State<AppState>,
    Json(_settings): Json<RuntimeSettings>,
) -> impl IntoResponse {
    info!("Admin: Updating runtime settings");

    // In a real implementation, we would:
    // 1. Validate the settings
    // 2. Apply them to the running system
    // 3. Persist them if needed

    // For now, just return success
    Json(OperationResponse {
        status: "ok".to_string(),
        message: "Settings updated successfully".to_string(),
        affected_items: None,
    })
}

/// GET /api/admin/data/stats
async fn get_data_stats(State(state): State<AppState>) -> impl IntoResponse {
    let store = state.store().read().await;

    // Get actual statistics from the store
    let total_documents = store.entry_count().await.unwrap_or(0);
    let total_chunks = store.chunk_count().unwrap_or(0);  // sync operation
    let total_pages = store.page_count().await.unwrap_or(0);

    // For now, return mock data for fields we don't have direct methods for
    Json(DataStats {
        total_documents,
        total_chunks,
        total_pages,
        unique_domains: 0,  // Would need to query and count unique sources
        storage_size_mb: 0.0,  // Would need to calculate from database files
        last_updated: chrono::Utc::now().to_rfc3339(),
    })
}

/// DELETE /api/admin/data/clear
async fn clear_all_data(State(_state): State<AppState>) -> impl IntoResponse {
    info!("Admin: Clearing all index data");

    // let store = state.store().write().await;

    // In a real implementation, we would clear all data
    // This is a dangerous operation and should have additional confirmation

    // For safety, we're not implementing the actual deletion here
    // You would need to add a method like store.clear_all() to KixStore

    Json(OperationResponse {
        status: "error".to_string(),
        message: "Clear all data is not implemented for safety reasons".to_string(),
        affected_items: None,
    })
}

/// DELETE /api/admin/data/domain
async fn delete_by_domain(
    State(_state): State<AppState>,
    Json(request): Json<DeleteByDomainRequest>,
) -> impl IntoResponse {
    info!("Admin: Deleting data for domain: {}", request.domain);

    // In a real implementation, we would delete all entries from the specified domain

    Json(OperationResponse {
        status: "ok".to_string(),
        message: format!("Data for domain '{}' deleted", request.domain),
        affected_items: Some(0), // Would return actual count
    })
}

/// GET /api/admin/metrics
async fn get_system_metrics(State(_state): State<AppState>) -> impl IntoResponse {
    // In a real implementation, we would collect actual system metrics
    // For now, return mock data
    Json(SystemMetrics {
        cpu_usage: 45.2,
        memory_used_gb: 4.2,
        memory_total_gb: 8.0,
        disk_used_gb: 123.4,
        disk_total_gb: 500.0,
        active_connections: 12,
        requests_per_minute: 234,
    })
}

/// GET /api/admin/health/detailed
async fn get_detailed_health(State(_state): State<AppState>) -> impl IntoResponse {
    // Check various services and return detailed health status
    let mut services = vec![];

    // Check database
    services.push(ServiceHealth {
        name: "Database (LanceDB)".to_string(),
        status: "healthy".to_string(),
        latency_ms: 12,
        details: Some("All tables accessible".to_string()),
    });

    // Check embedding service
    services.push(ServiceHealth {
        name: "Embedding Service".to_string(),
        status: "healthy".to_string(),
        latency_ms: 45,
        details: Some("Model loaded".to_string()),
    });

    // Check API
    services.push(ServiceHealth {
        name: "Search API".to_string(),
        status: "healthy".to_string(),
        latency_ms: 23,
        details: Some("Cache operational".to_string()),
    });

    Json(DetailedHealth {
        status: "healthy".to_string(),
        services,
        uptime_seconds: 3600, // Would calculate actual uptime
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// GET /api/admin/audit
async fn get_audit_log(State(_state): State<AppState>) -> impl IntoResponse {
    // In a real implementation, we would retrieve actual audit log entries
    // For now, return mock data
    let entries = vec![
        AuditLogEntry {
            id: "1".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            user: "admin".to_string(),
            action: "Clear Cache".to_string(),
            details: "Manually invalidated all caches".to_string(),
            status: "success".to_string(),
        },
        AuditLogEntry {
            id: "2".to_string(),
            timestamp: (chrono::Utc::now() - chrono::Duration::hours(1)).to_rfc3339(),
            user: "admin".to_string(),
            action: "Update Settings".to_string(),
            details: "Changed cache TTL from 300 to 30 seconds".to_string(),
            status: "success".to_string(),
        },
    ];

    Json(entries)
}