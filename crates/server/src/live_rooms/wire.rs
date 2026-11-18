//! The reference lib0 room envelope and numbered fragmentation.
use std::{sync::Arc, time::Duration};
use tokio::{
    sync::{OwnedSemaphorePermit, Semaphore},
    time::Instant,
};

pub(super) const FRAME_BYTES: usize = 1024 * 1024;
const PIECE_BYTES: usize = FRAME_BYTES / 2;
const MAX_MESSAGE: usize = 64 * 1024 * 1024;
const PIECE_IDLE: Duration = Duration::from_secs(15);
const MAX_HEADER: usize = 256;

/// A message after reassembly keeps its byte reservation until consumed.
pub(super) struct Packet {
    pub(super) room: String,
    pub(super) bytes: Vec<u8>,
    _budget: Option<OwnedSemaphorePermit>,
}

struct Partial {
    packet: Packet,
    total: usize,
    next: usize,
    deadline: Instant,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Refusal {
    Malformed,
    TooLarge(String),
    Busy,
}

#[derive(Default)]
pub(super) struct Reassembly(Option<Partial>);

impl Reassembly {
    pub(super) fn deadline(&self) -> Option<Instant> {
        self.0.as_ref().map(|p| p.deadline)
    }

    pub(super) fn discard(&mut self, room: &str) {
        if self.0.as_ref().is_some_and(|p| p.packet.room == room) {
            self.0 = None;
        }
    }

    pub(super) fn incoming(
        &mut self,
        frame: &[u8],
        budget: &Arc<Semaphore>,
        joined: impl FnOnce(&str) -> bool,
    ) -> Result<Option<Packet>, Refusal> {
        let (header, payload) = split(frame).ok_or(Refusal::Malformed)?;
        let Some(chunk) = header.strip_prefix('+') else {
            if !joined(header) {
                return Ok(None);
            }
            if self.0.as_ref().is_some_and(|p| p.packet.room == header) {
                return Err(Refusal::Malformed);
            }
            let permit = budget
                .clone()
                .try_acquire_many_owned(payload.len() as u32)
                .map_err(|_| Refusal::Busy)?;
            return Ok(Some(Packet {
                room: header.to_owned(),
                bytes: payload.to_vec(),
                _budget: Some(permit),
            }));
        };
        let (seq, rest) = chunk.split_once('/').ok_or(Refusal::Malformed)?;
        let (total, room) = rest.split_once('/').ok_or(Refusal::Malformed)?;
        if !joined(room) {
            return Ok(None);
        }
        let seq = seq.parse::<usize>().map_err(|_| Refusal::Malformed)?;
        let total = total.parse::<usize>().map_err(|_| Refusal::Malformed)?;
        if total > MAX_MESSAGE {
            return Err(Refusal::TooLarge(room.to_owned()));
        }
        if payload.is_empty() || payload.len() > total {
            return Err(Refusal::Malformed);
        }
        if self.0.is_none() {
            if seq != 0 {
                return Err(Refusal::Malformed);
            }
            let permit = budget
                .clone()
                .try_acquire_many_owned(total as u32)
                .map_err(|_| Refusal::Busy)?;
            self.0 = Some(Partial {
                packet: Packet {
                    room: room.to_owned(),
                    bytes: Vec::with_capacity(total),
                    _budget: Some(permit),
                },
                total,
                next: 0,
                deadline: Instant::now() + PIECE_IDLE,
            });
        }
        let partial = self.0.as_mut().expect("initialized above");
        if partial.packet.room != room
            || partial.total != total
            || partial.next != seq
            || partial.deadline <= Instant::now()
            || payload.len() > total - partial.packet.bytes.len()
        {
            return Err(Refusal::Malformed);
        }
        partial.packet.bytes.extend_from_slice(payload);
        partial.next += 1;
        partial.deadline = Instant::now() + PIECE_IDLE;
        if partial.packet.bytes.len() == total {
            return Ok(self.0.take().map(|p| p.packet));
        }
        Ok(None)
    }
}

pub(super) fn frames<'a>(room: &'a str, bytes: &'a [u8]) -> impl Iterator<Item = Vec<u8>> + 'a {
    bytes
        .chunks(PIECE_BYTES)
        .enumerate()
        .map(move |(seq, part)| {
            if bytes.len() <= PIECE_BYTES {
                frame(room, part)
            } else {
                frame(&format!("+{seq}/{}/{room}", bytes.len()), part)
            }
        })
}

fn frame(header: &str, msg: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(header.len() + msg.len() + 2);
    let mut len = header.len();
    while len >= 128 {
        out.push((len as u8) | 0x80);
        len >>= 7;
    }
    out.push(len as u8);
    out.extend_from_slice(header.as_bytes());
    out.extend_from_slice(msg);
    out
}

fn split(frame: &[u8]) -> Option<(&str, &[u8])> {
    let mut len = 0;
    for (i, byte) in frame.iter().copied().take(2).enumerate() {
        len |= usize::from(byte & 127) << (7 * i);
        if byte & 128 == 0 {
            if len == 0 || len > MAX_HEADER {
                return None;
            }
            let body = frame.get(i + 1..)?;
            let name = std::str::from_utf8(body.get(..len)?).ok()?;
            return Some((name, body.get(len..)?));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packets_reserve_bytes_until_consumed_and_reject_stale_memberships() {
        let input = vec![7; PIECE_BYTES + 1];
        let budget = Arc::new(Semaphore::new(input.len()));
        let mut receiver = Reassembly::default();
        let mut frames = frames("@7/space/kind/item", &input);
        let first = frames.next().unwrap();
        let second = frames.next().unwrap();
        assert!(
            receiver
                .incoming(&first, &budget, |_| false)
                .unwrap()
                .is_none()
        );
        assert_eq!(budget.available_permits(), input.len());
        assert!(
            receiver
                .incoming(&first, &budget, |_| true)
                .unwrap()
                .is_none()
        );
        assert_eq!(budget.available_permits(), 0);
        let packet = receiver
            .incoming(&second, &budget, |_| true)
            .unwrap()
            .unwrap();
        assert_eq!(packet.bytes, input);
        assert_eq!(budget.available_permits(), 0);
        drop(packet);
        assert_eq!(budget.available_permits(), input.len());
        assert!(matches!(
            receiver.incoming(&second, &budget, |_| true),
            Err(Refusal::Malformed)
        ));
    }
}
