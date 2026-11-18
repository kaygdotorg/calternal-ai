//! Admit one immutable Markdown record and place it through the existing outbox.
use super::{io, proof, revision::Revision};
use crate::{
    Content, Error, FaultPoint, Item, Request, Result, SpaceId, Summary,
    db::{Header, Space, bookkeeping},
    linked_write::Staged,
    now_rfc3339,
    types::{blocked, internal, missing},
};
use calternal_fsio::beneath::Folder;
use rusqlite::{OptionalExtension, TransactionBehavior, params};
use serde_json::{Value, json};
use std::{fs::File, time::Instant};
use uuid::Uuid;

const IDENTITY_BYTES: usize = "calternal-id: ".len() + 36 + 1;
const RECORD_BYTES: usize = 65536;
const PROOFS: usize = 32;

impl Content {
    /// Read thread metadata and the total staged byte length without loading
    /// any transcript body. One indexed read under the owning space slot.
    pub fn thread_head(&self, space: SpaceId, id: Uuid) -> Result<(Summary, usize)> {
        self.with_space(space, |s| {
            let mut stmt = s.conn.prepare_cached(
                "SELECT i.path,i.metadata,i.revision,i.updated_at,coalesce(p.offset,0)+length(v.bytes)
                 FROM items i JOIN versions v ON v.item=i.id AND v.seq=i.seq
                 LEFT JOIN thread_revision_patches p ON p.item=v.item AND p.seq=v.seq
                 WHERE i.id=?1 AND i.kind='threads' AND i.mat_state NOT IN ('purged','purging')",
            ).map_err(internal)?;
            let (path, metadata, revision, updated_at, len) = stmt.query_row([id.to_string()], |r| Ok((
                r.get::<_,String>(0)?, r.get::<_,String>(1)?, r.get::<_,String>(2)?,
                r.get::<_,String>(3)?, r.get::<_,u32>(4)?,
            ))).optional().map_err(internal)?.ok_or_else(missing)?;
            drop(stmt);
            s.schedule_thread_proof(id, len as usize);
            Ok((Summary {id,space,kind:"threads".into(),path,metadata:serde_json::from_str(&metadata).map_err(internal)?,
                revision,updated_at}, len as usize))
        })
    }

    /// Read a bounded prefix from the named current thread revision. It
    /// addresses staged Markdown, before placement inserts the identity line.
    pub fn thread_prefix(&self, space: SpaceId, id: Uuid, base: &str) -> Result<Vec<u8>> {
        self.with_space(space, |s| {
            let head = s.header(id)?;
            if head.kind != "threads" || head.path.starts_with(".trash/") {
                return Err(missing());
            }
            if head.revision != base {
                return Err(Error::conflict("item has a newer revision").with_details(
                    json!({"reason":"stale_revision","current_revision":head.revision}),
                ));
            }
            let revision = Revision::get(&s.conn, id.to_string(), head.seq)?.ok_or_else(missing)?;
            revision.read(&s.conn, 0..revision.size.min(8192))
        })
    }

