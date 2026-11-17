//! Content/control mutation admission, shared receipts and guarded Trash placement.
use super::*;
use crate::linked_write::{self, Retarget, Staged};
use rusqlite::TransactionBehavior;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Destination {
    Current,
    Trash,
    Restore,
}

impl Content {
    /// Changes one per-space control value and records the request's receipt, as
    /// one admission: a stored receipt for the same request replays its response
    /// (or is a conflict for different input) without calling `change`. `change`
    /// gets the current value and returns the new value and the response (an
    /// error writes nothing); both
    /// commit with an outbox intent in one FULL transaction, then the receipt is
    /// published to the space's journal. Callers authorize the space first.
    /// Cost: the space's lock (this space only), one receipt read, one FULL
    /// content transaction, one FULL journal commit and one FULL outbox retirement.
    pub fn set_control(
        &self,
        space: SpaceId,
        request: Request<'_>,
        key: &str,
        change: impl FnOnce(Option<Value>) -> Result<(Value, Value)>,
    ) -> Result<Value> {
        self.with_space(space, |s| {
            self.publish_pending(s)?;
            if let Some(saved) = self.journals.receipt(&s.journal, request.account, request.id)? {
                if saved.operation != request.operation || saved.fingerprint != request.fingerprint {
                    return Err(Error::request_reused());
                }
                return Ok(saved.result);
            }
            let tx = s
                .conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(internal)?;
            let current = tx
                .query_row("SELECT value FROM controls WHERE key=?1", [key], |r| {
                    r.get::<_, String>(0)
                })
                .optional()
                .map_err(internal)?
                .map(|v| serde_json::from_str(&v).map_err(internal))
                .transpose()?;
            let (next, response) = change(current)?;
            tx.execute(
                "INSERT INTO controls VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                params![key, next.to_string()],
            )
            .map_err(internal)?;
            tx.execute(
                "INSERT INTO outbox VALUES(?1,?2,?3,?4,NULL,'control',?5)",
                params![
                    request.account.to_string(),
                    request.id,
                    request.operation,
                    request.fingerprint,
                    response.to_string()
                ],
            )
            .map_err(internal)?;
            tx.commit().map_err(internal)?;
            self.publish_pending(s)?;
            Ok(response)
        })
    }
    /// Admits one write whose effect lives outside this crate (a directory change
    /// in a space) through the space's receipts, the same admission as every
    /// content and control write there: a stored receipt replays (or conflicts),
    /// otherwise `run` executes and its response becomes the receipt.
    ///
    /// The space's lock is held across `run` so that one request ID cannot start
    /// two different writes in the space at once; `run` must be short and must
    /// not touch this space's content. If the process stops after `run` commits
    /// and before the receipt does, a retry calls `run` again, so `run` must
    /// replay its own committed result (the directory keeps an atomic receipt).
    /// Cost: the space's lock, one receipt read, `run`, and one FULL journal commit.
    pub fn admit(
        &self,
        space: SpaceId,
        request: Request<'_>,
        run: impl FnOnce() -> Result<Value>,
    ) -> Result<Value> {
        self.admit_checked(space, request, run)
    }
    /// Admit a control mutation while holding the target item's space lock. The
    /// callback sees the current head before replay and may check its lifecycle;
    /// it must not re-enter content. A crash after its commit replays its own
    /// idempotent result. This fences deletion against turn admission without a
    /// second lock/store. Cost: `admit` plus one indexed head read; the lock spans
    /// the callback's job fsync to keep the lifecycle decision atomic within a space.
    /// Missing heads, reused request identities and callback/storage failures are
    /// returned without acknowledging a new control mutation.
    pub fn admit_item(
        &self,
        space: SpaceId,
        id: Uuid,
        request: Request<'_>,
        run: impl FnOnce(&Item, Option<Value>) -> Result<Value>,
    ) -> Result<Value> {
        self.with_space(space, |s| {
            self.publish_pending(s)?;
            let current = s.get(id)?;
            let saved = self
                .journals
                .receipt(&s.journal, request.account, request.id)?;
            if saved.as_ref().is_some_and(|r| {
                r.operation != request.operation || r.fingerprint != request.fingerprint
            }) {
                return Err(Error::request_reused());
            }
            let result = run(&current, saved.as_ref().map(|r| r.result.clone()))?;
            if saved.is_none() {
                self.journals.publish(
                    &s.journal,
                    request.account,
                    request.id,
                    request.operation,
                    request.fingerprint,
                    None,
                    "control",
                    result.clone(),
                )?;
            }
            Ok(result)
        })
    }
    fn admit_checked(
        &self,
        space: SpaceId,
        request: Request<'_>,
        run: impl FnOnce() -> Result<Value>,
    ) -> Result<Value> {
        self.with_space(space, |s| {
            self.publish_pending(s)?;
            if let Some(saved) = self
                .journals
                .receipt(&s.journal, request.account, request.id)?
            {
                if saved.operation != request.operation || saved.fingerprint != request.fingerprint
                {
                    return Err(Error::request_reused());
                }
                return Ok(saved.result);
            }
            let response = run()?;
            self.journals.publish(
                &s.journal,
                request.account,
                request.id,
                request.operation,
                request.fingerprint,
                None,
                "control",
                response.clone(),
            )?;
            Ok(response)
        })
    }
    /// Commits generic bytes/metadata and journal publication before acknowledgement.
    /// The lifecycle worker materializes userdata within ADR 0062's one-second
    /// maximum; a missed deadline refuses further writes until placement recovers.
    /// `id=None` creates a UUIDv7; updates require the current base token. The host
    /// supplies the operation response encoder and checks authority before calling.
    /// Request IDs are unique per (account, space); a reused one is a conflict.
    /// Cost: cache/space locks, bounded metadata/byte
    /// allocations, indexed head/name checks, one FULL content transaction, a catalog
    /// route insert for a new item, one FULL journal receipt/event commit,
    /// one FULL outbox retirement (prevents resurrecting expired receipts).
    /// File+directory fsync and NORMAL placement
    /// bookkeeping follow on the lifecycle worker. An update to a still-pending
    /// item first finishes that item's placement to retain outside-edit guards.
    /// First name allocation loads the directory-name cache; warm Linux allocations
    /// drain inherited name-watch events. Other platforms rescan that one directory.
    /// No unrelated-space history/content reads. This is durability, not a perf gate.
    #[allow(clippy::too_many_arguments)]
    pub fn write(
        &self,
        space: SpaceId,
        id: Option<Uuid>,
        kind: &str,
        base: Option<&str>,
        metadata: Value,
        bytes: &[u8],
        request: Request<'_>,
        result: impl FnOnce(&Item) -> Value,
    ) -> Result<Item> {
        self.write_placed(
            space,
            id,
            kind,
            base,
            metadata,
            bytes,
            request,
            result,
            Destination::Current,
            None,
            || Ok(()),
        )
    }

