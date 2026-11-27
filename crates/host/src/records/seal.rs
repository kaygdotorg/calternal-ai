//! Authenticates an append using its on-disk header and the prior ledger head.
use crate::internal;
use crate::space::{Head, Space};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use calternal_types::{AccountId, Actor, Result, Via};

pub(super) const PREFIX: &str = " <!--ca1 ";
pub(super) const SUFFIX: &str = "-->";
const TAG_BYTES: usize = 16;
const TAG_CHARS: usize = 22;

pub(super) struct Claim {
    pub(super) actor: Actor,
    pub(super) at: i64,
    pub(super) anchor: String,
    pub(super) tag: [u8; TAG_BYTES],
    pub(super) effect: [u8; 32],
}

pub(super) fn key_anchor(
    space: &Space,
    item: uuid::Uuid,
    ordinal: u64,
    offset: u64,
    actor: &Actor,
    gesture: &[u8; 32],
) -> Result<String> {
    let mut hash = blake3::Hasher::new_keyed(&space.key);
    hash.update(b"calternal-host:append-anchor:v1\0");
    hash.update(item.as_bytes());
    hash.update(&ordinal.to_le_bytes());
    hash.update(&offset.to_le_bytes());
    let identity = serde_json::to_vec(actor).map_err(internal)?;
    hash.update(&(identity.len() as u64).to_le_bytes());
    hash.update(&identity);
    hash.update(gesture);
    Ok(super::anchor_text(
        hash.finalize().as_bytes()[..10]
            .try_into()
            .expect("ten bytes"),
    ))
}

pub(super) fn seal(space: &Space, head: &Head, actor: &Actor, payload: &mut Vec<u8>) -> Result<()> {
    let tag = mac(space, head, actor, blake3::hash(payload))?;
    let run = match actor.via {
        Some(Via::Run { id, .. }) => format!(".{id}"),
        _ => String::new(),
    };
    let suffix = format!(
        "{PREFIX}{}{run}{SUFFIX}",
        URL_SAFE_NO_PAD.encode(&tag.as_bytes()[..TAG_BYTES])
    );
    let newline = payload
        .iter()
        .position(|b| *b == b'\n')
        .expect("record has a header");
    payload.splice(newline..newline, suffix.bytes());
    Ok(())
}

pub(super) fn verify(space: &Space, head: &Head, tail: &[u8]) -> Result<Option<Claim>> {
    let decoded = || {
        let newline = tail.iter().position(|b| *b == b'\n')?;
        let line = std::str::from_utf8(&tail[..newline]).ok()?;
        let (header, encoded) = line.rsplit_once(PREFIX)?;
        let encoded = encoded.strip_suffix(SUFFIX)?;
        let (encoded, run) = match encoded.split_once('.') {
            Some((tag, id)) => (tag, Some(uuid::Uuid::parse_str(id).ok()?)),
            None => (encoded, None),
        };
        if encoded.len() != TAG_CHARS {
            return None;
        }
        let tag: [u8; TAG_BYTES] = URL_SAFE_NO_PAD.decode(encoded).ok()?.try_into().ok()?;
        let anchor = super::header_anchor(header)?.to_owned();
        let (_, rest) = header.rsplit_once("** · ")?;
        let (stamp, _) = rest.rsplit_once(" ^")?;
        let minute = time::PrimitiveDateTime::parse(
            stamp,
            time::macros::format_description!("[year]-[month]-[day] [hour]:[minute]"),
        )
        .ok()?
        .assume_utc()
        .unix_timestamp()
            * 1000;
        let (owner, plugin) = (
            AccountId::parse(&head.author).ok()?,
            head.kind.split_once('.')?.0,
        );
        Some((header, newline, tag, anchor, minute, owner, plugin, run))
    };
    let Some((header, newline, tag, anchor, at, owner, plugin, run)) = decoded() else {
        return Ok(None);
    };
    let mut payload = blake3::Hasher::new();
    payload.update(header.as_bytes());
    payload.update(&tail[newline..]);
    let payload = payload.finalize();
    // Slice 1 admits only the personal-space owner. The owning plugin is
    // the only delegated path that can append to this kind.
    let candidates = match run {
        Some(id) => [Some(Actor::run(owner, plugin, id)), None],
        None => [
            Some(Actor::account(owner)),
            Some(Actor::plugin(owner, plugin)),
        ],
    };
    for actor in candidates.into_iter().flatten() {
        let expected = mac(space, head, &actor, payload)?;
        // BLAKE3 Hash equality is constant time. Pad both truncated tags.
        let mut claimed = [0u8; 32];
        let mut computed = [0u8; 32];
        claimed[..TAG_BYTES].copy_from_slice(&tag);
        computed[..TAG_BYTES].copy_from_slice(&expected.as_bytes()[..TAG_BYTES]);
        if blake3::Hash::from(claimed) == blake3::Hash::from(computed) {
            return Ok(Some(Claim {
                actor,
                at,
                anchor,
                tag,
                effect: space.hash(&tail[newline + 1..]),
            }));
        }
    }
    Ok(None)
}

fn mac(space: &Space, head: &Head, actor: &Actor, payload: blake3::Hash) -> Result<blake3::Hash> {
    let mut tag = blake3::Hasher::new_keyed(&space.key);
    tag.update(b"calternal-host:append:v1\0");
    tag.update(head.id.as_bytes());
    tag.update(&(head.version + 1).to_le_bytes());
    tag.update(&head.length.to_le_bytes());
    let account = actor.account.as_uuid();
    tag.update(account.as_bytes());
    match &actor.via {
        None => tag.update(&[0]),
        Some(Via::Plugin { plugin }) => {
            tag.update(&[1]);
            tag.update(&(plugin.len() as u64).to_le_bytes());
            tag.update(plugin.as_bytes())
        }
        Some(Via::Run { plugin, id }) => {
            tag.update(&[2]);
            tag.update(id.as_bytes());
            tag.update(&(plugin.len() as u64).to_le_bytes());
            tag.update(plugin.as_bytes())
        }
        Some(Via::Agent { .. }) => return Err(internal("append actor has no supported identity")),
    };
    tag.update(payload.as_bytes());
    Ok(tag.finalize())
}
