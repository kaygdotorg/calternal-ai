//! Reads. Each runs on its own read-only connection, so reads never wait
//! for the writer and the writer never waits for reads (SQLite WAL mode).

use crate::IdemRecord;
use crate::schema::open_reader;
use crate::shared::Shared;
use crate::{Error, JournalEntry, Result};
use rusqlite::{OptionalExtension, params};
use std::path::Path;

/// A safety cap on one read, whatever the caller asks for. Page sizes are
/// the caller's policy; this only stops a runaway request.
pub const MAX_PAGE: usize = 10_000;

pub(crate) fn idempotency(path: &Path, key: &str) -> Result<Option<IdemRecord>> {
    let conn = open_reader(path)?;
    conn.query_row(
        "SELECT action, payload_hash, result, seq FROM idempotency WHERE key = ?1",
        [key],
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
            ))
        },
    )
    .optional()
    .map_err(Error::internal)?
    .map(|(action, payload_hash, result, seq)| {
        Ok(IdemRecord {
            action,
            payload_hash,
            result: serde_json::from_str(&result).map_err(Error::internal)?,
            seq,
        })
    })
    .transpose()
}

/// Reads one journal page, reuses recent signed tokens, and caches the last
/// cursor for the next page request when restore detection is enabled.
pub(crate) fn since(
    path: &Path,
    shared: &Shared,
    after: i64,
    limit: usize,
) -> Result<Vec<JournalEntry>> {
    let conn = open_reader(path)?;
    let mut stmt = conn
        .prepare(
            "SELECT seq, at_ms, kind, item, data, installation, request_id, author
             FROM journal WHERE seq > ?1 ORDER BY seq LIMIT ?2",
        )
        .map_err(Error::internal)?;
    let rows = stmt
        .query_map(params![after, limit.clamp(1, MAX_PAGE) as i64], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, String>(7)?,
            ))
        })
        .map_err(Error::internal)?;
    let epoch = shared.epoch();
    let mut out = Vec::new();
    for row in rows {
        let (seq, at_ms, kind, item, data, installation, request_id, author) =
            row.map_err(Error::internal)?;
        out.push(JournalEntry {
            author: author.parse().map_err(Error::internal)?,
            seq,
            token: String::new(),
            at_ms,
            kind,
            item,
            data: serde_json::from_str(&data).map_err(Error::internal)?,
            installation,
            request_id,
        });
    }
    drop(stmt);
    if !out.is_empty() {
        let (cached, has_misses) = shared.issued_page(&epoch, out.iter().map(|entry| entry.seq));
        if has_misses {
            let mut minted = Vec::new();
            let mut protected = Vec::new();
            for (entry, cached_token) in out.iter_mut().zip(cached) {
                match cached_token {
                    Some(token) => {
                        protected.push(entry.seq);
                        entry.token = token;
                    }
                    None => {
                        let token = shared.mint(
                            &epoch,
                            entry.seq,
                            entry.at_ms,
                            &entry.kind,
                            &entry.item,
                            entry.request_id.as_deref(),
                        );
                        minted.push((entry.seq, token.clone()));
                        entry.token = token;
                    }
                }
            }
            shared.remember_issued_page(&epoch, &minted, protected.into_iter());
        } else {
            for (entry, token) in out.iter_mut().zip(cached) {
                entry.token = token.expect("a complete token-cache hit has every token");
            }
        }
        if let Some(last) = out.last() {
            shared.remember_proof(&epoch, &last.token, last.seq);
        }
    }
    Ok(out)
}
