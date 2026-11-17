//! Cancellable one-connection journal following. Explicit acceptance persists a
//! bookmark only after the caller has applied the event; there is no merge engine.
use super::state::BookmarkStorage;
use crate::{CancellationToken, Client, ClientError};
use crate::{ItemCache, OfflineOutbox};
use calternal_core::{LiveFrame, LiveSubscription};
use std::{sync::Arc, time::Duration};

/// One ordered live consumer, scoped to the server/account that owns its storage.
/// Buffers at most 2 MiB of SSE bytes. No background task survives dropping it.
/// Reconnects from accepted bookmarks with backoff capped at 30 seconds. Reset,
/// authorization and protocol failures stop; callers explicitly reload/sign in.
pub struct LiveConnection {
    client: Client,
    subscriptions: Vec<LiveSubscription>,
    storage: Arc<dyn BookmarkStorage>,
    response: Option<reqwest::Response>,
    buffer: Vec<u8>,
    cancellation: CancellationToken,
    delay: Duration,
    stopped: bool,
    cache: ItemCache,
    outbox: Arc<tokio::sync::Mutex<OfflineOutbox>>,
}
impl Client {
    /// Prepare one connection after loading bookmarks, without opening the socket.
    /// Storage must be exclusive to this server/account; implementations perform
    /// blocking I/O off executor workers. Cost: one storage read per space, bounded
    /// at 16 spaces. Invalid UUIDs or duplicate subscriptions fail before network I/O.
    pub async fn subscribe(
        &self,
        spaces: Vec<String>,
        storage: Arc<dyn BookmarkStorage>,
        cancellation: CancellationToken,
    ) -> Result<LiveConnection, ClientError> {
        if spaces.is_empty() || spaces.len() > 16 {
            return Err(ClientError::Configuration("provide 1 to 16 spaces"));
        }
        let mut subscriptions: Vec<LiveSubscription> = Vec::new();
        for space in spaces {
            let space = uuid::Uuid::parse_str(&space)
                .map_err(|_| ClientError::Configuration("space UUID required"))?
                .to_string();
            if subscriptions.iter().any(|s| s.space_id == space) {
                return Err(ClientError::Configuration("distinct space UUIDs required"));
            }
            let bookmark = storage.load(&space).await?;
            subscriptions.push(LiveSubscription {
                space_id: space,
                bookmark,
            });
        }
        Ok(LiveConnection {
            client: self.clone(),
            subscriptions,
            storage,
            response: None,
            buffer: Vec::new(),
            cancellation,
            delay: Duration::from_millis(250),
            stopped: false,
            cache: ItemCache::new(256, None)?,
            outbox: Arc::new(tokio::sync::Mutex::new(OfflineOutbox::default())),
        })
    }
}
impl LiveConnection {
    /// Borrow this account's offline input queue, for visible pending input,
    /// enqueue and cancellation. Constant cost; no I/O. It drains after every
    /// successful connect, before returning any new live frame. Drain holds this
    /// per-account async mutex across requests so input cannot change in flight.
    /// Queueing while the socket waits needs no connection lock. Never share it
    /// across accounts. Refusal retains the head and ends the current next call.
    pub fn outbox(&self) -> Arc<tokio::sync::Mutex<OfflineOutbox>> {
        self.outbox.clone()
    }

    /// Install the platform's cache before consuming events. Constant ownership
    /// transfer; no I/O. Acceptance invalidates changed items before bookmarks.
    pub fn set_cache(&mut self, cache: ItemCache) {
        self.cache = cache;
    }

