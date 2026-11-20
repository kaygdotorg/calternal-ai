//! Record files: append, delete for everyone, list, and checking outside changes.
//!
//! A record is a host-written header line, a blank line, the body, a blank line:
//! `**Lee** · 2026-11-20 17:02 ^k2m8q1w4zre7k3f9`. The stamp (person, plugin,
//! time, hash) lives in host state; the header is a readable copy and is never
//! read for authority.
//!
//! D5, recommended option: host state keeps each record's offset, length and
//! keyed hash, **not its text**. Consequences met in this file:
//! - a changed body cannot be put back, only detected (`verify` marks it `tampered`);
//! - a changed header can be put back, because a header is derived from the stamp;
//! - crash recovery for an in-place rewrite has no copy to roll forward from (not built).
use crate::call::{Ctx, receipt, stamp};
use crate::host::{FileRow, Space, fail};
use calternal_fsio::TEMP_PREFIX;
use calternal_types::Error;
use rusqlite::params;
use serde_json::{Value, json};
use std::io::Write;
use std::os::unix::fs::FileExt;

/// What a deleted record's body becomes.
const DELETED: &str = "*deleted*";
const CROCKFORD: &[u8; 32] = b"0123456789abcdefghjkmnpqrstvwxyz";

/// A fresh 16-character Crockford anchor (80 random bits, the ADR 0056 shape).
fn anchor() -> String {
    let bytes = uuid::Uuid::new_v4().into_bytes();
    let mut bits: u128 = 0;
    for b in &bytes[..10] {
        bits = (bits << 8) | u128::from(*b);
    }
    (0..16)
        .map(|i| CROCKFORD[((bits >> (75 - 5 * i)) & 31) as usize] as char)
        .collect()
}

/// The header line for a record, without its newline.
fn header(name: &str, at_ms: i64, anchor: &str) -> String {
    let t = time::OffsetDateTime::from_unix_timestamp(at_ms / 1000)
        .unwrap_or(time::OffsetDateTime::UNIX_EPOCH);
    let when = t
        .format(time::macros::format_description!(
            "[year]-[month]-[day] [hour]:[minute]"
        ))
        .unwrap_or_default();
    format!("**{name}** · {when} ^{anchor}")
}

/// `(name, anchor)` when `line` is a host-shaped header line.
fn parse_header(line: &str) -> Option<(&str, &str)> {
    let rest = line.strip_prefix("**")?;
    let (name, rest) = rest.split_once("** · ")?;
    let (when, anchor) = rest.rsplit_once(" ^")?;
    let ok_when = when.len() == 16
        && when.as_bytes()[4] == b'-'
        && when.as_bytes()[10] == b' '
        && when.as_bytes()[13] == b':';
    let ok_anchor = anchor.len() == 16 && anchor.bytes().all(|b| CROCKFORD.contains(&b));
    (ok_when && ok_anchor && !name.is_empty()).then_some((name, anchor))
}

/// The record's bytes: header, blank line, body (header-looking lines escaped), blank line.
fn record_bytes(header: &str, body: &str) -> String {
    let body: Vec<String> = body
        .trim_end_matches('\n')
        .lines()
        .map(|l| {
            if parse_header(l.trim_start_matches('\\')).is_some() {
                format!("\\{l}")
            } else {
                l.to_owned()
            }
        })
        .collect();
    format!("{header}\n\n{}\n\n", body.join("\n"))
}

/// One record region found by parsing: anchor, byte offset, length, header length (with its newline).
struct Region {
    anchor: String,
    off: usize,
    len: usize,
    header_len: usize,
}

/// Split `text[start..]` into record regions, each from a header line to the
/// next. `None` when anything other than blank lines precedes the first header.
fn regions(text: &str, start: usize) -> Option<Vec<Region>> {
    let mut out: Vec<Region> = Vec::new();
    let mut at = start;
    for line in text[start..].split_inclusive('\n') {
        if let Some((_, a)) = parse_header(line.trim_end_matches('\n')) {
            if let Some(last) = out.last_mut() {
                last.len = at - last.off;
            }
            out.push(Region {
                anchor: a.to_owned(),
                off: at,
                len: 0,
                header_len: line.len(),
            });
        } else if out.is_empty() && !line.trim().is_empty() {
            return None;
        }
        at += line.len();
    }
    if let Some(last) = out.last_mut() {
        last.len = at - last.off;
    }
    Some(out)
}

