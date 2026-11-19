//! Background generation build and journal replay. The index owns no authority.
use super::{GENERATION, MessageDecoder, MessageRecord, WIDTH, codec, internal, path, source};
use crate::{Content, Result};
use calternal_store::{ProjectionJournal, SyncToken};
use calternal_types::SpaceId;
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::Path,
    time::Duration,
};

#[cfg(test)]
mod tests;

const SCAN: i64 = 16;
const PAGE_BYTES: u64 = 16 * 1024;
const FREE_FLOOR: u64 = 1024 * 1024 * 1024;
const SCHEMA: &str = include_str!("schema.sql");
/// Schema version of `search-messages-v1.all.db`, separate from the generation.
const MIGRATIONS: calternal_store::migrations::Schema<'_> =
    calternal_store::migrations::Schema::new("message search projection", 1, &[SCHEMA], &[]);
const SECONDARY: &str = "CREATE UNIQUE INDEX IF NOT EXISTS locations_id ON locations(thread,id);
    CREATE INDEX IF NOT EXISTS locations_thread ON locations(thread,start);
    CREATE INDEX IF NOT EXISTS postings_newest ON postings(term,stamp DESC,newest DESC,block);";

pub(in crate::search) enum Progress {
    Ready,
    More,
    Idle,
}

/// Advance the per-space generation toward the durable journal head.
/// Each scan step handles a bounded head page and each live transaction
/// replaces one complete revision and its watermark together.
pub(in crate::search) fn advance(
    content: &Content,
    space: SpaceId,
    journal: &ProjectionJournal,
    head: &SyncToken,
    content_path: &Path,
    decode: MessageDecoder,
) -> Result<Progress> {
    let mut primary = crate::pool::open_reader(content_path).map_err(internal)?;
    let path = path(&content.roots.appdata, space);
    if !path.is_file() && !source::exists(&primary)? {
        return Ok(Progress::Ready);
    }
    calternal_fsio::private::file(&path).map_err(internal)?;
    let mut db = crate::search::wal::open(&path).map_err(internal)?;
    db.busy_timeout(Duration::from_millis(25))
        .map_err(internal)?;
    db.execute_batch(
        "PRAGMA page_size=16384; PRAGMA cache_size=-32768; PRAGMA synchronous=NORMAL;",
    )
    .map_err(internal)?;
    if !reserve(&db, &path)? {
        return Ok(Progress::Idle);
    }
    calternal_store::migrations::migrate(&db, MIGRATIONS).map_err(internal)?;
    let schema = state(&db).map_err(internal)?;
    if schema.as_ref().is_none_or(|s| !s.matches(head)) {
        db.pragma_update(None, "journal_mode", "DELETE")
            .map_err(internal)?;
        let tx = db.transaction().map_err(internal)?;
        tx.execute_batch(
            "DROP TABLE IF EXISTS postings;
             DROP TABLE IF EXISTS blocks;
             DROP TABLE IF EXISTS terms;
             DROP TABLE IF EXISTS locations;
             DROP TABLE IF EXISTS threads;
             DELETE FROM build_state;",
        )
        .map_err(internal)?;
        tx.execute_batch(SCHEMA).map_err(internal)?;
        tx.execute(
            "INSERT INTO build_state VALUES(1,?1,?2,?3,0,0,0)",
            params![GENERATION, head.epoch, head.seq],
        )
        .map_err(internal)?;
        tx.commit().map_err(internal)?;
        return Ok(Progress::More);
    }
    let mut state = schema.expect("matching state");
    if !crate::search::worker::checkpoint_if_needed(&db, &path)? {
        return Ok(Progress::Idle);
    }
    if let Some(after) = state.after {
        let snapshot = primary.transaction().map_err(internal)?;
        if crate::search::source::pending(&snapshot)? {
            return Ok(Progress::Idle);
        }
        let folder = calternal_fsio::beneath::Folder::open_existing(&content.roots.userdata_ssd)
            .and_then(|root| root.open_sub(&format!("spaces/{space}"), false))
            .ok();
        let heads = source::page(&snapshot, after, SCAN)?;
        let tx = db.transaction().map_err(internal)?;
        let mut batch = Vec::new();
        for thread in &heads {
            ingest(&snapshot, folder.as_ref(), &tx, thread, decode, &mut batch)?;
        }
        // Each run is time sorted and its postings are written in B-tree key
        // order. The block maxima form the merged time order across runs.
        write_run(&tx, &mut state, &mut batch)?;
        state.after = (heads.len() == SCAN as usize).then(|| heads.last().unwrap().slot);
        save(&tx, &state)?;
        tx.commit().map_err(internal)?;
        return Ok(Progress::More);
    }
    if !state.ready {
        // The bulk load is private until readiness. Build this index once,
        // using SQLite's sorted CREATE INDEX path in rollback mode.
        db.execute_batch(SECONDARY).map_err(internal)?;
        db.pragma_update(None, "journal_mode", "WAL")
            .map_err(internal)?;
    }
    if state.ready && state.watermark == head.seq {
        return Ok(Progress::Ready);
    }
    let changes = if state.watermark == head.seq {
        Vec::new()
    } else {
        journal.since(state.watermark, 1).map_err(internal)?
    };
    let snapshot = primary.transaction().map_err(internal)?;
    if crate::search::source::pending(&snapshot)? {
        return Ok(Progress::Idle);
    }
    let tx = db.transaction().map_err(internal)?;
    for event in changes {
        if let Some(id) = event.item {
            let thread = source::get(&snapshot, &id)?;
            if !replace(&snapshot, &tx, &mut state, &id, thread.as_ref(), decode)? {
                // Keep this journal event as the continuation cursor. The
                // projection revision records progress within the patch chain.
                save(&tx, &state)?;
                tx.commit().map_err(internal)?;
                return Ok(Progress::More);
            }
        }
        state.watermark = event.sequence;
    }
    let current = journal.head().map_err(internal)?;
    state.ready |= current.seq == state.watermark;
    save(&tx, &state)?;
    tx.commit().map_err(internal)?;
    Ok(
        if state.ready && state.matches(&current) && state.watermark == current.seq {
            Progress::Ready
        } else {
            Progress::More
        },
    )
}

