//! Read-only journal access for rebuildable per-space projections and diagnostics.
use crate::{Error, ErrorCode, Result, SpaceId, SyncToken, receipt_key, schema};
use calternal_types::AccountId;
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;

/// One durable scan position. Non-content events still advance the watermark.
pub struct ProjectionChange {
    /// The durable sequence of this entry.
    pub sequence: i64,
    /// Item UUID for a published content revision.
    pub item: Option<String>,
}

/// A reader over one protected journal. It never activates a journal writer
/// or issues a client sync token. Hold it inside the space lifecycle guard.
pub struct ProjectionJournal(Connection);

impl ProjectionJournal {
    /// Open the current binding, or return None for an uninitialized space.
    /// Cost: one SQLite read handle and one indexed binding read.
    pub fn open(appdata: &Path, space: SpaceId) -> Result<Option<Self>> {
        let path = appdata
            .join("spaces")
            .join(space.to_string())
            .join("content.db");
        if !path.is_file() {
            return Ok(None);
        }
        let conn = schema::open_reader(&path)?;
        conn.pragma_update(None, "cache_size", -128)
            .map_err(Error::internal)?;
        let initialized: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='meta' AND type='table')",
            [], |row| row.get(0),
        ).map_err(Error::internal)?;
        if !initialized {
            return Ok(None);
        }
        let binding = conn
            .query_row("SELECT value FROM meta WHERE key='space_id'", [], |row| {
                row.get::<_, String>(0)
            })
            .optional()
            .map_err(Error::internal)?;
        let Some(binding) = binding else {
            return Ok(None);
        };
        if binding != space.to_string() {
            return Err(Error::new(
                ErrorCode::InvalidInput,
                "journal belongs to another space",
            ));
        }
        Ok(Some(Self(conn)))
    }

    /// Read the generation and newest durable sequence in one SQLite snapshot.
    /// Cost: two indexed seeks, with no user event allocations.
    pub fn head(&self) -> Result<SyncToken> {
        self.0.prepare_cached(
            "SELECT value,(SELECT coalesce(max(seq),0) FROM journal) FROM meta WHERE key='epoch'",
        ).map_err(Error::internal)?
            .query_row([], |row| Ok(SyncToken { epoch: row.get(0)?, seq: row.get(1)? }))
            .map_err(Error::internal)
    }

    /// Find the earliest journal receipt timestamp among the supplied outbox
    /// keys. Unpublished intents have no matching receipt. Cost: one indexed
    /// lookup per pin in one read snapshot, without loading replies or writing.
    /// Returns an error if the receipt table cannot be read.
    pub fn oldest_receipt_ms(&self, pins: &[(AccountId, String)]) -> Result<Option<i64>> {
        let keys: Vec<_> = pins
            .iter()
            .map(|(account, request)| receipt_key(*account, request))
            .collect();
        let keys = serde_json::to_string(&keys).map_err(Error::internal)?;
        self.0
            .query_row(
                "SELECT min(at_ms) FROM idempotency WHERE key IN (SELECT value FROM json_each(?1))",
                [keys],
                |row| row.get(0),
            )
            .map_err(Error::internal)
    }

    /// Read at most 128 entries after the durable position, in commit order.
    /// Cost: one indexed range query. Callers verify the epoch before publish.
    pub fn since(&self, after: i64, limit: usize) -> Result<Vec<ProjectionChange>> {
        let mut stmt = self.0.prepare_cached(
            "SELECT seq, CASE WHEN item<>'' AND json_type(data,'$.revision')='text' THEN item END
             FROM journal WHERE seq>?1 ORDER BY seq LIMIT ?2",
        ).map_err(Error::internal)?;
        let rows = stmt
            .query_map(params![after, limit.clamp(1, 128) as i64], |row| {
                Ok(ProjectionChange {
                    sequence: row.get(0)?,
                    item: row.get(1)?,
                })
            })
            .map_err(Error::internal)?;
        rows.collect::<rusqlite::Result<_>>()
            .map_err(Error::internal)
    }
}