/// A record as host state knows it.
struct Rec {
    anchor: String,
    off: u64,
    len: u64,
    person: String,
    at: i64,
    hash: Vec<u8>,
    state: String,
}

fn recs(space: &Space, file: &str) -> Result<Vec<Rec>, Error> {
    let mut stmt = space
        .db
        .prepare_cached("SELECT anchor, off, len, person, at, hash, state FROM records WHERE file = ?1 ORDER BY off")
        .map_err(fail)?;
    let rows = stmt
        .query_map([file], |r| {
            Ok(Rec {
                anchor: r.get(0)?,
                off: r.get::<_, i64>(1)? as u64,
                len: r.get::<_, i64>(2)? as u64,
                person: r.get(3)?,
                at: r.get(4)?,
                hash: r.get(5)?,
                state: r.get(6)?,
            })
        })
        .map_err(fail)?;
    rows.collect::<Result<_, _>>().map_err(fail)
}

fn record_file(space: &Space, input: &Value) -> Result<FileRow, Error> {
    let row = space.file(input["file"].as_str().unwrap_or_default())?;
    match space.kind(&row.kind) {
        Some((_, k)) if k.records => Ok(row),
        _ => Err(Error::invalid_input("not a record file")),
    }
}

/// Open a record file for writing in place. `Folder` (crates/fsio) has no
/// writable open beneath a folder, so the path is checked as text and opened
/// without following a final symlink: a weaker fence than `RESOLVE_BENEATH`.
fn open_rw(space: &Space, path: &str) -> Result<std::fs::File, Error> {
    use std::os::unix::fs::OpenOptionsExt;
    calternal_fsio::beneath::plain_parts(path).map_err(|_| Error::invalid_input("bad path"))?;
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(0o400000 /* O_NOFOLLOW */)
        .open(space.root.join(path))
        .map_err(fail)
}

/// `record.append {file, body}`. Cost when the file is as the host left it:
/// 1 indexed read (head), 1 stat, 1 ledger commit (record + stamp + head + receipt),
/// 1 positioned write and 1 `fdatasync`, 1 `fstat`. No read of the file, no hash of it.
///
/// # Errors
/// `invalid_input` for an empty body; `conflict` when the file changed during the write.
pub fn append(ctx: &Ctx, space: &mut Space, input: &Value) -> Result<Value, Error> {
    let mut row = record_file(space, input)?;
    let body = input.get("body").and_then(Value::as_str).unwrap_or("");
    if body.trim().is_empty() || body.len() > 64 * 1024 {
        return Err(Error::input_field("/body", "1 to 65536 bytes of text"));
    }
    if verify(space, &row)?.is_some() {
        row = space.file(&row.id)?;
    }
    let at = calternal_types::now_ms();
    let a = anchor();
    let bytes = record_bytes(
        &header(&space.display_name(&ctx.person.to_string()), at, &a),
        body,
    );
    let hash = space.hash(bytes.as_bytes());
    let tx = space.db.transaction().map_err(fail)?;
    tx.execute(
        "INSERT INTO records(anchor, file, off, len, person, at, hash, state) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'live')",
        params![a, row.id, row.len as i64, bytes.len() as i64, ctx.person.to_string(), at, hash],
    )
    .map_err(fail)?;
    tx.execute(
        "UPDATE files SET len = len + ?2, rev = rev + 1 WHERE id = ?1",
        params![row.id, bytes.len() as i64],
    )
    .map_err(fail)?;
    let seq = stamp(&tx, ctx, &row.id, row.rev + 1, Some(&a), "append", &hash)?;
    let result = json!({ "anchor": a, "seq": seq });
    receipt(&tx, ctx, "record.append", &result)?;
    tx.commit().map_err(fail)?;
    let f = open_rw(space, &row.path)?;
    if f.metadata().map_err(fail)?.len() != row.len {
        return Err(Error::conflict(
            "the file changed while appending; the record is in host state but not on disk",
        ));
    }
    f.write_all_at(bytes.as_bytes(), row.len).map_err(fail)?;
    f.sync_data().map_err(fail)?;
    let seen = space.stat(&row.path)?;
    space.seen.insert(row.id, seen);
    Ok(result)
}

