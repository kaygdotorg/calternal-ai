//! Authenticates the last append without keeping a second copy of its text.
use crate::declarations;
use crate::internal;
use crate::space::{Head, ReceiptProof, Space};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use calternal_types::{AccountId, Actor, Result, Via};
use uuid::Uuid;

pub(super) const PREFIX: &str = " <!--calternal-append:v1 ";
const SUFFIX: &str = "-->";
const MAC_BYTES: usize = 16;
const PROOF_BYTES: usize = 64;
const MAX_ENCODED: usize = 232;

pub(super) struct Claim {
    pub(super) actor: Actor,
    pub(super) at: i64,
    pub(super) proof: Option<ReceiptProof>,
}

impl Claim {
    pub(super) fn seal(&self, space: &Space, head: &Head, payload: &mut Vec<u8>) -> Result<()> {
        let plugin = match &self.actor.via {
            None => "",
            Some(Via::Plugin { plugin }) if declarations::name(plugin) => plugin.as_str(),
            _ => return Err(internal("append actor has no supported identity")),
        };
        let mut metadata = Vec::with_capacity(110 + plugin.len());
        metadata.extend_from_slice(self.actor.account.as_uuid().as_bytes());
        metadata.extend_from_slice(&self.at.to_le_bytes());
        metadata.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        metadata.push(plugin.len() as u8);
        metadata.extend_from_slice(plugin.as_bytes());
        metadata.push(u8::from(self.proof.is_some()));
        if let Some(proof) = self.proof {
            metadata.extend_from_slice(&proof.gesture);
            metadata.extend_from_slice(&proof.fingerprint);
        }
        let tag = mac(space, head, &metadata, blake3::hash(payload));
        metadata.extend_from_slice(&tag.as_bytes()[..MAC_BYTES]);
        let suffix = format!("{PREFIX}{}{SUFFIX}", URL_SAFE_NO_PAD.encode(metadata));
        let newline = payload
            .iter()
            .position(|b| *b == b'\n')
            .expect("record has a header");
        payload.splice(newline..newline, suffix.bytes());
        Ok(())
    }

    pub(super) fn verify(space: &Space, head: &Head, tail: &[u8]) -> Option<(Self, String)> {
        let newline = tail.iter().position(|b| *b == b'\n')?;
        let line = std::str::from_utf8(&tail[..newline]).ok()?;
        let (header, encoded) = line.rsplit_once(PREFIX)?;
        let anchor = super::header_anchor(header)?.to_owned();
        let encoded = encoded.strip_suffix(SUFFIX)?;
        if encoded.len() > MAX_ENCODED {
            return None;
        }
        let raw = URL_SAFE_NO_PAD.decode(encoded).ok()?;
        let (metadata, tag) = raw.split_at_checked(raw.len().checked_sub(MAC_BYTES)?)?;
        let raw_head = metadata.get(..29)?;
        let length = u32::from_le_bytes(raw_head[24..28].try_into().ok()?) as usize;
        let body = &tail[newline..];
        if header.len() + body.len() != length {
            return None;
        }
        let mut payload = blake3::Hasher::new();
        payload.update(header.as_bytes());
        payload.update(body);
        let expected = mac(space, head, metadata, payload.finalize());
        // BLAKE3 Hash equality uses a constant-time comparison. Pad both
        // truncated tags before comparing them.
        let mut claimed = [0u8; 32];
        let mut computed = [0u8; 32];
        claimed[..MAC_BYTES].copy_from_slice(tag);
        computed[..MAC_BYTES].copy_from_slice(&expected.as_bytes()[..MAC_BYTES]);
        if blake3::Hash::from(claimed) != blake3::Hash::from(computed) {
            return None;
        }
        let account =
            AccountId::from_uuid(Uuid::from_bytes(raw_head[..16].try_into().ok()?)).ok()?;
        let at = i64::from_le_bytes(raw_head[16..24].try_into().ok()?);
        let plugin_end = 29 + raw_head[28] as usize;
        let plugin = std::str::from_utf8(metadata.get(29..plugin_end)?).ok()?;
        let actor = if plugin.is_empty() {
            Actor::account(account)
        } else if declarations::name(plugin) {
            Actor::plugin(account, plugin)
        } else {
            return None;
        };
        let proof = match metadata.get(plugin_end..)? {
            [0] => None,
            [1, rest @ ..] if rest.len() == PROOF_BYTES => Some(ReceiptProof {
                gesture: rest[..32].try_into().ok()?,
                fingerprint: rest[32..].try_into().ok()?,
            }),
            _ => return None,
        };
        Some((Self { actor, at, proof }, anchor))
    }
}

fn mac(space: &Space, head: &Head, metadata: &[u8], payload: blake3::Hash) -> blake3::Hash {
    let mut tag = blake3::Hasher::new_keyed(&space.key);
    tag.update(b"calternal-host:append:v1\0");
    tag.update(head.id.as_bytes());
    tag.update(&(head.version + 1).to_le_bytes());
    tag.update(&head.length.to_le_bytes());
    tag.update(&(metadata.len() as u64).to_le_bytes());
    tag.update(metadata);
    tag.update(payload.as_bytes());
    tag.finalize()
}
