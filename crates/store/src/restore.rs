//! Finding out the journal went back in time, and what to do about it (#918).
//!
//! It can happen three ways: a backup is restored, the host or VM is rolled
//! back to a snapshot, or a power cut loses the end of the write-ahead log
//! (`synchronous=NORMAL`) and its seqs are handed out again for different
//! entries. The answer is always the same: a **new epoch**, so every device
//! throws away its position and reads again. Nothing else is touched.

use crate::highwater::Mark;
use crate::shared::Shared;
use crate::token::check_at;
use crate::{Error, Result};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use std::path::Path;

/// Reads one value from the store's small metadata table.
fn meta(conn: &Connection, key: &str) -> Result<Option<String>> {
    conn.query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0))
        .optional()
        .map_err(Error::internal)
}

/// Saves one metadata value without changing the journal's commit policy.
fn set_meta(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO meta (key, value) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        [key, value],
    )
    .map_err(Error::internal)?;
    Ok(())
}

/// The head when the current epoch began (0 for the first one).
pub(crate) fn epoch_head(conn: &Connection) -> Result<i64> {
    Ok(meta(conn, "epoch_head")?
        .and_then(|v| v.parse().ok())
        .unwrap_or(0))
}

/// At open, compares the bounded witness read with the indexed journal head.
///
/// - an unreadable or torn witness is missing;
/// - missing after initialization means restored;
/// - missing on first use initializes without rotating;
/// - another epoch, a future position, or a different entry means restored;
/// - a valid older mark is left durable and advanced by the flush worker.
pub(crate) fn restored_at_open(
    conn: &Connection,
    shared: &Shared,
    head: i64,
    mark: Option<Mark>,
    initialized: bool,
) -> Result<bool> {
    if shared.guard.is_none() {
        return Ok(false);
    }
    let epoch = shared.epoch();
    let restored = match &mark {
        None if initialized => true,
        None => false,
        Some(m) if m.epoch != *epoch || m.seq > head => true,
        Some(m) => {
            let check = if m.seq == 0 {
                Some(0)
            } else {
                match shared.last_check(m.seq) {
                    Some(c) => Some(c),
                    None => check_at(conn, m.seq)?,
                }
            };
            check != Some(m.check)
        }
    };
    if !restored {
        match mark.as_ref().filter(|m| m.epoch == *epoch) {
            Some(m) if m.seq == head => {
                let mut written = shared.file.lock().unwrap_or_else(|p| p.into_inner());
                written.epoch.clone_from(&m.epoch);
                written.seq = m.seq;
                written.check = m.check;
            }
            Some(_) => {
                // The older mark already protects the journal tail it covers.
                // Refresh its high-water on the normal idle/staleness worker.
                shared.flush_activity.observe_head_change();
            }
            None => {
                // First use has no prior witness to protect the store. Make this
                // mark durable before setting the flag or returning signed tokens.
                write_now(conn, shared)?;
            }
        }
        if !initialized {
            set_meta(conn, "highwater", "1")?;
        }
    }
    Ok(restored)
}

/// Starts a new epoch. Safe to ask twice: if the epoch is no longer
/// `observed`, someone already did it.
///
/// A fresh open can pass its already-checked head and entry check. The writer
/// passes `None` because it reads the database after a restore proof.
///
/// Order matters. The new epoch is committed with FULL sync first, then the
/// high-water file is durably written through its current path. Only then do
/// new tokens go out, so a replaced inode cannot authorize the new epoch.
pub(crate) fn rotate(
    conn: &mut Connection,
    shared: &Shared,
    observed: &str,
    known_head: Option<(i64, u32)>,
) -> Result<()> {
    let mut written = shared.file.lock().unwrap_or_else(|p| p.into_inner());
    if *shared.epoch() != *observed {
        return Ok(());
    }
    let stored_epoch = meta(conn, "epoch")?.unwrap_or_else(|| observed.to_owned());
    let (new, head, epoch_head) = if stored_epoch != observed {
        // The database commit succeeded on an earlier attempt but the
        // witness write did not. Complete that same rotation instead of
        // creating another epoch.
        let head = match known_head {
            Some((head, _)) => head,
            None => crate::schema::read_head(conn, false)?.0,
        };
        (stored_epoch, head, epoch_head(conn)?)
    } else {
        let new = uuid::Uuid::new_v4().simple().to_string()[..12].to_owned();
        conn.pragma_update(None, "synchronous", "FULL")
            .map_err(Error::internal)?;
        let committed = (|| -> Result<(i64, i64)> {
            let tx = conn.transaction().map_err(Error::internal)?;
            let head = match known_head {
                Some((head, _)) => head,
                None => crate::schema::read_head(&tx, false)?.0,
            };
            set_meta(&tx, "epoch", &new)?;
            set_meta(&tx, "epoch_head", &head.to_string())?;
            set_meta(&tx, "highwater", "1")?;
            tx.commit().map_err(Error::internal)?;
            Ok((head, head))
        })();
        if let Err(e) = conn.pragma_update(None, "synchronous", "NORMAL") {
            tracing::warn!(%e, "journal retains stronger sync after a rotation");
        }
        let (head, epoch_head) = committed?;
        (new, head, epoch_head)
    };
    let check = match known_head.filter(|(known, _)| *known == head) {
        Some((_, check)) => check,
        None if head > 0 => check_at(conn, head)?.unwrap_or(0),
        None => 0,
    };
    // The database may already hold this epoch, but nobody can use it until
    // its external witness has reached durable storage.
    write_mark(shared, &new, head, check).map_err(Error::internal)?;
    shared
        .head
        .store(head, std::sync::atomic::Ordering::Release);
    shared
        .epoch_head
        .store(epoch_head, std::sync::atomic::Ordering::Release);
    shared.set_last(head, check);
    shared.set_epoch(&new);
    written.epoch = new.clone();
    written.seq = head;
    written.check = check;
    shared.flush_activity.mark_written();
    shared.restored.send_modify(|n| *n += 1);
    tracing::warn!(head, "the save log went back in time: new epoch");
    Ok(())
}