/// `record.delete {file, anchor}`: delete for everyone. Only the record's author
/// may. The body becomes `*deleted*`, padded to the old length so it is written
/// in place; a body shorter than that rewrites the rest of the file. Old hashes
/// of the record are scrubbed from the stamps and the WAL is checkpointed.
///
/// # Errors
/// `forbidden` for anyone but the author; `not_found` for an unknown or deleted record.
pub fn delete(ctx: &Ctx, space: &mut Space, input: &Value) -> Result<Value, Error> {
    let mut row = record_file(space, input)?;
    if verify(space, &row)?.is_some() {
        row = space.file(&row.id)?;
    }
    let want = input
        .get("anchor")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let rec = recs(space, &row.id)?
        .into_iter()
        .find(|r| r.anchor == want && r.state != "deleted")
        .ok_or_else(|| Error::not_found("no such record"))?;
    if rec.person != ctx.person.to_string() {
        return Err(Error::forbidden("only the record's author may delete it"));
    }
    let f = open_rw(space, &row.path)?;
    let mut old = vec![0; rec.len as usize];
    f.read_exact_at(&mut old, rec.off).map_err(fail)?;
    let old = String::from_utf8(old).map_err(fail)?;
    let head = old.split_inclusive('\n').next().unwrap_or("");
    let body_len = old.len().saturating_sub(head.len() + 3);
    let new = if body_len >= DELETED.len() {
        let pad = body_len - DELETED.len();
        let pad = if pad == 0 {
            String::new()
        } else {
            format!("\n{}", " ".repeat(pad - 1))
        };
        format!("{head}\n{DELETED}{pad}\n\n")
    } else {
        format!("{head}\n{DELETED}\n\n")
    };
    let delta = new.len() as i64 - old.len() as i64;
    let hash = space.hash(new.as_bytes());
    let result = json!({ "anchor": rec.anchor, "in_place": delta == 0 });
    let tx = space.db.transaction().map_err(fail)?;
    tx.execute(
        "UPDATE records SET hash = ?2, state = 'deleted', len = ?3 WHERE anchor = ?1",
        params![rec.anchor, hash, new.len() as i64],
    )
    .map_err(fail)?;
    if delta != 0 {
        tx.execute(
            "UPDATE records SET off = off + ?3 WHERE file = ?1 AND off > ?2",
            params![row.id, rec.off as i64, delta],
        )
        .map_err(fail)?;
    }
    tx.execute(
        "UPDATE stamps SET hash = NULL WHERE anchor = ?1",
        [&rec.anchor],
    )
    .map_err(fail)?;
    tx.execute(
        "UPDATE files SET len = len + ?2, rev = rev + 1 WHERE id = ?1",
        params![row.id, delta],
    )
    .map_err(fail)?;
    stamp(
        &tx,
        ctx,
        &row.id,
        row.rev + 1,
        Some(&rec.anchor),
        "delete",
        &hash,
    )?;
    receipt(&tx, ctx, "record.delete", &result)?;
    tx.commit().map_err(fail)?;
    if delta == 0 {
        f.write_all_at(new.as_bytes(), rec.off).map_err(fail)?;
        f.sync_data().map_err(fail)?;
        let seen = space.stat(&row.path)?;
        space.seen.insert(row.id.clone(), seen);
    } else {
        let mut all = std::fs::read(space.root.join(&row.path)).map_err(fail)?;
        all.splice(rec.off as usize..(rec.off + rec.len) as usize, new.bytes());
        crate::files::replace(space, &row.path, &all, &ctx.request)?;
    }
    space
        .db
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .map_err(fail)?;
    Ok(result)
}

