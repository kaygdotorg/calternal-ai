//! Offline input is retained as ordered immutable requests, never merged locally.
use crate::{CancellationToken, Client, ClientError};
use calternal_core::{OperationRequest, PendingRequest};
use serde_json::Value;
use std::collections::VecDeque;

/// Ephemeral offline input for one server/account. At most 256 envelopes of at
/// most 2 MiB each; caller selects a smaller bound. Platform code owns durable
/// offline retention. Refusals retain the head and stop draining; no reordering.
#[derive(Default)]
pub struct OfflineOutbox {
    queue: VecDeque<PendingRequest>,
}
impl OfflineOutbox {
    /// Queue a validated stable request. A duplicate ID with different input is
    /// refused; identical requests coalesce. O(queue length + input bytes), no I/O.
    pub fn enqueue(&mut self, request: PendingRequest) -> Result<(), ClientError> {
        if let Some(prior) = self
            .queue
            .iter()
            .find(|p| p.request().request_id == request.request().request_id)
        {
            if prior.request() == request.request() {
                return Ok(());
            }
            return Err(ClientError::Configuration(
                "outbox identity reused with different input",
            ));
        }
        if self.queue.len() >= 256 {
            return Err(ClientError::Configuration("outbox capacity reached"));
        }
        self.queue.push_back(request);
        Ok(())
    }
    /// Borrow pending envelopes for visible platform rendering. O(queue length)
    /// iteration, no copies or I/O. Never move these into a different account.
    pub fn pending(&self) -> impl Iterator<Item = &OperationRequest> {
        self.queue.iter().map(PendingRequest::request)
    }
    /// Cancel one unsent envelope. O(queue length), no server effect or I/O.
    /// Do not call during an uncertain in-flight send; retain that identity instead.
    pub fn cancel(&mut self, request_id: &str) {
        self.queue.retain(|p| p.request().request_id != request_id);
    }
    /// Drain in insertion order after reconnect. Every call uses the server's
    /// ordinary operation gate to recheck access and its existing receipt system.
    /// On any error the head and all later requests remain, including uncertain
    /// transport outcomes. Cost: sequential operation round trips, no local fsync.
    pub async fn drain(
        &mut self,
        client: &Client,
        cancellation: &CancellationToken,
    ) -> Result<Vec<Value>, ClientError> {
        let mut results = Vec::new();
        while let Some(request) = self.queue.front() {
            let result = client
                .execute_cancellable(request.request(), cancellation)
                .await?;
            self.queue.pop_front();
            results.push(result);
        }
        Ok(results)
    }
}
