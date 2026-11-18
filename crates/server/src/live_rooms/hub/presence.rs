//! Ownership and host attribution for ephemeral cursor state.

use super::*;

impl Lease {
    pub(in crate::live_rooms) fn presence_snapshot(&self) -> Vec<u8> {
        let clients = self
            .room
            .members
            .lock()
            .unwrap()
            .values()
            .filter_map(|member| member.presence.clone())
            .collect();
        Message::Awareness(AwarenessUpdate { clients }).encode_v1()
    }

    pub(in crate::live_rooms) fn awareness(&self, update: AwarenessUpdate) -> Result<(), Error> {
        let invalid = || Error::invalid_input("invalid room awareness");
        if update.clients.len() != 1 {
            return Err(invalid());
        }
        let (client, entry) = update.clients.into_iter().next().ok_or_else(invalid)?;
        if client.get() > 9_007_199_254_740_991
            || entry.clock == u32::MAX
            || entry.json.len() > 2048
        {
            return Err(invalid());
        }
        let value: serde_json::Value = serde_json::from_str(&entry.json).map_err(|_| invalid())?;
        if !value.is_null() && !value.is_object() {
            return Err(invalid());
        }
        let mut members = self.room.members.lock().unwrap();
        if members.iter().any(|(id, member)| {
            *id != self.id
                && member
                    .presence
                    .as_ref()
                    .is_some_and(|(existing, _)| *existing == client)
        }) {
            return Err(invalid());
        }
        let participant = members.get_mut(&self.id).ok_or_else(closed)?;
        if let Some((existing, previous)) = &participant.presence {
            if *existing != client {
                return Err(invalid());
            }
            if previous.clock >= entry.clock {
                return Ok(());
            }
        }
        let json = if value.is_null() {
            "null".into()
        } else {
            // The presence identity comes from the authenticated host session.
            serde_json::json!({"account_id":participant.account,"cursor":value.get("cursor")})
                .to_string()
                .into()
        };
        let entry = AwarenessUpdateEntry {
            clock: entry.clock,
            json,
        };
        participant.presence = Some((client, entry.clone()));
        drop(members);
        self.room.send(
            Message::Awareness(AwarenessUpdate {
                clients: HashMap::from([(client, entry)]),
            })
            .encode_v1(),
        )
    }
}
