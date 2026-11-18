//! Byte-range reads across an immutable legacy snapshot and later append records.
use crate::{Result, types::internal};
use rusqlite::{Connection, OptionalExtension, params};
use std::ops::Range;

/// First writer version that can read a head stored as an append patch.
pub(crate) const PATCH_SCHEMA_VERSION: i64 = 9;

/// Add immutable revision ranges to the existing content database.
pub(crate) const SCHEMA: &str = "
CREATE TABLE thread_revision_patches(
    item TEXT NOT NULL,
    seq INTEGER NOT NULL,
    base_seq INTEGER NOT NULL,
    offset INTEGER NOT NULL CHECK(offset > 0),
    length INTEGER NOT NULL CHECK(length BETWEEN 1 AND 65536),
    witness BLOB NOT NULL CHECK(length(witness) = 32),
    digest BLOB NOT NULL CHECK(length(digest) = 32),
    file_digest TEXT NOT NULL CHECK(length(file_digest) = 64),
    PRIMARY KEY(item, seq),
    CHECK(base_seq < seq),
    FOREIGN KEY(item, seq) REFERENCES versions(item, seq) ON DELETE CASCADE,
    FOREIGN KEY(item, base_seq) REFERENCES versions(item, seq) ON DELETE CASCADE
) STRICT, WITHOUT ROWID;
CREATE UNIQUE INDEX thread_patch_range ON thread_revision_patches(item, base_seq, offset);
";

/// Location of a revision in the content DB. For a patch, the complete range
/// includes its snapshot and every preceding patch of that snapshot.
pub(crate) struct Revision {
    pub(crate) item: String,
    pub(crate) seq: i64,
    pub(crate) rowid: i64,
    pub(crate) base_seq: i64,
    pub(crate) size: usize,
}

impl Revision {
    pub(crate) fn get(db: &Connection, item: String, seq: i64) -> Result<Option<Self>> {
        db.prepare_cached(
            "SELECT v.rowid, coalesce(p.base_seq,v.seq), coalesce(p.offset,0)+length(v.bytes)
             FROM versions v LEFT JOIN thread_revision_patches p USING(item,seq)
             WHERE v.item=?1 AND v.seq=?2",
        )
        .map_err(internal)?
        .query_row(params![item, seq], |row| {
            Ok(Self {
                item: item.clone(),
                seq,
                rowid: row.get(0)?,
                base_seq: row.get(1)?,
                size: row.get::<_, u32>(2)? as usize,
            })
        })
        .optional()
        .map_err(internal)
    }

    /// Allocate only the requested window. The predecessor extent is found by
    /// index, then only overlapping extent BLOBs are opened, oldest first.
    pub(crate) fn read(&self, db: &Connection, range: Range<usize>) -> Result<Vec<u8>> {
        if range.start > range.end || range.end > self.size {
            return Err(internal("invalid revision range"));
        }
        let mut bytes = vec![0; range.len()];
        if bytes.is_empty() {
            return Ok(bytes);
        }
        if self.seq == self.base_seq {
            read_blob(db, self.rowid, range.start, &mut bytes)?;
            return Ok(bytes);
        }
        let mut cursor = range.start;
        let (base, size): (i64, usize) = db
            .prepare_cached("SELECT rowid,length(bytes) FROM versions WHERE item=?1 AND seq=?2")
            .map_err(internal)?
            .query_row(params![self.item, self.base_seq], |row| {
                Ok((row.get(0)?, row.get::<_, u32>(1)? as usize))
            })
            .map_err(internal)?;
        if cursor < size {
            let end = range.end.min(size);
            read_blob(db, base, cursor, &mut bytes[..end - cursor])?;
            cursor = end;
        }
        if cursor == range.end {
            return Ok(bytes);
        }
        let mut stmt = db
            .prepare_cached(
                "SELECT p.offset,p.length,v.rowid
             FROM thread_revision_patches p JOIN versions v USING(item,seq)
             WHERE p.item=?1 AND p.base_seq=?2 AND p.seq<=?3 AND p.offset<?4
               AND p.offset>=coalesce((
                   SELECT max(offset) FROM thread_revision_patches
                   WHERE item=?1 AND base_seq=?2 AND offset<=?5
               ), ?5)
             ORDER BY p.offset",
            )
            .map_err(internal)?;
        let mut rows = stmt
            .query(params![
                self.item,
                self.base_seq,
                self.seq,
                range.end as i64,
                cursor as i64
            ])
            .map_err(internal)?;
        while let Some(row) = rows.next().map_err(internal)? {
            let offset = row.get::<_, u32>(0).map_err(internal)? as usize;
            let length = row.get::<_, u32>(1).map_err(internal)? as usize;
            let end = range.end.min(offset.saturating_add(length));
            if cursor < offset || end <= cursor {
                return Err(internal("revision patch chain has a gap"));
            }
            read_blob(
                db,
                row.get(2).map_err(internal)?,
                cursor - offset,
                &mut bytes[cursor - range.start..end - range.start],
            )?;
            cursor = end;
        }
        if cursor != range.end {
            return Err(internal("revision patch chain is incomplete"));
        }
        Ok(bytes)
    }
}

fn read_blob(db: &Connection, rowid: i64, offset: usize, out: &mut [u8]) -> Result<()> {
    db.blob_open("main", "versions", "bytes", rowid, true)
        .map_err(internal)?
        .read_at_exact(out, offset)
        .map_err(internal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_revisions_and_bounded_ranges_across_patches_are_readable_after_migration() {
        use calternal_store::migrations::{inspect, migrate_release, require_writer_version};

        let mut db = Connection::open_in_memory().unwrap();
        let [previous, patch_release] = crate::db::thread_patch_transition();
        assert_eq!(patch_release.version(), previous.version() + 1);
        assert_eq!(patch_release.version(), PATCH_SCHEMA_VERSION);
        migrate_release(&db, previous, false).unwrap();
        db.execute_batch(
            "INSERT INTO items VALUES ('t','threads','{}','v1',1,'now','thread.md','thread.md',NULL,'materialized',NULL);
             INSERT INTO versions VALUES ('t',1,'v1','{}',X'616263','now','owner');",
        )
        .unwrap();

        migrate_release(&db, patch_release, true).unwrap();
        assert_eq!(inspect(&db, previous).unwrap(), patch_release.version());
        assert_eq!(
            Revision::get(&db, "t".into(), 1)
                .unwrap()
                .unwrap()
                .read(&db, 0..3)
                .unwrap(),
            b"abc"
        );

        let tx = db.transaction().unwrap();
        require_writer_version(&tx, PATCH_SCHEMA_VERSION).unwrap();
        tx.execute_batch(
            "INSERT INTO versions VALUES
             ('t',2,'v2','{}',X'6465','now','owner'),
             ('t',3,'v3','{}',X'666768','now','owner'),
             ('t',4,'v4','{}',X'7A','now','owner');
             INSERT INTO thread_revision_patches VALUES
             ('t',2,1,3,2,zeroblob(32),zeroblob(32), printf('%064d',0)),
             ('t',3,1,5,3,zeroblob(32),zeroblob(32), printf('%064d',0));",
        )
        .unwrap();
        tx.commit().unwrap();
        assert!(inspect(&db, previous).is_err());
        for (seq, range, expected) in [
            (1, 0..3, b"abc".as_slice()),
            (2, 2..5, b"cde"),
            (3, 4..7, b"efg"),
            (4, 0..1, b"z"),
        ] {
            let revision = Revision::get(&db, "t".into(), seq).unwrap().unwrap();
            assert_eq!(revision.read(&db, range).unwrap(), expected);
        }
    }
}
