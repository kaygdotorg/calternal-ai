//! Newest-first merge of compressed posting blocks and exact revision reads.
use super::{WIDTH, codec, internal, path, source, writer};
use crate::search::{IndexedHit, IndexedPage, SearchKey, SearchProbe};
use crate::{Content, Error, Result, SpaceId};
use calternal_store::SyncToken;
use rusqlite::{Connection, OptionalExtension, params};
use std::{collections::HashMap, ops::Range};
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};
use uuid::Uuid;

const CANDIDATES: usize = 2048;
pub(super) type Key = (i64, Uuid);

pub(in crate::search) fn search(
    appdata: &std::path::Path,
    primary: &Connection,
    space: SpaceId,
    head: &SyncToken,
    terms: &[String],
    phrase: bool,
    probe: &SearchProbe<'_>,
) -> Result<IndexedPage> {
    let path = path(appdata, space);
    if !path.is_file() {
        return Ok(IndexedPage {
            hits: Vec::new(),
            partial: source::exists(primary)?,
        });
    }
    let pending = || IndexedPage {
        hits: Vec::new(),
        partial: true,
    };
    let Some(db) = crate::search::readers::while_available(|| {
        let db = crate::pool::open_reader(&path)?;
        db.busy_timeout(std::time::Duration::from_millis(20))?;
        db.pragma_update(None, "cache_size", -256)?;
        Ok(db)
    })?
    else {
        return Ok(pending());
    };
    let index = match crate::search::readers::while_available(|| db.unchecked_transaction())? {
        Some(index) => index,
        None => return Ok(pending()),
    };
    let state = crate::search::readers::while_available(|| writer::state(&index))?.flatten();
    let Some(state) = state.filter(|s| s.ready && s.matches(head)) else {
        return Ok(pending());
    };
    let mut hit_page = matches(&index, primary, terms, phrase, probe)?;
    hit_page.partial |= state.watermark != head.seq;
    Ok(hit_page)
}

struct Location {
    key: Key,
    offset_seconds: i32,
    thread: String,
    revision: String,
    bytes: Range<usize>,
}

fn location(row: &rusqlite::Row<'_>) -> rusqlite::Result<Location> {
    let start = row.get::<_, u32>(5)? as usize;
    let length = row.get::<_, u32>(6)? as usize;
    Ok(Location {
        key: (row.get(0)?, super::uuid(row, 1)?),
        offset_seconds: row.get(2)?,
        thread: row.get(3)?,
        revision: row.get(4)?,
        bytes: start..start.saturating_add(length),
    })
}

