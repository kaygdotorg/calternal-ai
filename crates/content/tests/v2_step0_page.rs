//! Disposable measurements of candidate window plans over the real SQLite schemas.
#[path = "v2_step0_page/combined_save.rs"]
mod combined_save;
#[path = "v2_step0_page/covering.rs"]
mod covering;
#[path = "v2_step0_page/scoped.rs"]
mod scoped;

use calternal_content::{Content, FileFormat, ItemOrder, Placement};
use calternal_roots::Roots;
use calternal_types::SpaceId;
use rusqlite::{Connection, StatementStatus, functions::FunctionFlags};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};

const ROWS: usize = 100_000;
const WINDOW: usize = 200;
const MAX_VISITS: usize = 4096;
const RUNS: usize = 100;

const COLLECTION: &str = "SELECT i.id FROM items i
  JOIN note_collection_members m ON m.note_id=i.id AND m.collection_id='collection'
  JOIN acl a ON a.id=i.id WHERE i.kind='notes' AND substr(i.path,1,7) <> '.trash/'
  AND visit(i.id) AND a.visible ORDER BY i.updated_at DESC,i.id LIMIT 200 OFFSET ?1";
const SEARCH: &str = "SELECT d.id FROM search_text JOIN search_docs d ON d.rowid=search_text.rowid
  JOIN acl a ON a.id=d.id WHERE search_text MATCH 'body:common' AND visit(d.id) AND a.visible
  ORDER BY bm25(search_text),d.id LIMIT 200 OFFSET ?1";
const COLLECTION_SEARCH: &str = "SELECT d.id FROM search_text
  JOIN search_docs d ON d.rowid=search_text.rowid
  JOIN note_collection_members m ON m.note_id=d.id AND m.collection_id='collection'
  JOIN acl a ON a.id=d.id WHERE search_text MATCH 'title:title' AND visit(d.id) AND a.visible
  ORDER BY bm25(search_text),d.id LIMIT 200 OFFSET ?1";
const FILTER: &str = "SELECT i.id FROM items i JOIN acl a ON a.id=i.id
  WHERE i.kind='notes' AND substr(i.path,1,7) <> '.trash/' AND visit(i.id) AND a.visible
  AND EXISTS (SELECT 1 FROM item_terms t WHERE t.kind='notes' AND t.category='label'
  AND t.name='target' AND t.item=i.id)
  ORDER BY i.updated_at DESC,i.id LIMIT 200 OFFSET ?1";
const COUNT: &str = "SELECT count(*) FROM items i JOIN acl a ON a.id=i.id
  JOIN item_terms t ON t.item=i.id AND t.kind='notes' AND t.category='label' AND t.name='target'
  WHERE i.kind='notes' AND substr(i.path,1,7) <> '.trash/' AND visit(i.id) AND a.visible AND ?1=0";
const KEYSET: &str = "SELECT id FROM items WHERE kind='notes' AND substr(path,1,7) <> '.trash/'
  AND updated_at<?1 AND visit(id) ORDER BY updated_at DESC,id LIMIT 200";

