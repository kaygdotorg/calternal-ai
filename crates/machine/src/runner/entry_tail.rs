//! Commit cursor and bounded relay metadata for one machine-local entry log.
use calternal_plugin_host::Error;
use std::collections::VecDeque;

const MAX_IN_FLIGHT: usize = 8;

/// Only submitted offsets stay in memory. The file retains the full backlog.
#[derive(Default)]
pub(super) struct EntryTail {
    acked: u64,
    offset: u64,
    submitted: VecDeque<u64>,
}

impl EntryTail {
    /// Advance to a committed prefix of already-submitted output.
    pub(super) fn acknowledge(&mut self, through: u64) -> Result<u64, Error> {
        let count = through
            .checked_sub(self.acked)
            .and_then(|count| usize::try_from(count).ok())
            .filter(|&count| count <= self.submitted.len())
            .ok_or(Error::Refused)?;
        for end in self.submitted.drain(..count) {
            self.offset = end;
        }
        self.acked = through;
        Ok(self.offset)
    }

    /// Whether the relay can send another frame. Retransmitting an outstanding
    /// frame takes no additional window slot.
    pub(super) fn can_send(&self, cursor: u64) -> bool {
        let sent = self.acked + self.submitted.len() as u64;
        cursor < sent || self.submitted.len() < MAX_IN_FLIGHT
    }

    /// Retain the byte end of a submitted frame until the host commits it.
    pub(super) fn sent(&mut self, sequence: u64, end: u64) {
        if sequence > self.acked + self.submitted.len() as u64 {
            self.submitted.push_back(end);
        }
    }

    /// Highest committed producer sequence.
    pub(super) fn acked(&self) -> u64 {
        self.acked
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lost_ack_recovers_exact_byte_offset_and_rejects_inconsistent_cursor() {
        let mut tail = EntryTail::default();
        assert!(tail.acknowledge(1).is_err());
        tail.sent(1, 42);
        tail.sent(2, 101);
        assert_eq!(tail.acknowledge(1), Ok(42));
        assert!(tail.acknowledge(0).is_err());
        assert_eq!(tail.acknowledge(2), Ok(101));
        assert!(tail.acknowledge(3).is_err());
    }

    #[test]
    fn relay_window_limits_new_submissions_and_allows_replay() {
        let mut tail = EntryTail::default();
        for seq in 1..=MAX_IN_FLIGHT as u64 {
            assert!(tail.can_send(seq - 1));
            tail.sent(seq, seq * 100);
        }
        assert!(!tail.can_send(MAX_IN_FLIGHT as u64));
        assert!(tail.can_send(0));
        tail.sent(1, 100);
        assert_eq!(tail.acknowledge(1), Ok(100));
        assert!(tail.can_send(MAX_IN_FLIGHT as u64));
    }
}
