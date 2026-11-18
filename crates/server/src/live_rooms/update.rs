//! Binary sync admission, durable check ordering, and protocol refusals.

use super::*;
use calternal_content::Request as WriteRequest;
use calternal_types::ErrorCode;
use yrs::{
    encoding::read::{Cursor, Read},
    sync::{Message, MessageReader, SyncMessage},
    updates::{decoder::DecoderV1, encoder::Encode},
};

impl<D: Directory> Connection<D> {
    pub(super) async fn binary(&mut self, raw: &[u8]) -> Result<(), Error> {
        let incoming =
            self.reassembly
                .incoming(raw, self.host.store.live_rooms().memory(), |room| {
                    self.members.contains_key(room)
                });
        let packet = match incoming {
            Ok(Some(packet)) => packet,
            Ok(None) => return Ok(()),
            Err(wire::Refusal::TooLarge(address)) => {
                let (room, membership) = unaddress(&address).ok_or_else(closed)?;
                self.json(json!({"type":"too_large","room":room,"membership":membership}))
                    .await?;
                return self.left(&address).await;
            }
            Err(_) => return Err(Error::invalid_input("invalid room frame")),
        };
        let mut decoder = DecoderV1::new(Cursor::new(&packet.bytes));
        for message in MessageReader::new(&mut decoder) {
            let message = message.map_err(|_| Error::invalid_input("invalid room message"))?;
            if let Err(error) = self.apply_message(&packet.room, message).await {
                let retry = matches!(error.code, ErrorCode::Unavailable | ErrorCode::Internal);
                if let Some(member) = self.members.get_mut(&packet.room) {
                    member.resync = true;
                    member.held = None;
                }
                let refusal = serde_json::to_vec(&error.code).map_err(|_| closed())?;
                self.send(
                    &packet.room,
                    &Message::Custom(if retry { 103 } else { 102 }, refusal).encode_v1(),
                )
                .await?;
                if retry
                    || error
                        .details
                        .as_ref()
                        .and_then(|detail| detail["reason"].as_str())
                        == Some("lineage_changed")
                    || matches!(
                        error.code,
                        ErrorCode::Forbidden | ErrorCode::Unauthorized | ErrorCode::NotFound
                    )
                {
                    self.left(&packet.room).await?;
                }
                break;
            }
        }
        Ok(())
    }

    async fn apply_message(&mut self, address: &str, message: Message) -> Result<(), Error> {
        let member = self.members.get(address).ok_or_else(closed)?;
        self.authorize(&member.key, &member.plugin, member.generation, false)
            .await?;
        if *member.kicked.borrow() {
            return Err(hidden());
        }
        match message {
            Message::Sync(SyncMessage::SyncStep1(vector)) => {
                self.send(address, &self.members[address].lease.state(Some(&vector))?)
                    .await
            }
            Message::Sync(SyncMessage::SyncStep2(bytes)) => self.update(address, bytes, true).await,
            Message::Sync(SyncMessage::Update(bytes)) => self.update(address, bytes, false).await,
            Message::Awareness(update) => member.lease.awareness(update),
            Message::AwarenessQuery => {
                self.send(address, &self.members[address].lease.presence_snapshot())
                    .await
            }
            Message::Custom(100, data) => self.check(address, &data).await,
            _ => Ok(()),
        }
    }

    async fn update(&mut self, address: &str, bytes: Vec<u8>, step2: bool) -> Result<(), Error> {
        let member = self.members.get(address).ok_or_else(closed)?;
        if member.resync && !step2 {
            return Ok(());
        }
        if bytes != [0, 0] {
            self.authorize(&member.key, &member.plugin, member.generation, true)
                .await?;
            if *member.kicked.borrow() {
                return Err(hidden());
            }
            let (key, lineage, content, account) = (
                member.key.clone(),
                member.lineage.clone(),
                self.content.clone(),
                self.account,
            );
            let preview = member.lease.preview();
            let result = disk(move || {
                let mut data = Vec::with_capacity(bytes.len() + 128);
                data.extend_from_slice(key.item.as_bytes());
                data.extend_from_slice(lineage.as_bytes());
                data.extend_from_slice(&bytes);
                let fingerprint = crate::store::sha256_hex(&data);
                let request_id = format!("room/{fingerprint:0>128}");
                let operation = format!("{}.room-update", key.kind);
                let request = WriteRequest {
                    account,
                    via: None,
                    id: &request_id,
                    operation: &operation,
                    fingerprint: &fingerprint,
                };
                let result = content.apply_room_update(
                    key.space,
                    &key.kind,
                    key.item,
                    &lineage,
                    &bytes,
                    request,
                    |update| preview.publish(update),
                );
                preview.finish(result.is_ok());
                match result {
                    Ok(_) => Ok(()),
                    Err(error) => {
                        if content
                            .receipt(key.space, account, &request_id)?
                            .is_some_and(|r| {
                                r.operation == operation && r.fingerprint == fingerprint
                            })
                        {
                            Ok(())
                        } else {
                            Err(error)
                        }
                    }
                }
            })
            .await;
            if let Err(error) = result {
                if error
                    .details
                    .as_ref()
                    .and_then(|detail| detail["reason"].as_str())
                    == Some("resync")
                {
                    let member = self.members.get_mut(address).ok_or_else(closed)?;
                    member.resync = true;
                    let state = member.lease.state(None)?;
                    return self.send(address, &state).await;
                }
                return Err(error);
            }
        } else if member.resync {
            return Ok(());
        }
        if step2 {
            let member = self.members.get_mut(address).ok_or_else(closed)?;
            member.resync = false;
            if let Some(held) = member.held.take() {
                return self.saved(address, held).await;
            }
        }
        Ok(())
    }

    async fn check(&mut self, address: &str, bytes: &[u8]) -> Result<(), Error> {
        let member = self.members.get_mut(address).ok_or_else(closed)?;
        let mut cursor = Cursor::new(bytes);
        let id: u64 = cursor
            .read_var()
            .map_err(|_| Error::invalid_input("invalid room save check"))?;
        if id <= member.check || id > SAFE_INT || cursor.has_content() {
            return Err(Error::invalid_input("stale room save check"));
        }
        member.check = id;
        if member.resync {
            member.held = Some(id);
            return Ok(());
        }
        self.saved(address, id).await
    }

    async fn saved(&mut self, address: &str, id: u64) -> Result<(), Error> {
        let member = self.members.get(address).ok_or_else(closed)?;
        self.authorize(&member.key, &member.plugin, member.generation, false)
            .await?;
        member.lease.wait_durable().await?;
        if *member.kicked.borrow() {
            return Err(hidden());
        }
        self.authorize(&member.key, &member.plugin, member.generation, false)
            .await?;
        use yrs::encoding::write::Write;
        let mut bytes = Vec::new();
        bytes.write_var(id);
        self.send(address, &Message::Custom(101, bytes).encode_v1())
            .await
    }
}
