//! Two-flush single append, crash adoption and retry receipt promotion.
use super::{Input, Record, Signature, encode, link, seal};
use crate::internal;
use crate::space::{Head, Receipt, Space};
use calternal_types::{Actor, Error, Result};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

/// Fast path adapted from the host spike. The checked directory descriptor
/// pins the read handle and procfs reopens the same inode with write access.
/// The operator keeps the storage tree exclusive to the host user during
/// the call. The inode must also exclude writes by its group and other users.
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
    use std::os::unix::fs::{FileExt, MetadataExt};

    let mut read = space.open_regular(&head.path).map_err(internal)?;
    let metadata = read.metadata().map_err(internal)?;
    let initial = Signature::from(&metadata);
    if metadata.uid() != space.storage_owner || metadata.mode() & 0o022 != 0 {
        return Err(Error::forbidden("The host must have exclusive write access to the record file."));
    }
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
    if let Some(receipt) = receipt {
        receipt.check_capacity(&space.db)?;
    }
    let file = writable(&read, initial)?;
    let end = head.length;
    let (next, record, bytes) = make_append(space, &head, actor, input, now, receipt)?;
    let output = append_result(&next, &record);
    let pending = Pending { next, record, actor, receipt };

    // The FULL WAL commit is the durable replay source. On any subsequent
    // error, even an uncertain commit, the next call must recover first.
    space.recovery_needed = true;
    pending.stage(&space.db, &bytes)?;
    file.write_all_at(&bytes, end)
        .and_then(|_| file.sync_data())
        .map_err(internal)?;
    pending.finalize_warmed(space, &output)?;
    space.recovery_needed = false;
    let next = pending.next;
    let id = next.id;
    space.rows.get_mut(&id).expect("indexed record item").head = next;
    if let Ok(metadata) = file.metadata() {
        space.seen.insert(id, Signature::from(&metadata));
    } else {
        space.seen.remove(&id);
    }
    Ok(output)
}

#[cfg(target_os = "linux")]
fn writable(read: &std::fs::File, initial: Signature) -> Result<std::fs::File> {
    use std::os::fd::AsRawFd;

    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(format!("/proc/self/fd/{}", read.as_raw_fd()))
        .map_err(internal)?;
    if Signature::from(&file.metadata().map_err(internal)?) != initial {
        return Err(Error::conflict("The record stream changed during the call."));
    }
    Ok(file)
}

#[derive(Serialize, Deserialize)]
pub(super) struct Pending<A, R> {
    pub(super) next: Head,
    pub(super) record: Record,
    pub(super) actor: A,
    pub(super) receipt: Option<R>,
}

