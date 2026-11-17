//! Account feed transport bridge, with explicit acknowledgement and cancellation.
use crate::{NativeCancellation, NativeError};
use calternal_client::{AccountConnection, BookmarkStorage, MemoryBookmarks};
use calternal_core::AccountFrame;
use std::sync::Arc;
/// Single native account-feed consumer. The connection lock spans socket waiting;
/// cancellation has its own token and never needs that lock. One outstanding
/// frame bounds native buffering and prevents reading past unaccepted state.
#[cfg_attr(feature = "uniffi", derive(uniffi::Object))]
pub struct NativeAccountLive {
    connection: tokio::sync::Mutex<AccountConnection>,
    pending: tokio::sync::Mutex<Option<AccountFrame>>,
    delivery: tokio::sync::Semaphore,
    cancellation: calternal_client::CancellationToken,
}
#[cfg_attr(feature = "uniffi", uniffi::export(async_runtime = "tokio"))]
impl NativeAccountLive {
    /// Wait for one account invalidation as JSON. The platform reloads authorized
    /// drafts/pending input before accepting it. Cancellation interrupts socket,
    /// acknowledgement and reconnect waits; at most 64 KiB buffered, no disk I/O.
    pub async fn next(&self) -> Result<String, NativeError> {
        let permit = tokio::select! {
        biased;_=self.cancellation.cancelled()=>return Err(NativeError::Cancelled),permit=self.delivery.acquire()=>permit.expect("delivery")};
        let frame = self.connection.lock().await.next().await?;
        let json = serde_json::to_string(&frame).expect("frame");
        if frame != AccountFrame::Reset {
            *self.pending.lock().await = Some(frame);
            permit.forget();
        }
        Ok(json)
    }
    /// Acknowledge exactly the delivered frame after applying the reload and
    /// persisting its bookmark. Constant-sized JSON parse and memory write;
    /// invalid, duplicate and reset acknowledgements fail without advancing.
    pub async fn accept(&self, frame_json: String) -> Result<(), NativeError> {
        let frame: AccountFrame =
            serde_json::from_str(&frame_json).map_err(|_| NativeError::Failure {
                details: "invalid account frame".into(),
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
impl NativeAccountLive {
    pub(crate) async fn open(
        client: &calternal_client::Client,
        bookmark: Option<String>,
        cancellation: Arc<NativeCancellation>,
    ) -> Result<Arc<Self>, NativeError> {
        let storage = Arc::new(MemoryBookmarks::default());
        if let Some(bookmark) = bookmark {
            if bookmark.len() > 128 {
                return Err(NativeError::Failure {
                    details: "account bookmark exceeds limit".into(),
                });
            }
            storage.save("account", &bookmark).await?;
        }
        let connection = client
            .subscribe_account(storage, cancellation.token.clone())
            .await?;
        Ok(Arc::new(Self {
            connection: tokio::sync::Mutex::new(connection),
            pending: tokio::sync::Mutex::new(None),
            delivery: tokio::sync::Semaphore::new(1),
            cancellation: cancellation.token.clone(),
        }))
    }
}
