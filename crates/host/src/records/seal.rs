//! Authenticates an append using its on-disk header and the prior ledger head.
use super::RunWriter;
use crate::internal;
use crate::space::{Head, Space};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use calternal_types::{AccountId, Actor, Result, Via};

pub(super) const PREFIX: &str = " <!--ca1 ";
pub(super) const SUFFIX: &str = "-->";
const TAG_BYTES: usize = 16;
const CLAIM_CHARS: usize = 24;

pub(super) struct Claim {
    pub(super) actor: Actor,
    pub(super) at: i64,
    pub(super) anchor: String,
    pub(super) run: Option<RunWriter>,
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

pub(super) fn seal(
    space: &Space,
    head: &Head,
    actor: &Actor,
    at: i64,
    writer: Option<&RunWriter>,
    payload: &mut Vec<u8>,
) -> Result<()> {
    let within_minute = (at.rem_euclid(60_000) as u16).to_le_bytes();
    let tag = mac(
        space,
        head,
        actor,
        at,
        blake3::hash(payload),
        writer.map(|run| run.plugin.as_str()),
    )?;
    let mut claim = [0; TAG_BYTES + 2];
    claim[..2].copy_from_slice(&within_minute);
    claim[2..].copy_from_slice(&tag.as_bytes()[..TAG_BYTES]);
    let run = match actor.via {
        Some(Via::Run { id, .. }) => match writer {
            Some(run) => format!(".{id}.{}", URL_SAFE_NO_PAD.encode(&run.plugin)),
            None => format!(".{id}"),
        },
        _ => String::new(),
    };
    let suffix = format!("{PREFIX}{}{run}{SUFFIX}", URL_SAFE_NO_PAD.encode(claim));
    let newline = payload
        .iter()
        .position(|b| *b == b'\n')
        .expect("record has a header");
    payload.splice(newline..newline, suffix.bytes());
    Ok(())
}

// A legacy suffix carries only a UUID. The extended suffix also carries
// the starting harness, separately from the plugin authorized to append.
fn decode_run(suffix: &str) -> Option<(uuid::Uuid, Option<&str>)> {
    let (id, plugin) = suffix
        .split_once('.')
        .map_or((suffix, None), |(id, plugin)| (id, Some(plugin)));
    Some((uuid::Uuid::parse_str(id).ok()?, plugin))
}

pub(super) fn run_stamp(header: &str) -> Option<(uuid::Uuid, Option<&str>)> {
    let (_, stamp) = header.strip_suffix(SUFFIX)?.rsplit_once(PREFIX)?;
    let (_, suffix) = stamp.split_once('.')?;
    decode_run(suffix)
}

pub(super) fn verify(space: &Space, head: &Head, tail: &[u8]) -> Result<Option<Claim>> {
    let decoded = || {
        let newline = tail.iter().position(|b| *b == b'\n')?;
        let line = std::str::from_utf8(&tail[..newline]).ok()?;
        let (header, encoded) = line.rsplit_once(PREFIX)?;
        let encoded = encoded.strip_suffix(SUFFIX)?;
        let (encoded, run) = match encoded.split_once('.') {
            Some((tag, suffix)) => (tag, Some(decode_run(suffix)?)),
            None => (encoded, None),
        };
        if encoded.len() != CLAIM_CHARS {
            return None;
        }
        let claim: [u8; TAG_BYTES + 2] = URL_SAFE_NO_PAD.decode(encoded).ok()?.try_into().ok()?;
        let within_minute = i64::from(u16::from_le_bytes(claim[..2].try_into().ok()?));
        if within_minute >= 60_000 {
            return None;
        }
        let tag: [u8; TAG_BYTES] = claim[2..].try_into().ok()?;
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
        Some((
            header,
            newline,
            tag,
            anchor,
            minute + within_minute,
            owner,
            plugin,
            run,
        ))
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
        Some((id, _)) => [Some(Actor::run(owner, plugin, id)), None],
        None => [
            Some(Actor::account(owner)),
            Some(Actor::plugin(owner, plugin)),
        ],
    };
    let writer = match run {
        Some((id, Some(encoded))) => {
            let Some(plugin) = URL_SAFE_NO_PAD
                .decode(encoded)
                .ok()
                .and_then(|bytes| String::from_utf8(bytes).ok())
            else {
                return Ok(None);
            };
            Some(RunWriter { id, plugin })
        }
        _ => None,
    };
    for actor in candidates.into_iter().flatten() {
        let expected = mac(
            space,
            head,
            &actor,
            at,
            payload,
            writer.as_ref().map(|run| run.plugin.as_str()),
        )?;
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
                run: writer,
            }));
        }
    }
    Ok(None)
}

fn mac(
    space: &Space,
    head: &Head,
    actor: &Actor,
    at: i64,
    payload: blake3::Hash,
    writer_plugin: Option<&str>,
) -> Result<blake3::Hash> {
    let mut tag = blake3::Hasher::new_keyed(&space.key);
    tag.update(b"calternal-host:append:v2\0");
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
    tag.update(&at.to_le_bytes());
    tag.update(payload.as_bytes());
    if let Some(plugin) = writer_plugin {
        tag.update(b"starting-harness\0");
        tag.update(&(plugin.len() as u64).to_le_bytes());
        tag.update(plugin.as_bytes());
    }
    Ok(tag.finalize())
}