impl Pending<&Actor, &Receipt> {
    pub(super) fn stage(&self, db: &Connection, bytes: &[u8]) -> Result<()> {
        db.execute(
            "INSERT INTO pending_append VALUES(1,?1,?2)",
            params![serde_json::to_string(self).map_err(internal)?, bytes],
        )
        .map_err(internal)?;
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn finalize_warmed(&self, space: &mut Space, output: &Value) -> Result<()> {
        // WAL NORMAL preserves atomicity. If this commit disappears in a
        // power loss the preceding FULL commit and synced file can replay it.
        // Keep autocheckpointing on the next FULL commit, outside this phase.
        let db = &mut space.db;
        let checkpoint: i64 = db.pragma_query_value(None, "wal_autocheckpoint", |r| r.get(0))
            .map_err(internal)?;
        db.pragma_update(None, "wal_autocheckpoint", 0).map_err(internal)?;
        let result = (|| {
            db.pragma_update(None, "synchronous", "NORMAL").map_err(internal)?;
            self.finalize(db, space.retention_ms, output)
        })();
        let sync = db.pragma_update(None, "synchronous", "FULL").map_err(internal);
        let checkpoint = db.pragma_update(None, "wal_autocheckpoint", checkpoint).map_err(internal);
        result.and(sync).and(checkpoint)
    }

    fn finalize(&self, db: &mut Connection, retention: i64, output: &Value) -> Result<()> {
        let tx = db.transaction().map_err(internal)?;
        stamp_append(&tx, retention, &self.next, &self.record, self.actor, self.receipt, output)?;
        tx.execute("DELETE FROM pending_append WHERE id=1", []).map_err(internal)?;
        tx.commit().map_err(internal)
    }
}

/// Resolve a durable preparation before exposing the reopened space.
pub(crate) fn recover_pending(space: &mut Space) -> Result<()> {
    let pending: Option<(String, Vec<u8>)> = space.db.query_row(
        "SELECT data,bytes FROM pending_append WHERE id=1",
        [], |row| Ok((row.get(0)?, row.get(1)?)),
    ).optional().map_err(internal)?;
    let Some((data, expected)) = pending else { return Ok(()); };
    let pending: Pending<Actor, Receipt> = serde_json::from_str(&data).map_err(internal)?;
    let prior: String = space.db.query_row("SELECT data FROM head WHERE id=?1",
        [pending.next.id.to_string()], |row| row.get(0)).map_err(internal)?;
    let prior: Head = serde_json::from_str(&prior).map_err(internal)?;
    if prior.length != pending.record.offset || prior.version + 1 != pending.next.version {
        return Err(internal("Pending append has a different ledger head"));
    }
    let file = space.open_regular(&prior.path).map_err(internal)?;
    use std::os::unix::fs::MetadataExt;
    let metadata = file.metadata().map_err(internal)?;
    if metadata.uid() != space.storage_owner || metadata.mode() & 0o022 != 0 {
        return Err(Error::forbidden("The host must have exclusive write access to the record file."));
    }
    let signature = Signature::from(&metadata);
    let bytes = crate::space::bounded_read(file.try_clone().map_err(internal)?, crate::space::ITEM_BYTES)?;
    let prefix = bytes.get(..prior.prefix as usize);
    let tail = bytes.get(prior.length as usize..);
    if !prefix.is_some_and(|prefix| space.hash(prefix) == prior.hash)
        || tail.is_none()
        || space.records(prior.id)?.iter().any(|row| {
            !row.bytes(&bytes).is_some_and(|bytes| space.hash(bytes) == row.hash)
        })
    {
        return Err(Error::conflict("The pending append's ledger prefix changed on disk."));
    }
    let tail = tail.expect("verified length");
    if Signature::from(&file.metadata().map_err(internal)?) != signature {
        return Err(Error::conflict("The recovery file changed during verification."));
    }
    if tail.starts_with(&expected) {
        file.sync_data().map_err(internal)?;
        let pending = Pending {
            next: pending.next,
            record: pending.record,
            actor: &pending.actor,
            receipt: pending.receipt.as_ref(),
        };
        let output = append_result(&pending.next, &pending.record);
        return pending.finalize(&mut space.db, space.retention_ms, &output);
    }
    if expected.starts_with(tail) {
        #[cfg(target_os = "linux")]
        if !tail.is_empty() {
            writable(&file, signature)?.set_len(prior.length).map_err(internal)?;
        }
        #[cfg(not(target_os = "linux"))]
        if !tail.is_empty() {
            return Err(Error::conflict("Interrupted append recovery requires the Linux host."));
        }
        file.sync_data().map_err(internal)?;
    }
    // Nonmatching bytes belong to the outside-edit path. Keep them intact.
    space.db.execute("DELETE FROM pending_append WHERE id=1", []).map_err(internal)?;
    Ok(())
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
    let fresh = claim.at >= calternal_types::now_ms() - space.retention_ms;
    if fresh && !crate::space::receipt_capacity(&space.db)? {
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
    if fresh {
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
    tx.execute(
        "DELETE FROM recovered_appends WHERE tag=?1",
        [recovered.tag],
    )
    .map_err(internal)?;
    receipt.store(&tx, &result, recovered.at)?;
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