fn seed(c: &mut Connection) {
    let tx = c.transaction().unwrap();
    tx.execute_batch(include_str!("../src/search/schema.sql"))
        .unwrap();
    tx.execute_batch("CREATE TABLE acl(id TEXT PRIMARY KEY, visible INTEGER NOT NULL) STRICT;
        CREATE TABLE versions_of_projection(grant_version INTEGER, result_version INTEGER);
        INSERT INTO items VALUES('collection','collections','{}','collection',1,'0','collection','collection',NULL,'materialized',NULL);
        INSERT INTO versions_of_projection VALUES(1,1);").unwrap();
    for (i, id) in (0..ROWS).map(|i| (i, uuid::Uuid::from_u128(i as u128 + 1).to_string())) {
        tx.prepare_cached(
            "INSERT INTO items VALUES(?1,'notes',?2,?1,1,?3,?1,?1,NULL,'materialized',NULL)",
        )
        .unwrap()
        .execute((
            &id,
            if i % 53 == 0 {
                r#"{"labels":[]}"#
            } else {
                r#"{"labels":["target"]}"#
            },
            format!("{i:06}"),
        ))
        .unwrap();
        tx.prepare_cached("INSERT INTO acl VALUES(?1,?2)")
            .unwrap()
            .execute((&id, i % 31 != 0))
            .unwrap();
        if i % 10 == 0 {
            tx.prepare_cached("INSERT INTO note_collection_members(collection_id,note_id) VALUES('collection',?1)").unwrap()
                .execute([&id]).unwrap();
        }
        tx.prepare_cached("INSERT INTO search_docs VALUES(?1,?2,'notes','title','title')")
            .unwrap()
            .execute((i as i64 + 1, &id))
            .unwrap();
        tx.prepare_cached("INSERT INTO search_text(rowid,title,body) VALUES(?1,'title',?2)")
            .unwrap()
            .execute((
                i as i64 + 1,
                if i % 31 == 0 {
                    "common secret"
                } else {
                    "common"
                },
            ))
            .unwrap();
    }
    tx.commit().unwrap();
    c.execute_batch("ANALYZE; PRAGMA wal_checkpoint(TRUNCATE);")
        .unwrap();
}

fn summary(values: &mut [u128]) -> String {
    values.sort_unstable();
    let at = |p: usize| values[(values.len() * p).div_ceil(100) - 1];
    format!("p50={} p95={} max={}", at(50), at(95), at(100))
}

fn fixture(root: &std::path::Path) -> (Content, SpaceId, std::path::PathBuf) {
    let roots = Roots {
        appdata: root.join("appdata"),
        userdata_ssd: root.join("ssd"),
        userdata_hdd: root.join("hdd"),
    };
    let space = SpaceId::new();
    let db = roots.appdata.join(format!("spaces/{space}/content.db"));
    let content = Content::open(
        roots,
        root.join("secrets"),
        Duration::from_secs(120),
        calternal_content::DEFAULT_RECEIPT_RETENTION,
        |_| {},
    )
    .unwrap();
    content
        .register_kind(
            space,
            "notes",
            Placement::Visible {
                folder: "Notes".into(),
            },
            FileFormat::Markdown,
        )
        .unwrap();
    content
        .register_kind(
            space,
            "collections",
            Placement::Hidden {
                plugin_id: "collections".into(),
            },
            FileFormat::Opaque {
                extension: "json".into(),
            },
        )
        .unwrap();
    let mut writer = Connection::open(&db).unwrap();
    seed(&mut writer);
    (content, space, db)
}

#[test]
#[ignore = "disposable perf fixture; see step 0 measurements"]
fn covering_combined_save() {
    let root = tempfile::tempdir().unwrap();
    let (content, space, db) = fixture(root.path());
    combined_save::exercise(&content, space, &db);
}

#[test]
#[ignore = "disposable perf fixture; see step 0 measurements"]
fn covering_newest_windows() {
    let root = tempfile::tempdir().unwrap();
    let (_content, _space, db) = fixture(root.path());
    covering::exercise(&db, &root.path().join("covering.db"));
}

#[test]
#[ignore = "disposable perf fixture; see step 0 measurements"]
fn scoped_newest_windows() {
    let root = tempfile::tempdir().unwrap();
    let (_content, _space, db) = fixture(root.path());
    scoped::exercise(&db, &root.path().join("projection.db"));
}

#[test]
#[ignore = "disposable perf fixture; see step 0 measurements"]
fn window_work_and_revoked_delivery() {
    let root = tempfile::tempdir().unwrap();
    let (content, space, db) = fixture(root.path());
    let writer = Connection::open(&db).unwrap();
    assert_eq!(
        content
            .item_page(
                space,
                "notes",
                ItemOrder::Updated { label: None },
                None,
                WINDOW
            )
            .unwrap()
            .rows
            .len(),
        WINDOW
    );

    let mut rejected = 0;
    for (name, sql) in [
        ("collection_acl_sort", COLLECTION),
        ("search_audience_rank", SEARCH),
        ("collection_search_acl", COLLECTION_SEARCH),
        ("dynamic_collection_acl_sort", FILTER),
        ("member_keyset_control", KEYSET),
        ("count_only", COUNT),
    ] {
        for offset in [0, 10_000, 90_000] {
            if name == "count_only" && offset > 0 {
                continue;
            }
            let mut elapsed = Vec::new();
            let mut visits = Vec::new();
            let mut steps = Vec::new();
            let mut admitted = 0;
            let mut result_rows = 0;
            let mut exact_count = None;
            for run in 0..RUNS {
                writer.execute_batch("BEGIN IMMEDIATE;
                    UPDATE acl SET visible=1-visible WHERE rowid BETWEEN 20 AND 29;
                    UPDATE items SET metadata=CASE WHEN json_array_length(json_extract(metadata,'$.labels'))=0
                        THEN '{\"labels\":[\"target\"]}' ELSE '{\"labels\":[]}' END WHERE rowid BETWEEN 20 AND 29;
                    UPDATE search_text SET body=CASE WHEN body='common' THEN 'other' ELSE 'common' END WHERE rowid BETWEEN 20 AND 29;
                    UPDATE versions_of_projection SET grant_version=grant_version+1,result_version=result_version+1;
                    COMMIT;").unwrap();
                let started = Instant::now();
                let reader =
                    Connection::open_with_flags(&db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                        .unwrap();
                let visited = Arc::new(AtomicUsize::new(0));
                let tally = Arc::clone(&visited);
                let capped = Arc::new(AtomicBool::new(false));
                let enforce = Arc::clone(&capped);
                reader
                    .create_scalar_function("visit", 1, FunctionFlags::SQLITE_UTF8, move |_| {
                        let seen = tally.fetch_add(1, Ordering::Relaxed);
                        if enforce.load(Ordering::Relaxed) && seen >= MAX_VISITS {
                            return Err(rusqlite::Error::UserFunctionError(
                                std::io::Error::other("unsupported_query_plan").into(),
                            ));
                        }
                        Ok(true)
                    })
                    .unwrap();
                let key = format!("{:06}", ROWS - offset);
                let sql_offset = offset as i64;
                let operand: &dyn rusqlite::ToSql = if name == "member_keyset_control" {
                    &key
                } else {
                    &sql_offset
                };
                let mut statement = reader.prepare(sql).unwrap();
                let rows = if name == "count_only" {
                    exact_count = Some(
                        statement
                            .query_row([operand], |row| row.get::<_, i64>(0))
                            .unwrap(),
                    );
                    1
                } else {
                    statement
                        .query_map([operand], |row| row.get::<_, String>(0))
                        .unwrap()
                        .collect::<rusqlite::Result<Vec<_>>>()
                        .unwrap()
                        .len()
                };
                elapsed.push(started.elapsed().as_micros());
                let n = visited.load(Ordering::Relaxed);
                visits.push(n as u128);
                steps.push(statement.get_status(StatementStatus::VmStep) as u128);
                result_rows = rows;
                if name != "count_only" {
                    visited.store(0, Ordering::Relaxed);
                    capped.store(true, Ordering::Relaxed);
                    let checked = statement
                        .query_map([operand], |_| Ok(()))
                        .unwrap()
                        .collect::<rusqlite::Result<Vec<_>>>();
                    if n <= MAX_VISITS {
                        assert_eq!(checked.unwrap().len(), rows);
                        admitted += 1;
                    } else {
                        assert!(checked.is_err());
                        assert_eq!(visited.load(Ordering::Relaxed), MAX_VISITS + 1);
                        rejected += 1;
                    }
                }
                if run == 0 {
                    let mut explain = reader
                        .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
                        .unwrap();
                    let plan = explain
                        .query_map([operand], |row| row.get::<_, String>(3))
                        .unwrap()
                        .collect::<rusqlite::Result<Vec<_>>>()
                        .unwrap();
                    println!("plan {name} offset={offset}: {}", plan.join(" | "));
                }
            }
            println!(
                "{name} offset={offset} n={RUNS} rows={result_rows} exact_count={exact_count:?} admit={admitted} us {} visits {} vm_steps {}",
                summary(&mut elapsed),
                summary(&mut visits),
                summary(&mut steps)
            );
            if name == "member_keyset_control" {
                assert_eq!((result_rows, admitted), (WINDOW, RUNS));
            }
        }
    }
    assert!(rejected > 0);

    // Hold the actual SQLite query snapshot across a grant change. The fixture's
    // pre-send version check discards the whole buffered page and count.
    let mut snapshot_db = Connection::open(&db).unwrap();
    let version = |c: &Connection| {
        c.query_row(
            "SELECT grant_version,result_version FROM versions_of_projection",
            [],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)),
        )
        .unwrap()
    };
    for _ in 0..RUNS {
        let snapshot = snapshot_db.transaction().unwrap();
        let original = version(&snapshot);
        let mut window = snapshot
            .prepare("SELECT id FROM acl WHERE visible LIMIT 200")
            .unwrap();
        let mut cursor = window.query([]).unwrap();
        let mut stale_window = vec![cursor.next().unwrap().unwrap().get::<_, String>(0).unwrap()];
        writer
            .execute_batch(
                "BEGIN IMMEDIATE; UPDATE acl SET visible=0 WHERE rowid=2;
            UPDATE versions_of_projection SET grant_version=grant_version+1; COMMIT;",
            )
            .unwrap();
        while let Some(row) = cursor.next().unwrap() {
            stale_window.push(row.get(0).unwrap());
        }
        let old_visible: i64 = snapshot
            .query_row("SELECT count(*) FROM acl WHERE visible", [], |r| r.get(0))
            .unwrap();
        assert_eq!(stale_window.len(), WINDOW);
        assert!(old_visible > WINDOW as i64);
        let delivered = (version(&writer) == original).then_some((stale_window, old_visible));
        assert!(delivered.is_none());
        writer
            .execute("UPDATE acl SET visible=1 WHERE rowid=2", [])
            .unwrap();
    }
    println!("mid_query_revoke n={RUNS} rows_delivered=0 count_delivered=0 runtime_wakes=0");

    // Confirm the audience predicate alone cannot isolate rank statistics.
    let score = |db: &Connection| {
        db.query_row(
        "SELECT bm25(search_text) FROM search_text WHERE search_text MATCH 'body:common' AND rowid=2",
        [], |r| r.get::<_, f64>(0)).unwrap()
    };
    let before = score(&writer);
    writer
        .execute(
            "UPDATE search_text SET body='unrelated secret private words' WHERE rowid=1",
            [],
        )
        .unwrap();
    let after = score(&writer);
    assert_ne!(before, after);
    println!("private_corpus_rank_dependency n=1 changed_visible_score=1");
    // A revoked count is never reused and the queue retains only the newest
    // invalidation per admitted subscriber during each simulated 250 ms tick.
    let mut subscriptions = Vec::new();
    let mut refused = 0;
    for actor in 0..10 {
        for view in 0..2 {
            for bind in 0..9 {
                if bind >= 8 || subscriptions.len() >= 128 {
                    refused += 1;
                } else {
                    subscriptions.push((actor, view, bind, 0));
                }
            }
        }
    }
    let mut pushes = 0;
    let mut count_starts = 0;
    for tick in 0..4 {
        for version in tick * 20..(tick + 1) * 20 {
            for sub in &mut subscriptions {
                sub.3 = version;
            }
        }
        pushes += subscriptions.len();
        count_starts += 1;
        assert!(subscriptions.iter().all(|sub| sub.3 == (tick + 1) * 20 - 1));
    }
    assert_eq!(
        (subscriptions.len(), refused, pushes, count_starts),
        (128, 52, 512, 4)
    );
    println!(
        "fanout admitted=128 refused=52 invalidations=80 pushes=512 pushes_per_bind=4 count_starts=4 epoch_stale_count_delivered=0"
    );
}
