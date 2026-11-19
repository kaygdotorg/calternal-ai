//! Scoped newest-first indexes with eager exact-rank maintenance.
use super::{ROWS, RUNS, WINDOW, summary};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use std::path::Path;
use std::time::Instant;

const AUDIENCE: i64 = 1;
const DYNAMIC: i64 = 1;
const MANUAL: i64 = 2;

const SCHEMA: &str = "
    CREATE TABLE scoped_membership(
        collection INTEGER NOT NULL,
        audience INTEGER NOT NULL,
        newest INTEGER NOT NULL,
        id TEXT NOT NULL,
        PRIMARY KEY(collection,audience,newest,id)
    ) WITHOUT ROWID, STRICT;
    CREATE TABLE scoped_rank(
        collection INTEGER NOT NULL,
        audience INTEGER NOT NULL,
        rank INTEGER NOT NULL,
        newest INTEGER NOT NULL,
        id TEXT NOT NULL,
        PRIMARY KEY(collection,audience,rank)
    ) WITHOUT ROWID, STRICT;
    CREATE TABLE projection_version(grant INTEGER NOT NULL, result INTEGER NOT NULL);
    INSERT INTO projection_version VALUES(1,1);";

// Content's UTC timestamps sort lexically (descending); the fixture stores
// negative integer timestamps to express the mixed DESC/ASC tuple as one seek.
const SEEK: &str = "SELECT id FROM scoped_membership
    WHERE collection=?1 AND audience=?2 AND (newest,id)>=(?3,?4)
    ORDER BY newest,id LIMIT ?5";
const ANCHOR: &str = "SELECT newest,id FROM scoped_rank
    WHERE collection=?1 AND audience=?2 AND rank=?3";

fn rerank(conn: &Connection, collection: i64) -> usize {
    conn.execute(
        "DELETE FROM scoped_rank WHERE collection=?1 AND audience=?2",
        params![collection, AUDIENCE],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO scoped_rank
         SELECT collection,audience,row_number() OVER (ORDER BY newest,id)-1,newest,id
         FROM scoped_membership WHERE collection=?1 AND audience=?2",
        params![collection, AUDIENCE],
    )
    .unwrap()
}

fn seed_projection(conn: &mut Connection, source: &Path) {
    conn.pragma_update(None, "journal_mode", "WAL").unwrap();
    conn.pragma_update(None, "synchronous", "FULL").unwrap();
    conn.execute_batch(SCHEMA).unwrap();
    conn.execute("ATTACH DATABASE ?1 AS source", [source.to_str().unwrap()])
        .unwrap();
    let tx = conn.transaction().unwrap();
    // ACL is part of the maintained projection. No post-seek row is filtered;
    // an audience can only seek and count inside its own sorted keys.
    tx.execute(
        "INSERT INTO scoped_membership
         SELECT ?1,?2,-CAST(i.updated_at AS INTEGER),i.id FROM source.items i
         JOIN source.acl a ON a.id=i.id AND a.visible
         JOIN source.item_terms t ON t.item=i.id AND t.kind='notes'
            AND t.category='label' AND t.name='target'",
        params![DYNAMIC, AUDIENCE],
    )
    .unwrap();
    tx.execute(
        "INSERT INTO scoped_membership
         SELECT ?1,?2,-CAST(i.updated_at AS INTEGER),i.id FROM source.items i
         JOIN source.acl a ON a.id=i.id AND a.visible
         JOIN source.note_collection_members m ON m.note_id=i.id AND m.collection_id='collection'",
        params![MANUAL, AUDIENCE],
    )
    .unwrap();
    assert!(rerank(&tx, DYNAMIC) >= 90_000 + WINDOW);
    assert!(rerank(&tx, MANUAL) < 10_000);
    tx.commit().unwrap();
    conn.execute_batch("DETACH DATABASE source; PRAGMA wal_checkpoint(TRUNCATE);")
        .unwrap();
}

fn version(conn: &Connection) -> (i64, i64) {
    conn.query_row("SELECT grant,result FROM projection_version", [], |r| {
        Ok((r.get(0)?, r.get(1)?))
    })
    .unwrap()
}

fn window(conn: &mut Connection, collection: i64, offset: i64) -> (Vec<String>, usize, (i64, i64)) {
    let tx = conn.transaction().unwrap();
    let version = version(&tx);
    let anchor = tx
        .query_row(ANCHOR, params![collection, AUDIENCE, offset], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
        })
        .optional()
        .unwrap();
    let Some((newest, id)) = anchor else {
        return (Vec::new(), 0, version);
    };
    let rows = tx
        .prepare(SEEK)
        .unwrap()
        .query_map(
            params![collection, AUDIENCE, newest, id, WINDOW as i64],
            |r| r.get(0),
        )
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    let visits = rows.len() + 1; // Covering ordered range + one locator row.
    (rows, visits, version)
}

fn explain(conn: &Connection, sql: &str, p: impl rusqlite::Params) {
    let lines = conn
        .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
        .unwrap()
        .query_map(p, |r| r.get::<_, String>(3))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    assert!(
        lines
            .iter()
            .all(|line| line.starts_with("SEARCH ") && line.contains("PRIMARY KEY"))
    );
    println!("scoped plan {}", lines.join(" | "));
}

