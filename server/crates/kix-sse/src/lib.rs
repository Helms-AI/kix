//! Server-Sent Events infrastructure for real-time updates

pub mod event;
pub mod stream;
pub mod manager;

pub use event::{Event, EventType};
pub use stream::{SseConnection, SseStream};
pub use manager::{ConnectionManager, ConnectionError};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum SseError {
    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Broadcast error: {0}")]
    Broadcast(String),

    #[error("Rate limit exceeded")]
    RateLimited,
}