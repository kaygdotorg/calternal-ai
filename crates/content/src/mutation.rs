//! Durable content revisions and bounded asynchronous userdata materialization.
use crate::{
    Content, Error, ErrorCode, FaultPoint, Item, Request, Result, files, now_rfc3339,
    types::{internal, missing},
};
use calternal_types::SpaceId;
use rusqlite::{OptionalExtension, TransactionBehavior, params};
use serde_json::Value;
use std::time::{Duration, Instant};
use uuid::Uuid;
impl Content {
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
        let metadata_json = metadata.to_string();
        if bytes.len() > 2 * 1024 * 1024
            || metadata_json.len() > 256 * 1024
            || request.id.len() > 256
        {
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
                s.place(id,self.fault)?;
                let (current_kind,revision,seq,path,sha):(String,String,i64,String,Option<String>)=s.conn.query_row("SELECT kind,revision,seq,path,file_hash FROM items WHERE id=?1",[id.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).optional().map_err(internal)?.ok_or_else(missing)?;
                if current_kind!=kind{return Err(missing());}
                s.check_file(id)?;
                if base!=Some(revision.as_str()) {return Err(Error {code:ErrorCode::Conflict,message:"item has a newer revision".into(),details:Some(serde_json::json!({"reason":"stale_revision","current_revision":revision}))});}
                (id,seq+1,Some(path),sha)
            }else{(Uuid::now_v7(),1,None,None)};
            let title=metadata.get("title").and_then(Value::as_str).unwrap_or(kind);
            let path=s.choose_path(kind,title,old_path.as_deref())?;
            let item=Item {id,space,kind:kind.into(),metadata,bytes:bytes.to_vec(),revision:format!("v{seq}.{}",Uuid::now_v7().simple()),updated_at:now_rfc3339()};
            let response=result(&item);
            let tx=s.conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(internal)?;
            tx.execute("INSERT INTO items(id,kind,metadata,revision,seq,updated_at,path,folded_path,old_path,mat_state,file_hash) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,'pending',?10) ON CONFLICT(id) DO UPDATE SET metadata=excluded.metadata,revision=excluded.revision,seq=excluded.seq,updated_at=excluded.updated_at,path=excluded.path,folded_path=excluded.folded_path,old_path=excluded.old_path,mat_state='pending'",params![id.to_string(),kind,metadata_json,item.revision,seq,item.updated_at,path,calternal_fsio::names::fold(&path),old_path.filter(|old|old!=&path),expected]).map_err(internal)?;
            tx.execute("INSERT INTO versions VALUES(?1,?2,?3,?4,?5,?6,?7)",params![id.to_string(),seq,item.revision,metadata_json,bytes,item.updated_at,request.account.to_string()]).map_err(internal)?;
            tx.execute("INSERT INTO outbox VALUES(?1,?2,?3,?4,?5,?6,?7)",params![request.account.to_string(),request.id,request.operation,request.fingerprint,id.to_string(),item.revision,response.to_string()]).map_err(internal)?;
            tx.commit().map_err(internal)?;
            (self.fault)(FaultPoint::AfterCommit);
            self.publish_pending(s)?;
            // Metadata/bytes and receipt are durable now. The existing lifecycle
            // worker places the file; an older pending deadline is never extended.
            s.pending_since.get_or_insert_with(Instant::now);
            Ok(item)
        })
    }
}