    /// Append one newline-terminated Markdown record after checking the current
    /// revision. Stores just the fragment, receipt and recovery intent in FULL
    /// content, checks 4 KiB of the named file, writes the new record once, then
    /// syncs and publishes through the existing placed-outbox barrier.
    ///
    /// A missing or changed file proof schedules background verification and
    /// returns a retryable error. External edits and ambiguous recovery are
    /// preserved. The caller supplies authorization and validates record framing.
    pub fn append_thread_record(
        &self,
        space: SpaceId,
        id: Uuid,
        base: &str,
        metadata: Value,
        record: &[u8],
        request: Request<'_>,
        receipt: impl FnOnce(&str) -> Value,
    ) -> Result<String> {
        if record.is_empty() || record.len() > RECORD_BYTES || !record.ends_with(b"\n") {
            return Err(Error::invalid_input(
                "append needs one bounded Markdown record",
            ));
        }
        Content::validate_write_size("threads", &metadata, record)?;
        let _actor = self.lifecycle.step(request.account.as_uuid())?;
        self.with_space(space, |s| {
            self.publish_pending(s)?;
            let mut head = s.header(id)?;
            if head.kind != "threads" || head.path.starts_with(".trash/") {
                return Err(missing());
            }
            if head.revision != base {
                return Err(Error::conflict("item has a newer revision")
                    .with_details(json!({"reason":"stale_revision","current_revision":head.revision})));
            }
            if head.state == "pending" {
                s.place_header(id, head, None, self.fault)?;
                head = s.header(id)?;
            }
            if head.state != "materialized" {
                return Err(blocked());
            }
            let old = Revision::get(&s.conn, id.to_string(), head.seq)?.ok_or_else(missing)?;
            let offset = old.size + IDENTITY_BYTES;
            if offset + record.len() > crate::MAX_THREAD_BYTES {
                return Err(Error::conflict("Thread reached its 16 MiB limit. Earlier output is saved; the latest output was not saved. Start a new thread to continue.")
                    .with_details(json!({"reason":"thread_full"})));
            }
            let folder = Folder::open_existing(&s.files).map_err(internal)?;
            let stamp = proof::Stamp::new(&folder.file_identity(&head.path).map_err(proof::file_error)?);
            let mut ready = s.require_proof(id, &head, head.seq, offset, stamp)?;
            if ready.stamp.len() != offset as u64 {
                return Err(blocked());
            }
            if self.journals.receipt(&s.journal, request.account, request.id)?.is_some() {
                return Err(Error::request_reused());
            }
            let witness = blake3::hash(&ready.tail);
            ready.advance(record);
            let digest = blake3::hash(record);
            let full = ready.hasher.finalize().to_hex().to_string();
            let seq = head.seq + 1;
            let revision = Staged::version(seq);
            let staged = Staged {
                item: Item { id,space,kind:head.kind,metadata,bytes:record.to_vec(),
                    revision:revision.clone(),updated_at:now_rfc3339() },
                seq,path:head.path,expected:head.file_hash,room:None,
            };
            let tx = s.conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(internal)?;
            calternal_store::migrations::require_writer_version(
                &tx, super::revision::PATCH_SCHEMA_VERSION,
            ).map_err(internal)?;
            staged.write(&tx, None, &request.account.to_string())?;
            tx.execute(
                "INSERT INTO thread_revision_patches VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                params![id.to_string(),seq,old.base_seq,old.size as i64,record.len() as i64,
                    witness.as_bytes(),digest.as_bytes(),full],
            ).map_err(internal)?;
            staged.outbox(&tx, request, &receipt(&revision))?;
            staged.require_placement(&tx, request)?;
            tx.commit().map_err(internal)?;
            s.pending_since.get_or_insert_with(Instant::now);
            s.append_recovering = true;
            (self.fault)(FaultPoint::AfterCommit);
            self.publish_pending(s)?;
            s.append_recovering = false;
            if s.header(id)?.revision != revision {
                return Err(blocked());
            }
            Ok(revision)
        })
    }
}

impl Space {
    pub(crate) fn schedule_thread_proof(&mut self, id: Uuid, size: usize) {
        if self.append_proofs.contains_key(&id) {
            return;
        }
        let _ = (|| -> Result<()> {
            let head = self.header(id)?;
            if head.kind != "threads"
                || head.state != "materialized"
                || head.path.starts_with(".trash/")
            {
                return Ok(());
            }
            let folder = Folder::open_existing(&self.files).map_err(internal)?;
            let stamp = proof::Stamp::new(
                &folder
                    .file_identity(&head.path)
                    .map_err(proof::file_error)?,
            );
            self.require_proof(id, &head, head.seq, size + IDENTITY_BYTES, stamp)
                .map(|_| ())
        })();
    }

    fn require_proof(
        &mut self,
        id: Uuid,
        head: &Header,
        seq: i64,
        offset: usize,
        actual: proof::Stamp,
    ) -> Result<proof::Ready> {
        if let Some((seen, result)) = self.append_proofs.get(&id)
            && *seen == seq
        {
            let ready = result.as_ref().map_err(Clone::clone)?;
            if ready.stamp == actual {
                return Ok(ready.clone());
            }
        }
        if self.append_task.is_none() {
            self.append_task = Some(proof::Task {
                id,
                seq,
                root: self.files.clone(),
                path: head.path.clone(),
                prefix: offset as u64,
                digest: head.file_hash.clone().ok_or_else(blocked)?,
            });
        }
        Err(proof::verifying())
    }

    pub(crate) fn remember_append_proof(
        &mut self,
        id: Uuid,
        seq: i64,
        verified: Result<proof::Ready>,
    ) {
        if self.append_proofs.len() >= PROOFS {
            self.append_proofs.clear();
        }
        if !verified.as_ref().is_err_and(proof::pending) {
            self.append_proofs.insert(id, (seq, verified));
        }
    }

