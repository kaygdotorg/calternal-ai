//! Bounded and fenced exec output upload.
use super::*;
use crate::protocol::{Chunk, ChunkEvent, ExitInfo, OutputAck, OutputPost};
use crate::sandbox::{Exec, OutputEvent};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use std::collections::VecDeque;

/// Largest upload body, in raw output bytes.
const POST_BYTES: usize = 1024 * 1024;
/// Short wait to batch small writes into one upload.
const COALESCE: Duration = Duration::from_millis(20);

impl Runner {
    /// Wait for fenced or acknowledged output tasks before role shutdown.
    pub(crate) async fn drain_outputs(&self) {
        // After the session stops dispatching, the count can only fall. Each
        // exec holds its reference until upload is acknowledged or fenced.
        while Arc::strong_count(&self.uploads) > 1 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }

    /// Number the exec's events and upload them until the exit is acknowledged,
    /// the lease is fenced or the machine is revoked. Worker replies can
    /// unblock a broker round trip, so only ordinary exec output waits to batch.
    pub(super) async fn upload(
        &self,
        lease_id: Uuid,
        fence: u64,
        exec_id: Uuid,
        mut exec: Exec,
        batch: bool,
    ) {
        let mut pending: VecDeque<(Chunk, usize)> = VecDeque::new();
        let (mut seq, mut bytes, mut ended, mut backoff) =
            (0u64, 0usize, false, Duration::from_millis(200));
        let push = |ev: OutputEvent,
                    seq: &mut u64,
                    pending: &mut VecDeque<(Chunk, usize)>,
                    bytes: &mut usize| {
            *seq += 1;
            let (event, size, last) = match ev {
                OutputEvent::Data(stream, b) => (
                    ChunkEvent::Data {
                        stream,
                        data: B64.encode(&b),
                    },
                    b.len(),
                    false,
                ),
                OutputEvent::Exit(info) => (ChunkEvent::Exit(info), 0, true),
            };
            pending.push_back((Chunk { seq: *seq, event }, size));
            *bytes += size;
            last
        };
        loop {
            if !ended && pending.is_empty() {
                match exec.events.recv().await {
                    Some(ev) => ended |= push(ev, &mut seq, &mut pending, &mut bytes),
                    None => {
                        let lost = OutputEvent::Exit(ExitInfo {
                            error: Some("output ended without exit".into()),
                            ..ExitInfo::default()
                        });
                        ended |= push(lost, &mut seq, &mut pending, &mut bytes);
                    }
                }
                if !ended && batch {
                    tokio::time::sleep(COALESCE).await;
                }
            }
            while !ended && bytes < UNACKED_BYTES {
                match exec.events.try_recv() {
                    Ok(ev) => ended |= push(ev, &mut seq, &mut pending, &mut bytes),
                    Err(_) => break,
                }
            }
            let mut size = 0;
            let chunks: Vec<Chunk> = pending
                .iter()
                .take_while(|(_, n)| {
                    size += n;
                    size <= POST_BYTES || size == *n
                })
                .map(|(c, _)| c.clone())
                .collect();
            let post = OutputPost {
                lease_id,
                fence,
                exec_id,
                chunks,
            };
            match self
                .link
                .post::<_, OutputAck>(
                    "/api/v1/machine/output",
                    &post,
                    true,
                    Duration::from_secs(30),
                )
                .await
            {
                Ok(ack) => {
                    backoff = Duration::from_millis(200);
                    while pending.front().is_some_and(|(c, _)| c.seq <= ack.acked) {
                        bytes -= pending.pop_front().expect("front").1;
                    }
                    if ended && pending.is_empty() {
                        return;
                    }
                    if let Some(ms) = ack.retry_after_ms {
                        tokio::time::sleep(Duration::from_millis(ms.min(5_000))).await;
                    }
                }
                Err(Error::Revoked) => return,
                Err(e) if matches!(e.status(), Some(404 | 409)) => {
                    tracing::info!(lease = %lease_id, exec = %exec_id, "output refused: lease fenced");
                    return;
                }
                Err(e) => {
                    if e.status() == Some(401) {
                        self.link.wait_for_session().await;
                    }
                    tracing::debug!(error = %e, "output upload failed; retrying");
                    tokio::time::sleep(backoff).await;
                    backoff = (backoff * 2).min(Duration::from_secs(5));
                }
            }
        }
    }
}
