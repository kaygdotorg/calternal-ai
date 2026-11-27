//! Two-flush single append, crash adoption and retry receipt promotion.
use super::{Input, Record, Signature, encode, link, seal};
use crate::internal;
use crate::space::{Head, Receipt, Space};
use calternal_types::{Actor, Error, Result};
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use uuid::Uuid;

/// Fast path adapted from the host spike. The checked directory descriptor
/// pins the read handle and procfs reopens the same inode with write access.
#[cfg(target_os = "linux")]
pub(crate) fn append(
    space: &mut Space,
    actor: &Actor,
    head: Head,
    input: Input,
    now: i64,
    receipt: Option<&crate::space::Receipt>,
) -> Result<Value> {
    use std::io::Read;
    use std::os::{
        fd::AsRawFd,
        unix::fs::{FileExt, MetadataExt},
    };

    let mut read = space.open_regular(&head.path).map_err(internal)?;
    let metadata = read.metadata().map_err(internal)?;
    let initial = Signature::from(&metadata);
    if metadata.nlink() != 1 || metadata.len() != head.length {
        return Err(Error::conflict("The record stream has outside changes."));
    }
    if space.seen.get(&head.id) != Some(&initial) {
        let rows = space.records(head.id)?;
        let mut bytes = Vec::with_capacity(head.length as usize);
        (&mut read)
            .take(head.length + 1)
            .read_to_end(&mut bytes)
            .map_err(internal)?;
        if bytes.len() as u64 != head.length
            || !bytes
                .get(..head.prefix as usize)
                .is_some_and(|p| space.hash(p) == head.hash)
            || rows.iter().any(|row| {
                !row.bytes(&bytes)
                    .is_some_and(|data| space.hash(data) == row.hash)
            })
        {
            return Err(Error::conflict("The record stream has outside changes."));
        }
    }
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(format!("/proc/self/fd/{}", read.as_raw_fd()))
        .map_err(internal)?;
    if Signature::from(&file.metadata().map_err(internal)?) != initial {
        return Err(Error::conflict(
            "The record stream changed during the call.",
        ));
    }
    let end = head.length;
    let (next, record, bytes) = make_append(space, &head, actor, input, now, receipt)?;
    let output = append_result(&next, &record);
    let tx = space.db.transaction().map_err(internal)?;
    stamp_append(
        &tx,
        space.retention_ms,
        &next,
        &record,
        actor,
        receipt,
        &output,
    )?;
    if let Err(error) = file
        .write_all_at(&bytes, end)
        .and_then(|_| file.sync_data())
    {
        file.set_len(end)
            .and_then(|_| file.sync_data())
            .map_err(internal)?;
        return Err(internal(error));
    }
    if let Err(error) = tx.commit() {
        space.recovery_needed = true;
        return Err(internal(error));
    }
    let id = next.id;
    space.rows.get_mut(&id).expect("indexed record item").head = next;
    if let Ok(metadata) = file.metadata() {
        space.seen.insert(id, Signature::from(&metadata));
    } else {
        space.seen.remove(&id);
    }
    Ok(output)
}

pub(super) fn make_append(
    space: &Space,
    head: &Head,
    actor: &Actor,
    input: Input,
    now: i64,
    receipt: Option<&Receipt>,
) -> Result<(Head, Record, Vec<u8>)> {
    let kind = space.kind(&head.kind)?;
    let (mut record, mut bytes) = encode(
        space,
        &kind,
        actor,
        input,
        None,
        (head.length, head.version + 1),
        now,
    )?;
    if let Some(receipt) = receipt {
        record.anchor = seal::key_anchor(
            space,
            head.id,
            head.version + 1,
            head.length,
            actor,
            &receipt.proof.gesture,
        )?;
        let newline = bytes
            .iter()
            .position(|byte| *byte == b'\n')
            .expect("record header");
        bytes[newline - 16..newline].copy_from_slice(record.anchor.as_bytes());
    }
    seal::seal(space, head, actor, now, &mut bytes)?;
    record.length = bytes.len() as u64;
    record.hash = space.hash(&bytes);
    let mut next = head.clone();
    next.version += 1;
    next.edited = now;
    next.length += record.length;
    if next.length > crate::space::ITEM_BYTES as u64 {
        return Err(Error::new(
            calternal_types::ErrorCode::TooLarge,
            "The item exceeds 32 MiB.",
        ));
    }
    Ok((next, record, bytes))
}

