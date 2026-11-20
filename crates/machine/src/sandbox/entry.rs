//! Ordered stdout from a single exec, with no authority carried by the entry.
use calternal_plugin_host::{Error, entry};
use serde_json::Value;

use super::{Exec, OutputEvent};
use crate::protocol::Stream;

/// Bounded, ordered stdout for one contribution process.
pub(crate) struct EntryOutput {
    exec: Exec,
    chunk: Vec<u8>,
    offset: usize,
    line: Vec<u8>,
    next_sequence: u64,
    terminal: Option<Value>,
    finished: bool,
}

impl EntryOutput {
    /// Start at producer sequence one. Retains at most one bounded entry line
    /// and the remainder of one existing exec chunk.
    pub(crate) fn new(exec: Exec) -> Self {
        Self {
            exec,
            chunk: Vec::new(),
            offset: 0,
            line: Vec::new(),
            next_sequence: 1,
            terminal: None,
            finished: false,
        }
    }

    /// Return the next checked frame, or None after the terminal. Waits for
    /// exit before releasing the terminal so trailing stdout cannot pass as
    /// success. Missing exits, malformed lines and gaps fail the stream.
    pub(crate) async fn next(&mut self) -> Result<Option<Value>, Error> {
        while !self.finished {
            let pending = &self.chunk[self.offset..];
            if !pending.is_empty() {
                if self.terminal.is_some() {
                    return Err(Error::Invalid);
                }
                let complete = pending.iter().position(|&byte| byte == b'\n');
                let count = complete.map_or(pending.len(), |end| end + 1);
                if count > entry::MAX_ENTRY_FRAME_BYTES - self.line.len() {
                    return Err(Error::TooLarge);
                }
                self.line.extend_from_slice(&pending[..count]);
                self.offset += count;
                if complete.is_none() {
                    continue;
                }
                let frame = entry::decode_frame(&self.line)?;
                self.line.clear();
                // The pinned schema limits seq to exact JS integers.
                if frame["seq"].as_f64() != Some(self.next_sequence as f64) {
                    return Err(Error::Invalid);
                }
                self.next_sequence += 1;
                if frame["kind"] == "output" {
                    return Ok(Some(frame));
                }
                self.terminal = Some(frame);
                continue;
            }
            match self.exec.events.recv().await {
                Some(OutputEvent::Data(Stream::Stdout, chunk)) => {
                    self.chunk = chunk;
                    self.offset = 0;
                }
                Some(OutputEvent::Data(Stream::Stderr, _)) => {}
                Some(OutputEvent::Exit(info)) => {
                    self.finished = true;
                    let terminal = self.terminal.take().ok_or(Error::Invocation)?;
                    if terminal["state"] == "ok" && info.code != Some(0) {
                        return Err(Error::Invocation);
                    }
                    return Ok(Some(terminal));
                }
                None => return Err(Error::Invocation),
            }
        }
        Ok(None)
    }
}
