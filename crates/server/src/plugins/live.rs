//! Live job streams: a bounded in-memory chunk buffer per job, its run epoch and a
//! change signal. Nothing here is durable; see [`super::jobs`] for the records.
//!
//! Cost: every operation takes the stream's own lock for a buffer update or copy;
//! none performs I/O.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::Value;
use tokio::sync::watch;
use uuid::Uuid;

use super::jobs::{MAX_BUFFERED_BYTES, MAX_BUFFERED_CHUNKS, MAX_CHUNK_BYTES, bounded_json};
use super::worker::CancelFlag;

/// A chunk event as delivered to subscribers.
#[derive(Debug, Clone)]
pub struct Chunk {
    /// Sequence number within the job's stream.
    pub seq: u64,
    /// Serialized payload.
    pub data: Arc<str>,
}

pub(super) struct Buffer {
    pub(super) epoch: String,
    pub(super) run: u32,
    pub(super) next_seq: u64,
    pub(super) chunks: VecDeque<Chunk>,
    pub(super) bytes: usize,
    pub(super) subscribers: usize,
    pub(super) cancel: Option<Arc<CancelFlag>>,
    pub(super) running: bool,
    pub(super) retain_until: Option<std::time::Instant>,
}

/// In-memory stream of one job: bounded chunk buffer, run epoch and change signal.
pub struct Live {
    buffer: Mutex<Buffer>,
    changed: watch::Sender<u64>,
}

/// What a subscriber reads from a live stream in one delivery.
pub struct Batch {
    /// Current run epoch.
    pub epoch: String,
    /// First sequence still buffered (or the next one when empty).
    pub oldest: u64,
    /// Chunks at or after the requested sequence.
    pub chunks: Vec<Chunk>,
}

impl Live {
    pub(super) fn new() -> Self {
        Self {
            buffer: Mutex::new(Buffer {
                epoch: Uuid::now_v7().to_string(),
                run: 0,
                next_seq: 0,
                chunks: VecDeque::new(),
                bytes: 0,
                subscribers: 0,
                cancel: None,
                running: false,
                retain_until: None,
            }),
            changed: watch::channel(0).0,
        }
    }

    pub(super) fn lock(&self) -> std::sync::MutexGuard<'_, Buffer> {
        self.buffer.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub(super) fn bump(&self) {
        self.changed.send_modify(|v| *v = v.wrapping_add(1));
    }

    /// Keep a completed manual-turn stream for late subscribers for thirty seconds (the
    /// job service also caps completed retained streams at 64). No I/O.
    pub fn retain_for_subscriber(&self) {
        self.lock().retain_until =
            Some(std::time::Instant::now() + std::time::Duration::from_secs(630));
    }

    /// Whether run `run` is still the current, uncancelled run. Cost: one lock.
    pub fn is_current(&self, run: u32) -> bool {
        let b = self.lock();
        b.running && b.run == run && !b.cancel.as_ref().is_some_and(|c| c.is_cancelled())
    }

    /// Append one chunk from run `run`; late output from another run, a stopped run
    /// or an oversized payload is dropped. Cost: one lock, O(evicted chunks).
    pub fn publish(&self, run: u32, data: &Value) -> bool {
        let Ok(text) = bounded_json(data, MAX_CHUNK_BYTES) else {
            return false;
        };
        {
            let mut b = self.lock();
            if !b.running || b.run != run {
                return false;
            }
            let seq = b.next_seq;
            b.next_seq += 1;
            b.bytes += text.len();
            b.chunks.push_back(Chunk {
                seq,
                data: Arc::from(text),
            });
            while b.chunks.len() > MAX_BUFFERED_CHUNKS || b.bytes > MAX_BUFFERED_BYTES {
                if let Some(old) = b.chunks.pop_front() {
                    b.bytes -= old.data.len();
                }
            }
        }
        self.bump();
        true
    }

    /// The next sequence number, recorded with a checkpoint. Cost: one lock.
    pub fn next_seq(&self) -> u64 {
        self.lock().next_seq
    }

    /// Chunks with sequence `>= from`. Cost: one lock, clones at most the buffer.
    pub fn read(&self, from: u64) -> Batch {
        let b = self.lock();
        Batch {
            epoch: b.epoch.clone(),
            oldest: b.chunks.front().map_or(b.next_seq, |c| c.seq),
            chunks: b.chunks.iter().filter(|c| c.seq >= from).cloned().collect(),
        }
    }

    /// A receiver that wakes on every chunk and state change.
    pub fn watch(&self) -> watch::Receiver<u64> {
        self.changed.subscribe()
    }
}

pub(super) type LiveMap = Arc<Mutex<HashMap<String, Arc<Live>>>>;

/// Subscriber slot; when dropped it releases its counts and drops a stopped stream's
/// buffer once nobody is subscribed.
pub struct Subscription {
    /// The job's live stream.
    pub live: Arc<Live>,
    pub(super) id: String,
    pub(super) map: LiveMap,
    pub(super) total: Arc<AtomicUsize>,
}

impl Drop for Subscription {
    fn drop(&mut self) {
        let mut map = self.map.lock().unwrap_or_else(|p| p.into_inner());
        let idle = {
            let mut b = self.live.lock();
            b.subscribers -= 1;
            !b.running && b.subscribers == 0
        };
        if idle {
            map.remove(&self.id);
        }
        self.total.fetch_sub(1, Ordering::AcqRel);
    }
}