    /// Wait for the next event; reconnect on network loss with the accepted cursor.
    /// Cancellation interrupts socket reads and backoff. EOF never advances a cursor.
    /// Cost: bounded byte parsing and at most one in-flight HTTP request. Callers
    /// must accept each delivered entry before requesting another to avoid replay.
    pub async fn next(&mut self) -> Result<LiveFrame, ClientError> {
        if self.stopped {
            return Err(ClientError::Protocol("subscription requires reload"));
        }
        let cancellation = self.cancellation.clone();
        tokio::select! {
            _ = cancellation.cancelled() => Err(ClientError::Cancelled),
            result = self.receive() => result,
        }
    }
    async fn receive(&mut self) -> Result<LiveFrame, ClientError> {
        loop {
            if self.response.is_none() {
                let response = self
                    .client
                    .http
                    .get(
                        self.client
                            .base
                            .join("api/v1/live")
                            .expect("constant route"),
                    )
                    .query(&[(
                        "subscriptions",
                        serde_json::to_string(&self.subscriptions).expect("subscriptions"),
                    )])
                    .timeout(Duration::from_secs(24 * 60 * 60))
                    .send()
                    .await;
                let response = match response {
                    Ok(r) => r,
                    Err(_) => {
                        self.backoff().await;
                        continue;
                    }
                };
                if response.status().as_u16() == 401 {
                    return Err(ClientError::ReauthenticationRequired);
                }
                if !response.status().is_success() {
                    self.stopped = true;
                    let (status, bytes) = Client::body(response).await?;
                    return Err(Client::failure(status, &bytes, None)?);
                }
                if response
                    .headers()
                    .get("content-type")
                    .and_then(|h| h.to_str().ok())
                    .is_none_or(|s| !s.starts_with("text/event-stream"))
                {
                    self.stopped = true;
                    return Err(ClientError::Protocol("expected event stream"));
                }
                self.response = Some(response);
                if let Err(error) = self
                    .outbox
                    .lock()
                    .await
                    .drain(&self.client, &self.cancellation)
                    .await
                {
                    self.response = None;
                    return Err(error);
                }
            }
            if let Some(end) = self.buffer.windows(2).position(|w| w == b"\n\n") {
                let bytes: Vec<_> = self.buffer.drain(..end + 2).collect();
                let text = std::str::from_utf8(&bytes)
                    .map_err(|_| ClientError::Protocol("invalid live UTF-8"))?;
                let data = text
                    .lines()
                    .filter_map(|line| line.strip_prefix("data: "))
                    .collect::<Vec<_>>()
                    .join("\n");
                if data.is_empty() {
                    continue;
                }
                let frame: LiveFrame = serde_json::from_str(&data)
                    .map_err(|_| ClientError::Protocol("invalid live frame"))?;
                let space = match &frame {
                    LiveFrame::Entry { space_id, .. }
                    | LiveFrame::Ready { space_id, .. }
                    | LiveFrame::Reset { space_id } => space_id,
                };
                if !self.subscriptions.iter().any(|s| s.space_id == *space) {
                    return Err(ClientError::Protocol("foreign live space"));
                }
                self.delay = Duration::from_millis(250);
                if matches!(frame, LiveFrame::Reset { .. }) {
                    self.stopped = true;
                    self.response = None;
                    self.cache.clear();
                }
                return Ok(frame);
            }
            match self.response.as_mut().expect("open response").chunk().await {
                Ok(Some(bytes)) => {
                    if self.buffer.len() + bytes.len() > 2 * 1024 * 1024 {
                        self.stopped = true;
                        return Err(ClientError::Protocol("live frame exceeds limit"));
                    }
                    self.buffer.extend_from_slice(&bytes);
                }
                _ => {
                    self.response = None;
                    self.buffer.clear();
                    self.backoff().await;
                }
            }
        }
    }
    async fn backoff(&mut self) {
        tokio::time::sleep(self.delay).await;
        self.delay = (self.delay * 2).min(Duration::from_secs(30));
    }
    /// Accept an applied event, persisting its opaque bookmark before updating the
    /// reconnect cursor. Cost: one platform storage write (file implementation uses
    /// atomic replacement and two fsyncs). Reset is never accepted automatically.
    pub async fn accept(&mut self, frame: &LiveFrame) -> Result<(), ClientError> {
        let (space, bookmark) = match frame {
            LiveFrame::Entry { space_id, entry } => (
                space_id,
                entry
                    .get("token")
                    .and_then(|t| t.as_str())
                    .ok_or(ClientError::Protocol("entry lacks bookmark"))?,
            ),
            LiveFrame::Ready { space_id, bookmark } => (space_id, bookmark.as_str()),
            LiveFrame::Reset { .. } => {
                return Err(ClientError::Protocol("reload before accepting reset"));
            }
        };
        if let LiveFrame::Entry { entry, .. } = frame {
            if let Some(item) = entry
                .get("item")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
            {
                self.cache.put(item, None).await?;
            }
        }
        let sub = self
            .subscriptions
            .iter_mut()
            .find(|s| s.space_id == *space)
            .ok_or(ClientError::Protocol("foreign live space"))?;
        self.storage.save(space, bookmark).await?;
        sub.bookmark = Some(bookmark.to_owned());
        Ok(())
    }
}
