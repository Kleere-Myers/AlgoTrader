use std::convert::Infallible;

use axum::response::sse::{Event, KeepAlive, Sse};
use futures::stream::Stream;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

use crate::models::SseEvent;

/// Broadcaster backed by a tokio broadcast channel.
#[derive(Clone)]
pub struct SseBroadcaster {
    tx: broadcast::Sender<SseEvent>,
}

impl SseBroadcaster {
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx }
    }

    /// Send an event to all connected SSE clients.
    pub fn send(&self, event: SseEvent) {
        // Ignore error (no receivers connected)
        let _ = self.tx.send(event);
    }

    /// Create an SSE stream for an Axum handler.
    pub fn subscribe(
        &self,
    ) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
        let rx = self.tx.subscribe();
        let stream = BroadcastStream::new(rx).filter_map(|result| match result {
            Ok(event) => {
                let json = serde_json::to_string(&event).unwrap_or_default();
                Some(Ok(Event::default().data(json)))
            }
            Err(_) => None, // lagged — skip
        });
        Sse::new(stream).keep_alive(KeepAlive::default())
    }
}