/// `record.list {file}`: records in file order, each with its author **from the
/// stamp** (never from the header line), its state and its body as the file has it.
///
/// # Errors
/// `not_found` for an unknown file.
pub fn list(_ctx: &Ctx, space: &mut Space, input: &Value) -> Result<Value, Error> {
    let mut row = record_file(space, input)?;
    let checked = verify(space, &row)?;
    if checked.is_some() {
        row = space.file(&row.id)?;
    }
    let text = std::fs::read_to_string(space.root.join(&row.path)).map_err(fail)?;
    let mut out = Vec::new();
    for r in recs(space, &row.id)? {
        let region = text
            .get(r.off as usize..(r.off + r.len) as usize)
            .unwrap_or("");
        let head = region.split_inclusive('\n').next().unwrap_or("");
        let body = region[head.len()..].trim_matches('\n').trim_end();
        out.push(
            json!({ "anchor": r.anchor, "author": space.display_name(&r.person), "person": r.person,
            "state": r.state, "at": r.at, "body": if r.state == "deleted" { "" } else { body } }),
        );
    }
    Ok(json!({ "records": out, "checked": checked }))
}

/// Check a record file against host state when it is not as the host left it.
/// Returns `None` when nothing changed, else what was found and done:
/// - a pure append of host-shaped records is accepted, each stamped `outside`;
/// - any other change writes a conflict copy of the changed bytes beside the
///   file; changed headers are put back from their stamps; when every record
///   can be put back, the record file is restored, otherwise the records whose
///   bodies changed are marked `tampered` (D5: there is no copy of the text).
///
/// Cost: 1 stat on the fast path; a whole-file read and one hash per record otherwise.
fn verify(space: &mut Space, row: &FileRow) -> Result<Option<Value>, Error> {
    let now = space.stat(&row.path)?;
    if space.seen.get(&row.id) == Some(&now) {
        return Ok(None);
    }
    let text = std::fs::read_to_string(space.root.join(&row.path)).map_err(fail)?;
    let bytes = text.as_bytes();
    let known = recs(space, &row.id)?;
    let prefix_ok = bytes.len() as u64 >= row.prefix_len
        && space.hash(&bytes[..row.prefix_len as usize]) == row.hash;
    let intact = |r: &Rec| {
        bytes
            .get(r.off as usize..(r.off + r.len) as usize)
            .is_some_and(|b| space.hash(b) == r.hash)
    };
    let all_intact = prefix_ok && known.iter().all(intact);
    let tail = (bytes.len() as u64 >= row.len)
        .then(|| regions(&text, row.len as usize))
        .flatten();
    let report = if all_intact && let Some(tail) = tail {
        if tail.is_empty() && bytes.len() as u64 == row.len {
            None
        } else {
            let tx = space.db.transaction().map_err(fail)?;
            let at = calternal_types::now_ms();
            let key: Vec<u8> = tx
                .query_row("SELECT key FROM space", [], |r| r.get(0))
                .map_err(fail)?;
            let key: [u8; 32] = key.try_into().map_err(|_| Error::internal("bad key"))?;
            let mut accepted = Vec::new();
            for g in &tail {
                let h = blake3::keyed_hash(&key, &bytes[g.off..g.off + g.len])
                    .as_bytes()
                    .to_vec();
                tx.execute(
                    "INSERT OR IGNORE INTO records(anchor, file, off, len, person, at, hash, state) VALUES (?1, ?2, ?3, ?4, 'outside', ?5, ?6, 'live')",
                    params![g.anchor, row.id, g.off as i64, g.len as i64, at, h],
                )
                .map_err(fail)?;
                tx.execute(
                    "INSERT INTO stamps(file, rev, anchor, op, person, plugin, request, at, hash) VALUES (?1, ?2, ?3, 'append', 'outside', 'outside', 'outside', ?4, ?5)",
                    params![row.id, row.rev + 1, g.anchor, at, h],
                )
                .map_err(fail)?;
                accepted.push(g.anchor.clone());
            }
            tx.execute(
                "UPDATE files SET len = ?2, rev = rev + 1 WHERE id = ?1",
                params![row.id, bytes.len() as i64],
            )
            .map_err(fail)?;
            tx.commit().map_err(fail)?;
            Some(json!({ "outside_appends": accepted }))
        }
    } else {
        Some(conflict(space, row, &text, &known, prefix_ok)?)
    };
    let now = space.stat(&row.path)?;
    space.seen.insert(row.id.clone(), now);
    Ok(report)
}

