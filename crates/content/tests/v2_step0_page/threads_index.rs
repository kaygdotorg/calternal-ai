//! Scoped Threads Bind over the existing message schema and Elias–Fano codec.
#[path = "../../src/search/messages/codec.rs"]
mod codec;
#[path = "threads_combined.rs"]
pub(super) mod combined;
use super::{RUNS, WINDOW, summary};
use calternal_content::message_terms;
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    path::Path,
    time::Instant,
};
use uuid::Uuid;

const WIDTH: usize = 16_384;
const THREADS: usize = 100_000;
const BUDGET: usize = 160_000;
const ROW_BUDGET: usize = 4_096;
const MESSAGE_SCHEMA: &str = include_str!("../../src/search/messages/schema.sql");
const TITLE_SCHEMA: &str = include_str!("../../src/search/schema.sql");
const SECONDARY: &str = "
 CREATE UNIQUE INDEX locations_id ON locations(thread,id);
 CREATE INDEX postings_newest ON postings(term,stamp DESC,newest DESC,block);
 CREATE TABLE term_stats(term INTEGER PRIMARY KEY, doc INTEGER NOT NULL) STRICT;
 CREATE TABLE scoped_state(authority INTEGER NOT NULL, result INTEGER NOT NULL);
 INSERT INTO scoped_state VALUES(1,1);";

pub(super) fn message_schema(conn: &Connection) {
    conn.pragma_update(None, "page_size", 16_384).unwrap();
    conn.execute_batch(MESSAGE_SCHEMA).unwrap();
    conn.execute_batch(TITLE_SCHEMA).unwrap();
    conn.execute_batch(SECONDARY).unwrap();
}

fn id(thread: usize) -> String {
    Uuid::from_u128(thread as u128 + 3).to_string()
}

fn amount(thread: usize) -> usize {
    // A small-and-long-tail mix, from 2 to 32 messages.
    match thread % 20 {
        0 => 32,
        1..=3 => 16,
        4..=9 => 8,
        _ => 2,
    }
}

fn body(thread: usize, turn: usize) -> String {
    let phrases = [
        "I checked the current project notes before taking the next step. The previous decision still fits the schedule.",
        "The new draft is ready for review. Please check the timeline and confirm whether the checklist is complete.",
        "I looked through the reference material and kept the existing approach. The remaining questions concern the next meeting.",
        "We can try a smaller change first. I'll send an update when the verification finishes and keep a record of the outcome.",
    ];
    let band = (thread.wrapping_mul(37) + turn * 19) % 1_000;
    let topic = if band < 24 {
        "analysis analytic atlas marker"
    } else if band < 48 {
        "analysis analytic atlas"
    } else if band < 80 {
        "analysis analytic"
    } else if band < 125 {
        "analysis"
    } else {
        "agenda"
    };
    format!(
        "{} {topic} topic{:03} case{:03}",
        phrases[(thread + turn) % phrases.len()],
        thread % 347,
        (thread * 7 + turn) % 937
    )
}

struct Row {
    slot: i64,
    ordinal: usize,
    words: Vec<String>,
}

fn write_block(tx: &Connection, block: usize, rows: &[Row]) {
    if rows.is_empty() {
        return;
    }
    let message = |ordinal: usize| Uuid::from_u128(1_000_000 + ordinal as u128);
    let mut loc = tx
        .prepare_cached("INSERT INTO locations VALUES(?1,?2,?3,0,?4,0,0)")
        .unwrap();
    let mut terms = BTreeMap::<&str, Vec<u16>>::new();
    tx.execute(
        "INSERT INTO blocks VALUES(?1,?2,?3)",
        params![
            block as i64,
            (rows[0].ordinal / 4) as i64,
            message(rows[0].ordinal).as_bytes()
        ],
    )
    .unwrap();
    for (i, row) in rows.iter().enumerate() {
        loc.execute(params![
            (block * WIDTH + i) as i64,
            message(row.ordinal).as_bytes(),
            (row.ordinal / 4) as i64,
            row.slot
        ])
        .unwrap();
        for word in &row.words {
            terms.entry(word).or_default().push(i as u16);
        }
    }
    for (word, slots) in terms {
        tx.prepare_cached("INSERT OR IGNORE INTO terms(word) VALUES(?1)")
            .unwrap()
            .execute([word])
            .unwrap();
        let term: i64 = tx
            .prepare_cached("SELECT id FROM terms WHERE word=?1")
            .unwrap()
            .query_row([word], |r| r.get(0))
            .unwrap();
        let last = rows[*slots.last().unwrap() as usize].ordinal;
        tx.prepare_cached("INSERT INTO postings VALUES(?1,?2,?3,?4,?5)")
            .unwrap()
            .execute(params![
                block as i64,
                term,
                (last / 4) as i64,
                message(last).as_bytes(),
                codec::encode(&slots)
            ])
            .unwrap();
        tx.prepare_cached("INSERT INTO term_stats VALUES(?1,?2) ON CONFLICT(term) DO UPDATE SET doc=doc+excluded.doc")
            .unwrap().execute(params![term,slots.len() as i64]).unwrap();
    }
}

