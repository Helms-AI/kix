//! SSE stream handling with Axum

use std::convert::Infallible;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use futures_util::stream::Stream;
use tokio::sync::broadcast;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tokio_stream::wrappers::BroadcastStream;
use uuid::Uuid;

use crate::event::{Event, EventType};

/// SSE connection stream
pub struct SseConnection {
    pub id: Uuid,
    pub job_id: Option<Uuid>,
    receiver: BroadcastStream<Event>,
    created_at: Instant,
}

impl SseConnection {
    /// Create a new SSE connection
    pub fn new(receiver: broadcast::Receiver<Event>, job_id: Option<Uuid>) -> Self {
        Self {
            id: Uuid::new_v4(),
            job_id,
            receiver: BroadcastStream::new(receiver),
            created_at: Instant::now(),
        }
    }

    /// Convert to Axum SSE response
    pub fn into_sse(self) -> Sse<SseStream> {
        let stream = SseStream {
            inner: Box::pin(self.receiver),
            _connection_id: self.id,
        };

        Sse::new(stream)
            .keep_alive(
                KeepAlive::new()
                    .interval(Duration::from_secs(30))
                    .text("heartbeat"),
            )
    }

    /// Get connection age
    pub fn age(&self) -> Duration {
        self.created_at.elapsed()
    }
}

/// Custom stream wrapper for SSE
pub struct SseStream {
    inner: Pin<Box<BroadcastStream<Event>>>,
    _connection_id: Uuid,
}

impl Stream for SseStream {
    type Item = Result<SseEvent, Infallible>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.inner.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(event))) => {
                let event_data: Result<String, serde_json::Error> = event.to_sse_data();
                let sse_event = match event_data {
                    Ok(data) => SseEvent::default()
                        .id(event.id.to_string())
                        .event(event_type_name(&event.data))
                        .data(data),
                    Err(e) => SseEvent::default()
                        .event("error")
                        .data(format!("Serialization error: {}", e)),
                };
                Poll::Ready(Some(Ok(sse_event)))
            }
            Poll::Ready(Some(Err(BroadcastStreamRecvError::Lagged(n)))) => {
                let warning = SseEvent::default()
                    .event("warning")
                    .data(format!(r#"{{"message":"Missed {} events"}}"#, n));
                Poll::Ready(Some(Ok(warning)))
            }
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// Get event type name for SSE event field
fn event_type_name(event: &EventType) -> &'static str {
    match event {
        EventType::JobStarted { .. } => "job_started",
        EventType::Progress { .. } => "progress",
        EventType::ItemProcessed { .. } => "item_processed",
        EventType::Error { .. } => "error",
        EventType::JobCompleted { .. } => "job_completed",
        EventType::JobCancelled { .. } => "job_cancelled",
        EventType::JobHistoryUpdated { .. } => "job_history_updated",
        EventType::ItemDiscovered { .. } => "item_discovered",
        EventType::ItemStarted { .. } => "item_started",
        EventType::Heartbeat { .. } => "heartbeat",
    }
}