fn reserve(db: &Connection, path: &Path) -> Result<bool> {
    let free = calternal_fsio::available_bytes(path).map_err(internal)?;
    let size = std::fs::metadata(path).map_err(internal)?.len();
    // Allow the current database size for an unpublished rollback transaction
    // and a separate 64 MiB for live WAL. Stop before spending the reserve.
    let reserve = FREE_FLOOR + size + super::super::wal::LIMIT as u64;
    if free <= reserve {
        return Ok(false);
    }
    let pages = db
        .pragma_query_value(None, "page_count", |r| r.get::<_, i64>(0))
        .map_err(internal)?;
    db.pragma_update(
        None,
        "max_page_count",
        pages.saturating_add(((free - reserve) / PAGE_BYTES) as i64),
    )
    .map_err(internal)?;
    Ok(true)
}

pub(super) struct State {
    generation: String,
    epoch: String,
    pub(super) watermark: i64,
    after: Option<i64>,
    pub(super) ready: bool,
    next_block: i64,
}

impl State {
    pub(super) fn matches(&self, head: &SyncToken) -> bool {
        self.generation == GENERATION && self.epoch == head.epoch && self.watermark <= head.seq
    }
}

pub(super) fn state(db: &Connection) -> rusqlite::Result<Option<State>> {
    if !db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='build_state')",
        [],
        |r| r.get::<_, bool>(0),
    )? {
        return Ok(None);
    }
    db.prepare_cached(
        "SELECT generation,epoch,watermark,scan_after,ready,next_block FROM build_state",
    )?
    .query_row([], |r| {
        Ok(State {
            generation: r.get(0)?,
            epoch: r.get(1)?,
            watermark: r.get(2)?,
            after: r.get(3)?,
            ready: r.get(4)?,
            next_block: r.get(5)?,
        })
    })
    .optional()
}

fn save(tx: &Transaction<'_>, state: &State) -> Result<()> {
    tx.execute(
        "UPDATE build_state SET watermark=?1,scan_after=?2,ready=?3,next_block=?4 WHERE id=1",
        params![state.watermark, state.after, state.ready, state.next_block],
    )
    .map_err(internal)?;
    Ok(())
}

struct Input {
    record: MessageRecord,
    thread: i64,
    words: Vec<String>,
}

