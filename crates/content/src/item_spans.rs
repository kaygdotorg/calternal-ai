//! Recorded attested write spans and body provenance for checked items.
//!
//! A span is one composed record (package framing around a host-selected
//! middle) inside an item body. It lives from the revision that wrote it
//! (`born`) until the first revision that no longer contains it unchanged
//! (`retired`), so an append records one row and never copies earlier spans.
//! Any body change to a checked item that the attested writer did not make,
//! whether a raw plugin write or another host writer, retires every live span
//! and marks the body unproven from that revision onward. A plugin cannot
//! clear that mark. The tables are part of the slice-2 schema batch in
//! `item_write::SCHEMA`.
use crate::{Result, internal, missing};
use calternal_types::{Error, SpaceId};
use rusqlite::{OptionalExtension, Transaction, params};
use serde_json::Value;
use uuid::Uuid;

/// One host-composed record to bind in the same commit as its body bytes.
/// Offsets are relative to the start of the record, which the writer places
/// at the end of the prior body for an append and at zero for a whole body.
#[derive(Clone, Copy)]
pub struct SpanRecord<'a> {
    /// Host-minted opaque span ref returned in the receipt.
    pub span: &'a str,
    /// Byte length of the whole record, framing included. Nonzero.
    pub length: usize,
    /// Start of the exact middle within the record.
    pub middle_offset: usize,
    /// Byte length of the exact middle; zero for an empty middle.
    pub middle_length: usize,
    /// `ItemSpanOrigin` of the middle; required exactly when it is nonempty.
    pub origin: Option<&'a Value>,
    /// Item-local entry anchor bound once to this record.
    pub anchor: Option<&'a str>,
}

/// One recorded span at a pinned revision, in absolute body offsets.
#[derive(Clone, Debug, PartialEq)]
pub struct RecordedSpan {
    /// Opaque span ref. Grants nothing.
    pub span: String,
    /// Absolute offset of the whole record.
    pub offset: u64,
    /// Byte length of the whole record.
    pub length: u64,
    /// Absolute offset of the exact middle.
    pub middle_offset: u64,
    /// Byte length of the exact middle.
    pub middle_length: u64,
    /// Middle attribution, absent for an empty middle.
    pub origin: Option<Value>,
    /// Entry anchor bound to this record, if any.
    pub anchor: Option<String>,
}

/// A bounded page of recorded spans for one pinned revision.
#[derive(Clone, Debug, PartialEq)]
pub struct SpanPage {
    /// Whether the revision has complete checked body provenance.
    pub attested: bool,
    /// Spans ordered by increasing offset, after the requested cursor.
    pub spans: Vec<RecordedSpan>,
    /// Start offset of the last returned span when more spans follow.
    pub next: Option<u64>,
}

/// How the writer changes the body under an attested record.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecordAt {
    /// After the prior body; earlier spans stay live and unchanged.
    End(usize),
    /// The record is the whole new body; earlier spans retire.
    Whole,
}

/// Mark the version about to be inserted as written by the attested writer,
/// so the generic retirement trigger leaves the item's live spans alone.
pub(crate) fn begin(tx: &Transaction<'_>, revision: &str) -> Result<()> {
    tx.execute("INSERT INTO attested_item_writes VALUES(?1)", [revision])
        .map_err(internal)?;
    Ok(())
}

/// Bind the record to the just-inserted version and clear the marker.
/// An anchor already bound to a live record of this item is a conflict
/// before any effect commits. Cost: two indexed writes, one indexed read.
pub(crate) fn record(
    tx: &Transaction<'_>,
    item: Uuid,
    seq: i64,
    revision: &str,
    at: RecordAt,
    span: SpanRecord<'_>,
) -> Result<()> {
    let id = item.to_string();
    if at == RecordAt::Whole {
        tx.execute(
            "UPDATE item_write_spans SET retired=?2 WHERE item=?1 AND retired IS NULL",
            params![id, seq],
        )
        .map_err(internal)?;
    }
    if let Some(anchor) = span.anchor {
        let taken: bool = tx
            .prepare_cached(
                "SELECT EXISTS(SELECT 1 FROM item_write_spans
                 WHERE item=?1 AND anchor=?2 AND retired IS NULL)",
            )
            .map_err(internal)?
            .query_row(params![id, anchor], |row| row.get(0))
            .map_err(internal)?;
        if taken {
            return Err(Error::conflict("entry anchor is already recorded")
                .with_details(serde_json::json!({"reason":"anchor_conflict"})));
        }
    }
    let start = match at {
        RecordAt::End(prior) => prior,
        RecordAt::Whole => 0,
    };
    let origin = span.origin.map(Value::to_string);
    tx.prepare_cached("INSERT INTO item_write_spans VALUES(?1,?2,?3,?4,NULL,?5,?6,?7,?8,?9)")
        .map_err(internal)?
        .execute(params![
            id,
            seq,
            start as i64,
            span.span,
            span.length as i64,
            (start + span.middle_offset) as i64,
            span.middle_length as i64,
            origin,
            span.anchor,
        ])
        .map_err(internal)?;
    tx.execute(
        "DELETE FROM attested_item_writes WHERE revision=?1",
        [revision],
    )
    .map_err(internal)?;
    Ok(())
}

