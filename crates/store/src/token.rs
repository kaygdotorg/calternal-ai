//! Signed sync tokens (#918): `epoch-seq-c-m`.
//!
//! - `c`: 32 bits hashed from the journal entry the token points at, so a
//!   token knows *which* entry it saw, not just a number. If the log goes
//!   back in time and hands the same seq to a different entry, `c` no
//!   longer matches.
//! - `m`: 32 bits keyed over `epoch ‖ seq ‖ c` with a per-space key that
//!   lives outside every backup. A garbled or made-up token fails here and
//!   is only ever "invalid"; it can never cause a rotation.

use crate::{Error, Result};
use calternal_fsio::write_atomic_mode;
use rusqlite::{Connection, OptionalExtension};
use std::path::Path;

/// The per-space key behind `m`.
pub(crate) struct Key([u8; 32]);

impl Key {
    /// Reads `token.key`, or makes one. Two random UUIDs hashed together
    /// give 32 bytes from the system's random source.
    pub fn load_or_create(dir: &Path) -> Result<Self> {
        if let Some(key) = Self::load_existing(dir) {
            return Ok(key);
        }
        let path = dir.join("token.key");
        let mut h = blake3::Hasher::new();
        h.update(uuid::Uuid::new_v4().as_bytes());
        h.update(uuid::Uuid::new_v4().as_bytes());
        let key = *h.finalize().as_bytes();
        write_atomic_mode(&path, &key, 0o600).map_err(Error::internal)?;
        Ok(Self(key))
    }

    /// Reads an existing valid key without creating or changing any file.
    pub fn load_existing(dir: &Path) -> Option<Self> {
        let bytes = std::fs::read(dir.join("token.key")).ok()?;
        let key = <[u8; 32]>::try_from(bytes.as_slice()).ok()?;
        Some(Self(key))
    }

    /// Derives a key for the explicit space binding, even if a seed is copied.
    pub(crate) fn for_space(self, space: crate::SpaceId) -> Self {
        let mut material = [0u8; 48];
        material[..32].copy_from_slice(&self.0);
        material[32..].copy_from_slice(space.as_uuid().as_bytes());
        Self(blake3::derive_key(
            "calternal space journal cursor v1",
            &material,
        ))
    }

    /// The `m` part.
    ///
    /// One keyed hash over one stack buffer: the same bytes as feeding the
    /// parts in turn (BLAKE3 hashes a stream and its concatenation alike),
    /// without the per-call cost of a streaming hasher. Minting runs once
    /// per entry on every catch-up page, so this is a hot path.
    pub fn tag(&self, epoch: &str, seq: i64, check: u32) -> u32 {
        let mut buf = Packed::new();
        if buf.push(epoch.as_bytes())
            && buf.push(&[0])
            && buf.push(&seq.to_le_bytes())
            && buf.push(&check.to_le_bytes())
        {
            return first32(blake3::keyed_hash(&self.0, buf.bytes()).as_bytes());
        }
        let mut h = blake3::Hasher::new_keyed(&self.0);
        h.update(epoch.as_bytes());
        h.update(&[0]);
        h.update(&seq.to_le_bytes());
        h.update(&check.to_le_bytes());
        first32(h.finalize().as_bytes())
    }
}

/// The `c` part, from the columns that identify an entry.
///
/// Hashes one stack buffer when the columns fit (they almost always do),
/// else streams them; both give the same value.
pub(crate) fn entry_check(
    seq: i64,
    at_ms: i64,
    kind: &str,
    item: &str,
    request_id: Option<&str>,
) -> u32 {
    let parts = [kind, item, request_id.unwrap_or("")];
    let mut buf = Packed::new();
    let fits = buf.push(&seq.to_le_bytes())
        && buf.push(&at_ms.to_le_bytes())
        && parts
            .iter()
            .all(|part| buf.push(part.as_bytes()) && buf.push(&[0]));
    if fits {
        return first32(blake3::hash(buf.bytes()).as_bytes());
    }
    let mut h = blake3::Hasher::new();
    h.update(&seq.to_le_bytes());
    h.update(&at_ms.to_le_bytes());
    for part in parts {
        h.update(part.as_bytes());
        h.update(&[0]);
    }
    first32(h.finalize().as_bytes())
}

