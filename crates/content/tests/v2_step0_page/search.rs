//! Scoped Notes probes over the existing FTS5 and title-range schemas.
use super::{RUNS, WINDOW, summary};
use calternal_content::{fold_title, message_terms};
use rusqlite::{Connection, OpenFlags, params};
use std::path::Path;
use std::time::Instant;

pub(super) const SCHEMA: &str = include_str!("../../src/search/schema.sql");
const CORPUS_LIMIT: i64 = 100_000;
const POSTING_BUDGET: i64 = 160_000;
const MAX_OFFSET: i64 = 2_000;
const FILTER: &str = "kind='notes' AND title_key>=?1 AND title_key<?2";

#[derive(Clone, Copy)]
enum Query<'a> {
    Text(&'a str),
    TokenPrefix(&'a str),
    TitlePrefix(&'a str),
}

#[derive(Debug, PartialEq, Eq)]
struct Window {
    ids: Vec<String>,
    visits: i64,
    estimated_postings: i64,
}

fn read(path: &Path) -> Connection {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap()
}

fn prepare_source(source: &Path) {
    let conn = Connection::open(source).unwrap();
    conn.execute_batch(
        "BEGIN IMMEDIATE;
         DELETE FROM search_text;
         UPDATE search_docs SET title=printf('%s %06d',
           CASE WHEN ((rowid-1)/10)%100<38 THEN 'Notebook' ELSE 'Archive' END,rowid);
         UPDATE search_docs SET title_key=lower(title);
         INSERT INTO search_text(rowid,title,body)
         SELECT rowid,title,
           'ordinary organization regular ubiquitous notes archive meeting documents reference project weekly ' ||
           CASE WHEN ((rowid-1)/10)%100<55 THEN 'alpha ' ELSE 'delta ' END ||
           CASE WHEN ((rowid-1)/10)%100 BETWEEN 10 AND 59 THEN 'beta ' ELSE 'epsilon ' END ||
           CASE WHEN ((rowid-1)/10)%100 BETWEEN 20 AND 69 THEN 'gamma' ELSE 'theta' END
         FROM search_docs;
         COMMIT;",
    ).unwrap();
}

fn seed(source: &Path, path: &Path, collection: bool) {
    let conn = Connection::open(path).unwrap();
    conn.execute_batch(SCHEMA).unwrap();
    conn.execute_batch(
        "CREATE VIRTUAL TABLE vocab USING fts5vocab(search_text, 'row');
         CREATE TABLE corpus(n INTEGER NOT NULL CHECK(n<=100000));",
    )
    .unwrap();
    conn.execute("ATTACH DATABASE ?1 AS source", [source.to_str().unwrap()])
        .unwrap();
    conn.execute_batch("BEGIN IMMEDIATE").unwrap();
    conn.execute(
        "INSERT INTO search_docs
         SELECT row_number() OVER (ORDER BY d.id),d.id,d.kind,d.title,d.title_key
         FROM source.search_docs d JOIN source.acl a ON a.id=d.id
         WHERE d.kind='notes' AND a.visible
          AND (NOT ?1 OR EXISTS (SELECT 1 FROM source.note_collection_members m
                       WHERE m.collection_id='collection' AND m.note_id=d.id))",
        [collection],
    )
    .unwrap();
    conn.execute_batch(
        "INSERT INTO search_text(rowid,title,body)
         SELECT d.rowid,d.title,s.body FROM search_docs d
         JOIN source.search_docs original ON original.id=d.id
         JOIN source.search_text s ON s.rowid=original.rowid;
         INSERT INTO corpus SELECT count(*) FROM search_docs;
         COMMIT;
         INSERT INTO search_text(search_text) VALUES('optimize');
         CREATE TABLE term_stats(term TEXT PRIMARY KEY,doc INTEGER NOT NULL)
             WITHOUT ROWID, STRICT;
         INSERT INTO term_stats SELECT term,doc FROM vocab;
         ANALYZE;",
    )
    .unwrap();
    let n: i64 = conn
        .query_row("SELECT n FROM corpus", [], |row| row.get(0))
        .unwrap();
    assert!(n <= CORPUS_LIMIT);
    let notes = conn
        .query_row("SELECT sum(length(body)) FROM search_text", [], |r| {
            r.get::<_, i64>(0)
        })
        .unwrap();
    println!(
        "scoped_search_size collection={collection} rows={n} body_bytes={notes} file_bytes={}",
        std::fs::metadata(path).unwrap().len()
    );
}

fn text_plan(conn: &Connection, input: &str) -> Option<(String, Vec<i64>)> {
    if input.len() > 96
        || !input
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b' ')
    {
        return None;
    }
    // Both the existing message token helper and unicode61 fold these ASCII words
    // identically. The pilot has no phrase, fuzzy, OR, or raw MATCH grammar.
    let words = message_terms(input).collect::<Vec<_>>();
    if !(1..=3).contains(&words.len()) {
        return None;
    }
    let mut frequencies = conn
        .prepare_cached("SELECT coalesce((SELECT doc FROM term_stats WHERE term=?1),0)")
        .unwrap();
    let docs = words
        .iter()
        .map(|word| {
            frequencies
                .query_row([word], |r| r.get::<_, i64>(0))
                .unwrap()
        })
        .collect::<Vec<_>>();
    let expression = words
        .iter()
        .map(|word| format!("{word:?}"))
        .collect::<Vec<_>>()
        .join(" AND ");
    Some((expression, docs))
}

fn prefix_range(value: &str) -> Option<(String, String)> {
    if !(2..=80).contains(&value.len())
        || !value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b' ')
    {
        return None;
    }
    let start = fold_title(value);
    let mut end = start.as_bytes().to_vec();
    *end.last_mut()? += 1;
    Some((start, String::from_utf8(end).unwrap()))
}