/// Mark a newly created checked item whose first body is opaque.
pub(crate) fn unproven_from(tx: &Transaction<'_>, item: Uuid, seq: i64) -> Result<()> {
    tx.execute(
        "INSERT OR IGNORE INTO item_body_unproven VALUES(?1,?2)",
        params![item.to_string(), seq],
    )
    .map_err(internal)?;
    Ok(())
}

impl crate::Content {
    /// Page the recorded spans of one retained revision of a live checked
    /// item of `kind`, after the record starting at `after`. The caller
    /// authorizes body reads for the exact revision before and after this
    /// call and binds the cursor. Wrong kind, purged, trashed and unknown
    /// revisions share one refusal. Cost: two indexed reads and one indexed
    /// range scan of at most `limit + 1` rows; no body bytes are read.
    pub fn item_spans(
        &self,
        space: SpaceId,
        kind: &str,
        item: Uuid,
        revision: &str,
        after: Option<u64>,
        limit: usize,
    ) -> Result<SpanPage> {
        self.with_space(space, |s| {
            let seq: i64 = s
                .conn
                .prepare_cached(
                    "SELECT v.seq FROM versions v JOIN items i ON i.id=v.item
                     WHERE v.item=?1 AND v.revision=?2 AND i.kind=?3
                     AND i.mat_state NOT IN ('purging','purged')
                     AND substr(i.path,1,7) <> '.trash/'",
                )
                .map_err(internal)?
                .query_row(params![item.to_string(), revision, kind], |row| row.get(0))
                .optional()
                .map_err(internal)?
                .ok_or_else(missing)?;
            let attested: bool = s
                .conn
                .prepare_cached(
                    "SELECT NOT EXISTS(SELECT 1 FROM item_body_unproven WHERE item=?1 AND since<=?2)",
                )
                .map_err(internal)?
                .query_row(params![item.to_string(), seq], |row| row.get(0))
                .map_err(internal)?;
            let mut statement = s
                .conn
                .prepare_cached(
                    "SELECT span,start,length,middle_start,middle_length,origin,anchor
                     FROM item_write_spans
                     WHERE item=?1 AND born<=?2 AND (retired IS NULL OR retired>?2) AND start>?3
                     ORDER BY start LIMIT ?4",
                )
                .map_err(internal)?;
            let mut spans = statement
                .query_map(
                    params![
                        item.to_string(),
                        seq,
                        after.map_or(-1, |offset| offset as i64),
                        (limit + 1) as i64
                    ],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, i64>(1)?,
                            row.get::<_, i64>(2)?,
                            row.get::<_, i64>(3)?,
                            row.get::<_, i64>(4)?,
                            row.get::<_, Option<String>>(5)?,
                            row.get::<_, Option<String>>(6)?,
                        ))
                    },
                )
                .map_err(internal)?
                .map(|row| {
                    let (span, offset, length, middle_offset, middle_length, origin, anchor) =
                        row.map_err(internal)?;
                    Ok(RecordedSpan {
                        span,
                        offset: offset as u64,
                        length: length as u64,
                        middle_offset: middle_offset as u64,
                        middle_length: middle_length as u64,
                        origin: origin
                            .map(|origin| serde_json::from_str(&origin).map_err(internal))
                            .transpose()?,
                        anchor,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            let next = (spans.len() > limit).then(|| {
                spans.truncate(limit);
                spans.last().map_or(0, |span| span.offset)
            });
            Ok(SpanPage {
                attested,
                spans,
                next,
            })
        })
    }
}