pub(super) fn seed(path: &Path, hidden_edit: bool) -> (usize, usize, usize) {
    let mut conn = Connection::open(path).unwrap();
    message_schema(&conn);
    let tx = conn.transaction().unwrap();
    let mut batch = Vec::with_capacity(WIDTH);
    let (mut messages, mut bytes, mut threads, mut block) = (0, 0, 0, 0);
    for thread in 0..THREADS {
        // Mixed visibility from a single scope fixed before indexing. An
        // excluded thread can change both title and transcript in the source.
        if thread % 11 == 0 {
            if hidden_edit && thread == 0 {
                assert!(message_terms("analysis analytic atlas privateonly").count() == 4);
            }
            continue;
        }
        let title = if thread % 3 == 0 {
            format!("Project {thread:06}")
        } else {
            format!("Notes {thread:06}")
        };
        tx.prepare_cached("INSERT INTO search_docs VALUES(?1,?2,'threads',?3,?4)")
            .unwrap()
            .execute(params![
                thread as i64 + 2,
                id(thread),
                title,
                title.to_lowercase()
            ])
            .unwrap();
        tx.prepare_cached("INSERT INTO threads VALUES(?1,?2,'fixture')")
            .unwrap()
            .execute(params![thread as i64 + 2, id(thread)])
            .unwrap();
        threads += 1;
        for turn in 0..amount(thread) {
            let body = body(thread, turn);
            bytes += body.len();
            messages += 1;
            let mut words = message_terms(&body).collect::<Vec<_>>();
            words.sort_unstable();
            words.dedup();
            batch.push(Row {
                slot: thread as i64 + 2,
                ordinal: messages,
                words,
            });
            if batch.len() == WIDTH {
                write_block(&tx, block, &batch);
                block += 1;
                batch.clear();
            }
        }
    }
    write_block(&tx, block, &batch);
    tx.commit().unwrap();
    (threads, messages, bytes)
}

