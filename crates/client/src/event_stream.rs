//! Shared bounded SSE decoding for journal and job subscriptions.
use crate::{CancellationToken, Client, ClientError};
use serde_json::Value;
use std::time::Duration;

pub(crate) struct Frame {
    pub name: String,
    pub id: Option<String>,
    pub data: String,
}
pub(crate) fn take(buffer: &mut Vec<u8>) -> Result<Option<Frame>, ClientError> {
    while let Some(end) = buffer.windows(2).position(|w| w == b"\n\n") {
        let bytes: Vec<_> = buffer.drain(..end + 2).collect();
        let text = std::str::from_utf8(&bytes)
            .map_err(|_| ClientError::Protocol("invalid event UTF-8"))?;
        let data = text
            .lines()
            .filter_map(|l| l.strip_prefix("data: "))
            .collect::<Vec<_>>()
            .join("\n");
        if data.is_empty() {
            continue;
        }
        return Ok(Some(Frame {
            name: text
                .lines()
                .find_map(|l| l.strip_prefix("event: "))
                .unwrap_or("message")
                .into(),
            id: text
                .lines()
                .find_map(|l| l.strip_prefix("id: "))
                .map(str::to_owned),
            data,
        }));
    }
    Ok(None)
}

/// One event in an authorized job stream, retaining its resume identity.
#[derive(Debug, Clone)]
pub struct JobEvent {
    /// Snapshot, chunk, reset, state or end.
    pub kind: String,
    /// Stream epoch and sequence when the event advances the chunk cursor.
    pub id: Option<String>,
    /// Bounded structured payload.
    pub data: Value,
}
/// Cancellable job subscription using the same decoder as space live feeds.
/// Buffers at most 2 MiB, reconnects from the last delivered chunk after loss,
/// and creates no background task. A reset explicitly reports lost live history.
pub struct JobSubscription {
    client: Client,
    job: String,
    response: Option<reqwest::Response>,
    buffer: Vec<u8>,
    after: Option<String>,
    cancellation: CancellationToken,
    ended: bool,
}
impl Client {
    /// Prepare a UUID-selected job stream without network I/O. Authority is checked
    /// by the server at opening and every delivery; no bearer is exposed.
    pub fn subscribe_job(
        &self,
        job: &str,
        cancellation: CancellationToken,
    ) -> Result<JobSubscription, ClientError> {
        let job = uuid::Uuid::parse_str(job)
            .map_err(|_| ClientError::Configuration("job UUID required"))?
            .to_string();
        Ok(JobSubscription {
            client: self.clone(),
            job,
            response: None,
            buffer: Vec::new(),
            after: None,
            cancellation,
            ended: false,
        })
    }
}
impl JobSubscription {
    /// Await one ordered event. Cancellation interrupts network and backoff waits.
    /// Transport loss reconnects without rerunning the turn; auth/protocol errors stop.
    pub async fn next(&mut self) -> Result<Option<JobEvent>, ClientError> {
        if self.ended {
            return Ok(None);
        }
        let cancellation = self.cancellation.clone();
        tokio::select! {biased; _=cancellation.cancelled()=>Err(ClientError::Cancelled),result=self.receive()=>result}
    }
    async fn receive(&mut self) -> Result<Option<JobEvent>, ClientError> {
        let mut delay = Duration::from_millis(250);
        loop {
            if self.response.is_none() {
                let mut request = self
                    .client
                    .http
                    .get(
                        self.client
                            .base
                            .join(&format!("api/v1/jobs/{}/events", self.job))
                            .expect("UUID route"),
                    )
                    .timeout(Duration::from_secs(86400));
                if let Some(after) = &self.after {
                    request = request.header("last-event-id", after)
                }
                match request.send().await {
                    Ok(r) if r.status().is_success() => {
                        if !r
                            .headers()
                            .get("content-type")
                            .and_then(|h| h.to_str().ok())
                            .is_some_and(|h| h.starts_with("text/event-stream"))
                        {
                            return Err(ClientError::Protocol("expected job event stream"));
                        }
                        self.response = Some(r);
                    }
                    Ok(r) => {
                        let (status, bytes) = Client::body(r).await?;
                        return Err(Client::failure(status, &bytes, None)?);
                    }
                    Err(_) => {
                        tokio::time::sleep(delay).await;
                        delay = (delay * 2).min(Duration::from_secs(30));
                        continue;
                    }
                }
            }
            if let Some(frame) = take(&mut self.buffer)? {
                let data = serde_json::from_str(&frame.data)
                    .map_err(|_| ClientError::Protocol("invalid job event"))?;
                if frame.id.is_some() {
                    self.after = frame.id.clone()
                }
                self.ended = frame.name == "end";
                return Ok(Some(JobEvent {
                    kind: frame.name,
                    id: frame.id,
                    data,
                }));
            }
            match self.response.as_mut().expect("connected").chunk().await {
                Ok(Some(bytes)) => {
                    if self.buffer.len() + bytes.len() > 2 * 1024 * 1024 {
                        return Err(ClientError::Protocol("job event exceeds limit"));
                    }
                    self.buffer.extend_from_slice(&bytes)
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
}