fn ingest(
    primary: &Connection,
    folder: Option<&calternal_fsio::beneath::Folder>,
    tx: &Transaction<'_>,
    thread: &source::Head,
    decode: MessageDecoder,
    batch: &mut Vec<Input>,
) -> Result<()> {
    let bytes = source::bytes(primary, folder, thread)?;
    tx.execute(
        "INSERT INTO threads VALUES(?1,?2,?3)",
        params![thread.slot, thread.id, thread.revision],
    )
    .map_err(internal)?;
    ingest_records(&bytes, thread.slot, 0, false, decode, batch)
}

fn ingest_records(
    bytes: &[u8],
    slot: i64,
    offset: usize,
    fragment: bool,
    decode: MessageDecoder,
    batch: &mut Vec<Input>,
) -> Result<()> {
    for mut record in decode(bytes, fragment)? {
        let body = bytes
            .get(record.body.clone())
            .and_then(|b| std::str::from_utf8(b).ok())
            .ok_or_else(|| internal("invalid message range"))?;
        let mut words: Vec<_> = super::message_terms(body).collect();
        words.sort_unstable();
        words.dedup();
        record.record.start += offset;
        record.record.end += offset;
        record.body.start += offset;
        record.body.end += offset;
        batch.push(Input {
            record,
            thread: slot,
            words,
        });
    }
    Ok(())
}

fn write_run(tx: &Transaction<'_>, state: &mut State, batch: &mut [Input]) -> Result<()> {
    batch.sort_unstable_by_key(|input| (input.record.stamp, input.record.id));
    let mut loc = tx
        .prepare_cached("INSERT INTO locations VALUES(?1,?2,?3,?4,?5,?6,?7)")
        .map_err(internal)?;
    let mut find = tx
        .prepare_cached("SELECT id FROM terms WHERE word=?1")
        .map_err(internal)?;
    let mut terms = HashMap::<&str, i64>::new();
    for block in batch.chunks(WIDTH) {
        let block_id = state.next_block;
        state.next_block += 1;
        let first = &block[0].record;
        tx.prepare_cached("INSERT INTO blocks VALUES(?1,?2,?3)")
            .map_err(internal)?
            .execute(params![block_id, first.stamp, first.id.as_bytes()])
            .map_err(internal)?;
        let mut postings = BTreeMap::<i64, Vec<u16>>::new();
        for (offset, input) in block.iter().enumerate() {
            let r = &input.record;
            loc.execute(params![
                block_id * WIDTH as i64 + offset as i64,
                r.id.as_bytes(),
                r.stamp,
                r.offset_seconds,
                input.thread,
                r.record.start as i64,
                r.record.len() as i64
            ])
            .map_err(internal)?;
            for word in &input.words {
                let term = match terms.get(word.as_str()) {
                    Some(id) => *id,
                    None => {
                        let id = match find
                            .query_row([word], |r| r.get(0))
                            .optional()
                            .map_err(internal)?
                        {
                            Some(id) => id,
                            None => tx
                                .prepare_cached("INSERT INTO terms(word) VALUES(?1) RETURNING id")
                                .map_err(internal)?
                                .query_row([word], |r| r.get(0))
                                .map_err(internal)?,
                        };
                        terms.insert(word.as_str(), id);
                        id
                    }
                };
                postings.entry(term).or_default().push(offset as u16);
            }
        }
        for (term, positions) in postings {
            let last = &block[usize::from(*positions.last().unwrap())].record;
            write_posting(tx, term, block_id, (last.stamp, last.id), &positions)?;
        }
    }
    Ok(())
}

fn write_posting(
    tx: &Transaction<'_>,
    term: i64,
    block: i64,
    newest: (i64, uuid::Uuid),
    positions: &[u16],
) -> Result<()> {
    tx.prepare_cached("INSERT INTO postings VALUES(?1,?2,?3,?4,?5)")
        .map_err(internal)?
        .execute(params![
            block,
            term,
            newest.0,
            newest.1.as_bytes(),
            codec::encode(positions)
        ])
        .map_err(internal)?;
    Ok(())
}