/// The slow half of `verify`: conflict copy, header repair, restore or mark tampered.
fn conflict(
    space: &mut Space,
    row: &FileRow,
    text: &str,
    known: &[Rec],
    prefix_ok: bool,
) -> Result<Value, Error> {
    let (dir, name) = row.path.rsplit_once('/').unwrap_or(("", &row.path));
    let stem = name.trim_end_matches(".md");
    let stamp_text = time::OffsetDateTime::now_utc()
        .format(time::macros::format_description!(
            "[year]-[month]-[day] [hour][minute][second]"
        ))
        .unwrap_or_default();
    let folder = space.folder.open_sub(dir, false).map_err(fail)?;
    let mut temp = folder
        .create_temp(&format!("{TEMP_PREFIX}conflict-{}", row.id))
        .map_err(fail)?;
    temp.file().write_all(text.as_bytes()).map_err(fail)?;
    let copy = temp
        .persist(|n| {
            Some(if n == 1 {
                format!("{stem} (conflict {stamp_text}).md")
            } else {
                format!("{stem} (conflict {stamp_text} {n}).md")
            })
        })
        .map_err(fail)?;
    let start = if prefix_ok {
        row.prefix_len as usize
    } else {
        text.find("\n**").map_or(text.len(), |i| i + 1)
    };
    let found = regions(text, start).unwrap_or_default();
    let mut restored = String::new();
    let mut tampered = Vec::new();
    let mut moved = Vec::new();
    for r in known {
        let Some(g) = found.iter().find(|g| g.anchor == r.anchor) else {
            tampered.push(r.anchor.clone());
            continue;
        };
        let raw = &text[g.off..g.off + g.len];
        if space.hash(raw.as_bytes()) == r.hash {
            restored.push_str(raw);
            moved.push((
                r.anchor.clone(),
                g.off,
                g.len,
                r.state.clone(),
                r.hash.clone(),
            ));
            continue;
        }
        let rest = &text[g.off + g.header_len..g.off + g.len];
        let candidate = format!(
            "{}\n{rest}",
            header(&space.display_name(&r.person), r.at, &r.anchor)
        );
        if space.hash(candidate.as_bytes()) == r.hash {
            restored.push_str(&candidate);
            moved.push((
                r.anchor.clone(),
                g.off,
                candidate.len(),
                r.state.clone(),
                r.hash.clone(),
            ));
        } else {
            tampered.push(r.anchor.clone());
            moved.push((
                r.anchor.clone(),
                g.off,
                g.len,
                "tampered".into(),
                space.hash(raw.as_bytes()),
            ));
        }
    }
    let can_restore = prefix_ok && tampered.is_empty();
    if can_restore {
        let all = format!("{}{restored}", &text[..row.prefix_len as usize]);
        crate::files::replace(
            space,
            &row.path,
            all.as_bytes(),
            &format!("restore-{}", row.id),
        )?;
    } else {
        // Keep the file as it now is: every record found takes its new place; a changed
        // one is marked tampered and its hash follows the new bytes, so the next check
        // passes. A repaired header is not written back, so its hash follows the bytes too.
        let updates: Vec<(String, i64, i64, String, Vec<u8>)> = moved
            .iter()
            .map(|(a, off, _, state, _)| {
                let l = found.iter().find(|g| g.off == *off).map_or(0, |g| g.len);
                (
                    a.clone(),
                    *off as i64,
                    l as i64,
                    state.clone(),
                    space.hash(&text.as_bytes()[*off..*off + l]),
                )
            })
            .collect();
        let tx = space.db.transaction().map_err(fail)?;
        for (a, off, len, state, hash) in &updates {
            tx.execute(
                "UPDATE records SET state = ?4, off = ?2, len = ?3, hash = ?5 WHERE anchor = ?1",
                params![a, off, len, state, hash],
            )
            .map_err(fail)?;
        }
        tx.execute(
            "UPDATE files SET len = ?2 WHERE id = ?1",
            params![row.id, text.len() as i64],
        )
        .map_err(fail)?;
        tx.commit().map_err(fail)?;
    }
    Ok(
        json!({ "conflict_copy": format!("{dir}/{copy}"), "restored": can_restore, "tampered": tampered }),
    )
}
