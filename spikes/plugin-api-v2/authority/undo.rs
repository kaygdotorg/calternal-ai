//! Protected Pin Undo uses the content transaction's durable inverse.
use super::*;
use calternal_content::Request;
use ring::hmac;
use subtle::ConstantTimeEq;

pub(crate) fn load_undo_key(appdata: &std::path::Path) -> hmac::Key {
    let path = appdata.join("secrets/step0-undo.key");
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes.try_into().expect("invalid fixture Undo key size"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let mut key = [0u8; 32];
            getrandom::fill(&mut key).unwrap();
            calternal_fsio::private::publish(&path, &key).unwrap();
            key
        }
        Err(e) => panic!("could not read fixture Undo key: {e}"),
    };
    hmac::Key::new(hmac::HMAC_SHA256, &bytes)
}

pub(crate) fn undo_token(
    key: &hmac::Key,
    actor: AccountId,
    space: SpaceId,
    activation: Uuid,
) -> String {
    let signed = hmac::sign(key, format!("{actor}/{space}/{activation}").as_bytes());
    format!("{activation}.{}", hex::encode(&signed.as_ref()[..16]))
}

pub(crate) fn pin_outcome(activation: Uuid, token: &str) -> Value {
    let expiry = calternal_content::format_rfc3339(
        time::OffsetDateTime::now_utc() + time::Duration::minutes(10),
    );
    json!({"activation_id":activation,"state":"ok","undo":{"ref":token,"label":"Undo pin","expires_at":expiry}})
}

impl Host {
    pub(super) fn undo_ref(&self, actor: AccountId, space: SpaceId, activation: Uuid) -> String {
        undo_token(&self.undo_key, actor, space, activation)
    }

    pub(super) async fn undo(
        &self,
        who: Identity,
        session: Uuid,
        lease: Uuid,
        input: &Value,
    ) -> Reply<Value> {
        let (_, view) = self.lease(who, session, lease).await?;
        let token = input["undo_ref"].as_str().ok_or(INVALID)?;
        let activation = token
            .split_once('.')
            .and_then(|(ref_id, _)| Uuid::parse_str(ref_id).ok())
            .ok_or(HIDDEN)?;
        if !bool::from(
            token
                .as_bytes()
                .ct_eq(self.undo_ref(who.id, view.space, activation).as_bytes()),
        ) {
            return Err(HIDDEN);
        }
        let _fence = view.gate.lock().await;
        self.lease(who, session, lease).await?;
        if !self.editor(who, &view).await {
            return Err(FORBIDDEN);
        }
        if let Some(display) = &view.display {
            return display.undo(who.id, activation);
        }
        let original = {
            let state = self.state.clone();
            let space = view.space;
            tokio::task::spawn_blocking(move || {
                state
                    .store
                    .content()
                    .receipt(space, who.id, &format!("step0/{activation}"))
            })
            .await
            .unwrap()
            .map_err(|_| STALE)?
            .filter(|r| r.operation == Action::Pin.name())
            .ok_or(HIDDEN)?
        };
        let row = ref_at(&original.result, "target")?;
        if !self.current_item(who, &view, row, true).await {
            return Err(HIDDEN);
        }
        let outcome = &original.result["outcome"]["undo"];
        let expires_at = outcome["expires_at"]
            .as_str()
            .and_then(|expiry| {
                time::OffsetDateTime::parse(expiry, &time::format_description::well_known::Rfc3339)
                    .ok()
            })
            .ok_or(HIDDEN)?;
        if expires_at <= time::OffsetDateTime::now_utc() || outcome["ref"] != token {
            return Err(HIDDEN);
        }
        let state = self.state.clone();
        let space = view.space;
        tokio::task::spawn_blocking(move || {
            let content = state.store.content();
            let request_id = format!("step0-undo/{activation}");
            if let Some(replay) = content
                .receipt(space, who.id, &request_id)
                .map_err(|_| STALE)?
            {
                return Ok(replay.result);
            }
            let current = content.get(space, row).map_err(|_| HIDDEN)?;
            if original.result["post_revision"] != current.revision {
                return Err(STALE);
            }
            let mut metadata = current.metadata;
            if let Some(pinned) = original.result["inverse"]["pinned"].as_bool() {
                metadata["pinned"] = json!(pinned);
            } else {
                metadata.as_object_mut().unwrap().remove("pinned");
            }
            let result = json!({"activation_id":activation,"state":"ok"});
            content
                .write(
                    space,
                    Some(row),
                    NOTES,
                    Some(&current.revision),
                    metadata,
                    &current.bytes,
                    Request {
                        account: who.id,
                        via: None,
                        id: &request_id,
                        operation: "host.undo",
                        fingerprint: &request_id,
                    },
                    |_| result.clone(),
                )
                .map_err(|_| STALE)?;
            Ok(result)
        })
        .await
        .unwrap()
    }
}
