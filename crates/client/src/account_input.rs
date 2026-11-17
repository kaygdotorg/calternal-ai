//! Account-private input operations and an explicitly accepted SSE subscription.
use crate::{BookmarkStorage, CancellationToken, Client, ClientError};
use calternal_core::{AccountFrame, AccountInputResult, PendingQueueInput};
use std::{sync::Arc, time::Duration};
/// One account feed consumer with at most 64 KiB of transport buffering. State is
/// reloaded through operations; accept a bookmark only after applying that reload.
/// Storage belongs exclusively to this server/account. Reset requires a new snapshot.
pub struct AccountConnection {
    client: Client,
    storage: Arc<dyn BookmarkStorage>,
    bookmark: Option<String>,
    response: Option<reqwest::Response>,
    buffer: Vec<u8>,
    cancellation: CancellationToken,
    stopped: bool,
}
impl Client {
    /// Submit this account's server-held queue on reconnect, skipping requests
    /// that need the person's confirmation. Sequential bounded round trips; any
    /// refusal retains the current and remaining entries on the server.
    pub async fn drain_pending(&self, cancel: &CancellationToken) -> Result<(), ClientError> {
        let list = Client::pending("pending.list", &serde_json::json!({}), None)?;
        let rows: calternal_core::AccountInputListResult =
            self.execute_cancellable(list.request(), cancel).await?;
        for row in rows.records {
            if row.state != "confirmation_required" {
                let submit = Client::pending(
                    "pending.submit",
                    &calternal_core::PendingInput {
                        space_id: row.space_id,
                        id: row.id,
                    },
                    None,
                )?;
                let _: AccountInputResult =
                    self.execute_cancellable(submit.request(), cancel).await?;
            }
        }
        Ok(())
    }
    /// Prepare an account subscription without network I/O. One bookmark read;
    /// persistent storage implementations must move blocking work off the executor.
    pub async fn subscribe_account(
        &self,
        storage: Arc<dyn BookmarkStorage>,
        cancellation: CancellationToken,
    ) -> Result<AccountConnection, ClientError> {
        let bookmark = storage.load("account").await?;
        Ok(AccountConnection {
            client: self.clone(),
            storage,
            bookmark,
            response: None,
            buffer: Vec::new(),
            cancellation,
            stopped: false,
        })
    }
    /// Persist one offline envelope as visible account input, then attempt it through
    /// ordinary receipt dispatch. Two round trips, retaining its request ID. Old
    /// input returns confirmation_required and remains visible until confirmed/cancelled.
    pub async fn queue_submission(
        &self,
        input: &PendingQueueInput,
        cancel: &CancellationToken,
    ) -> Result<AccountInputResult, ClientError> {
        let queue = Client::pending("pending.queue", input, None)?;
        let result: AccountInputResult = self.execute_cancellable(queue.request(), cancel).await?;
        if result
            .record
            .as_ref()
            .is_some_and(|r| r.state == "confirmation_required")
        {
            return Ok(result);
        }
        let submit = Client::pending(
            "pending.submit",
            &calternal_core::PendingInput {
                space_id: input.space_id.clone(),
                id: input.request.request_id.clone(),
            },
            None,
        )?;
        self.execute_cancellable(submit.request(), cancel).await
    }
}
impl AccountConnection {
    /// Await one account frame, reconnecting from the last explicitly accepted
    /// bookmark with bounded backoff. Cancellation stops socket and reconnect waits.
    /// Each frame is bounded to 64 KiB; authorization errors stop the stream.
    pub async fn next(&mut self) -> Result<AccountFrame, ClientError> {
        let cancel = self.cancellation.clone();
        tokio::select! {
        biased;_=cancel.cancelled()=>Err(ClientError::Cancelled),r=self.receive()=>r}
    }
    async fn receive(&mut self) -> Result<AccountFrame, ClientError> {
        let mut delay = Duration::from_millis(250);
        loop {
            if self.stopped {
                return Err(ClientError::Protocol(
                    "account subscription requires reload",
                ));
            }
            if self.response.is_none() {
                let mut req = self
                    .client
                    .http
                    .get(
                        self.client
                            .base
                            .join("api/v1/account/live?readiness=true")
                            .expect("route"),
                    )
                    .timeout(Duration::from_secs(86400));
                if let Some(b) = &self.bookmark {
                    req = req.query(&[("bookmark", b)]);
                }
                match req.send().await {
                    Ok(r) if r.status().is_success() => {
                        if !r
                            .headers()
                            .get("content-type")
                            .and_then(|h| h.to_str().ok())
                            .is_some_and(|h| h.starts_with("text/event-stream"))
                        {
                            return Err(ClientError::Protocol("expected account event stream"));
                        }
                        self.response = Some(r);
                        self.client.drain_pending(&self.cancellation).await?;
                    }
                    Ok(r) => {
                        self.stopped = true;
                        let (status, body) = Client::body(r).await?;
                        return Err(Client::failure(status, &body, None)?);
                    }
                    Err(_) => {
                        tokio::time::sleep(delay).await;
                        delay = (delay * 2).min(Duration::from_secs(30));
                        continue;
                    }
                }
            }
            if let Some(end) = self.buffer.windows(2).position(|w| w == b"\n\n") {
                let bytes: Vec<_> = self.buffer.drain(..end + 2).collect();
                let text = std::str::from_utf8(&bytes)
                    .map_err(|_| ClientError::Protocol("invalid account frame"))?;
                let Some(data) = text.lines().find_map(|l| l.strip_prefix("data: ")) else {
                    continue;
                };
                let frame: AccountFrame = serde_json::from_str(data)
                    .map_err(|_| ClientError::Protocol("invalid account frame"))?;
                if frame == AccountFrame::Reset {
                    self.stopped = true;
                }
                return Ok(frame);
            }
            match self.response.as_mut().expect("response").chunk().await {
                Ok(Some(bytes)) => {
                    if self.buffer.len() + bytes.len() > 65536 {
                        return Err(ClientError::Protocol("account frame exceeds limit"));
                    }
                    self.buffer.extend_from_slice(&bytes);
                }
                _ => {
                    self.response = None;
                    self.buffer.clear();
                    tokio::time::sleep(delay).await;
                    delay = (delay * 2).min(Duration::from_secs(30));
                }
            }
        }
    }
    /// Resume after the caller reloads authorized account state. The next ready
    /// frame supplies a replacement bookmark; no network or durable I/O here.
    pub fn resume_after_reload(&mut self) {
        self.bookmark = None;
        self.response = None;
        self.buffer.clear();
        self.stopped = false;
    }
    /// Persist only after the consumer applies an authorized snapshot. One storage
    /// write; accepting reset is refused. Failure leaves the previous bookmark.
    pub async fn accept(&mut self, frame: &AccountFrame) -> Result<(), ClientError> {
        let bookmark = match frame {
            AccountFrame::Changed { bookmark } | AccountFrame::Ready { bookmark } => bookmark,
            AccountFrame::ReadinessChanged => return Ok(()),
            AccountFrame::Reset => {
                return Err(ClientError::Protocol("reload account before resubscribing"));
            }
        };
        self.storage.save("account", bookmark).await?;
        self.bookmark = Some(bookmark.clone());
        Ok(())
    }
}