fn replace(
    primary: &Connection,
    tx: &Transaction<'_>,
    state: &mut State,
    id: &str,
    thread: Option<&source::Head>,
    decode: MessageDecoder,
) -> Result<bool> {
    if let Some((slot, revision)) = tx
        .prepare_cached("SELECT slot,revision FROM threads WHERE id=?1")
        .map_err(internal)?
        .query_row([id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
        .optional()
        .map_err(internal)?
    {
        if thread.is_some_and(|t| t.revision == revision) {
            return Ok(true);
        }
        if let Some(thread) = thread
            && let Some(fragments) = source::fragments(primary, thread, &revision)?
        {
            let mut batch = Vec::new();
            for (offset, bytes) in fragments.rows {
                ingest_records(&bytes, slot, offset, true, decode, &mut batch)?;
            }
            write_run(tx, state, &mut batch)?;
            tx.execute(
                "UPDATE threads SET revision=?1 WHERE slot=?2",
                params![fragments.revision, slot],
            )
            .map_err(internal)?;
            return Ok(fragments.revision == thread.revision);
        }
        remove(tx, slot)?;
    }
    if let Some(thread) = thread {
        let mut batch = Vec::new();
        ingest(primary, None, tx, thread, decode, &mut batch)?;
        write_run(tx, state, &mut batch)?;
    }
    Ok(true)
}

fn remove(tx: &Transaction<'_>, slot: i64) -> Result<()> {
    let removed: Vec<i64> = tx
        .prepare_cached("DELETE FROM locations WHERE thread=?1 RETURNING position")
        .map_err(internal)?
        .query_map([slot], |r| r.get(0))
        .map_err(internal)?
        .collect::<rusqlite::Result<_>>()
        .map_err(internal)?;
    tx.execute("DELETE FROM threads WHERE slot=?1", [slot])
        .map_err(internal)?;
    let mut affected = BTreeMap::<i64, HashSet<u16>>::new();
    for position in removed {
        affected
            .entry(position / WIDTH as i64)
            .or_default()
            .insert((position % WIDTH as i64) as u16);
    }
    for (block, removed) in affected {
        let start = block * WIDTH as i64;
        let has_other = tx
            .prepare_cached(
                "SELECT EXISTS(SELECT 1 FROM locations WHERE position>=?1 AND position<?2)",
            )
            .map_err(internal)?
            .query_row(params![start, start + WIDTH as i64], |r| {
                r.get::<_, bool>(0)
            })
            .map_err(internal)?;
        if !has_other {
            tx.prepare_cached("DELETE FROM blocks WHERE id=?1")
                .map_err(internal)?
                .execute([block])
                .map_err(internal)?;
            let terms: Vec<i64> = tx
                .prepare_cached("DELETE FROM postings WHERE block=?1 RETURNING term")
                .map_err(internal)?
                .query_map([block], |r| r.get(0))
                .map_err(internal)?
                .collect::<rusqlite::Result<_>>()
                .map_err(internal)?;
            for term in terms {
                delete_unused(tx, term)?;
            }
            continue;
        }
        let postings: Vec<(i64, Vec<u8>)> = tx
            .prepare_cached("SELECT term,positions FROM postings WHERE block=?1")
            .map_err(internal)?
            .query_map([block], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(internal)?
            .collect::<rusqlite::Result<_>>()
            .map_err(internal)?;
        let mut affected_terms = Vec::new();
        for (term, packed) in postings {
            let mut positions =
                codec::decode(&packed).ok_or_else(|| internal("invalid message posting"))?;
            let before = positions.len();
            positions.retain(|p| !removed.contains(p));
            if before == positions.len() {
                continue;
            }
            tx.prepare_cached("DELETE FROM postings WHERE block=?1 AND term=?2")
                .map_err(internal)?
                .execute(params![block, term])
                .map_err(internal)?;
            if let Some(&max) = positions.last() {
                let mut key = tx
                    .prepare_cached("SELECT stamp,id FROM locations WHERE position=?1")
                    .map_err(internal)?;
                let last = key
                    .query_row([start + i64::from(max)], |r| {
                        Ok((r.get(0)?, super::uuid(r, 1)?))
                    })
                    .map_err(internal)?;
                write_posting(tx, term, block, last, &positions)?;
            } else {
                affected_terms.push(term);
            }
        }
        for term in affected_terms {
            delete_unused(tx, term)?;
        }
    }
    Ok(())
}

fn delete_unused(tx: &Transaction<'_>, term: i64) -> Result<()> {
    tx.prepare_cached(
        "DELETE FROM terms WHERE id=?1 AND NOT EXISTS(SELECT 1 FROM postings WHERE term=?1)",
    )
    .map_err(internal)?
    .execute([term])
    .map_err(internal)?;
    Ok(())
}
