//! Existing search worker publication lag, followed by scoped fixture publication.
use super::{RUNS, search::SCHEMA, summary};
use calternal_content::{
    Content, FileFormat, Item, Placement, Request, SearchConfig, SearchKind, SearchMatch,
    SearchProbe, message_terms,
};
use calternal_roots::Roots;
use calternal_types::{AccountId, SpaceId};
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

const BUSY_SPACES: usize = 6;
const POLL: Duration = Duration::from_millis(2);
const TIMEOUT: Duration = Duration::from_secs(10);

fn register(content: &Content, space: SpaceId) {
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
}

fn save(
    content: &Content,
    space: SpaceId,
    actor: AccountId,
    before: Option<&Item>,
    seq: usize,
    body: &str,
) -> Item {
    let key = format!("lag-{seq}");
    content
        .write(
            space,
            before.map(|item| item.id),
            "notes",
            before.map(|item| item.revision.as_str()),
            json!({"title":"Search lag"}),
            body.as_bytes(),
            Request {
                account: actor,
                via: None,
                id: &key,
                operation: "notes.update",
                fingerprint: &key,
            },
            |_| json!({}),
        )
        .unwrap()
}

fn page(content: &Content, config: &SearchConfig, space: SpaceId, term: &str) -> (bool, bool) {
    let expression = format!("{term:?}");
    let title = format!("title:{expression}");
    let page = content
        .search_page(
            config,
            space,
            &SearchProbe {
                matching: SearchMatch::Text {
                    all: &expression,
                    title: &title,
                    message_terms: &[],
                    phrase: false,
                    messages_only: false,
                },
                kinds: r#"["notes"]"#,
                item_ids: None,
                after: None,
                limit: 3,
            },
        )
        .unwrap();
    (page.partial, !page.hits.is_empty())
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Stamp {
    authority: usize,
    revision: usize,
}

fn loading() -> Value {
    json!({"bind":"search","state":"loading","result_version":null,"offset":0,"rows":[]})
}

fn delivery(current: Stamp, projected: Option<Stamp>, rows: impl FnOnce() -> Value) -> Value {
    if projected != Some(current) {
        return loading();
    }
    rows()
}

fn publish(corpus: &mut Connection, marker: &str, body: &str, stamp: Stamp) {
    let tx = corpus.transaction().unwrap();
    let old = tx
        .query_row("SELECT body FROM search_text WHERE rowid=1", [], |r| {
            r.get::<_, String>(0)
        })
        .ok();
    let tokens = |body: &str| {
        message_terms(body)
            .chain(message_terms("Search lag"))
            .collect::<std::collections::BTreeSet<_>>()
    };
    if let Some(old) = old {
        for term in tokens(&old) {
            tx.execute("UPDATE term_stats SET doc=doc-1 WHERE term=?1", [&term])
                .unwrap();
            tx.execute("DELETE FROM term_stats WHERE term=?1 AND doc=0", [&term])
                .unwrap();
        }
    }
    for term in tokens(body) {
        tx.execute(
            "INSERT INTO term_stats VALUES(?1,1)
           ON CONFLICT(term) DO UPDATE SET doc=doc+1",
            [&term],
        )
        .unwrap();
    }
    tx.execute("DELETE FROM search_text WHERE rowid=1", [])
        .unwrap();
    tx.execute(
        "INSERT INTO search_text(rowid,title,body) VALUES(1,'Search lag',?1)",
        [body],
    )
    .unwrap();
    tx.execute(
        "UPDATE ready_stamp SET authority=?1,revision=?2",
        params![stamp.authority as i64, stamp.revision as i64],
    )
    .unwrap();
    tx.commit().unwrap();
    let match_count: i64 = corpus
        .query_row(
            "SELECT count(*) FROM search_text WHERE search_text MATCH ?1",
            [marker],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(match_count, 1);
}

fn projection(path: &Path) -> Connection {
    let conn = Connection::open(path).unwrap();
    conn.execute_batch(SCHEMA).unwrap();
    conn.execute_batch(
        "PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;
        CREATE TABLE ready_stamp(authority INTEGER,revision INTEGER);
        CREATE VIRTUAL TABLE vocab USING fts5vocab(search_text,'row');
        CREATE TABLE term_stats(term TEXT PRIMARY KEY,doc INTEGER NOT NULL)
          WITHOUT ROWID, STRICT;
        INSERT INTO ready_stamp VALUES(0,0);",
    )
    .unwrap();
    conn
}

fn seed_scope(conn: &Connection, corpus_size: i64) {
    conn.execute(
        "INSERT INTO search_text(rowid,title,body)
         SELECT value+1,printf('Notebook %06d',value),
           'ordinary notebook files archived reference alpha beta gamma project weekly meeting'
         FROM (WITH RECURSIVE series(value) AS (SELECT 1 UNION ALL SELECT value+1 FROM series WHERE value<?1) SELECT value FROM series)",
        [corpus_size],
    ).unwrap();
    conn.execute_batch(
        "INSERT INTO search_text(search_text) VALUES('optimize');
        INSERT INTO term_stats SELECT term,doc FROM vocab;",
    )
    .unwrap();
}

struct Job {
    marker: String,
    body: String,
    stamp: Stamp,
    saved: Instant,
}

fn settle(content: &Content, config: &SearchConfig, space: SpaceId, job: &Job) {
    loop {
        let (behind, contains) = page(content, config, space, &job.marker);
        if !behind && contains {
            break;
        }
        assert!(
            job.saved.elapsed() < TIMEOUT,
            "search worker did not publish {}",
            job.marker
        );
        thread::sleep(POLL);
    }
}

fn probe_busy(content: &Content, config: &SearchConfig, space: SpaceId) {
    let _ = page(content, config, space, "busyfixture");
}

fn seed_existing_engine(path: &Path, space: SpaceId, actor: AccountId) {
    let conn = Connection::open(path.join(format!("spaces/{space}/content.db"))).unwrap();
    conn.execute_batch(
        "BEGIN IMMEDIATE;
         WITH RECURSIVE docs(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM docs WHERE n<99999)
         INSERT INTO items(id,kind,metadata,revision,seq,updated_at,path,folded_path,mat_state)
         SELECT printf('00000000-0000-0000-0000-%012d',n),'notes',
           json_object('title',printf('Notebook %06d',n)),printf('fixture-%d',n),1,
           '2026-01-01T00:00:00Z',printf('Notes/%d.md',n),printf('notes/%d.md',n),'materialized'
         FROM docs;",
    )
    .unwrap();
    conn.execute(
        "INSERT INTO versions
         SELECT id,1,revision,metadata,CAST('ordinary notebook archived reference alpha beta gamma' AS BLOB),updated_at,?1
         FROM items WHERE revision LIKE 'fixture-%'",
        [actor.to_string()],
    ).unwrap();
    conn.execute_batch("COMMIT; PRAGMA wal_checkpoint(TRUNCATE);")
        .unwrap();
}

pub(super) fn exercise(root: &Path) {
    let roots = Roots {
        appdata: root.join("appdata"),
        userdata_ssd: root.join("ssd"),
        userdata_hdd: root.join("hdd"),
    };
    let guards = calternal_roots::prepare(&roots, uuid::Uuid::now_v7(), true).unwrap();
    let appdata = roots.appdata.clone();
    let content = Arc::new(
        Content::open(
            roots,
            root.join("secrets"),
            Duration::from_secs(120),
            calternal_content::DEFAULT_RECEIPT_RETENTION,
            |_| {},
        )
        .unwrap(),
    );
    content.retain_root_guard(Arc::new(guards));
    let config = Arc::new(SearchConfig::new(vec![SearchKind {
        kind: "notes".into(),
        control: "fixture".into(),
        default_enabled: true,
        approval: 0,
        space: None,
    }]));
    let space = SpaceId::new();
    register(&content, space);
    let actor = AccountId::new();
    let mut latest = save(&content, space, actor, None, 0, "initial note");
    content.materialize(space, latest.id).unwrap();
    seed_existing_engine(&appdata, space, actor);

    let worker = content
        .configure_search(
            Box::new({
                let config = config.clone();
                move || Some(config.clone())
            }),
            |_| Ok(Vec::new()),
        )
        .unwrap();
    let warm = Instant::now();
    loop {
        if page(&content, &config, space, "initial") == (false, true) {
            break;
        }
        assert!(
            warm.elapsed() < Duration::from_secs(120),
            "search startup timed out"
        );
        thread::sleep(Duration::from_millis(20));
    }
    let scoped_space = projection(&root.join("lag-space.db"));
    let scoped_collection = projection(&root.join("lag-collection.db"));
    seed_scope(&scoped_space, 96_773);
    seed_scope(&scoped_collection, 9_676);

    let combined = std::env::var_os("STEP0_ALL_SEARCH").is_some();
    let (send, recv) = std::sync::mpsc::sync_channel::<Job>(0);
    let (ack, done) = std::sync::mpsc::sync_channel(0);
    let adapter_content = content.clone();
    let adapter_config = config.clone();
    let adapter = thread::spawn(move || {
        let mut scoped_space = scoped_space;
        let mut scoped_collection = scoped_collection;
        while let Ok(job) = recv.recv() {
            // The existing worker processes the real revision. The disposable
            // adapter then publishes that authorized input to two scope copies.
            settle(&adapter_content, &adapter_config, space, &job);
            let core_time = job.saved.elapsed().as_micros();
            publish(&mut scoped_space, &job.marker, &job.body, job.stamp);
            publish(&mut scoped_collection, &job.marker, &job.body, job.stamp);
            let scoped_time = job.saved.elapsed().as_micros();
            if combined {
                let query = format!("title:search AND {}", job.marker);
                for scope in [&scoped_space, &scoped_collection] {
                    let slot: i64 = scope.query_row(
                         "SELECT rowid FROM search_text WHERE search_text MATCH ?1 ORDER BY rowid LIMIT 1",
                         [&query], |r| r.get(0)
                    ).unwrap();
                    assert_eq!(slot, 1);
                }
            }
            ack.send((
                core_time,
                scoped_time,
                job.stamp,
                job.saved.elapsed().as_micros(),
            ))
            .unwrap();
        }
    });

    let stop = Arc::new(AtomicBool::new(false));
    let churn = Arc::new(AtomicUsize::new(0));
    let mut writers = Vec::new();
    let mut published = None;
    let mut total = 0;
    let samples = std::env::var("STEP0_LAG_RUNS").map_or(RUNS, |value| value.parse().unwrap());
    assert!(samples > 0);
    for busy in [false, true] {
        if busy {
            for _ in 0..BUSY_SPACES {
                let fixture_space = SpaceId::new();
                register(&content, fixture_space);
                let fixture = content.clone();
                let config = config.clone();
                let stop = stop.clone();
                let churn = churn.clone();
                writers.push(thread::spawn(move || {
                    let actor = AccountId::new();
                    let mut prior = None;
                    let mut seq = 0;
                    while !stop.load(Ordering::Acquire) {
                        let body = format!(
                            "busyfixture{seq} {}",
                            "agenda reference files notes ".repeat(250)
                        );
                        prior = Some(save(
                            &fixture,
                            fixture_space,
                            actor,
                            prior.as_ref(),
                            seq,
                            &body,
                        ));
                        let id = prior.as_ref().unwrap().id;
                        fixture.materialize(fixture_space, id).unwrap();
                        probe_busy(&fixture, &config, fixture_space);
                        churn.fetch_add(1, Ordering::Release);
                        seq += 1;
                        thread::sleep(Duration::from_millis(20));
                    }
                }));
            }
            let start = Instant::now();
            while churn.load(Ordering::Acquire) < BUSY_SPACES * 4 {
                assert!(start.elapsed() < TIMEOUT);
                thread::sleep(POLL);
            }
        }

        let start_churn = churn.load(Ordering::Acquire);
        let mut core_times = Vec::new();
        let mut scoped_times = Vec::new();
        let mut finished = Vec::new();
        for _ in 0..samples {
            total += 1;
            let stamp = Stamp {
                authority: 1,
                revision: total,
            };
            let marker = format!("searchmarker{total}");
            let body = format!(
                "{marker} {}",
                "project agenda notebook reference ".repeat(230)
            );
            latest = save(&content, space, actor, Some(&latest), total, &body);
            let saved = Instant::now();
            let response = delivery(stamp, published, || panic!("read stale projection"));
            assert_eq!(response, loading());
            send.send(Job {
                marker,
                body,
                stamp,
                saved,
            })
            .unwrap();
            let (core, scoped, revision, ready) = done.recv_timeout(TIMEOUT).unwrap();
            core_times.push(core);
            scoped_times.push(scoped);
            finished.push(ready);
            content.materialize(space, latest.id).unwrap();
            published = Some(revision);
            assert_eq!(
                delivery(stamp, published, || json!("ready")),
                json!("ready")
            );
            // Authority changes are independent of the content watermark. The
            // stale generation's rows/count are rejected synchronously.
            assert_eq!(
                delivery(
                    Stamp {
                        authority: 2,
                        ..stamp
                    },
                    published,
                    || { panic!("revoked generation evaluated") }
                ),
                loading()
            );
        }
        if combined {
            println!(
                "all_notes_lag busy={busy} n={samples} ready_us {}",
                summary(&mut finished)
            );
        }
        println!(
            "search_lag busy={busy} n={samples} competing_saves={} prepublish_loading={} revoked_deliveries=0 core_us {} scoped_us {}",
            churn.load(Ordering::Acquire) - start_churn,
            samples,
            summary(&mut core_times),
            summary(&mut scoped_times)
        );
    }
    stop.store(true, Ordering::Release);
    for writer in writers {
        writer.join().unwrap();
    }
    drop(send);
    adapter.join().unwrap();
    drop(content);
    worker.join().unwrap();
}
