//! Drain queued updates in order into one content transaction, as the
//! reference room drains queued history writes into one commit. No timer is
//! added: a room with one pending edit can commit immediately.

use super::*;
use calternal_content::{Request, RoomUpdate};
use std::collections::VecDeque;
use tokio::sync::oneshot;

const MAX_UPDATES: usize = PARTICIPANTS;
const MAX_BYTES: usize = 8 * 1024 * 1024;

struct Pending {
    member: Uuid,
    account: AccountId,
    lineage: String,
    bytes: Vec<u8>,
    reply: oneshot::Sender<Result<(), Error>>,
    _permit: OwnedSemaphorePermit,
}

#[derive(Default)]
pub(super) struct Queue {
    pending: VecDeque<Pending>,
    running: bool,
}

impl Queue {
    fn take(&mut self) -> Vec<Pending> {
        let Some(first) = self.pending.pop_front() else {
            self.running = false;
            return Vec::new();
        };
        let mut bytes = first.bytes.len();
        let mut batch = vec![first];
        while batch.len() < MAX_UPDATES
            && self.pending.front().is_some_and(|next| {
                next.lineage == batch[0].lineage && bytes + next.bytes.len() <= MAX_BYTES
            })
        {
            let next = self.pending.pop_front().expect("next pending update");
            bytes += next.bytes.len();
            batch.push(next);
        }
        batch
    }
}

impl Lease {
    pub(in crate::live_rooms) async fn write(
        &self,
        content: Arc<Content>,
        account: AccountId,
        lineage: String,
        bytes: Vec<u8>,
    ) -> Result<(), Error> {
        if bytes.len() > MAX_BYTES {
            return Err(Error::invalid_input("live update exceeds its size limit"));
        }
        let permit = self
            .room
            .memory
            .clone()
            .try_acquire_many_owned(bytes.len() as u32)
            .map_err(|_| Error::unavailable("live room update capacity is busy"))?;
        let (reply, result) = oneshot::channel();
        {
            let mut queue = self.room.writes.lock().unwrap();
            if queue.pending.len() == MAX_UPDATES {
                return Err(Error::unavailable("live room update queue is full"));
            }
            queue.pending.push_back(Pending {
                member: self.id,
                account,
                lineage,
                bytes,
                reply,
                _permit: permit,
            });
            if !queue.running {
                queue.running = true;
                tokio::spawn(self.room.clone().drain(content));
            }
        }
        result.await.map_err(|_| closed())?
    }
}

impl Room {
    pub(super) fn preview(self: &Arc<Self>, member: Uuid) -> Preview {
        Preview {
            room: self.clone(),
            member,
            generation: Cell::new(None),
        }
    }

    async fn drain(self: Arc<Self>, content: Arc<Content>) {
        loop {
            let batch = self.writes.lock().unwrap().take();
            if batch.is_empty() {
                return;
            }
            let (room, content) = (self.clone(), content.clone());
            let _ = disk(move || {
                room.commit(&content, batch);
                Ok(())
            })
            .await;
        }
    }

    fn commit(self: &Arc<Self>, content: &Content, batch: Vec<Pending>) {
        let preview = self.preview(batch[0].member);
        let result = self.apply(content, &batch, &preview);
        if batch.len() > 1 && result.is_err() && preview.generation.get().is_none() {
            // A malformed or revoked edit must not make another writer's
            // update fail. No tentative data left the failed transaction.
            for pending in batch {
                let preview = self.preview(pending.member);
                let result = self.apply(content, std::slice::from_ref(&pending), &preview);
                preview.finish(result.is_ok());
                let _ = pending.reply.send(result);
            }
        } else {
            preview.finish(result.is_ok());
            for pending in batch {
                let _ = pending.reply.send(result.clone());
            }
        }
    }

    fn apply(
        self: &Arc<Self>,
        content: &Content,
        batch: &[Pending],
        preview: &Preview,
    ) -> Result<(), Error> {
        // A failed preview invalidates this room. Updates queued behind it
        // must wait for reconnect, even before the follower sees that failure.
        if self.durability.borrow().failed {
            return Err(closed());
        }
        let key = &self.key;
        let first = &batch[0];
        let mut data = Vec::with_capacity(
            batch
                .iter()
                .map(|pending| pending.bytes.len())
                .sum::<usize>()
                + 128,
        );
        data.extend_from_slice(key.item.as_bytes());
        data.extend_from_slice(first.lineage.as_bytes());
        if let [only] = batch {
            data.extend_from_slice(&only.bytes);
        } else {
            data.extend_from_slice(b"calternal-room-group-v1");
            for pending in batch {
                data.extend_from_slice(pending.account.as_uuid().as_bytes());
                data.extend_from_slice(&(pending.bytes.len() as u64).to_le_bytes());
                data.extend_from_slice(&pending.bytes);
            }
        }
        let fingerprint = crate::store::sha256_hex(&data);
        let request_id = format!("room/{fingerprint:0>128}");
        let operation = format!("{}.room-update", key.kind);
        let request = Request {
            account: first.account,
            via: None,
            id: &request_id,
            operation: &operation,
            fingerprint: &fingerprint,
        };
        let updates: Vec<_> = batch
            .iter()
            .map(|pending| RoomUpdate {
                author: pending.account,
                bytes: &pending.bytes,
            })
            .collect();
        let apply = || {
            content.apply_room_updates(
                key.space,
                &key.kind,
                key.item,
                &first.lineage,
                &updates,
                request,
                |update| preview.publish(update, batch[1..].iter().map(|pending| pending.member)),
            )
        };
        let mut result = apply();
        if result.as_ref().err().is_some_and(|error| {
            error
                .details
                .as_ref()
                .and_then(|details| details["reason"].as_str())
                == Some("external_change")
        }) && content.reconcile_room_file(key.space, &key.kind, key.item, first.account)?
        {
            result = apply();
        }
        match result {
            Ok(_) => Ok(()),
            Err(error) => {
                if content
                    .receipt(key.space, first.account, request.id)?
                    .is_some_and(|receipt| {
                        receipt.operation == operation && receipt.fingerprint == fingerprint
                    })
                {
                    Ok(())
                } else {
                    Err(error)
                }
            }
        }
    }
}
