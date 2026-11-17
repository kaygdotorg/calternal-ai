//! Pull-based native async stream. Swift wraps `next` in AsyncThrowingStream and
//! cancels the shared NativeCancellation from its onTermination handler.
use crate::{NativeCancellation, NativeError};
use calternal_client::{BookmarkStorage, LiveConnection, MemoryBookmarks};
use calternal_core::LiveFrame;
use std::sync::Arc;

/// Single-consumer live feed. The async mutex serializes calls across native
/// tasks and intentionally spans a socket wait; cancellation has its own token
/// and never needs this lock. At most one bounded SSE buffer is retained.
#[cfg_attr(feature = "uniffi", derive(uniffi::Object))]
pub struct NativeLive {
    connection: tokio::sync::Mutex<LiveConnection>,
    outbox: Arc<tokio::sync::Mutex<calternal_client::OfflineOutbox>>,
    delivery: tokio::sync::Semaphore,
    pending: tokio::sync::Mutex<Option<LiveFrame>>,
    cancellation: calternal_client::CancellationToken,
}
#[cfg_attr(feature = "uniffi", uniffi::export(async_runtime = "tokio"))]
impl NativeLive {
    /// Retain offline input on this connection with the caller's stable request ID.
    /// Validates the core envelope and queues at most 256 requests. O(input bytes),
    /// no network or persistence; Apple persists its offline input separately.
    pub async fn queue_input(&self, envelope_json: String) -> Result<(), NativeError> {
        if envelope_json.len() > calternal_core::MAX_BODY_BYTES {
            return Err(NativeError::Failure {
                details: "offline input exceeds limit".into(),
            });
        }
        let envelope: calternal_core::OperationRequest = serde_json::from_str(&envelope_json)
            .map_err(|_| NativeError::Failure {
                details: "invalid offline envelope".into(),
            })?;
        let pending = calternal_client::Client::pending(
            &envelope.operation,
            &envelope.input,
            Some(envelope.request_id),
        )?;
        self.outbox.lock().await.enqueue(pending)?;
        Ok(())
    }
    /// Restore one platform-persisted PendingQueueInput, including its original
    /// offline creation time. At most 2 MiB; wrong schema, changed identity and
    /// full queues are refused. O(input bytes), no clock read or network effects.
    pub async fn restore_input(&self, pending_json: String) -> Result<(), NativeError> {
        if pending_json.len() > calternal_core::MAX_BODY_BYTES {
            return Err(NativeError::Failure {
                details: "offline input exceeds limit".into(),
            });
        }
        let input: calternal_core::PendingQueueInput = serde_json::from_str(&pending_json)
            .map_err(|_| NativeError::Failure {
                details: "invalid retained pending input".into(),
            })?;
        self.outbox.lock().await.restore(input)?;
        Ok(())
    }
    /// Return complete visible pending input as an ordered JSON array. O(queued bytes)
    /// serialization, no I/O; no credentials are included. Never restore into a
    /// different account. Unknown outcomes retain the same request envelope.
    pub async fn pending_input(&self) -> String {
        let outbox = self.outbox.lock().await;
        serde_json::to_string(&outbox.submissions().collect::<Vec<_>>()).expect("pending envelopes")
    }
    /// Read one server event as JSON. Cancellation interrupts network and backoff;
    /// O(event bytes) parsing/serialization with no platform writes. Swift persists
    /// its applied cache and bookmark before calling accept with the returned JSON.
    pub async fn next(&self) -> Result<String, NativeError> {
        let permit = tokio::select! {
            biased;
            _ = self.cancellation.cancelled() => return Err(NativeError::Cancelled),
            permit = self.delivery.acquire() => permit.expect("live delivery semaphore"),
        };
        let frame = self.connection.lock().await.next().await?;
        let json = serde_json::to_string(&frame).expect("frame");
        if !matches!(frame, LiveFrame::Reset { .. }) {
            *self.pending.lock().await = Some(frame);
            permit.forget();
        }
        Ok(json)
    }
    /// Acknowledge one applied event. Platform code must persist cache and bookmark
    /// before this call; memory then retains the bookmark for reconnect. No I/O.
    pub async fn accept(&self, frame_json: String) -> Result<(), NativeError> {
        let frame: LiveFrame =
            serde_json::from_str(&frame_json).map_err(|_| NativeError::Failure {
                details: "invalid live frame".into(),
            })?;
        let mut pending = self.pending.lock().await;
        if pending.as_ref() != Some(&frame) {
            return Err(NativeError::Failure {
                details: "accept the delivered frame exactly once".into(),
            });
        }
        self.connection.lock().await.accept(&frame).await?;
        pending.take();
        self.delivery.add_permits(1);
        Ok(())
    }
}
impl NativeLive {
    pub(crate) async fn open(
        client: &calternal_client::Client,
        spaces: Vec<String>,
        bookmarks_json: String,
        cancellation: Arc<NativeCancellation>,
    ) -> Result<Arc<Self>, NativeError> {
        if bookmarks_json.len() > 64 * 1024 {
            return Err(NativeError::Failure {
                details: "bookmark map exceeds limit".into(),
            });
        }
        let storage = Arc::new(MemoryBookmarks::default());
        let bookmarks: std::collections::HashMap<String, String> =
            serde_json::from_str(&bookmarks_json).map_err(|_| NativeError::Failure {
                details: "invalid bookmarks".into(),
            })?;
        for (space, bookmark) in bookmarks {
            storage.save(&space, &bookmark).await?;
        }
        let connection = client
            .subscribe(spaces, storage, cancellation.token.clone())
            .await?;
        let outbox = connection.outbox();
        Ok(Arc::new(Self {
            connection: tokio::sync::Mutex::new(connection),
            outbox,
            delivery: tokio::sync::Semaphore::new(1),
            pending: tokio::sync::Mutex::new(None),
            cancellation: cancellation.token.clone(),
        }))
    }
}
