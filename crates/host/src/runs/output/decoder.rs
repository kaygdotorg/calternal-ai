//! Bounded decoding of the adapter's newline-delimited stdout.

use calternal_types::MAX_FRAME_BYTES;
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
struct Frame {
    host_call: Option<Callback>,
    event: Option<AdapterEvent>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AdapterEvent {
    TextDelta {
        text: String,
    },
    Done {
        outcome: String,
    },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
pub(super) struct Callback {
    pub(super) id: u64,
    pub(super) call: String,
    pub(super) input: Value,
}

pub(super) enum Action {
    Callback(Callback),
    Reply { turn: u64, text: String },
}

#[derive(Default)]
pub(super) struct Decoder {
    pending: Vec<u8>,
    discarding: bool,
    reply: String,
    overflow: bool,
    turn: u64,
    callback: u64,
    collect_replies: bool,
}

impl Decoder {
    pub(super) fn new(collect_replies: bool) -> Self {
        Self {
            collect_replies,
            ..Self::default()
        }
    }

    pub(super) fn feed(&mut self, bytes: &[u8]) -> Vec<Action> {
        let mut actions = Vec::new();
        for segment in bytes.split_inclusive(|byte| *byte == b'\n') {
            let complete = segment.ends_with(b"\n");
            if self.discarding {
                self.discarding = !complete;
                continue;
            }
            if self.pending.len().saturating_add(segment.len()) > MAX_FRAME_BYTES + 1 {
                self.pending.clear();
                self.overflow = true;
                self.discarding = !complete;
                continue;
            }
            if complete && self.pending.is_empty() {
                self.parse(segment, &mut actions);
            } else {
                self.pending.extend_from_slice(segment);
                if complete {
                    let mut pending = std::mem::take(&mut self.pending);
                    self.parse(&pending, &mut actions);
                    pending.clear();
                    self.pending = pending;
                }
            }
        }
        actions
    }

    fn parse(&mut self, line: &[u8], actions: &mut Vec<Action>) {
        let Ok(frame) = serde_json::from_slice::<Frame>(line) else {
            return;
        };
        if let Some(callback) = frame.host_call {
            if callback.id > self.callback {
                self.callback = callback.id;
                actions.push(Action::Callback(callback));
            }
            return;
        }
        if !self.collect_replies {
            return;
        }
        match frame.event {
            Some(AdapterEvent::TextDelta { text }) => {
                if self.reply.len().saturating_add(text.len()) <= MAX_FRAME_BYTES {
                    self.reply.push_str(&text);
                } else {
                    self.overflow = true;
                }
            }
            Some(AdapterEvent::Done { outcome }) => {
                self.turn += 1;
                if outcome == "done" && !self.overflow && !self.reply.trim().is_empty() {
                    actions.push(Action::Reply {
                        turn: self.turn,
                        text: std::mem::take(&mut self.reply),
                    });
                }
                self.reply.clear();
                self.overflow = false;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oversized_adapter_line_is_discarded_until_its_delimiter() {
        let mut decoder = Decoder::new(true);
        assert!(decoder.feed(&vec![b'x'; MAX_FRAME_BYTES + 2]).is_empty());
        let actions = decoder.feed(
            b"{\"host_call\":{\"id\":1,\"call\":\"s.me\",\"input\":{}}}\n\
              {\"host_call\":{\"id\":2,\"call\":\"s.me\",\"input\":{}}}\n",
        );
        assert!(matches!(
            actions.as_slice(),
            [Action::Callback(Callback { id: 2, .. })]
        ));
    }
}