pub(crate) fn recover_tail(
    space: &mut Space,
    head: &mut Head,
    bytes: &[u8],
    file: &std::fs::File,
    signature: Signature,
) -> Result<()> {
    let tail = &bytes[head.length as usize..];
    let Some(claim) = seal::verify(space, head, tail)? else {
        return Ok(());
    };
    let rows = space.records(head.id)?;
    if rows.iter().any(|row| {
        !row.bytes(bytes)
            .is_some_and(|data| space.hash(data) == row.hash)
    }) {
        return Ok(());
    }
    let record = Record {
        anchor: claim.anchor,
        offset: head.length,
        length: tail.len() as u64,
        author: claim.actor.account.to_string(),
        at: claim.at,
        ordinal: head.version + 1,
        hash: space.hash(tail),
    };
    let mut next = head.clone();
    next.version = record.ordinal;
    next.edited = head.edited.max(record.at);
    next.length += record.length;
    let attribution = serde_json::to_string(&claim.actor).map_err(internal)?;
    // A process crash can leave complete, unflushed bytes in the page
    // cache. Re-establish file durability before committing the recovered receipt.
    if Signature::from(&file.metadata().map_err(internal)?) != signature {
        return Err(Error::conflict(
            "The recovery file changed during verification.",
        ));
    }
    file.sync_data().map_err(internal)?;
    let output = append_result(&next, &record);
    let tx = space.db.transaction().map_err(internal)?;
    stamp_append(
        &tx,
        space.retention_ms,
        &next,
        &record,
        &claim.actor,
        None,
        &output,
    )?;
    if record.at >= calternal_types::now_ms() - space.retention_ms {
        tx.execute(
            "INSERT INTO recovered_appends VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![
                &claim.tag[..],
                head.id.to_string(),
                attribution,
                record.ordinal as i64,
                record.offset as i64,
                record.anchor,
                record.at,
                &claim.effect[..],
                output.to_string(),
                head.kind,
            ],
        )
        .map_err(internal)?;
    }
    tx.commit().map_err(internal)?;
    *head = next;
    Ok(())
}

pub(crate) struct Recovered {
    pub(crate) item: Uuid,
    kind: String,
    tag: Vec<u8>,
    at: i64,
    effect: Vec<u8>,
    result: String,
}

pub(crate) fn recovered(
    space: &Space,
    actor: &Actor,
    receipt: &Receipt,
) -> Result<Option<Recovered>> {
    let attribution = serde_json::to_string(actor).map_err(internal)?;
    let cutoff = calternal_types::now_ms() - space.retention_ms;
    let mut stmt = space
        .db
        .prepare_cached(
            "SELECT tag,item,ordinal,offset,anchor,at,effect,result,kind FROM recovered_appends
         WHERE actor=?1 AND at>=?2",
        )
        .map_err(internal)?;
    let mut rows = stmt.query(params![attribution, cutoff]).map_err(internal)?;
    while let Some(row) = rows.next().map_err(internal)? {
        let item =
            Uuid::parse_str(&row.get::<_, String>(1).map_err(internal)?).map_err(internal)?;
        let ordinal = row.get::<_, i64>(2).map_err(internal)? as u64;
        let offset = row.get::<_, i64>(3).map_err(internal)? as u64;
        let anchor: String = row.get(4).map_err(internal)?;
        if seal::key_anchor(space, item, ordinal, offset, actor, &receipt.proof.gesture)? == anchor
        {
            return Ok(Some(Recovered {
                item,
                kind: row.get(8).map_err(internal)?,
                tag: row.get(0).map_err(internal)?,
                at: row.get(5).map_err(internal)?,
                effect: row.get(6).map_err(internal)?,
                result: row.get(7).map_err(internal)?,
            }));
        }
    }
    Ok(None)
}

pub(crate) fn replay_recovered(
    space: &mut Space,
    actor: &Actor,
    input: Input,
    receipt: &Receipt,
    recovered: Recovered,
) -> Result<Value> {
    let kind = space.kind(&recovered.kind)?;
    let (_, bytes) = encode(space, &kind, actor, input, None, (0, 0), recovered.at)?;
    let body = bytes
        .iter()
        .position(|byte| *byte == b'\n')
        .expect("record header")
        + 1;
    if space.hash(&bytes[body..]) != recovered.effect.as_slice() {
        return Err(Error::request_reused());
    }
    let result = serde_json::from_str(&recovered.result).map_err(internal)?;
    let tx = space.db.transaction().map_err(internal)?;
    receipt.store(&tx, &result, recovered.at)?;
    tx.execute(
        "DELETE FROM recovered_appends WHERE tag=?1",
        [recovered.tag],
    )
    .map_err(internal)?;
    tx.commit().map_err(internal)?;
    Ok(result)
}

pub(super) fn append_result(head: &Head, record: &Record) -> Value {
    crate::result(vec![json!({
        "id":head.id,"version":head.version,"anchor":record.anchor,
        "link":link(&head.path, &record.anchor),
    })])
}

fn stamp_append(
    tx: &Connection,
    retention_ms: i64,
    head: &Head,
    record: &Record,
    actor: &Actor,
    receipt: Option<&Receipt>,
    output: &Value,
) -> Result<()> {
    tx.execute(
        "DELETE FROM history WHERE at<?1",
        [calternal_types::now_ms() - retention_ms],
    )
    .map_err(internal)?;
    record.insert(tx, head.id)?;
    tx.execute(
        "UPDATE head SET data=?2 WHERE id=?1",
        params![
            head.id.to_string(),
            serde_json::to_string(head).map_err(internal)?
        ],
    )
    .map_err(internal)?;
    tx.execute(
        "INSERT INTO history(item,version,actor,action,anchor,at) VALUES(?1,?2,?3,'append record',?4,?5)",
        params![head.id.to_string(), head.version as i64, serde_json::to_string(actor).map_err(internal)?, record.anchor, record.at],
    ).map_err(internal)?;
    if let Some(receipt) = receipt {
        receipt.store(tx, output, record.at)?;
    }
    Ok(())
}
