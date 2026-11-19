//! One file lease and durability barrier for up to four consecutive patches.
use super::{
    io,
    proof::{self, Stamp},
    writer::{IDENTITY_BYTES, RECORD_BYTES},
};
use crate::{
    FaultPoint, Result,
    db::{Header, Space, bookkeeping},
    types::{blocked, internal},
};
use calternal_fsio::beneath::Folder;
use rusqlite::{OptionalExtension, params};
use std::{
    fs::File,
    time::{Duration, Instant},
};
use uuid::Uuid;

const RECORDS: i64 = 4;
const WINDOW: Duration = Duration::from_millis(2);

/// A live group is held by the space slot, with a lease on its named file.
pub(crate) struct Group {
    pub(super) first: i64,
    pub(super) last: i64,
    pub(super) file: File,
    id: Uuid,
    pub(super) closes: Instant,
}

impl Group {
    pub(super) fn new(
        id: Uuid,
        first: i64,
        folder: &Folder,
        path: &str,
        checkpoint: bool,
    ) -> Result<Self> {
        let file = folder.lease_append(path).map_err(proof::file_error)?;
        let window = if checkpoint { WINDOW } else { Duration::ZERO };
        Ok(Self {
            first,
            last: first - 1,
            file,
            id,
            closes: Instant::now() + window,
        })
    }

    /// Only a staging read or append for this thread can join an open group.
    pub(crate) fn accepts(&self, append_to: Option<Uuid>) -> bool {
        append_to == Some(self.id)
            && self.last - self.first + 1 < RECORDS
            && Instant::now() < self.closes
    }

    pub(super) fn deadline(&self) -> Instant {
        if self.last - self.first + 1 == RECORDS {
            Instant::now()
        } else {
            self.closes
        }
    }
}

impl Space {
    /// Complete a prepared group before its first journal publication. The
    /// warm path syncs its existing descriptor once. Cold recovery verifies
    /// the old prefix and repairs the committed records in order.
    pub(crate) fn place_thread_patch(
        &mut self,
        id: Uuid,
        head: &Header,
        fault: fn(FaultPoint),
    ) -> Result<bool> {
        if head.kind != "threads" {
            return Ok(false);
        }
        let latest = self
            .conn
            .query_row(
                "SELECT batch_start,file_digest FROM thread_revision_patches WHERE item=?1 AND seq=?2",
                params![id.to_string(), head.seq],
                |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(internal)?;
        let Some((first, full)) = latest else {
            return Ok(false);
        };
        let folder = Folder::open_existing(&self.files).map_err(internal)?;
        let (mut ready, file) = match self.append_group.take() {
            Some(group) if group.id == id && group.last == head.seq => {
                let stamp = named(&folder, &head.path, &group.file)?;
                let ready = self.require_proof(id, head, head.seq, stamp.len() as usize, stamp)?;
                (ready, group.file)
            }
            _ => self.replay_thread_group(id, head, &folder, first, fault)?,
        };
        if ready.hasher.finalize().to_hex().as_str() != full {
            return Err(internal("thread append file checksum differs"));
        }
        file.sync_all().map_err(internal)?;
        ready.stamp = named(&folder, &head.path, &file)?;
        ready.seq = head.seq;
        fault(FaultPoint::AfterPlacement);
        self.mark_thread_group_placed(id, head.seq, &full)?;
        self.remember_append_proof(id, head.seq, Ok(ready));
        Ok(true)
    }

    /// A receipt for the first patch proves the whole file group was synced
    /// before journal publication. Restore a lost placement mark on restart
    /// without changing an outside edit made after that publication.
    pub(crate) fn recover_published_thread_group(&mut self, item: &str, revision: &str) -> Result<bool> {
        if self.append_group.is_some() {
            return Ok(false);
        }
        let found = self.conn.query_row(
            "SELECT i.seq,p.file_digest
             FROM items i JOIN thread_revision_patches p ON p.item=i.id AND p.seq=i.seq
             JOIN versions v ON v.item=i.id AND v.seq=p.batch_start
             WHERE i.id=?1 AND i.mat_state='pending' AND v.revision=?2",
            params![item, revision],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
        ).optional().map_err(internal)?;
        let Some((seq, digest)) = found else {
            return Ok(false);
        };
        let id = Uuid::parse_str(item).map_err(internal)?;
        self.append_proofs.remove(&id);
        self.mark_thread_group_placed(id, seq, &digest)?;
        Ok(true)
    }

    fn mark_thread_group_placed(&self, id: Uuid, seq: i64, digest: &str) -> Result<()> {
        // Losing this NORMAL mark is safe: the FULL outbox still owns every
        // record until journal receipts and durable retirement exist.
        bookkeeping(&self.conn, || {
            self.conn
                .execute(
                    "UPDATE items SET mat_state='materialized',file_hash=?1,old_path=NULL
                     WHERE id=?2 AND seq=?3",
                    params![digest, id.to_string(), seq],
                )
                .map_err(internal)?;
            Ok(())
        })
    }

    fn replay_thread_group(
        &mut self,
        id: Uuid,
        head: &Header,
        folder: &Folder,
        first: i64,
        fault: fn(FaultPoint),
    ) -> Result<(proof::Ready, File)> {
        let records = {
            let mut stmt = self.conn.prepare_cached(
                "SELECT p.offset,p.witness,p.digest,p.prior_digest,p.file_digest,v.bytes
                 FROM thread_revision_patches p JOIN versions v USING(item,seq)
                 WHERE p.item=?1 AND p.seq BETWEEN ?2 AND ?3 AND p.batch_start=?2
                 ORDER BY p.seq"
            ).map_err(internal)?;
            stmt.query_map(params![id.to_string(), first, head.seq], |r| Ok((
                r.get::<_, u32>(0)? as usize + IDENTITY_BYTES,
                r.get::<_, [u8; 32]>(1)?,
                r.get::<_, [u8; 32]>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, Vec<u8>>(5)?,
            )))
            .map_err(internal)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(internal)?
        };
        if records.len() as i64 != head.seq - first + 1 || records.is_empty() {
            return Err(internal("missing thread append records"));
        }
        let mut file = folder.lease_append(&head.path).map_err(proof::file_error)?;
        let stamp = named(folder, &head.path, &file)?;
        let mut length = stamp.len() as usize;
        let last = records.last().expect("checked above");
        if length > last.0 + last.5.len() {
            return Err(blocked());
        }
        let mut ready = self.require_proof_at(id, head, first - 1, records[0].0, stamp, &records[0].3)?;
        let mut next = records[0].0;
        for (offset, witness, digest, prior, full, record) in records {
            if offset != next || record.is_empty() || record.len() > RECORD_BYTES
                || ready.hasher.finalize().to_hex().as_str() != prior
                || blake3::hash(&record).as_bytes() != &digest
            {
                return Err(internal("thread append records differ"));
            }
            next += record.len();
            fault(FaultPoint::BeforePlacement);
            io::place(
                &mut file, length.min(next) as u64, offset as u64, witness.into(), &record,
                Folder::check_append_lease,
            ).map_err(proof::file_error)?;
            length = length.max(next);
            ready.advance(&record);
            if ready.hasher.finalize().to_hex().as_str() != full {
                return Err(internal("thread append file checksum differs"));
            }
        }
        Ok((ready, file))
    }
}

pub(super) fn named(folder: &Folder, path: &str, file: &File) -> Result<Stamp> {
    let stamp = Stamp::new(&file.metadata().map_err(internal)?);
    if Stamp::new(&folder.file_identity(path).map_err(proof::file_error)?) != stamp {
        return Err(blocked());
    }
    Ok(stamp)
}
