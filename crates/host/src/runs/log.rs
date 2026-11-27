//! Bounded run output and exclusive offsets for replay.
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use base64::{Engine, engine::general_purpose::STANDARD};
use calternal_machine::run_service::{RunEvent, RunEventKind};
use calternal_types::{Error, Result};
use serde::Serialize;
use uuid::Uuid;

const MAX_EVENTS: usize = 256;
const MAX_BYTES: usize = 1024 * 1024;

#[derive(Default)]
pub(super) struct Log(Mutex<Retained>);

#[derive(Default)]
struct Retained {
    events: VecDeque<Arc<RunEvent>>,
    bytes: usize,
}

#[derive(Debug, Serialize)]
pub(crate) struct Chunk {
    offset: u64,
    #[serde(flatten)]
    output: Output,
}

#[derive(Debug, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
enum Output {
    Data {
        stream: calternal_machine::protocol::Stream,
        bytes: String,
    },
    Exit {
        code: Option<i32>,
        signal: Option<i32>,
    },
}

#[derive(Debug, Serialize)]
pub(crate) struct Batch {
    run: Uuid,
    offset: u64,
    stopped: bool,
    chunks: Vec<Chunk>,
}

impl Batch {
    pub(crate) fn offset(&self) -> u64 {
        self.offset
    }

    pub(crate) fn has_changes(&self) -> bool {
        self.stopped || !self.chunks.is_empty()
    }

    pub(crate) fn stopped(&self) -> bool {
        self.stopped
    }
}

impl Log {
    pub(super) fn publish(&self, event: RunEvent) {
        let mut retained = self.0.lock().expect("run log lock");
        let size = size(&event);
        while retained.events.len() >= MAX_EVENTS || retained.bytes + size > MAX_BYTES {
            let Some(old) = retained.events.pop_front() else {
                break;
            };
            retained.bytes -= self::size(&old);
        }
        retained.bytes += size;
        retained.events.push_back(Arc::new(event));
    }

    pub(super) fn after(&self, run: Uuid, offset: Option<u64>) -> Result<Batch> {
        let retained = self.0.lock().map_err(crate::internal)?;
        let first = retained.events.front().map_or(1, |event| event.sequence);
        let last = retained.events.back().map_or(0, |event| event.sequence);
        let offset = offset.unwrap_or(first - 1);
        if offset < first - 1 || offset > last {
            return Err(Error::conflict(
                "This run log offset is no longer available.",
            ));
        }
        let stopped = retained
            .events
            .back()
            .is_some_and(|event| matches!(event.event, RunEventKind::Exit(_)));
        let events = retained
            .events
            .iter()
            .filter(|event| event.sequence > offset)
            .cloned()
            .collect::<Vec<_>>();
        drop(retained);
        let chunks = events
            .iter()
            .map(|event| Chunk {
                offset: event.sequence,
                output: match &event.event {
                    RunEventKind::Data { stream, bytes } => Output::Data {
                        stream: *stream,
                        bytes: STANDARD.encode(bytes),
                    },
                    RunEventKind::Exit(exit) => Output::Exit {
                        code: exit.code,
                        signal: exit.signal,
                    },
                },
            })
            .collect();
        Ok(Batch {
            run,
            offset: last,
            stopped,
            chunks,
        })
    }
}

fn size(event: &RunEvent) -> usize {
    match &event.event {
        RunEventKind::Data { bytes, .. } => bytes.len(),
        RunEventKind::Exit(_) => 64,
    }
}