    /// Move an existing item into its space's Trash through the normal version,
    /// journal and guarded placement outbox. Identity and history remain resolvable;
    /// callers authorize deletion and provide tombstone metadata. Outside edits and
    /// stale revisions fail closed. `guard` runs after revision validation under
    /// this space's content lock, before mutation. It must not re-enter content;
    /// holding the lock across a job check fences admission against deletion.
    /// Authority cleanup follows the committed tombstone and must be replayable;
    /// a failed content commit must not forget a saved narrower mode.
    /// Trash placement completes before authority cleanup, using file/directory syncs.
    /// Callback failures preserve the current head. Cost: `write` plus
    /// the caller's bounded guard; no unrelated space waits on this lock.
    pub fn trash(
        &self,
        item: &Item,
        metadata: Value,
        bytes: &[u8],
        request: Request<'_>,
        result: impl FnOnce(&Item) -> Value,
        guard: impl FnOnce() -> Result<()>,
    ) -> Result<Item> {
        self.write_placed(
            item.space,
            Some(item.id),
            &item.kind,
            Some(&item.revision),
            metadata,
            bytes,
            request,
            result,
            Destination::Trash,
            None,
            guard,
        )
    }

    /// Restore a trashed item's same identity and bytes through the guarded outbox.
    /// The current revision must still match, and a claimed filename is skipped.
    /// Cost: the same per-space reads and durable commits as trash.
    pub fn restore(
        &self,
        item: &Item,
        request: Request<'_>,
        result: impl FnOnce(&Item) -> Value,
    ) -> Result<Item> {
        self.write_placed(
            item.space,
            Some(item.id),
            &item.kind,
            Some(&item.revision),
            item.metadata.clone(),
            &item.bytes,
            request,
            result,
            Destination::Restore,
            None,
            || Ok(()),
        )
    }

    /// Commit a title edit and any same-kind links to its former path in a single
    /// per-space content transaction. The supplied transform receives only the
    /// indexed source snapshots and selected paths and may not re-enter content.
    /// Placement completes before publishing the root receipt when sources are
    /// rewritten. Each changed source incurs one checked file read and its
    /// ordinary placement and journal costs. Oversized groups fail before commit.
    #[allow(clippy::too_many_arguments)]
    pub fn write_with_links(
        &self,
        space: SpaceId,
        id: Uuid,
        kind: &str,
        base: &str,
        metadata: Value,
        bytes: &[u8],
        request: Request<'_>,
        result: impl FnOnce(&Item) -> Value,
        retarget: impl Fn(&Item, &str, &str) -> Result<Option<(Value, Vec<u8>)>>,
    ) -> Result<Item> {
        self.write_placed(
            space,
            Some(id),
            kind,
            Some(base),
            metadata,
            bytes,
            request,
            result,
            Destination::Current,
            Some(&retarget),
            || Ok(()),
        )
    }