/// Durably writes the new epoch's witness before the epoch is published.
fn write_mark(shared: &Shared, epoch: &str, seq: i64, check: u32) -> std::io::Result<()> {
    match &shared.guard {
        Some(g) => g
            .head
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .write(&Mark {
                epoch: epoch.to_owned(),
                seq,
                check,
            }),
        None => Ok(()),
    }
}

/// Writes the file for the current epoch and head during startup or rotation.
/// The shared lock keeps the record from landing after a newer rotation.
pub(crate) fn write_now(conn: &Connection, shared: &Shared) -> Result<()> {
    let mut written = shared.file.lock().unwrap_or_else(|p| p.into_inner());
    let epoch = shared.epoch();
    let head = shared.head_now();
    if written.epoch == *epoch && written.seq == head {
        return Ok(());
    }
    let check = match shared.last_check(head) {
        Some(c) => c,
        None if head > 0 => check_at(conn, head)?.unwrap_or(0),
        None => 0,
    };
    write_mark(shared, &epoch, head, check).map_err(Error::internal)?;
    written.epoch = epoch.to_string();
    written.seq = head;
    written.check = check;
    shared.flush_activity.mark_written();
    Ok(())
}

/// Writes the current head from an indexed read on the flush task.
///
/// It takes no lock used by commits. The head's entry check comes from the
/// last durable witness, the cached head, or the token the writer cached for
/// that entry; only when all three miss does it open SQLite for one
/// primary-key lookup.
pub(crate) fn flush(shared: &Shared, store_path: &Path) -> Result<()> {
    flush_with_check(shared, |head| check_at_path(store_path, head))
}

/// Flushes after the writer drains, reusing its connection on a separate worker.
/// This keeps shutdown from reopening SQLite after its final WAL cleanup.
pub(crate) fn flush_after_writer(conn: &Connection, shared: &Shared) -> Result<()> {
    flush_with_check(shared, |head| Ok(check_at(conn, head)?.unwrap_or(0)))
}

/// Writes the latest witness after obtaining its entry check on the flush worker.
fn flush_with_check(shared: &Shared, check_head: impl FnOnce(i64) -> Result<u32>) -> Result<()> {
    let mut written = shared.file.lock().unwrap_or_else(|p| p.into_inner());
    let epoch = shared.epoch().to_string();
    let head = shared.head_now();
    let check = if written.epoch == epoch && written.seq == head {
        written.check
    } else if head == 0 {
        0
    } else if let Some(check) = shared.last_check(head) {
        check
    } else if let Some(check) = shared.issued_check(&epoch, head) {
        check
    } else {
        check_head(head)?
    };
    let mark = Mark {
        epoch,
        seq: head,
        check,
    };
    shared.set_last(head, check);
    if written.epoch == mark.epoch && written.seq == mark.seq && written.check == mark.check {
        if let Some(guard) = &shared.guard {
            guard
                .head
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .verify_path_or_recover(&mark)
                .map_err(Error::internal)?;
        }
        return Ok(());
    }
    write_mark(shared, &mark.epoch, mark.seq, mark.check).map_err(Error::internal)?;
    written.epoch = mark.epoch;
    written.seq = mark.seq;
    written.check = mark.check;
    Ok(())
}

/// Looks up one journal entry's check without taking a writer or save lock.
fn check_at_path(path: &Path, seq: i64) -> Result<u32> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(Error::internal)?;
    Ok(check_at(&conn, seq)?.unwrap_or(0))
}