#[derive(Clone, Copy, Debug)]
enum Shape {
    One,
    Two,
    Three,
    ThreeSelective,
    Prefix,
    Title,
    Broad,
}
impl Shape {
    fn plan(self, conn: &Connection) -> Vec<(i64, usize)> {
        if matches!(self, Self::Title) {
            return Vec::new();
        }
        if matches!(self, Self::Prefix) {
            let mut q=conn.prepare_cached("SELECT t.id,s.doc FROM terms t
              JOIN term_stats s ON s.term=t.id WHERE t.word>=?1 AND t.word<?2 ORDER BY t.word LIMIT 33").unwrap();
            return q
                .query_map(["analy", "analz"], |r| {
                    Ok((r.get(0)?, r.get::<_, i64>(1)? as usize))
                })
                .unwrap()
                .collect::<rusqlite::Result<_>>()
                .unwrap();
        }
        let terms = match self {
            Self::One => "analysis",
            Self::Two => "analysis analytic",
            Self::Three => "analysis analytic atlas",
            Self::ThreeSelective => "analysis analytic marker",
            Self::Broad => "the",
            _ => unreachable!(),
        };
        literal_plan(conn, terms)
    }
}

fn literal_plan(conn: &Connection, terms: &str) -> Vec<(i64, usize)> {
    let mut q = conn
        .prepare_cached(
            "SELECT t.id,s.doc FROM terms t JOIN term_stats s ON s.term=t.id WHERE t.word=?1",
        )
        .unwrap();
    let mut list = Vec::new();
    for term in message_terms(terms) {
        let Some(postings) = q
            .query_row([term], |r| Ok((r.get(0)?, r.get::<_, i64>(1)? as usize)))
            .optional()
            .unwrap()
        else {
            return Vec::new();
        };
        list.push(postings);
    }
    list.sort_unstable_by_key(|entry| entry.1);
    list
}

#[derive(Debug, PartialEq, Eq)]
struct Window {
    rows: Vec<String>,
    bound: usize,
    postings: usize,
    lookups: usize,
    blocks: usize,
    descriptors: usize,
    refine: bool,
}

fn record(conn: &Connection, shape: Shape, depth: usize) -> Window {
    let tx = conn.unchecked_transaction().unwrap();
    if matches!(shape, Shape::Title) {
        let mut q = tx
            .prepare_cached(
                "SELECT id FROM search_docs WHERE kind='threads'
         AND title_key>='project' AND title_key<'projecu' ORDER BY title_key,id LIMIT ?1 OFFSET ?2",
            )
            .unwrap();
        let rows = q
            .query_map(params![WINDOW as i64, depth as i64], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        return Window {
            lookups: depth + rows.len(),
            rows,
            bound: 0,
            postings: 0,
            blocks: 0,
            descriptors: 0,
            refine: false,
        };
    }
    let terms = shape.plan(&tx);
    ranked_messages(
        &tx,
        &terms,
        matches!(shape, Shape::Prefix),
        depth,
        WINDOW,
        &HashSet::new(),
        BUDGET,
        ROW_BUDGET,
    )
}

fn ranked_messages(
    conn: &Connection,
    terms: &[(i64, usize)],
    prefix: bool,
    depth: usize,
    size: usize,
    exclude: &HashSet<i64>,
    postings_budget: usize,
    row_budget: usize,
) -> Window {
    let bound = terms.iter().map(|x| x.1).sum::<usize>();
    let mut out = Window {
        rows: Vec::new(),
        bound,
        postings: 0,
        lookups: 0,
        blocks: 0,
        descriptors: 0,
        refine: bound > postings_budget || terms.len() > 32,
    };
    if out.refine || terms.is_empty() || size == 0 {
        return out;
    }
    type Key = (i64, u128);
    let key = |row: &rusqlite::Row<'_>, stamp, uid| -> rusqlite::Result<Key> {
        let bytes: [u8; 16] = row.get(uid)?;
        Ok((row.get(stamp)?, u128::from_be_bytes(bytes)))
    };
    let mut blocks = BTreeMap::<i64, Key>::new();
    let drivers = if prefix { terms } else { &terms[..1] };
    let mut index=conn.prepare_cached("SELECT block,stamp,newest FROM postings INDEXED BY postings_newest WHERE term=?1 ORDER BY stamp DESC,newest DESC,block").unwrap();
    for &(term, _) in drivers {
        let items = index
            .query_map([term], |r| Ok((r.get::<_, i64>(0)?, key(r, 1, 2)?)))
            .unwrap();
        for item in items {
            if out.descriptors == row_budget {
                out.refine = true;
                return out;
            }
            out.descriptors += 1;
            let (block, max) = item.unwrap();
            blocks
                .entry(block)
                .and_modify(|stamp| *stamp = (*stamp).max(max))
                .or_insert(max);
        }
    }
    let mut order = blocks.into_iter().collect::<Vec<_>>();
    order.sort_unstable_by_key(|(_, max)| std::cmp::Reverse(*max));
    let mut posting = conn
        .prepare_cached("SELECT positions FROM postings WHERE block=?1 AND term=?2")
        .unwrap();
    let mut loc = conn
        .prepare_cached("SELECT stamp,id,thread FROM locations WHERE position=?1")
        .unwrap();
    let mut title = conn
        .prepare_cached("SELECT id FROM threads WHERE slot=?1")
        .unwrap();
    let mut ranked = BTreeSet::<(Key, i64)>::new();
    let mut by_thread = HashMap::<i64, Key>::new();
    let target = depth + size;
    'blocks: for (block, max) in order {
        if ranked.len() == target && ranked.first().unwrap().0 >= max {
            break;
        }
        if out.descriptors + out.blocks + out.lookups + size >= row_budget {
            out.refine = true;
            break;
        }
        out.blocks += 1;
        let mut hits = Vec::new();
        for (i, &(term, _)) in terms.iter().enumerate() {
            let data = posting
                .query_row(params![block, term], |r| r.get::<_, Vec<u8>>(0))
                .optional()
                .unwrap();
            let offsets = data
                .map(|raw| codec::decode(&raw).unwrap())
                .unwrap_or_default();
            out.postings += offsets.len();
            assert!(out.postings <= postings_budget);
            if prefix {
                hits.extend(offsets);
            } else if i == 0 {
                hits = offsets;
            } else {
                hits.retain(|offset| offsets.binary_search(offset).is_ok());
            }
            if !prefix && hits.is_empty() {
                continue 'blocks;
            }
        }
        if prefix {
            hits.sort_unstable();
            hits.dedup();
        }
        for offset in hits.into_iter().rev() {
            if out.descriptors + out.blocks + out.lookups + size >= row_budget {
                out.refine = true;
                break 'blocks;
            }
            let (stamp, slot): (Key, i64) = loc
                .query_row([block * WIDTH as i64 + i64::from(offset)], |r| {
                    Ok((key(r, 0, 1)?, r.get(2)?))
                })
                .unwrap();
            out.lookups += 1;
            if exclude.contains(&slot) {
                continue;
            }
            if ranked.len() == target && ranked.first().unwrap().0 >= stamp {
                break;
            }
            if let Some(prior) = by_thread.get(&slot).copied() {
                if prior >= stamp {
                    continue;
                }
                ranked.remove(&(prior, slot));
            }
            by_thread.insert(slot, stamp);
            ranked.insert((stamp, slot));
            if ranked.len() > target {
                let (_, old) = ranked.pop_first().unwrap();
                by_thread.remove(&old);
            }
        }
    }
    for (_, slot) in ranked.into_iter().rev().skip(depth).take(size) {
        out.rows
            .push(title.query_row([slot], |r| r.get(0)).unwrap());
        out.lookups += 1;
    }
    out
}