fn matches(
    index: &Connection,
    primary: &Connection,
    terms: &[String],
    phrase: bool,
    probe: &SearchProbe<'_>,
) -> Result<IndexedPage> {
    let mut page = IndexedPage::empty();
    let mut ids = Vec::new();
    let mut term = index
        .prepare_cached("SELECT id FROM terms WHERE word=?1")
        .map_err(internal)?;
    for word in terms {
        let Some(id) = term
            .query_row([word], |r| r.get::<_, i64>(0))
            .optional()
            .map_err(internal)?
        else {
            return Ok(page);
        };
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    let Some((&driver, others)) = ids.split_first() else {
        return Ok(page);
    };
    let before = match probe.after {
        Some(key) if key.0 > 2 => return Ok(page),
        Some(key) if key.0 == 2 => {
            let raw = u64::from_str_radix(&key.1, 16).map_err(internal)?;
            Some((
                (raw ^ (1 << 63) ^ u64::MAX) as i64,
                Uuid::from_u128(Uuid::parse_str(&key.2).map_err(internal)?.as_u128() ^ u128::MAX),
            ))
        }
        _ => None,
    };
    let mut blocks = index
        .prepare_cached(
            "SELECT p.stamp,p.newest,p.block,p.positions FROM postings p INDEXED BY postings_newest
         JOIN blocks b ON b.id=p.block WHERE p.term=?1
         AND (?2 IS NULL OR (b.oldest_stamp,b.oldest)<(?2,?3))
         ORDER BY p.stamp DESC,p.newest DESC,p.block",
        )
        .map_err(internal)?;
    let mut blocks = blocks
        .query(params![
            driver,
            before.map(|b| b.0),
            before.map(|b| *b.1.as_bytes())
        ])
        .map_err(internal)?;
    let mut find_other = index
        .prepare_cached("SELECT positions FROM postings WHERE block=?1 AND term=?2")
        .map_err(internal)?;
    let mut locations = index
        .prepare_cached(
            "SELECT l.stamp,l.id,l.offset_seconds,t.id,t.revision,l.start,l.length FROM locations l
         JOIN threads t ON t.slot=l.thread WHERE l.position=?1",
        )
        .map_err(internal)?;
    let mut revisions = HashMap::<String, Option<source::Revision>>::new();
    let limit = probe.limit.min(51);
    let mut hits: Vec<(Key, IndexedHit)> = Vec::new();
    let mut work = 0;
    'blocks: while let Some(row) = blocks.next().map_err(internal)? {
        let newest: Key = (
            row.get(0).map_err(internal)?,
            super::uuid(row, 1).map_err(internal)?,
        );
        if hits.len() == limit && hits.last().is_some_and(|(key, _)| *key >= newest) {
            break;
        }
        let id: i64 = row.get(2).map_err(internal)?;
        let mut positions = positions(
            row.get_ref(3)
                .map_err(internal)?
                .as_blob()
                .map_err(internal)?,
        )?;
        for &term in others {
            let Some(packed) = find_other
                .query_row(params![id, term], |r| r.get::<_, Vec<u8>>(0))
                .optional()
                .map_err(internal)?
            else {
                continue 'blocks;
            };
            let other = self::positions(&packed)?;
            positions.retain(|p| other.binary_search(p).is_ok());
        }
        for position in positions.into_iter().rev() {
            let loc = locations
                .query_row([id * WIDTH as i64 + i64::from(position)], location)
                .map_err(internal)?;
            if before.is_some_and(|key| loc.key >= key) {
                continue;
            }
            if hits.len() == limit && hits.last().is_some_and(|(key, _)| *key >= loc.key) {
                break;
            }
            work += 1;
            if work > CANDIDATES {
                page.partial = true;
                break 'blocks;
            }
            let revision = match revisions.entry(loc.thread.clone()) {
                std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
                std::collections::hash_map::Entry::Vacant(e) => e.insert(source::revision(
                    primary,
                    Uuid::parse_str(&loc.thread).map_err(internal)?,
                    &loc.revision,
                )?),
            };
            let Some(revision) = revision else {
                continue;
            };
            let bytes = source::slice(primary, revision, loc.bytes)?;
            let record = std::str::from_utf8(&bytes).map_err(internal)?;
            let Some((_, body)) = record.split_once(" -->\n\n") else {
                return Err(internal("invalid indexed message record"));
            };
            let Some(excerpt) = excerpt(body, terms, phrase) else {
                continue;
            };
            let at = OffsetDateTime::from_unix_timestamp(loc.key.0)
                .map_err(internal)?
                .to_offset(UtcOffset::from_whole_seconds(loc.offset_seconds).map_err(internal)?)
                .format(&Rfc3339)
                .map_err(internal)?;
            let hit = IndexedHit {
                key: SearchKey(
                    2,
                    format!("{:016x}", (loc.key.0 as u64) ^ (1 << 63) ^ u64::MAX),
                    Uuid::from_u128(loc.key.1.as_u128() ^ u128::MAX).to_string(),
                ),
                id: loc.thread,
                kind: "threads".into(),
                title: revision.title.clone(),
                excerpt,
                message: Some((loc.key.1, at)),
            };
            let at = hits.partition_point(|(key, _)| *key > loc.key);
            hits.insert(at, (loc.key, hit));
            hits.truncate(limit);
        }
    }
    page.hits = hits.into_iter().map(|(_, hit)| hit).collect();
    Ok(page)
}

fn positions(raw: &[u8]) -> Result<Vec<u16>> {
    codec::decode(raw).ok_or_else(|| internal("invalid indexed message positions"))
}

fn excerpt(body: &str, terms: &[String], phrase: bool) -> Option<String> {
    let mut range = None;
    let mut window = std::collections::VecDeque::new();
    for word in body
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
    {
        let offset = word.as_ptr() as usize - body.as_ptr() as usize;
        let end = offset + word.len();
        let folded = crate::search::fold_title(word);
        if !phrase {
            if terms.contains(&folded) {
                range = Some(offset..end);
                break;
            }
            continue;
        }
        window.push_back((offset, folded));
        if window.len() > terms.len() {
            window.pop_front();
        }
        if window.len() == terms.len() && window.iter().map(|(_, word)| word).eq(terms) {
            range = Some(window.front()?.0..end);
            break;
        }
    }
    let range = range?;
    let left = body[..range.start]
        .char_indices()
        .rev()
        .nth(50)
        .map_or(0, |(i, _)| i);
    let safe = |text: &str| {
        text.chars()
            .filter(|c| !matches!(c, '\u{1}' | '\u{2}'))
            .collect::<String>()
    };
    let tail: String = body[range.end..].chars().take(100).collect();
    Some(format!(
        "{}\u{1}{}\u{2}{}",
        safe(&body[left..range.start]),
        safe(&body[range.clone()]),
        safe(&tail)
    ))
}