fn mutation(
    writer: &mut Connection,
    kind: &str,
    run: usize,
    item: &str,
    initial_key: i64,
) -> (usize, usize) {
    let tx = writer.transaction().unwrap();
    let changes = match kind {
        "save" => {
            tx.execute(
                "UPDATE scoped_membership SET newest=?1 WHERE audience=?2 AND id=?3
                 AND collection IN (?4,?5) AND newest=?6",
                params![
                    if run % 2 == 0 { -(ROWS as i64) } else { 1 },
                    AUDIENCE,item,DYNAMIC,MANUAL,
                    if run == 0 { initial_key } else if run % 2 == 0 { 1 } else { -(ROWS as i64) }
                ],
            ).unwrap()
        }
        "membership" | "grant" if run % 2 == 0 => tx.execute(
            "DELETE FROM scoped_membership WHERE collection=?1 AND audience=?2 AND newest=1 AND id=?3",
            params![DYNAMIC,AUDIENCE,item],
        ).unwrap(),
        "membership" | "grant" => tx.execute(
            "INSERT INTO scoped_membership VALUES(?1,?2,1,?3)",
            params![DYNAMIC,AUDIENCE,item],
        ).unwrap(),
        _ => unreachable!(),
    };
    assert_eq!(changes, if kind == "save" { 2 } else { 1 });
    let mut rewritten = rerank(&tx, DYNAMIC);
    if kind == "save" {
        rewritten += rerank(&tx, MANUAL);
    }
    tx.execute(
        "UPDATE projection_version SET result=result+1,grant=grant+?1",
        [i64::from(kind == "grant")],
    )
    .unwrap();
    tx.commit().unwrap();
    (changes, rewritten)
}

pub(super) fn exercise(source: &Path, projection: &Path) {
    let mut writer = Connection::open(projection).unwrap();
    seed_projection(&mut writer, source);
    explain(&writer, ANCHOR, params![DYNAMIC, AUDIENCE, 0]);
    explain(
        &writer,
        SEEK,
        params![DYNAMIC, AUDIENCE, -100_000, "", WINDOW as i64],
    );
    let bytes = |pragma| {
        writer
            .pragma_query_value(None, pragma, |r| r.get::<_, i64>(0))
            .unwrap()
    };
    println!(
        "scoped storage main_bytes={} live_page_bytes={} authorized_rows={} source_rows={ROWS}",
        bytes("page_count") * bytes("page_size"),
        (bytes("page_count") - bytes("freelist_count")) * bytes("page_size"),
        writer
            .query_row("SELECT count(*) FROM scoped_membership", [], |r| r
                .get::<_, i64>(0))
            .unwrap()
    );

    // Select a row in both collections, so one item save rewrites both scopes.
    let (item, initial_key): (String, i64) = writer.query_row(
        "SELECT id,newest FROM scoped_membership WHERE collection=?1 AND audience=?2
         INTERSECT SELECT id,newest FROM scoped_membership WHERE collection=?3 AND audience=?2 LIMIT 1",
        params![MANUAL,AUDIENCE,DYNAMIC], |r| Ok((r.get(0)?,r.get(1)?)),
    ).unwrap();
    let mut window_samples = std::collections::BTreeMap::new();
    for kind in ["save", "membership", "grant"] {
        let mut elapsed = Vec::new();
        let mut ranks = Vec::new();
        let mut membership = 0;
        for run in 0..RUNS {
            let start = Instant::now();
            let (changed, rewritten) = mutation(&mut writer, kind, run, &item, initial_key);
            elapsed.push(start.elapsed().as_micros());
            ranks.push(rewritten as u128);
            membership = changed;
            if kind == "grant" {
                let stamp = version(&writer);
                for collection in [DYNAMIC, MANUAL] {
                    for offset in [0, 10_000, 90_000] {
                        let start = Instant::now();
                        let mut reader = Connection::open_with_flags(
                            projection,
                            OpenFlags::SQLITE_OPEN_READ_ONLY,
                        )
                        .unwrap();
                        let (rows, visited, snapshot) = window(&mut reader, collection, offset);
                        assert_eq!(stamp, snapshot);
                        let elapsed = start.elapsed().as_micros();
                        assert_eq!(
                            rows.len(),
                            if collection == MANUAL && offset > 0 {
                                0
                            } else {
                                WINDOW
                            }
                        );
                        assert!(visited <= WINDOW + 1);
                        let (times, visits) = window_samples
                            .entry((collection, offset))
                            .or_insert_with(|| (Vec::new(), Vec::new()));
                        times.push(elapsed);
                        visits.push(visited as u128);
                    }
                }
                writer
                    .execute("UPDATE projection_version SET grant=grant+1", [])
                    .unwrap();
                assert_ne!(version(&writer), stamp);
            }
        }
        println!(
            "scoped {kind} n={RUNS} changed_memberships={membership} us {} rank_rows_reinserted {}",
            summary(&mut elapsed),
            summary(&mut ranks)
        );
    }

    for ((collection, offset), (mut elapsed, mut visits)) in window_samples {
        println!(
            "scoped window collection={collection} offset={offset} n={RUNS} us {} visits {}",
            summary(&mut elapsed),
            summary(&mut visits)
        );
    }
}
