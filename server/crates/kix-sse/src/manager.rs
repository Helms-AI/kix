//! SSE connection management and broadcasting

use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use tokio::sync::broadcast;
use tracing::{debug, info};
use uuid::Uuid;

use crate::event::Event;
use crate::stream::SseConnection;
use crate::SseError;

/// Connection info for tracking
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub id: Uuid,
    pub job_id: Option<Uuid>,
    pub client_ip: IpAddr,
    pub created_at: Instant,
    pub last_activity: Instant,
}

/// Error types for connection management
#[derive(Debug, thiserror::Error)]
pub enum ConnectionError {
    #[error("Rate limit exceeded for IP {0}")]
    RateLimited(IpAddr),

    #[error("Maximum connections reached")]
    MaxConnectionsReached,

    #[error("Job not found: {0}")]
    JobNotFound(Uuid),

    #[error("Connection not found: {0}")]
    ConnectionNotFound(Uuid),
}

/// Configuration for connection manager
#[derive(Debug, Clone)]
pub struct ConnectionManagerConfig {
    /// Maximum connections per IP address
    pub max_connections_per_ip: usize,
    /// Maximum total connections
    pub max_total_connections: usize,
    /// Connection timeout duration
    pub connection_timeout: Duration,
    /// Broadcast channel capacity
    pub channel_capacity: usize,
    /// Cleanup interval
    pub cleanup_interval: Duration,
}

impl Default for ConnectionManagerConfig {
    fn default() -> Self {
        Self {
            max_connections_per_ip: 50,  // Increased for localhost development
            max_total_connections: 1000,
            connection_timeout: Duration::from_secs(300), // 5 minutes
            channel_capacity: 1024,
            cleanup_interval: Duration::from_secs(60),
        }
    }
}

/// Manages SSE connections and event broadcasting
pub struct ConnectionManager {
    /// Active connections
    connections: DashMap<Uuid, ConnectionInfo>,
    /// Per-job broadcast channels
    job_channels: DashMap<Uuid, broadcast::Sender<Event>>,
    /// Global broadcast channel for all events
    global_sender: broadcast::Sender<Event>,
    /// Configuration
    config: ConnectionManagerConfig,
}

impl ConnectionManager {
    /// Create a new connection manager
    pub fn new(config: ConnectionManagerConfig) -> Self {
        let (global_sender, _) = broadcast::channel(config.channel_capacity);

        Self {
            connections: DashMap::new(),
            job_channels: DashMap::new(),
            global_sender,
            config,
        }
    }

    /// Create a new connection manager with default config
    pub fn with_defaults() -> Self {
        Self::new(ConnectionManagerConfig::default())
    }

    /// Create a new SSE connection for a specific job
    pub fn connect_to_job(
        &self,
        job_id: Uuid,
        client_ip: IpAddr,
    ) -> Result<SseConnection, ConnectionError> {
        self.check_rate_limit(&client_ip)?;

        let sender = self
            .job_channels
            .entry(job_id)
            .or_insert_with(|| broadcast::channel(self.config.channel_capacity).0)
            .clone();

        let receiver = sender.subscribe();
        let connection = SseConnection::new(receiver, Some(job_id));

        self.register_connection(&connection, client_ip, Some(job_id));

        info!(
            connection_id = %connection.id,
            job_id = %job_id,
            client_ip = %client_ip,
            "New SSE connection for job"
        );

        Ok(connection)
    }

    /// Create a new SSE connection for global events
    pub fn connect_global(&self, client_ip: IpAddr) -> Result<SseConnection, ConnectionError> {
        self.check_rate_limit(&client_ip)?;

        let receiver = self.global_sender.subscribe();
        let connection = SseConnection::new(receiver, None);

        self.register_connection(&connection, client_ip, None);

        info!(
            connection_id = %connection.id,
            client_ip = %client_ip,
            "New global SSE connection"
        );

        Ok(connection)
    }

    /// Broadcast an event to a specific job's subscribers
    pub fn broadcast_to_job(&self, job_id: Uuid, event: Event) -> Result<usize, SseError> {
        // Also send to global channel
        let _ = self.global_sender.send(event.clone());

        if let Some(sender) = self.job_channels.get(&job_id) {
            match sender.send(event) {
                Ok(count) => {
                    debug!(job_id = %job_id, subscribers = count, "Event broadcast to job");
                    Ok(count)
                }
                Err(_) => {
                    // No active subscribers
                    debug!(job_id = %job_id, "No subscribers for job");
                    Ok(0)
                }
            }
        } else {
            debug!(job_id = %job_id, "No channel for job, creating one");
            Ok(0)
        }
    }

