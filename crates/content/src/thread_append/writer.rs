//! Admit one immutable Markdown record and place it through the existing outbox.
use super::{io, proof, revision::Revision};
use crate::{
    Content, Error, FaultPoint, Item, Request, Result, SpaceId, Summary,
    db::{Header, Space},
    linked_write::Staged,
    now_rfc3339,
    types::{blocked, internal, missing},
};
use calternal_fsio::beneath::Folder;
use rusqlite::{OptionalExtension, TransactionBehavior, params};
use serde_json::{Value, json};
use std::time::Instant;
use super::group::{Group, named};
use uuid::Uuid;

pub(super) const IDENTITY_BYTES: usize = "calternal-id: ".len() + 36 + 1;
pub(super) const RECORD_BYTES: usize = 65536;
const PROOFS: usize = 32;

impl Content {
    /// Read thread metadata and the total staged byte length without loading
    /// any transcript body. One indexed read under the owning space slot.
    pub fn thread_head(&self, space: SpaceId, id: Uuid) -> Result<(Summary, usize)> {
        self.read_thread_head(space, id, None)
    }

    /// Check for a committed retry receipt before preparing a checkpoint. Inputs
    /// already in an open batch wait for that batch to publish on retry.
    pub fn thread_append_receipt(
        &self,
        space: SpaceId,
        id: Uuid,
        account: calternal_types::AccountId,
        request: &str,
    ) -> Result<Option<crate::Receipt>> {
        self.with_space_access(space, Some(id), |s| {
            if s.conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM outbox WHERE account=?1 AND request=?2 AND rowid>?3)",
                params![account.to_string(),request,s.published_append_cutoff()],
                |r| r.get(0),
            ).map_err(internal)? || s.append_group.is_none() {
                self.publish_pending(s)?;
                s.append_recovering = false;
            }
            self.journals.receipt(&s.journal, account, request)
        })
    }

    /// Read the staging head for an output producer. The returned revision
    /// can be waiting for its grouped file sync and must stay inside admission.
    pub fn thread_head_for_append(&self, space: SpaceId, id: Uuid) -> Result<(Summary, usize)> {
        self.read_thread_head(space, id, Some(id))
    }

    fn read_thread_head(
        &self,
        space: SpaceId,
        id: Uuid,
        append_to: Option<Uuid>,
    ) -> Result<(Summary, usize)> {
        self.with_space_access(space, append_to, |s| {
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
        self.read_thread_prefix(space, id, base, None)
    }

    /// Read the fixed document envelope for an output producer staging an
    /// append. The prefix cannot include any of the queued Markdown records.
    pub fn thread_prefix_for_append(
        &self,
        space: SpaceId,
        id: Uuid,
        base: &str,
    ) -> Result<Vec<u8>> {
        self.read_thread_prefix(space, id, base, Some(id))
    }

    fn read_thread_prefix(
        &self,
        space: SpaceId,
        id: Uuid,
        base: &str,
        append_to: Option<Uuid>,
    ) -> Result<Vec<u8>> {
        self.with_space_access(space, append_to, |s| {
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
        let _actor = self.lifecycle.step(request.account.as_uuid())?;
        let (revision, deadline) = self.stage_thread_record(space, id, base, metadata, record, request, receipt)?;
        // Give another prepared checkpoint a bounded chance to join this file
        // barrier. No space lock or async executor thread is held while waiting.
        if let Some(delay) = deadline.checked_duration_since(Instant::now()) {
            std::thread::sleep(delay);
        }
        self.with_space(space, |s| {
            let saved = self.journals.receipt(&s.journal, request.account, request.id)?;
            if !saved.is_some_and(|receipt| receipt.fingerprint == request.fingerprint) {
                return Err(blocked());
            }
            Ok(revision)
        })
    }
    fn stage_thread_record(
        &self,
        space: SpaceId,
        id: Uuid,
        base: &str,
        metadata: Value,
        record: &[u8],
        request: Request<'_>,
        receipt: impl FnOnce(&str) -> Value,
    ) -> Result<(String, Instant)> {
        if record.is_empty() || record.len() > RECORD_BYTES || !record.ends_with(b"\n") {
            return Err(Error::invalid_input(
                "append needs one bounded Markdown record",
            ));
        }
        Content::validate_write_size("threads", &metadata, record)?;
        self.with_space_access(space, Some(id), |s| {
            if s.append_group.is_none() {
                self.publish_pending(s)?;
            }
            let mut head = s.header(id)?;
            if head.kind != "threads" || head.path.starts_with(".trash/") {
                return Err(missing());
            }
            if head.revision != base {
                return Err(Error::conflict("item has a newer revision")
                    .with_details(json!({"reason":"stale_revision","current_revision":head.revision})));
            }
            if head.state == "pending" && s.append_group.is_none() {
                s.place_header(id, head, None, self.fault)?;
                head = s.header(id)?;
            }
            if head.state != "materialized" && s.append_group.is_none() {
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
            let prior = ready.hasher.finalize().to_hex().to_string();
            ready.advance(record);
            let digest = blake3::hash(record);
            let full = ready.hasher.finalize().to_hex().to_string();
            let seq = head.seq + 1;
            let batch_start = s.append_group.as_ref().map_or(seq, |group| group.first);
            let revision = Staged::version(seq);
            let staged = Staged {
                item: Item { id,space,kind:head.kind,metadata,bytes:record.to_vec(),
                    revision:revision.clone(),updated_at:now_rfc3339() },
                seq,path:head.path.clone(),expected:Some(prior.clone()),room:None,
            };
            let cutoff = s.published_append_cutoff();
            let tx = s.conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(internal)?;
            calternal_store::migrations::require_writer_version(
                &tx, super::revision::PATCH_SCHEMA_VERSION,
            ).map_err(internal)?;
            staged.write(&tx, None, &request.account.to_string())?;
            tx.execute(
                "INSERT INTO thread_revision_patches VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
                params![id.to_string(),seq,old.base_seq,old.size as i64,record.len() as i64,
                    witness.as_bytes(),digest.as_bytes(),full,prior,batch_start],
            ).map_err(internal)?;
            staged.outbox(&tx, request, &receipt(&revision))?;
            staged.require_placement(&tx, request)?;
            if cutoff > 0 {
                tx.execute("DELETE FROM outbox WHERE rowid<=?1", [cutoff])
                    .map_err(internal)?;
            }
            tx.commit().map_err(internal)?;
            s.published_appends = None;
            s.pending_since.get_or_insert_with(Instant::now);
            s.append_recovering = true;
            (self.fault)(FaultPoint::AfterCommit);
            let mut group = match s.append_group.take() {
                Some(group) => group,
                None => Group::new(id, seq, &folder, &head.path, request.operation == "threads.checkpoint")?,
            };
            let current = named(&folder, &head.path, &group.file)?;
            if current != stamp {
                return Err(blocked());
            }
            (self.fault)(FaultPoint::BeforePlacement);
            io::place(&mut group.file, stamp.len(), offset as u64, witness, record, Folder::check_append_lease)
                .map_err(proof::file_error)?;
            ready.stamp = named(&folder, &head.path, &group.file)?;
            ready.seq = seq;
            group.last = seq;
            let deadline = group.deadline();
            s.remember_append_proof(id, seq, Ok(ready));
            s.append_group = Some(group);
            Ok((revision, deadline))
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

    pub(super) fn require_proof(
        &mut self,
        id: Uuid,
        head: &Header,
        seq: i64,
        offset: usize,
        actual: proof::Stamp,
    ) -> Result<proof::Ready> {
        self.require_proof_at(id, head, seq, offset, actual, head.file_hash.as_deref().ok_or_else(blocked)?)
    }

    pub(super) fn require_proof_at(
        &mut self,
        id: Uuid,
        head: &Header,
        seq: i64,
        offset: usize,
        actual: proof::Stamp,
        expected: &str,
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
                digest: expected.to_owned(),
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
        let batch = self.conn.prepare_cached(
            "SELECT p.batch_start,i.seq,first.prior_digest
             FROM outbox o JOIN versions v ON v.item=o.item AND v.revision=o.revision
             JOIN items i ON i.id=v.item
             JOIN thread_revision_patches p ON p.item=i.id AND p.seq=i.seq
             JOIN thread_revision_patches first ON first.item=p.item AND first.seq=p.batch_start
             WHERE o.account=?1 AND o.request=?2 AND i.id=?3 AND o.revision=?4
               AND i.mat_state='pending' AND v.seq=p.batch_start"
        ).map_err(internal)?.query_row(params![account,request,item,revision], |r| Ok((
            r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?,
        ))).optional().map_err(internal)?;
        let Some((first, last, prior)) = batch else { return Ok(false) };
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(internal)?;
        let changed = tx.execute(
            "UPDATE items SET (metadata,revision,seq,updated_at) =
                  (SELECT metadata,revision,seq,updated_at FROM versions WHERE item=?1 AND seq=?2-1),
                  mat_state='blocked',old_path=NULL,file_hash=?4
             WHERE id=?1 AND seq=?3",
             params![item,first,last,prior],
        ).map_err(internal)?;
        if changed != 1 {
            return Err(internal("thread append conflict lost its head"));
        }
        tx.execute(
            "DELETE FROM outbox WHERE item=?1 AND revision IN
                (SELECT revision FROM versions WHERE item=?1 AND seq BETWEEN ?2 AND ?3)",
            params![item,first,last],
        ).map_err(internal)?;
        tx.execute(
            "DELETE FROM versions WHERE item=?1 AND seq BETWEEN ?2 AND ?3",
            params![item,first,last],
        ).map_err(internal)?;
        tx.commit().map_err(internal)?;
        Ok(true)
    }

}

#[cfg(test)]
#[path = "writer_tests.rs"]
mod tests;