    pub(crate) fn validate_write_size(metadata: &Value, bytes: &[u8]) -> Result<()> {
        if bytes.len() > 2 * 1024 * 1024 || metadata.to_string().len() > crate::MAX_METADATA_BYTES {
            return Err(Error::invalid_input("content input is too large"));
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn write_placed(
        &self,
        space: SpaceId,
        id: Option<Uuid>,
        kind: &str,
        base: Option<&str>,
        metadata: Value,
        bytes: &[u8],
        request: Request<'_>,
        result: impl FnOnce(&Item) -> Value,
        destination: Destination,
        retarget: Option<&Retarget<'_>>,
        guard: impl FnOnce() -> Result<()>,
    ) -> Result<Item> {
        Self::validate_write_size(&metadata, bytes)?;
        if request.id.len() > 256 {
            return Err(Error::invalid_input("content input is too large"));
        }
        files::validate_kind(kind)?;
        self.with_space(space,|s| {
            // Refuse further growth when a failed/slow disk misses ADR 0062's
            // maximum; the lifecycle worker continues retrying durable intents.
            if s.pending_since.is_some_and(|at| at.elapsed() >= Duration::from_secs(1)) {
                return Err(Error::unavailable("userdata materialization exceeded one second; check the SSD root and retry"));
            }
            s.validate_bytes(kind,bytes)?;
            self.publish_pending(s)?;
            // Request IDs are unique per (account, space): the host replays a matching
            // receipt before calling, so any receipt here is a reuse.
            if self.journals.receipt(&s.journal,request.account,request.id)?.is_some(){return Err(Error::request_reused());}
            let (id,seq,old_path,expected)=if let Some(id)=id {
                let mut head = s.header(id)?;
                if head.kind != kind || head.state == "purged" || head.state == "purging" {
                    return Err(missing());
                }
                if base != Some(head.revision.as_str()) {
                    return Err(Error {code:ErrorCode::Conflict,message:"item has a newer revision".into(),details:Some(serde_json::json!({"reason":"stale_revision","current_revision":head.revision}))});
                }
                if head.path.starts_with(".trash/") != (destination == Destination::Restore) {
                    return Err(Error::conflict(if destination == Destination::Restore {
                        "item is not in Trash"
                    } else {
                        "restore this item from Trash first"
                    }));
                }
                if head.state == "pending" {
                    s.place_header(id,head,None,self.fault)?;
                    // Placement changes the expected hash/state. Reload only on
                    // this pending slow path, never for an ordinary warm save.
                    head = s.header(id)?;
                }
                let sha = s.check_header(id,&head)?;
                let (seq,path) = (head.seq,head.path);
                (id,seq+1,Some(path),sha)
            }else{(Uuid::now_v7(),1,None,None)};
            guard()?;
            let title=metadata.get("title").and_then(Value::as_str).unwrap_or(kind);
            let path = match destination {
                Destination::Trash => s.trash_path(kind, id)?,
                Destination::Restore => {
                    let preferred = s.trash_origin(id)?;
                    s.choose_path(kind, title, None, preferred.as_deref())?
                }
                Destination::Current => s.choose_path(kind, title, old_path.as_deref(), None)?,
            };
            let item=Item {id,space,kind:kind.into(),metadata,bytes:bytes.to_vec(),revision:Staged::version(seq),updated_at:now_rfc3339()};
            let mut root = Staged {item,seq,path,expected};
            let related = if let (Some(edit), Some(old)) = (retarget, old_path.as_deref()) {
                s.stage_related(&mut root,old,edit,self.fault)?
            } else {
                Vec::new()
            };
            let response=result(&root.item);
            let tx=s.conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(internal)?;
            root.write(&tx, old_path.as_deref(), &request.account.to_string())?;
            match destination {
                Destination::Trash => {
                    tx.execute("INSERT INTO trash_origins VALUES(?1,?2)", params![id.to_string(),old_path]).map_err(internal)?;
                }
                Destination::Restore => {
                    tx.execute("DELETE FROM trash_origins WHERE item=?1", [id.to_string()]).map_err(internal)?;
                }
                Destination::Current => {}
            }
            root.outbox(&tx,request,&response)?;
            linked_write::stage_related_outbox(&tx,&root,&related,request)?;
            tx.commit().map_err(internal)?;
            (self.fault)(FaultPoint::AfterCommit);
            self.publish_pending(s)?;
            if destination != Destination::Current || retarget.is_some() {
                s.place_snapshot(id,Some(&root.item),self.fault)?;
            } else {
                s.pending_since.get_or_insert_with(Instant::now);
            }
            Ok(root.item)
        })
    }
}
