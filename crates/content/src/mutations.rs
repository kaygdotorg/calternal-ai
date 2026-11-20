//! Content/control mutation admission, shared receipts and guarded Trash placement.
use super::*;
use crate::rooms::Body;
use rusqlite::TransactionBehavior;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Destination {
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
        let _actor = self.lifecycle.step(request.account.as_uuid())?;
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
                    request.outbox_result(&response)
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
    /// Authorize an irreversible directory erasure against this space's
    /// existing receipt namespace. Reuses its lifecycle lock and closes it
    /// through the callback. The removed journal receives no new receipt.
    pub fn admit_erasure<T>(
        &self,
        space: SpaceId,
        request: Request<'_>,
        erase: impl FnOnce() -> Result<T>,
    ) -> Result<T> {
        let _actor = self.lifecycle.step(request.account.as_uuid())?;
        self.with_space(space, |s| {
            self.publish_pending(s)?;
            if let Some(saved) = self
                .journals
                .receipt(&s.journal, request.account, request.id)?
                && (saved.operation != request.operation
                    || saved.fingerprint != request.fingerprint)
            {
                return Err(Error::request_reused());
            }
            erase()
        })
    }

    /// Admit a control mutation while holding the target item's space lock. The
    /// callback sees the current head before replay and may check its lifecycle;
    /// it must not re-enter content. A crash after its commit replays its own
    /// idempotent result. This fences archive and deletion against turn
    /// admission without a second lock or store. A caller can publish a refusal
    /// as a receipt when it must survive retry. Cost: `admit` plus one indexed
    /// head read. The lock spans the callback's job fsync to keep the lifecycle
    /// decision atomic within a space.
    /// Missing heads, reused request identities and callback/storage failures are
    /// returned without acknowledging a new control mutation.
    pub fn admit_item(
        &self,
        space: SpaceId,
        id: Uuid,
        request: Request<'_>,
        run: impl FnOnce(&Item, Option<Value>) -> Result<Value>,
    ) -> Result<Value> {
        let _actor = self.lifecycle.step(request.account.as_uuid())?;
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
                    request.via,
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
        let _actor = self.lifecycle.step(request.account.as_uuid())?;
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
                request.via,
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
            Body::Replace(bytes),
            request,
            result,
            Destination::Current,
            None,
            None,
            None,
            || Ok(()),
            |_| Ok(()),
            None,
        )
    }

    /// Create one item under a hard live-item cap checked within the space
    /// writer lock. The count stops after the supplied bound using the kind
    /// index. Commit, placement and replay follow the ordinary write path.
    #[allow(clippy::too_many_arguments)]
    pub fn create_bounded(
        &self,
        space: SpaceId,
        kind: &str,
        limit: u32,
        metadata: Value,
        bytes: &[u8],
        request: Request<'_>,
        result: impl FnOnce(&Item) -> Value,
    ) -> Result<Item> {
        self.write_placed(
            space,
            None,
            kind,
            None,
            metadata,
            Body::Replace(bytes),
            request,
            result,
            Destination::Current,
            None,
            Some(limit),
            None,
            || Ok(()),
            |_| Ok(()),
            None,
        )
    }

    /// Create an item with an identity reserved by a durable host outbox.
    /// The ID is checked under the space writer lock before the normal write.
    /// A previously committed identity cannot be allocated again, even after
    /// the request receipt expires. Callers must supply a host-generated UUID.
    #[allow(clippy::too_many_arguments)]
    pub fn create_assigned(
        &self,
        space: SpaceId,
        id: Uuid,
        kind: &str,
        metadata: Value,
        bytes: &[u8],
        request: Request<'_>,
        result: impl FnOnce(&Item) -> Value,
    ) -> Result<Item> {
        self.write_placed(
            space,
            None,
            kind,
            None,
            metadata,
            Body::Replace(bytes),
            request,
            result,
            Destination::Current,
            None,
            None,
            Some(id),
            || Ok(()),
            |_| Ok(()),
            None,
        )
    }

    /// Write one shared note after rechecking live collection and directory
    /// authority under the space lock. The callback may consult the auth worker
    /// after reading this note's indexed memberships and must not re-enter
    /// content. It runs before exposing receipt or revision conflict details.
    #[allow(clippy::too_many_arguments)]
    pub fn write_shared_note(
        &self,
        space: SpaceId,
        id: Uuid,
        base: &str,
        metadata: Value,
        bytes: &[u8],
        request: Request<'_>,
        result: impl FnOnce(&Item) -> Value,
        authorize: impl FnOnce(Vec<Uuid>) -> Result<()>,
    ) -> Result<Item> {
        self.write_placed(
            space,
            Some(id),
            "notes",
            Some(base),
            metadata,
            Body::Replace(bytes),
            request,
            result,
            Destination::Current,
            None,
            None,
            None,
            || Ok(()),
            |space| authorize(space.sharing_collections(id)?),
            None,
        )
    }

    /// Append to the current text or patch a retained base for an opted-in kind.
    /// Metadata is inherited from the current head. Refuses unregistered kinds,
    /// unsafe offsets and missing patch bases before committing. Cost: the same
    /// indexed read, merge, commit and placement path as a live replacement.
    #[allow(clippy::too_many_arguments)]
    pub fn edit_live(
        &self,
        space: SpaceId,
        id: Uuid,
        kind: &str,
        base: Option<&str>,
        edit: LiveEdit<'_>,
        request: Request<'_>,
        result: impl FnOnce(&Item) -> Value,
    ) -> Result<Item> {
        match (edit, base) {
            (LiveEdit::Append(_), None) | (LiveEdit::Patch(_), Some(_)) => {}
            _ => {
                return Err(Error::invalid_input(
                    "append takes no base; patch requires a base",
                ));
            }
        }
        self.write_placed(
            space,
            Some(id),
            kind,
            base,
            Value::Null,
            Body::Live(edit),
            request,
            result,
            Destination::Current,
            None,
            None,
            None,
            || Ok(()),
            |_| Ok(()),
            None,
        )
    }

    /// Apply a binary Yjs update against a live item's current lineage and
    /// commit its text, checkpoint, revision and receipt through content.
    /// A wrong lineage, held causal dependency or invalid document is refused.
    /// The preview callback may queue bytes for peers just before commit. It
    /// must be bounded and cannot re-enter content. Preview delivery is volatile.
    /// Cost: one current checkpoint, CRDT apply and the shared commit path.
    #[allow(clippy::too_many_arguments)]
    pub fn apply_room_update(
        &self,
        space: SpaceId,
        kind: &str,
        item: Uuid,
        lineage: &str,
        update: &[u8],
        request: Request<'_>,
        preview: impl Fn(&[u8]) -> Result<()>,
    ) -> Result<Item> {
        self.write_placed(
            space,
            Some(item),
            kind,
            None,
            Value::Null,
            Body::Wire {
                lineage,
                update,
                preview: &preview,
            },
            request,
            |item| serde_json::json!({"id":item.id,"revision":item.revision}),
            Destination::Current,
            None,
            None,
            None,
            || Ok(()),
            |_| Ok(()),
            None,
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
            Body::Replace(bytes),
            request,
            result,
            Destination::Trash,
            None,
            None,
            None,
            guard,
            |_| Ok(()),
            None,
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
            Body::Replace(&item.bytes),
            request,
            result,
            Destination::Restore,
            None,
            None,
            None,
            || Ok(()),
            |_| Ok(()),
            None,
        )
    }

    /// Commit a planned rename and freeze its inbound link revisions for replay.
    /// Places the root before admitting any dependent edit. Link edits and
    /// the inverse use bounded worker turns. The transform receives exact source
    /// snapshots and must not re-enter content. Each changed file uses guarded
    /// placement and one journal event. The job records revisions for undo.
    #[allow(clippy::too_many_arguments)]
    pub fn write_with_links(
        &self,
        job: Uuid,
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
            Body::Replace(bytes),
            request,
            result,
            Destination::Current,
            Some((job, &retarget)),
            None,
            None,
            || Ok(()),
            |_| Ok(()),
            None,
        )
    }
}
