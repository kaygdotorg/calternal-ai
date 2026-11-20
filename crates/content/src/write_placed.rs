//! Shared content writer transaction and outbox admission.
use super::*;
use crate::item_write::{CheckedItemBody, CheckedWrite};
use crate::linked_write::{Retarget, Staged};
use crate::mutations::Destination;
use crate::rooms::Body;
use rusqlite::TransactionBehavior;

impl Content {
    pub(crate) fn validate_write_size(metadata: &Value, bytes: &[u8]) -> Result<()> {
        if bytes.len() > crate::MAX_DOCUMENT_BYTES
            || metadata.to_string().len() > crate::MAX_METADATA_BYTES
        {
            return Err(Error::invalid_input("content input is too large"));
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn write_placed(
        &self,
        space: SpaceId,
        id: Option<Uuid>,
        kind: &str,
        base: Option<&str>,
        metadata: Value,
        body: Body<'_>,
        request: Request<'_>,
        result: impl FnOnce(&Item) -> Value,
        destination: Destination,
        retarget: Option<(Uuid, &Retarget<'_>)>,
        limit: Option<u32>,
        assigned: Option<Uuid>,
        guard: impl FnOnce() -> Result<()>,
        authorize: impl FnOnce(&crate::db::Space) -> Result<()>,
        checked: Option<CheckedWrite<'_>>,
    ) -> Result<Item> {
        let _actor = self.lifecycle.step(request.account.as_uuid())?;
        body.validate(&metadata)?;
        if request.id.len() > 256 {
            return Err(Error::invalid_input("content input is too large"));
        }
        files::validate_kind(kind)?;
        self.with_space(space,|s| {
            let live = s.live_editing(kind)?;
            let mut metadata = metadata;
            let mut bytes = match body {
                Body::Replace(bytes) => std::borrow::Cow::Borrowed(bytes),
                _ if live => std::borrow::Cow::Borrowed(&[][..]),
                _ => return Err(Error::invalid_input("kind does not enable live editing")),
            };
            let mut room = None;
            let mut preview_update = None;
            // Refuse further growth when a failed/slow disk misses ADR 0062's
            // maximum; the lifecycle worker continues retrying durable intents.
            if s.pending_since.is_some_and(|at| at.elapsed() >= Duration::from_secs(1)) {
                return Err(Error::unavailable("userdata materialization exceeded one second; check the SSD root and retry"));
            }
            authorize(s)?;
            if let Some(limit) = limit {
                let count: u32 = s.conn.query_row(
                    "SELECT COUNT(*) FROM (SELECT 1 FROM items WHERE kind=?1
                        AND path NOT LIKE '.trash/%' AND mat_state NOT IN ('purged','purging') LIMIT ?2)",
                    params![kind,limit], |row| row.get(0),
                ).map_err(internal)?;
                if count >= limit {
                    return Err(Error::conflict("the space has reached its limit for this item kind"));
                }
            }
            s.validate_bytes(kind,&bytes)?;
            self.publish_pending(s)?;
            // Request IDs are unique per (account, space): the host replays a matching
            // receipt before calling, so any receipt here is a reuse.
            if self.journals.receipt(&s.journal,request.account,request.id)?.is_some(){return Err(Error::request_reused());}
            #[cfg(feature = "perf-timings")]
            let file_guard = calternal_store::timings::SaveStage::FileGuard.start();
            let (id,seq,old_path,expected)=if let Some(id)=id {
                let mut head = s.header(id)?;
                if head.kind != kind || head.state == "purged" || head.state == "purging" {
                    return Err(missing());
                }
                // Undo needs the exact pre-rename revision, including any room writes.
                let merge_stale = checked.is_none() && live && destination == Destination::Current && retarget.is_none()
                    && (base.is_some() || matches!(body, Body::Live(LiveEdit::Append(_)) | Body::Wire { .. }));
                if !merge_stale && base != Some(head.revision.as_str()) {
                    return Err(Error {code:ErrorCode::Conflict,message:"item has a newer revision".into(),details:Some(serde_json::json!({"reason":"stale_revision","current_revision":head.revision}))});
                }
                if head.path.starts_with(".trash/") != (destination == Destination::Restore) {
                    return Err(Error::conflict(if destination == Destination::Restore {
                        "item is not in Trash"
                    } else {
                        "restore this item from Trash first"
                    }));
                }
                // A room frame keeps the same path and metadata. Consecutive
                // admitted frames can share the lifecycle worker's file flush
                // while each acknowledged revision remains durable in SQLite.
                let share_placement = matches!(body, Body::Wire { .. } | Body::Outside { .. })
                    && head.old_path.is_none();
                if head.state == "pending" && !share_placement {
                    s.place_header(id,head,None,self.fault)?;
                    // Placement changes the expected hash/state. Reload only on
                    // this pending slow path, never for an ordinary warm save.
                    head = s.header(id)?;
                }
                let sha = if let Body::Outside { observed, .. } = body {
                    // Several sessions can observe the same outside edit before
                    // one archives it. The file base moves once across accounts.
                    if head.file_hash.as_deref() == Some(observed) {
                        return s.get(id);
                    }
                    s.verify_external(&head, observed)?;
                    Some(observed.to_owned())
                } else {
                    s.check_header(id,&head)?
                };
                if let Some(edit) = &checked {
                    let mut current = s.get(id)?;
                    current.metadata.as_object_mut().ok_or_else(|| internal("stored metadata is not an object"))?
                        .append(metadata.as_object_mut().ok_or_else(|| internal("checked metadata is not an object"))?);
                    metadata = current.metadata.clone();
                    if live {
                        let checkpoint = match s.checkpoint(id, head.seq)? {
                            Some(checkpoint) => checkpoint,
                            None => s.seed_checkpoint(&current)?,
                        };
                        room = Some(match edit.body {
                            CheckedItemBody::Keep => checkpoint,
                            CheckedItemBody::Replace(replacement) => checkpoint.rewrite(replacement)?,
                            CheckedItemBody::Append(_) => {
                                return Err(Error::invalid_input("append is unavailable for a live room"));
                            }
                        });
                    }
                    match edit.body {
                        CheckedItemBody::Replace(_) => (),
                        CheckedItemBody::Keep => bytes = std::borrow::Cow::Owned(current.bytes),
                        CheckedItemBody::Append(addition) => {
                            if current.bytes.len().saturating_add(addition.len()) > MAX_DOCUMENT_BYTES {
                                return Err(Error::invalid_input("append exceeds the document size limit"));
                            }
                            current.bytes.extend_from_slice(addition);
                            bytes = std::borrow::Cow::Owned(current.bytes);
                        }
                    }
                } else if live {
                    let current = s.get(id)?;
                    if !matches!(body, Body::Replace(_)) {
                        metadata = current.metadata.clone();
                    }
                    let (merged, checkpoint, delta) = s.merge_room_write(
                        &current, head.seq, base.unwrap_or(&head.revision), &mut metadata, body,
                    )?;
                    if delta.as_deref() == Some(&[0, 0]) {
                        return Ok(current);
                    }
                    preview_update = delta;
                    Self::validate_write_size(&metadata, &merged)?;
                    bytes = std::borrow::Cow::Owned(merged);
                    room = Some(checkpoint);
                }
                let (seq,path) = (head.seq,head.path);
                (id,seq+1,Some(path),sha)
            }else{
                if let Some(id) = assigned {
                    let exists: bool = s.conn.query_row(
                        "SELECT EXISTS(SELECT 1 FROM items WHERE id=?1)", [id.to_string()], |row| row.get(0),
                    ).map_err(internal)?;
                    if exists {
                        return Err(Error::conflict("item identity is already assigned"));
                    }
                }
                (assigned.unwrap_or_else(Uuid::now_v7),1,None,None)
            };
            #[cfg(feature = "perf-timings")]
            file_guard.finish();
            if let Some(edit) = &checked {
                Self::validate_write_size(&metadata, &bytes)?;
                s.validate_bytes(kind, &bytes)?;
                (edit.validate)(&metadata)?;
            }
            let title=metadata.get("title").and_then(Value::as_str).unwrap_or(kind);
            #[cfg(feature = "perf-timings")]
            let filename = calternal_store::timings::SaveStage::Filename.start();
            let path = if let Some(edit) = &checked {
                s.checked_path(kind, edit.placement, old_path.as_deref())?
            } else { match destination {
                Destination::Trash => s.trash_path(kind, id)?,
                Destination::Restore => {
                    let preferred = s.trash_origin(id)?;
                    s.choose_path(kind, title, None, preferred.as_deref())?
                }
                Destination::Current => s.choose_path(kind, title, old_path.as_deref(), None)?,
            }};
            #[cfg(feature = "perf-timings")]
            filename.finish();
            // Body-only writes leave room delivery caches valid. Lifecycle,
            // membership and path changes can invalidate existing handles.
            let protect = checked.as_ref().is_some_and(|edit| edit.protect);
            let _revision = (protect || kind == "collections"
                || destination != Destination::Current
                || old_path.as_ref().is_some_and(|old| *old != path))
                .then(calternal_types::begin_item_access_change);
            let item=Item {id,space,kind:kind.into(),metadata,bytes:bytes.into_owned(),revision:Staged::version(seq),updated_at:now_rfc3339()};
            if live && room.is_none() {
                room = Some(s.seed_checkpoint(&item)?);
            }
            let mut root = Staged {item,seq,path,expected,room};
            let mut candidates = Vec::new();
            if let (Some((_, edit)), Some(old)) = (retarget, old_path.as_deref()) {
                crate::rename_job::ensure_idle(&s.conn)?;
                candidates = s.related_revisions(&root, old)?;
                if let Some((metadata, bytes)) = edit(&root.item, old, &root.path)? {
                    Self::validate_write_size(&metadata, &bytes)?;
                    if let Some(room) = root.room.take() {
                        root.room = Some(room.rewrite(&bytes)?);
                    }
                    root.item.metadata = metadata;
                    root.item.bytes = bytes;
                }
            }
            let response=result(&root.item);
            let tx=s.conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(internal)?;
            guard()?;
            root.write(&tx, old_path.as_deref(), &request.account.to_string())?;
            if protect {
                tx.execute("INSERT OR IGNORE INTO item_discovery_protection VALUES(?1)", [id.to_string()]).map_err(internal)?;
            }
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
            if let (Some((job, _)), Some(old), Some(base)) = (retarget, old_path.as_deref(), base) {
                crate::rename_job::record(&tx, job, &root, old, base, &candidates, request,
                    self.receipt_retention())?;
                root.require_placement(&tx,request)?;
            }
            if let (Body::Wire { preview, .. }, Some(update)) = (body, preview_update) {
                preview(&update)?;
                (self.fault)(FaultPoint::BeforeRoomCommit);
            }
            #[cfg(feature = "perf-timings")]
            let commit = calternal_store::timings::SaveStage::ContentCommit.start();
            tx.commit().map_err(internal)?;
            #[cfg(feature = "perf-timings")]
            commit.finish();
            (self.fault)(FaultPoint::AfterCommit);
            self.publish_pending(s)?;
            if destination != Destination::Current {
                s.place_snapshot(id,Some(&root.item),self.fault)?;
            } else if retarget.is_none() {
                s.pending_since.get_or_insert_with(Instant::now);
            }
            Ok(root.item)
        })
    }
}
