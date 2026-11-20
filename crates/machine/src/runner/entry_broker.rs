//! Relay an entry's broker callbacks between its private socket and the
//! exact attempt's carrier. Each entry-chosen callback ID maps to a fresh
//! carrier UUID, so the host never sees or trusts entry identifiers. The carrier frame supplies ticket, package and
//! digest from the machine's own binding; the entry sends only the checked
//! call. Calls wait while the server is unreachable and are resent on exact
//! reattachment, because a callback reply is never inferred locally.
use calternal_plugin_host::{
    entry::{self, MAX_BROKER_FRAME_BYTES, MAX_OUTSTANDING_CALLBACKS},
    space_channel::{SpaceMessage, SpaceMethod},
};
use calternal_types::ErrorCode;
use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::UnixListener,
    sync::mpsc,
};
use uuid::Uuid;

type Reply = (Uuid, Result<Value, ErrorCode>);

/// A checked, declared call under its machine-assigned carrier ID.
pub(super) struct Relayed {
    id: Uuid,
    method: SpaceMethod,
    input: Value,
}

/// Machine-side state of one entry's callbacks. Holds at most
/// [`MAX_OUTSTANDING_CALLBACKS`] calls of at most one broker frame each.
pub(super) struct BrokerRelay {
    task: tokio::task::JoinHandle<()>,
    calls: mpsc::Receiver<Relayed>,
    replies: mpsc::Sender<Reply>,
    // Open calls in arrival order; those before `sent` went on this carrier.
    order: VecDeque<Uuid>,
    sent: usize,
    open: HashMap<Uuid, (SpaceMethod, Value)>,
}

impl Drop for BrokerRelay {
    fn drop(&mut self) {
        // Closing the stream is the fence: no later reply reaches the entry.
        self.task.abort();
    }
}

impl BrokerRelay {
    /// Serve the first connection on `listener` for the declared methods.
    /// Later connections are refused by dropping the listener after accept.
    pub(super) fn start(listener: UnixListener, methods: Vec<SpaceMethod>) -> Self {
        let (calls_tx, calls) = mpsc::channel(MAX_OUTSTANDING_CALLBACKS);
        let (replies, replies_rx) = mpsc::channel(MAX_OUTSTANDING_CALLBACKS);
        Self {
            task: tokio::spawn(serve(listener, methods, calls_tx, replies_rx)),
            calls,
            replies,
            order: VecDeque::new(),
            sent: 0,
            open: HashMap::new(),
        }
    }

    /// Wait for the next checked call from the entry. Pending forever once
    /// the stream closed, so it can sit in a select loop.
    pub(super) async fn next_call(&mut self) -> Relayed {
        match self.calls.recv().await {
            Some(call) => call,
            None => std::future::pending().await,
        }
    }

    /// Record a call for delivery on the current or next carrier.
    pub(super) fn queue(&mut self, call: Relayed) {
        self.order.push_back(call.id);
        self.open.insert(call.id, (call.method, call.input));
    }

    /// The next call frame to send, if any. Clones one bounded input.
    pub(super) fn next_unsent(&self) -> Option<(Uuid, SpaceMessage)> {
        let id = *self.order.get(self.sent)?;
        let (method, input) = &self.open[&id];
        Some((
            id,
            SpaceMessage::Call {
                callback_id: id,
                method: method.clone(),
                input: input.clone(),
            },
        ))
    }

    /// The carrier accepted this call frame.
    pub(super) fn sent(&mut self, id: Uuid) {
        if self.order.get(self.sent) == Some(&id) {
            self.sent += 1;
        }
    }

    /// A fresh carrier proved the same run: resend every unanswered call in
    /// its original order. The host's request receipts make a resend of an
    /// effect that already committed a replay, not a second effect.
    pub(super) fn reattached(&mut self) {
        self.sent = 0;
    }

    /// Deliver the host's reply for an open call. Unknown or repeated IDs
    /// are ignored. Returns whether the reply matched.
    pub(super) fn reply(&mut self, id: Uuid, result: Result<Value, ErrorCode>) -> bool {
        if self.open.remove(&id).is_none() {
            return false;
        }
        let position = self.order.iter().position(|u| *u == id).expect("open call");
        self.order.remove(position);
        if position < self.sent {
            self.sent -= 1;
        }
        let _ = self.replies.try_send((id, result));
        true
    }
}

async fn serve(
    listener: UnixListener,
    methods: Vec<SpaceMethod>,
    calls: mpsc::Sender<Relayed>,
    mut replies: mpsc::Receiver<Reply>,
) {
    let Ok((stream, _)) = listener.accept().await else {
        return;
    };
    drop(listener);
    let (read, mut write) = stream.into_split();
    let mut read = BufReader::with_capacity(64 * 1024, read);
    // Carrier ID to the entry's own callback ID, and the open entry IDs.
    let mut open: HashMap<Uuid, String> = HashMap::new();
    let mut refs: HashSet<String> = HashSet::new();
    let mut line = Vec::new();
    let limit = MAX_BROKER_FRAME_BYTES + 1;
    loop {
        tokio::select! {
            reply = replies.recv() => {
                let Some((id, result)) = reply else { return };
                if let Some(callback) = open.remove(&id) {
                    refs.remove(&callback);
                    if write.write_all(&entry::encode_broker_reply(&callback, &result)).await.is_err() {
                        return;
                    }
                }
            }
            read = async {
                let room = (limit - line.len()) as u64;
                (&mut read).take(room).read_until(b'\n', &mut line).await
            } => {
                if !matches!(read, Ok(n) if n > 0) || !line.ends_with(b"\n") && line.len() >= limit {
                    return;
                }
                if !line.ends_with(b"\n") {
                    continue;
                }
                // A malformed or forged call has no trustworthy ID to answer.
                let Ok(call) = entry::decode_broker_call(&line) else { return };
                line.clear();
                let refusal = if refs.contains(&call.callback_id) {
                    return;
                } else if !methods.contains(&call.method) {
                    Some(ErrorCode::Forbidden)
                } else if open.len() == MAX_OUTSTANDING_CALLBACKS {
                    Some(ErrorCode::RetryAfter)
                } else {
                    None
                };
                if let Some(code) = refusal {
                    let reply = entry::encode_broker_reply(&call.callback_id, &Err(code));
                    if write.write_all(&reply).await.is_err() { return; }
                    continue;
                }
                let id = Uuid::new_v4();
                refs.insert(call.callback_id.clone());
                open.insert(id, call.callback_id);
                if calls.send(Relayed { id, method: call.method, input: call.input }).await.is_err() {
                    return;
                }
            }
        }
    }
}