impl Content {
    /// Read one current message frame by stable UUID inside an already
    /// authorized thread. Uses one locator lookup and one exact revision BLOB
    /// window; a missing, removed or stale message returns a caller-safe error.
    pub fn search_message(
        &self,
        space: SpaceId,
        thread: Uuid,
        message: Uuid,
    ) -> Result<(String, Vec<u8>)> {
        let _step = self.lifecycle.step(space.as_uuid())?;
        let path = path(&self.roots.appdata, space);
        let missing = || Error::not_found("thread message not found");
        if !path.is_file() {
            return Err(missing());
        }
        let db = crate::pool::open_reader(&path).map_err(internal)?;
        let range = db.prepare_cached(
            "SELECT t.revision,l.start,l.length FROM threads t JOIN locations l ON l.thread=t.slot
             JOIN build_state s ON s.id=1 AND s.ready=1 AND s.generation=?3
             WHERE t.id=?1 AND l.id=?2"
        ).map_err(internal)?.query_row(params![thread.to_string(),message.as_bytes(),super::GENERATION], |r| {
            let start = r.get::<_,u32>(1)? as usize;
            let length = r.get::<_,u32>(2)? as usize;
            Ok(super::MessageRange { revision:r.get(0)?,bytes:start..start.saturating_add(length) })
        }).optional().map_err(internal)?.ok_or_else(missing)?;
        let mut primary =
            crate::pool::open_reader(&crate::db::space_path(&self.roots.appdata, space))
                .map_err(internal)?;
        let snapshot = primary.transaction().map_err(internal)?;
        if crate::search::source::pending(&snapshot)? {
            return Err(Error::unavailable("thread message index is catching up"));
        }
        let revision = source::revision(&snapshot, thread, &range.revision)?.ok_or_else(missing)?;
        let bytes = source::slice(&snapshot, &revision, range.bytes)?;
        Ok((range.revision, bytes))
    }
}

impl Content {
    /// Open the end of the exact current thread from its ready record locator.
    /// Returns at most 64 records and 128 KiB of contiguous staged Markdown.
    /// If the newest record alone exceeds that bound, returns only its final
    /// 128 KiB and marks it as a clipped record. No userdata file is opened.
    pub fn search_thread_tail(
        &self,
        space: SpaceId,
        thread: Uuid,
        revision: &str,
    ) -> Result<(Vec<u8>, bool)> {
        const LIMIT: usize = 128 * 1024;
        let _step = self.lifecycle.step(space.as_uuid())?;
        let missing = || Error::unavailable("thread message index is catching up");
        let index = path(&self.roots.appdata, space);
        if !index.is_file() {
            return Err(missing());
        }
        let db = crate::pool::open_reader(&index).map_err(internal)?;
        let mut lines = db
            .prepare_cached(
                "SELECT l.start,l.length FROM threads t JOIN locations l ON l.thread=t.slot
             JOIN build_state s ON s.id=1 AND s.ready=1 AND s.generation=?3
             WHERE t.id=?1 AND t.revision=?2 ORDER BY l.start DESC LIMIT 64",
            )
            .map_err(internal)?;
        let mut rows = lines
            .query(params![thread.to_string(), revision, super::GENERATION])
            .map_err(internal)?;
        let Some(first) = rows.next().map_err(internal)? else {
            return Err(missing());
        };
        let latest: u32 = first.get(0).map_err(internal)?;
        let len: u32 = first.get(1).map_err(internal)?;
        let end = latest as usize + len as usize;
        let clipped = len as usize > LIMIT;
        let floor = end.saturating_sub(LIMIT);
        let mut start = floor.max(latest as usize);
        if !clipped {
            while let Some(row) = rows.next().map_err(internal)? {
                let earlier = row.get::<_, u32>(0).map_err(internal)? as usize;
                if earlier < floor {
                    break;
                }
                start = earlier;
            }
        }

        let mut primary =
            crate::pool::open_reader(&crate::db::space_path(&self.roots.appdata, space))
                .map_err(internal)?;
        let snapshot = primary.transaction().map_err(internal)?;
        if crate::search::source::pending(&snapshot)? {
            return Err(missing());
        }
        let current = source::revision(&snapshot, thread, revision)?.ok_or_else(missing)?;
        Ok((source::slice(&snapshot, &current, start..end)?, clipped))
    }
}