    /// Discard only an unpublished append whose outside file changed. The
    /// previous revision remains authoritative and the outside bytes remain
    /// in place. This lets unrelated outbox work proceed after the conflict.
    pub(crate) fn abort_thread_patch(
        &mut self,
        account: &str,
        request: &str,
        item: &str,
        revision: &str,
    ) -> Result<bool> {
        let seq: Option<i64> = self
            .conn
            .prepare_cached(
                "SELECT p.seq FROM thread_revision_patches p
             JOIN items i ON i.id=p.item AND i.seq=p.seq
             WHERE p.item=?1 AND i.revision=?2 AND i.mat_state='pending'",
            )
            .map_err(internal)?
            .query_row(params![item, revision], |r| r.get(0))
            .optional()
            .map_err(internal)?;
        let Some(seq) = seq else { return Ok(false) };
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(internal)?;
        let changed = tx.execute(
            "UPDATE items SET (metadata,revision,seq,updated_at) =
                  (SELECT metadata,revision,seq,updated_at FROM versions WHERE item=?1 AND seq=?2-1),
                  mat_state='blocked',old_path=NULL
             WHERE id=?1 AND seq=?2",
             params![item,seq],
        ).map_err(internal)?;
        if changed != 1 {
            return Err(internal("thread append conflict lost its head"));
        }
        tx.execute(
            "DELETE FROM outbox WHERE account=?1 AND request=?2 AND item=?3 AND revision=?4",
            params![account, request, item, revision],
        )
        .map_err(internal)?;
        tx.execute(
            "DELETE FROM versions WHERE item=?1 AND seq=?2",
            params![item, seq],
        )
        .map_err(internal)?;
        tx.commit().map_err(internal)?;
        Ok(true)
    }

    pub(crate) fn place_thread_patch(
        &mut self,
        id: Uuid,
        head: &Header,
        fault: fn(FaultPoint),
    ) -> Result<bool> {
        if head.kind != "threads" {
            return Ok(false);
        }
        let patch = self
            .conn
            .prepare_cached(
                "SELECT p.offset,p.witness,p.digest,p.file_digest,v.bytes
             FROM thread_revision_patches p JOIN versions v USING(item,seq)
             WHERE p.item=?1 AND p.seq=?2",
            )
            .map_err(internal)?
            .query_row(params![id.to_string(), head.seq], |r| {
                Ok((
                    r.get::<_, u32>(0)?,
                    r.get::<_, [u8; 32]>(1)?,
                    r.get::<_, [u8; 32]>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, Vec<u8>>(4)?,
                ))
            })
            .optional()
            .map_err(internal)?;
        let Some((offset, witness, digest, full, record)) = patch else {
            return Ok(false);
        };
        if blake3::hash(&record).as_bytes() != &digest || record.len() > RECORD_BYTES {
            return Err(internal("thread append record checksum differs"));
        }
        let offset = offset as usize + IDENTITY_BYTES;
        let folder = Folder::open_existing(&self.files).map_err(internal)?;
        let mut file = folder.lease_append(&head.path).map_err(proof::file_error)?;
        let stamp = proof::Stamp::new(&file.metadata().map_err(internal)?);
        let mut ready = self.require_proof(id, head, head.seq - 1, offset, stamp.clone())?;
        // Verify the name as well as the leased descriptor: a replacement
        // inode can be installed without opening the inode held by the lease.
        named(&folder, &head.path, &file)?;
        fault(FaultPoint::BeforePlacement);
        io::place(
            &mut file,
            stamp.len(),
            offset as u64,
            witness.into(),
            &record,
            Folder::check_append_lease,
        )
        .map_err(proof::file_error)?;
        file.sync_all().map_err(internal)?;
        ready.advance(&record);
        if ready.hasher.finalize().to_hex().as_str() != full {
            return Err(internal("thread append file checksum differs"));
        }
        ready.stamp = named(&folder, &head.path, &file)?;
        ready.seq = head.seq;
        fault(FaultPoint::AfterPlacement);
        // The outbox survives a lost NORMAL bookkeeping transaction. Its
        // prefix witness and prepared bytes make matching suffixes replayable.
        bookkeeping(&self.conn, || {
            self.conn
                .execute(
                    "UPDATE items SET mat_state='materialized',file_hash=?1,old_path=NULL
                WHERE id=?2 AND seq=?3",
                    params![full, id.to_string(), head.seq],
                )
                .map_err(internal)?;
            Ok(())
        })?;
        self.remember_append_proof(id, head.seq, Ok(ready));
        Ok(true)
    }
}

fn named(folder: &Folder, path: &str, file: &File) -> Result<proof::Stamp> {
    let stamp = proof::Stamp::new(&file.metadata().map_err(internal)?);
    if proof::Stamp::new(&folder.file_identity(path).map_err(proof::file_error)?) != stamp {
        return Err(blocked());
    }
    Ok(stamp)
}

#[cfg(test)]
#[path = "writer_tests.rs"]
mod tests;