fn token_prefix_plan(conn: &Connection, input: &str) -> Option<(String, Vec<i64>)> {
    if !input.bytes().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    let (start, end) = prefix_range(input)?;
    let mut stmt = conn
        .prepare_cached("SELECT doc FROM term_stats WHERE term>=?1 AND term<?2 LIMIT 33")
        .unwrap();
    let docs = stmt
        .query_map(params![start, end], |row| row.get::<_, i64>(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    Some((format!("{start:?}*"), docs))
}

const TEXT: &str =
    "SELECT rowid FROM search_text WHERE search_text MATCH ?1 ORDER BY rowid LIMIT ?2 OFFSET ?3";

const BOUNDED_TEXT: &str = "SELECT rowid FROM search_text
    WHERE search_text MATCH ?1 AND rowid BETWEEN 1 AND ?4 ORDER BY rowid LIMIT ?2 OFFSET ?3";

fn text_ids(conn: &Connection, expression: &str, offset: i64, cutoff: Option<i64>) -> Vec<String> {
    let mut matches = conn
        .prepare_cached(if cutoff.is_some() { BOUNDED_TEXT } else { TEXT })
        .unwrap();
    let mut docs = conn
        .prepare_cached("SELECT id FROM search_docs WHERE rowid=?1")
        .unwrap();
    let mut cursor = match cutoff {
        Some(end) => matches.query(params![expression, WINDOW as i64, offset, end]),
        None => matches.query(params![expression, WINDOW as i64, offset]),
    }
    .unwrap();
    let mut ids = Vec::new();
    while let Some(row) = cursor.next().unwrap() {
        let id = docs
            .query_row([row.get::<_, i64>(0).unwrap()], |row| row.get(0))
            .unwrap();
        ids.push(id);
    }
    ids
}

#[derive(Debug, PartialEq, Eq)]
enum Bind {
    Ready(Window),
    RefineQuery(Vec<String>),
    UnsupportedPlan,
}

fn run(conn: &Connection, query: Query<'_>, offset: i64) -> Bind {
    let txn = conn.unchecked_transaction().unwrap();
    if !(0..=MAX_OFFSET).contains(&offset) {
        return Bind::UnsupportedPlan;
    }
    let plan = match query {
        Query::Text(input) => text_plan(&txn, input),
        Query::TokenPrefix(value) => token_prefix_plan(&txn, value),
        Query::TitlePrefix(value) => {
            return match prefix_range(value) {
                Some((start, end)) => {
                    let sql = format!(
                        "SELECT id FROM search_docs WHERE {FILTER} ORDER BY title_key,id LIMIT ?3 OFFSET ?4"
                    );
                    let ids = txn
                        .prepare(&sql)
                        .unwrap()
                        .query_map(params![start, end, WINDOW as i64, offset], |r| r.get(0))
                        .unwrap()
                        .collect::<rusqlite::Result<Vec<_>>>()
                        .unwrap();
                    Bind::Ready(Window {
                        visits: offset + ids.len() as i64,
                        ids,
                        estimated_postings: 0,
                    })
                }
                None => Bind::UnsupportedPlan,
            };
        }
    };
    let Some((expression, docs)) = plan else {
        return Bind::UnsupportedPlan;
    };
    let postings = docs.iter().sum();
    if docs.len() > 32 || postings > POSTING_BUDGET {
        // Exact words support a bounded leading rowid interval in FTS5.
        // Dynamic token-prefix expansion can do work before the interval is
        // applied, so keep only a previously issued same-version window there.
        let rows = if matches!(query, Query::Text(_)) {
            text_ids(
                &txn,
                &expression,
                offset,
                Some(POSTING_BUDGET / docs.len() as i64),
            )
        } else {
            Vec::new()
        };
        return Bind::RefineQuery(rows);
    }
    let ids = text_ids(&txn, &expression, offset, None);
    Bind::Ready(Window {
        visits: offset + ids.len() as i64,
        ids,
        estimated_postings: postings,
    })
}

fn window(conn: &Connection, query: Query<'_>, offset: i64) -> Option<Window> {
    match run(conn, query, offset) {
        Bind::Ready(window) => Some(window),
        _ => None,
    }
}

fn exact_count(conn: &Connection, query: Query<'_>) -> Option<i64> {
    let txn = conn.unchecked_transaction().unwrap();
    let conn = &txn;
    match query {
        Query::Text(input) => {
            let (expression, docs) = text_plan(conn, input)?;
            match docs.as_slice() {
                [count] => Some(*count),
                _ => Some(
                    conn.query_row(
                        "SELECT count(*) FROM search_text WHERE search_text MATCH ?1",
                        [expression],
                        |r| r.get(0),
                    )
                    .unwrap(),
                ),
            }
        }
        Query::TokenPrefix(value) => {
            let (expression, _) = token_prefix_plan(conn, value)?;
            Some(
                conn.query_row(
                    "SELECT count(*) FROM search_text WHERE search_text MATCH ?1",
                    [expression],
                    |r| r.get(0),
                )
                .unwrap(),
            )
        }
        Query::TitlePrefix(value) => {
            let (start, end) = prefix_range(value)?;
            Some(
                conn.query_row(
                    &format!("SELECT count(*) FROM search_docs WHERE {FILTER}"),
                    params![start, end],
                    |r| r.get(0),
                )
                .unwrap(),
            )
        }
    }
}

const QUERIES: [(&str, Query<'_>); 5] = [
    ("one", Query::Text("alpha")),
    ("two", Query::Text("alpha beta")),
    ("three", Query::Text("alpha beta gamma")),
    ("title_prefix", Query::TitlePrefix("notebook")),
    ("token_prefix", Query::TokenPrefix("note")),
];

fn explain(conn: &Connection, sql: &str, params: impl rusqlite::Params) {
    let rows = conn
        .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
        .unwrap()
        .query_map(params, |r| r.get::<_, String>(3))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    assert!(
        rows.iter().all(|row| !row.contains("TEMP B-TREE")),
        "{rows:?}"
    );
    println!("scoped_search_plan {}", rows.join(" | "));
}

fn calibrate(conn: &Connection) {
    for expression in ["ordinary", "alpha AND beta gamma", "note*"] {
        let mut samples = Vec::new();
        let postings: i64 = if expression == "ordinary" {
            conn.query_row(
                "SELECT doc FROM term_stats WHERE term='ordinary'",
                [],
                |r| r.get(0),
            )
            .unwrap()
        } else if expression == "note*" {
            token_prefix_plan(conn, "note").unwrap().1.iter().sum()
        } else {
            text_plan(conn, "alpha beta gamma").unwrap().1.iter().sum()
        };
        for _ in 0..RUNS {
            let start = Instant::now();
            let rows: i64 = conn
                .query_row(
                    "SELECT count(*) FROM search_text WHERE search_text MATCH ?1",
                    [expression],
                    |r| r.get(0),
                )
                .unwrap();
            samples.push(start.elapsed().as_micros());
            assert!(rows >= 2200);
        }
        let mut normalized = samples
            .iter()
            .map(|time| time * 100_000 / postings as u128)
            .collect::<Vec<_>>();
        println!(
            "search_postings_calibration query={expression:?} n={RUNS} postings_upper={postings} us {} normalized_100k_us {}",
            summary(&mut samples),
            summary(&mut normalized)
        );
    }
}

pub(super) fn exercise(source: &Path, scratch: &Path) {
    prepare_source(source);
    for collection in [false, true] {
        let path = scratch.join(if collection {
            "collection-search.db"
        } else {
            "space-search.db"
        });
        seed(source, &path, collection);
        let conn = read(&path);
        if !collection {
            calibrate(&conn);
            explain(
                &conn,
                BOUNDED_TEXT,
                params![
                    "ordinary AND regular AND ubiquitous",
                    WINDOW as i64,
                    0,
                    POSTING_BUDGET / 3
                ],
            );
        }
        explain(&conn, TEXT, params!["alpha AND beta", WINDOW as i64, 2_000]);
        explain(
            &conn,
            &format!(
                "SELECT id FROM search_docs WHERE {FILTER}
            ORDER BY title_key,id LIMIT 200 OFFSET 2000"
            ),
            ["notebook", "notebool"],
        );
        explain(&conn, "SELECT doc FROM term_stats WHERE term=?1", ["alpha"]);
        for (name, query) in QUERIES {
            let expected_count = exact_count(&conn, query).unwrap();
            assert!(expected_count >= 2_000 + WINDOW as i64);
            for offset in [0, 200, 2_000] {
                let expected = window(&conn, query, offset).unwrap();
                assert_eq!(expected.ids.len(), WINDOW);
                let mut times = Vec::new();
                for _ in 0..RUNS {
                    let start = Instant::now();
                    let actual = window(&read(&path), query, offset).unwrap();
                    times.push(start.elapsed().as_micros());
                    assert_eq!(actual, expected);
                }
                println!(
                    "scoped_search collection={collection} terms={name} depth={offset} n={RUNS} row_visits={} doc_lookups={} postings_budget={} us {}",
                    expected.visits,
                    if name == "title_prefix" { 0 } else { WINDOW },
                    expected.estimated_postings,
                    summary(&mut times)
                );
            }
            let mut counts = Vec::new();
            for _ in 0..RUNS {
                let start = Instant::now();
                let actual = exact_count(&read(&path), query).unwrap();
                counts.push(start.elapsed().as_micros());
                assert_eq!(actual, expected_count);
            }
            println!(
                "scoped_count collection={collection} terms={name} n={RUNS} hits={expected_count} range_walk={} us {}",
                if name == "one" { 0 } else { expected_count },
                summary(&mut counts)
            );
        }
        // A hidden-only row can change text and ordering in the authoritative
        // source without writing to this corpus. Assert unchanged order and
        // count at every tested depth, and no match for its private term.
        let observed = |conn: &Connection| {
            QUERIES.map(|(_, query)| {
                (
                    [0, 200, 2_000].map(|offset| window(conn, query, offset).unwrap()),
                    exact_count(conn, query),
                )
            })
        };
        let before = observed(&conn);
        let writer = Connection::open(source).unwrap();
        let private_row = if collection { 2 } else { 1 };
        writer
            .execute(
                "UPDATE search_docs SET title='Notebook privateonly alpha beta gamma',
            title_key='notebook privateonly alpha beta gamma' WHERE rowid=?1",
                [private_row],
            )
            .unwrap();
        writer
            .execute(
                "UPDATE search_text SET title='Notebook privateonly alpha beta gamma',
             body='privateonly alpha beta gamma' WHERE rowid=?1",
                [private_row],
            )
            .unwrap();
        let rebuilt_path = scratch.join("check-rebuilt.db");
        seed(source, &rebuilt_path, collection);
        let rebuilt = read(&rebuilt_path);
        assert_eq!(observed(&rebuilt), before);
        assert_eq!(exact_count(&rebuilt, Query::Text("privateonly")), Some(0));
        assert!(
            window(&rebuilt, Query::Text("privateonly"), 0)
                .unwrap()
                .ids
                .is_empty()
        );
        drop(rebuilt);
        std::fs::remove_file(rebuilt_path).unwrap();
        assert!(window(&conn, Query::Text(r#"alpha OR "beta""#), 0).is_none());
        assert!(window(&conn, Query::Text("alpha beta gamma delta"), 0).is_none());
        assert!(window(&conn, Query::Text("alpha"), 2_001).is_none());
        assert!(window(&conn, Query::TitlePrefix(""), 0).is_none());
        if !collection {
            for (name, query) in [
                ("broad_and", Query::Text("ordinary regular ubiquitous")),
                ("broad_prefix", Query::TokenPrefix("or")),
            ] {
                let mut samples = Vec::new();
                let mut row_count = 0;
                for _ in 0..RUNS {
                    let start = Instant::now();
                    let Bind::RefineQuery(partial) = run(&read(&path), query, 2_000) else {
                        panic!("broad query should return typed refinement");
                    };
                    samples.push(start.elapsed().as_micros());
                    row_count = partial.len();
                    assert_eq!(row_count, if name == "broad_and" { WINDOW } else { 0 });
                }
                println!(
                    "scoped_refine shape={name} n={RUNS} rows={row_count} us {}",
                    summary(&mut samples)
                );
            }
        }
        let version = |db: &Connection| {
            db.query_row(
                "SELECT grant_version FROM versions_of_projection",
                [],
                |r| r.get::<_, i64>(0),
            )
            .unwrap()
        };
        let old_grant = version(&writer);
        let buffered = (
            window(&conn, Query::Text("alpha"), 0),
            exact_count(&conn, Query::Text("alpha")),
        );
        writer
            .execute_batch(
                "BEGIN IMMEDIATE;
            UPDATE acl SET visible=0 WHERE id=(SELECT id FROM search_docs WHERE rowid=11);
            UPDATE versions_of_projection SET grant_version=grant_version+1; COMMIT;",
            )
            .unwrap();
        assert!(
            (old_grant == version(&writer))
                .then_some(buffered)
                .is_none()
        );
        writer
            .execute_batch(
                "BEGIN IMMEDIATE;
            UPDATE acl SET visible=1 WHERE id=(SELECT id FROM search_docs WHERE rowid=11);
            UPDATE versions_of_projection SET grant_version=grant_version+1; COMMIT;",
            )
            .unwrap();
        println!(
            "scoped_search_isolation collection={collection} hidden_ranks_changed=0 hidden_counts_changed=0 revoked_deliveries=0"
        );
    }
}