fn read(path: &Path) -> Connection {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap()
}

fn state(conn: &Connection) -> (i64, i64) {
    conn.query_row("SELECT authority,result FROM scoped_state", [], |r| {
        Ok((r.get(0)?, r.get(1)?))
    })
    .unwrap()
}

fn expected(shape: Shape, depth: usize) -> Vec<String> {
    if matches!(shape, Shape::Title) {
        return (0..THREADS)
            .filter(|t| t % 11 != 0 && t % 3 == 0)
            .skip(depth)
            .take(WINDOW)
            .map(id)
            .collect();
    }
    let cutoff = match shape {
        Shape::One | Shape::Prefix => 125,
        Shape::Two => 80,
        Shape::Three => 48,
        Shape::ThreeSelective => 24,
        _ => 0,
    };
    (0..THREADS)
        .rev()
        .filter(|t| t % 11 != 0 && (0..amount(*t)).any(|turn| (t * 37 + turn * 19) % 1000 < cutoff))
        .skip(depth)
        .take(WINDOW)
        .map(id)
        .collect()
}

pub(super) fn exercise(root: &Path) {
    let db = root.join("thread-scope.db");
    let (threads, messages, bytes) = seed(&db, false);
    println!(
        "thread_corpus source_threads={THREADS} source_messages={} visible_threads={threads} visible_messages={messages} indexed_body_bytes={bytes} file_bytes={}",
        (0..THREADS).map(amount).sum::<usize>(),
        std::fs::metadata(&db).unwrap().len()
    );
    let verify = read(&db);
    let mut baseline = Vec::new();
    for shape in [
        Shape::One,
        Shape::Two,
        Shape::Three,
        Shape::ThreeSelective,
        Shape::Prefix,
        Shape::Title,
        Shape::Broad,
    ] {
        for depth in [0, 200, 2000] {
            let wanted = record(&verify, shape, depth);
            if !wanted.refine {
                assert_eq!(wanted.rows, expected(shape, depth));
            }
            if matches!(shape, Shape::Broad) {
                assert!(wanted.refine && wanted.postings == 0);
            }
            assert_eq!(
                wanted.rows.iter().collect::<HashSet<_>>().len(),
                wanted.rows.len()
            );
            let mut samples = Vec::new();
            for _ in 0..RUNS {
                let start = Instant::now();
                let actual = record(&read(&db), shape, depth);
                samples.push(start.elapsed().as_micros());
                assert_eq!(actual, wanted);
            }
            println!(
                "thread_probe shape={shape:?} depth={depth} n={RUNS} bound={} postings={} lookups={} blocks={} descriptors={} rows={} refine={} us {}",
                wanted.bound,
                wanted.postings,
                wanted.lookups,
                wanted.blocks,
                wanted.descriptors,
                wanted.rows.len(),
                wanted.refine,
                summary(&mut samples)
            );
            baseline.push((shape, depth, wanted));
        }
    }
    let changed = root.join("thread-after-hidden.db");
    assert_eq!(seed(&changed, true), (threads, messages, bytes));
    let fresh = read(&changed);
    assert_eq!(state(&verify), state(&fresh));
    for (shape, depth, wanted) in baseline {
        assert_eq!(record(&fresh, shape, depth), wanted);
    }
    println!("thread_hidden ranks_changed=0 state_changed=0 work_changed=0");
}

pub(super) fn publish_append(conn: &mut Connection, thread: &str, text: &str, serial: usize) {
    let tx = conn.transaction().unwrap();
    tx.execute(
        "INSERT OR IGNORE INTO threads VALUES(1,?1,'fixture-live')",
        [thread],
    )
    .unwrap();
    let mut words = message_terms(text).collect::<Vec<_>>();
    words.sort_unstable();
    words.dedup();
    let ordinal = 1_000_000 + serial;
    let block = 1_000 + serial;
    write_block(
        &tx,
        block,
        &[Row {
            slot: 1,
            ordinal,
            words,
        }],
    );
    tx.execute("UPDATE scoped_state SET result=result+1", [])
        .unwrap();
    tx.commit().unwrap();
}

pub(super) fn version(conn: &Connection) -> (i64, i64) {
    state(conn)
}

pub(super) fn visible_snapshot(conn: &Connection) -> ((i64, i64), Vec<String>, usize) {
    let window = record(conn, Shape::One, 0);
    (state(conn), window.rows, window.bound)
}