    /// Broadcast an event to all global subscribers
    pub fn broadcast_global(&self, event: Event) -> Result<usize, SseError> {
        match self.global_sender.send(event) {
            Ok(count) => {
                debug!(subscribers = count, "Event broadcast globally");
                Ok(count)
            }
            Err(_) => Ok(0),
        }
    }

    /// Get or create a job channel sender
    pub fn get_job_sender(&self, job_id: Uuid) -> broadcast::Sender<Event> {
        self.job_channels
            .entry(job_id)
            .or_insert_with(|| broadcast::channel(self.config.channel_capacity).0)
            .clone()
    }

    /// Remove connection when client disconnects
    pub fn disconnect(&self, connection_id: Uuid) {
        if let Some((_, info)) = self.connections.remove(&connection_id) {
            info!(
                connection_id = %connection_id,
                job_id = ?info.job_id,
                duration = ?info.created_at.elapsed(),
                "SSE connection closed"
            );
        }
    }

    /// Cleanup stale connections and empty job channels
    pub fn cleanup(&self) {
        let now = Instant::now();
        let timeout = self.config.connection_timeout;

        // Remove stale connections
        let stale_count = self
            .connections
            .iter()
            .filter(|entry| now.duration_since(entry.last_activity) > timeout)
            .map(|entry| *entry.key())
            .collect::<Vec<_>>();

        for id in &stale_count {
            self.connections.remove(id);
        }

        if !stale_count.is_empty() {
            info!(count = stale_count.len(), "Cleaned up stale connections");
        }

        // Remove empty job channels (no subscribers)
        let empty_channels = self
            .job_channels
            .iter()
            .filter(|entry| entry.receiver_count() == 0)
            .map(|entry| *entry.key())
            .collect::<Vec<_>>();

        for job_id in &empty_channels {
            self.job_channels.remove(job_id);
        }

        if !empty_channels.is_empty() {
            debug!(count = empty_channels.len(), "Cleaned up empty job channels");
        }
    }

    /// Get connection count
    pub fn connection_count(&self) -> usize {
        self.connections.len()
    }

    /// Get connection count for specific IP
    pub fn connections_for_ip(&self, ip: &IpAddr) -> usize {
        self.connections
            .iter()
            .filter(|entry| &entry.client_ip == ip)
            .count()
    }

    /// Check rate limit for IP
    fn check_rate_limit(&self, ip: &IpAddr) -> Result<(), ConnectionError> {
        if self.connections.len() >= self.config.max_total_connections {
            return Err(ConnectionError::MaxConnectionsReached);
        }

        if self.connections_for_ip(ip) >= self.config.max_connections_per_ip {
            return Err(ConnectionError::RateLimited(*ip));
        }

        Ok(())
    }

    /// Register a new connection
    fn register_connection(
        &self,
        connection: &SseConnection,
        client_ip: IpAddr,
        job_id: Option<Uuid>,
    ) {
        let now = Instant::now();
        self.connections.insert(
            connection.id,
            ConnectionInfo {
                id: connection.id,
                job_id,
                client_ip,
                created_at: now,
                last_activity: now,
            },
        );
    }
}

impl Default for ConnectionManager {
    fn default() -> Self {
        Self::with_defaults()
    }
}

/// Helper to create a shared connection manager
pub fn create_shared_manager(config: ConnectionManagerConfig) -> Arc<ConnectionManager> {
    Arc::new(ConnectionManager::new(config))
}

/// Spawns a background task that periodically cleans up stale SSE connections.
///
/// This should be called once at application startup with the shared connection manager.
/// The task runs indefinitely, cleaning up stale connections at the configured interval.
pub fn spawn_cleanup_task(manager: Arc<ConnectionManager>) {
    let interval = manager.config.cleanup_interval;

    tokio::spawn(async move {
        let mut interval_timer = tokio::time::interval(interval);
        interval_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        // Skip the immediate first tick
        interval_timer.tick().await;

        loop {
            interval_timer.tick().await;

            let connection_count = manager.connection_count();
            manager.cleanup();

            let new_count = manager.connection_count();
            if connection_count != new_count {
                info!(
                    "SSE cleanup: {} -> {} connections",
                    connection_count, new_count
                );
            }
        }
    });

    info!(
        "SSE cleanup task started (interval: {:?})",
        interval
    );
}