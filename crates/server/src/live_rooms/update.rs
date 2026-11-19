//! Binary sync admission, durable check ordering, and protocol refusals.

use super::*;
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
                return self.left(&address, None).await;
            }
            Err(_) => return Err(Error::invalid_input("invalid room frame")),
        };
        let mut decoder = DecoderV1::new(Cursor::new(&packet.bytes));
        for message in MessageReader::new(&mut decoder) {
            let message = message.map_err(|_| Error::invalid_input("invalid room message"))?;
            if let Err(error) = self.apply_message(&packet.room, message).await {
                if access_revoked(&error) {
                    return self.left(&packet.room, Some(&error)).await;
                }
                let retry = matches!(
                    error.code,
                    ErrorCode::RetryAfter | ErrorCode::Unavailable | ErrorCode::Internal
                );
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
                    self.left(&packet.room, None).await?;
                }
                break;
            }
        }
        Ok(())
    }

    async fn apply_message(&mut self, address: &str, message: Message) -> Result<(), Error> {
        let message = match message {
            Message::Sync(SyncMessage::SyncStep2(bytes)) => {
                return self.update(address, bytes, true).await;
            }
            Message::Sync(SyncMessage::Update(bytes)) => {
                return self.update(address, bytes, false).await;
            }
            Message::Custom(100, data) => return self.check(address, &data).await,
            message => message,
        };
        let member = self.members.get(address).ok_or_else(closed)?;
        self.authorize_member(member, false).await?;
        match message {
            Message::Sync(SyncMessage::SyncStep1(vector)) => {
                self.send(address, &self.members[address].lease.state(Some(&vector))?)
                    .await
            }
            Message::Awareness(update) => member.lease.awareness(update),
            Message::AwarenessQuery => {
                self.send(address, &self.members[address].lease.presence_snapshot())
                    .await
            }
            _ => Ok(()),
        }
    }

    /// Admit an edit at a stable Edit decision. Revocation rejects later
    /// admissions. An admitted edit can finish for other authorized peers
    /// unless eviction reaches its author before it enters preview fanout.
    async fn update(&mut self, address: &str, bytes: Vec<u8>, step2: bool) -> Result<(), Error> {
        let member = self.members.get(address).ok_or_else(closed)?;
        if member.resync && !step2 {
            return Ok(());
        }
        if bytes != [0, 0] {
            self.authorize_member(member, true).await?;
            if let Some(end) = *member.termination.borrow() {
                return Err(end.error());
            }
            let result = member
                .lease
                .write(
                    self.content.clone(),
                    self.account,
                    member.lineage.clone(),
                    bytes,
                )
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
        member.lease.wait_durable().await?;
        if let Some(end) = *member.termination.borrow() {
            return Err(end.error());
        }
        self.authorize_member(member, false).await?;
        use yrs::encoding::write::Write;
        let mut bytes = Vec::new();
        bytes.write_var(id);
        self.send(address, &Message::Custom(101, bytes).encode_v1())
            .await
    }
}
