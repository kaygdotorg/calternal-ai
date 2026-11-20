//! Machine-owned ordered output retained until the host commits its prefix.
use calternal_plugin_host::{Error, entry::MAX_ENTRY_FRAME_BYTES, space_channel::SpaceMessage};
use calternal_types::ErrorCode;
use serde_json::Value;
use std::collections::VecDeque;

use super::UNACKED_BYTES;

const MAX_PENDING_FRAMES: usize = 256;
const FAILURE_RESERVE: usize = 64;

#[derive(Default)]
pub(super) struct EntryTail {
    frames: VecDeque<(Result<Value, ErrorCode>, usize)>,
    bytes: usize,
    acked: u64,
    sent: u64,
}

impl EntryTail {
    /// Leave room for a whole frame before polling stdout. The underlying
    /// bounded pipe applies backpressure once this queue cannot reserve it.
    pub(super) fn can_read(&self) -> bool {
        self.frames.len() < MAX_PENDING_FRAMES - 1
            && self.bytes <= UNACKED_BYTES - MAX_ENTRY_FRAME_BYTES - FAILURE_RESERVE
    }

    /// Append the next checked producer frame.
    /// Costs one bounded byte count and retains one owned JSON value.
    pub(super) fn push(&mut self, value: Value) -> Result<(), Error> {
        debug_assert!(self.can_read());
        let size = serde_json::to_vec(&value)
            .map_err(|_| Error::Invalid)?
            .len()
            + 1;
        if size > MAX_ENTRY_FRAME_BYTES {
            return Err(Error::TooLarge);
        }
        self.frames.push_back((Ok(value), size));
        self.bytes += size;
        Ok(())
    }

    /// Append a local failure terminal using the reserved frame and bytes.
    pub(super) fn failed(&mut self) {
        self.frames
            .push_back((Err(ErrorCode::Unavailable), FAILURE_RESERVE));
        self.bytes += FAILURE_RESERVE;
    }

    /// Reclaim only a prefix the host durably committed. A recovery cursor
    /// may exceed the last ACK received, but cannot skip machine-unsent data.
    pub(super) fn acknowledge(&mut self, through: u64) -> Result<(), Error> {
        if !(self.acked..=self.sent).contains(&through) {
            return Err(Error::Refused);
        }
        for _ in self.acked..through {
            let (_, size) = self.frames.pop_front().expect("sent frame is retained");
            self.bytes -= size;
        }
        self.acked = through;
        Ok(())
    }

    /// Build one pending message without moving the retained frame. A
    /// replacement connection starts its send cursor at the checked ACK.
    pub(super) fn after(&self, cursor: u64) -> Option<(u64, SpaceMessage)> {
        let offset = cursor
            .checked_sub(self.acked)
            .and_then(|n| usize::try_from(n).ok())?;
        let (frame, _) = self.frames.get(offset)?;
        let message = match frame {
            Ok(data) if data["kind"] == "output" => SpaceMessage::Chunk { data: data.clone() },
            _ => SpaceMessage::Complete {
                result: frame.clone(),
            },
        };
        Some((cursor + 1, message))
    }

    /// Record the highest sequence queued for delivery on any carrier.
    pub(super) fn sent(&mut self, sequence: u64) {
        self.sent = self.sent.max(sequence);
    }

    /// Highest producer sequence as of this snapshot, including retained output.
    pub(super) fn produced(&self) -> u64 {
        self.acked + self.frames.len() as u64
    }

    /// All produced output has been acknowledged.
    pub(super) fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reconnect_replays_only_uncommitted_suffix_and_rejects_inconsistent_cursor() {
        let mut tail = EntryTail::default();
        for seq in 1..=3 {
            tail.push(json!({"seq":seq,"kind":"output","data":seq}))
                .unwrap();
        }
        assert!(tail.acknowledge(1).is_err());
        tail.sent(2);
        tail.acknowledge(1).unwrap();
        assert!(tail.acknowledge(0).is_err());
        assert_eq!(tail.after(1).unwrap().0, 2);
        // The server committed frame two but its ACK was lost on the old link.
        tail.acknowledge(2).unwrap();
        assert_eq!(tail.after(2).unwrap().0, 3);
        assert!(tail.acknowledge(3).is_err());
        tail.sent(3);
        tail.acknowledge(3).unwrap();
        assert!(tail.is_empty());
    }

    #[test]
    fn bounded_tail_resumes_reads_only_after_commit_ack() {
        let mut tail = EntryTail::default();
        let mut seq = 0;
        while tail.can_read() {
            seq += 1;
            tail.push(json!({"seq":seq,"kind":"output","data":"x".repeat(15_000)}))
                .unwrap();
        }
        assert!(seq > 1 && seq < MAX_PENDING_FRAMES);
        assert!(tail.bytes <= UNACKED_BYTES);
        tail.sent(1);
        tail.acknowledge(1).unwrap();
        assert!(tail.can_read());
        assert_eq!(tail.after(1).unwrap().0, 2);
    }
}
