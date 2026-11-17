//! Offline input is retained as ordered pending submissions, never merged locally.
use crate::{CancellationToken, Client, ClientError};
use calternal_core::{OperationRequest, PendingQueueInput, PendingRequest};
use serde_json::Value;
use std::collections::VecDeque;

/// Ephemeral offline input for one server/account. At most 256 envelopes of at
/// most 2 MiB each; caller selects a smaller bound. Platform code owns durable
/// offline retention. Refusals retain the head and stop draining; no reordering.
#[derive(Default)]
pub struct OfflineOutbox {
    queue: VecDeque<PendingQueueInput>,
}
impl OfflineOutbox {
    /// Queue a validated stable request. A duplicate ID with different input is
    /// refused; identical requests coalesce. O(queue length + input bytes), no I/O.
    pub fn enqueue(&mut self, request: PendingRequest) -> Result<(), ClientError> {
        if let Some(prior) = self
            .queue
            .iter()
            .find(|p| p.request.request_id == request.request().request_id)
        {
            if &prior.request == request.request() {
                return Ok(());
            }
            return Err(ClientError::Configuration(
                "outbox identity reused with different input",
            ));
        }
        if self.queue.len() >= 256 {
            return Err(ClientError::Configuration("outbox capacity reached"));
        }
        let envelope = request.request().clone();
        let space_id = envelope
            .input
            .get("space_id")
            .and_then(Value::as_str)
            .ok_or(ClientError::Configuration(
                "offline writes require their space UUID",
            ))?
            .to_owned();
        calternal_core::validate_resource_id(&space_id)?;
        let item_id = envelope
            .input
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let created_at_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| ClientError::Configuration("clock before Unix epoch"))?
            .as_millis() as i64;
        self.restore(PendingQueueInput {
            space_id,
            item_id,
            created_at_ms,
            request: envelope,
        })
    }
    /// Restore one account-scoped offline record without replacing its creation
    /// time or identity. Validates the complete queue contract, enforces the same
    /// count bound and coalesces exact duplicates. No I/O or clock reads; O(bytes).
    pub fn restore(&mut self, input: PendingQueueInput) -> Result<(), ClientError> {
        let value =
            serde_json::to_value(&input).map_err(|_| ClientError::Protocol("pending input"))?;
        let object =
            serde_json::from_value(value).map_err(|_| ClientError::Protocol("pending input"))?;
        calternal_core::validate_account_input("pending.queue", &object)?;
        if let Some(prior) = self
            .queue
            .iter()
            .find(|p| p.request.request_id == input.request.request_id)
        {
            return if prior == &input {
                Ok(())
            } else {
                Err(ClientError::Configuration(
                    "outbox identity reused with different input or creation time",
                ))
            };
        }
        if self.queue.len() >= 256 {
            return Err(ClientError::Configuration("outbox capacity reached"));
        }
        self.queue.push_back(input);
        Ok(())
    }
    /// Borrow complete records for durable platform retention. Preserve the
    /// creation time, scope and identity together in account-private storage.
    /// O(queue length) iteration, no copies, clock reads or I/O.
    pub fn submissions(&self) -> impl Iterator<Item = &PendingQueueInput> {
        self.queue.iter()
    }
    /// Borrow pending envelopes for visible platform rendering. O(queue length)
    /// iteration, no copies or I/O. Never move these into a different account.
    pub fn pending(&self) -> impl Iterator<Item = &OperationRequest> {
        self.queue.iter().map(|p| &p.request)
    }
    /// Cancel one unsent envelope. O(queue length), no server effect or I/O.
    /// Do not call during an uncertain in-flight send; retain that identity instead.
    pub fn cancel(&mut self, request_id: &str) {
        self.queue.retain(|p| p.request.request_id != request_id);
    }
    /// Drain in insertion order after reconnect. Every call uses the server's
    /// pending-submission operations to recheck access and reuse write receipts.
    /// On any error the head and all later requests remain, including uncertain
    /// transport outcomes. Cost: sequential operation round trips, no local fsync.
    pub async fn drain(
        &mut self,
        client: &Client,
        cancellation: &CancellationToken,
    ) -> Result<Vec<Value>, ClientError> {
        let mut results = Vec::new();
        while let Some(request) = self.queue.front() {
            let result = client.queue_submission(request, cancellation).await?;
            self.queue.pop_front();
            results.push(
                serde_json::to_value(result)
                    .map_err(|_| ClientError::Protocol("pending result"))?,
            );
        }
        Ok(results)
    }
}