/// Writes `epoch-seq-c-m` without the formatting machinery: minting runs
/// for every entry of every catch-up page.
pub(crate) fn signed_token(epoch: &str, seq: i64, check: u32, tag: u32) -> String {
    let mut digits = [0u8; 20];
    let mut at = digits.len();
    let mut n = seq.unsigned_abs();
    loop {
        at -= 1;
        digits[at] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    // epoch, "-", optional sign and up to 20 digits, then two "-" + 8 hex.
    let mut out = String::with_capacity(epoch.len() + 22 + 18);
    out.push_str(epoch);
    out.push('-');
    if seq < 0 {
        out.push('-');
    }
    // Only ASCII digits were written above.
    out.push_str(std::str::from_utf8(&digits[at..]).unwrap_or("0"));
    for word in [check, tag] {
        out.push('-');
        push_hex8(&mut out, word);
    }
    out
}

/// Appends `word` as eight lowercase hex digits, like `{:08x}`.
fn push_hex8(out: &mut String, word: u32) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for shift in (0..8).rev() {
        out.push(HEX[((word >> (shift * 4)) & 0xf) as usize] as char);
    }
}

/// A small stack buffer for one-shot hashing; `push` reports overflow
/// instead of growing, so callers fall back to streaming.
struct Packed {
    buf: [u8; 256],
    len: usize,
}

impl Packed {
    fn new() -> Self {
        Self {
            buf: [0; 256],
            len: 0,
        }
    }

    /// Appends `bytes`; false (and no change) when they do not fit.
    fn push(&mut self, bytes: &[u8]) -> bool {
        let Some(end) = self.len.checked_add(bytes.len()) else {
            return false;
        };
        let Some(slot) = self.buf.get_mut(self.len..end) else {
            return false;
        };
        slot.copy_from_slice(bytes);
        self.len = end;
        true
    }

    fn bytes(&self) -> &[u8] {
        &self.buf[..self.len]
    }
}

/// `c` of the entry at `seq`, or `None` if there is no such entry.
pub(crate) fn check_at(conn: &Connection, seq: i64) -> Result<Option<u32>> {
    conn.query_row(
        "SELECT at_ms, kind, item, request_id FROM journal WHERE seq = ?1",
        [seq],
        |r| {
            Ok(entry_check(
                seq,
                r.get(0)?,
                &r.get::<_, String>(1)?,
                &r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?.as_deref(),
            ))
        },
    )
    .optional()
    .map_err(Error::internal)
}

fn first32(b: &[u8; 32]) -> u32 {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

#[cfg(test)]
mod tests {
    //! The fast paths must give exactly the old values: tokens and witness
    //! checks already written stay valid.

    use super::*;

    /// The streaming form `entry_check` used before the one-shot buffer.
    fn check_reference(seq: i64, at_ms: i64, parts: [&str; 3]) -> u32 {
        let mut h = blake3::Hasher::new();
        h.update(&seq.to_le_bytes());
        h.update(&at_ms.to_le_bytes());
        for part in parts {
            h.update(part.as_bytes());
            h.update(&[0]);
        }
        first32(h.finalize().as_bytes())
    }

    /// The streaming form `Key::tag` used before the one-shot buffer.
    fn tag_reference(key: &[u8; 32], epoch: &str, seq: i64, check: u32) -> u32 {
        let mut h = blake3::Hasher::new_keyed(key);
        h.update(epoch.as_bytes());
        h.update(&[0]);
        h.update(&seq.to_le_bytes());
        h.update(&check.to_le_bytes());
        first32(h.finalize().as_bytes())
    }

    #[test]
    fn one_shot_hashes_match_the_streaming_values_short_and_long() {
        let long = "x".repeat(400);
        let key = Key([7; 32]);
        for (kind, item, rid) in [
            ("notes.edited", "n1", None),
            ("t.a", "", Some("r-1")),
            ("", "", None),
            ("notes.edited", long.as_str(), Some("r")),
        ] {
            for seq in [0, 1, 4_096, i64::MAX] {
                let c = entry_check(seq, 1_700_000_000_000, kind, item, rid);
                assert_eq!(
                    c,
                    check_reference(seq, 1_700_000_000_000, [kind, item, rid.unwrap_or("")])
                );
                for epoch in ["0a1b2c3d4e5f", long.as_str()] {
                    assert_eq!(key.tag(epoch, seq, c), tag_reference(&key.0, epoch, seq, c));
                }
            }
        }
    }

    #[test]
    fn signed_token_matches_the_formatted_text() {
        for (epoch, seq, c, m) in [
            ("0a1b2c3d4e5f", 0, 0, 0),
            ("ab", 7, 0xbeef, 0xcafe_0001),
            ("e", i64::MAX, u32::MAX, 1),
            ("e", -12, 0x10, 0xffff_fff0),
        ] {
            assert_eq!(
                signed_token(epoch, seq, c, m),
                format!("{epoch}-{seq}-{c:08x}-{m:08x}")
            );
        }
    }
}
