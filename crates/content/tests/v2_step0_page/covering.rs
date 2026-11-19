//! Covering membership ranges with one transactional count per scope.
use super::scoped::{AUDIENCE, DYNAMIC, MANUAL, explain, seed_membership};
use super::{ROWS, RUNS, WINDOW, summary};
use rusqlite::{Connection, OpenFlags, StatementStatus, params};
use std::path::Path;
use std::time::Instant;

pub(super) const COUNTS: &str = "
    CREATE TABLE scoped_counts(
        collection INTEGER NOT NULL,audience INTEGER NOT NULL,n INTEGER NOT NULL CHECK(n>=0),
        PRIMARY KEY(collection,audience)
    ) WITHOUT ROWID, STRICT;
    CREATE TRIGGER membership_insert AFTER INSERT ON scoped_membership BEGIN
        INSERT INTO scoped_counts VALUES(NEW.collection,NEW.audience,1)
        ON CONFLICT(collection,audience) DO UPDATE SET n=n+1;
    END;
    CREATE TRIGGER membership_delete AFTER DELETE ON scoped_membership BEGIN
        UPDATE scoped_counts SET n=n-1
        WHERE collection=OLD.collection AND audience=OLD.audience;
    END;";
const COUNT: &str = "SELECT n FROM scoped_counts WHERE collection=?1 AND audience=?2";
const OFFSETS: [i64; 4] = [0, 10_000, 50_000, 90_000];

fn count(conn: &Connection, collection: i64) -> i64 {
    conn.query_row(COUNT, params![collection, AUDIENCE], |r| r.get(0))
        .unwrap()
}

fn reader(path: &Path) -> Connection {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap()
}

fn ordered_sql(reverse: bool, predicate: &str, limit: &str) -> String {
    let order = if reverse {
        "newest DESC,id DESC"
    } else {
        "newest,id"
    };
    format!(
        "SELECT newest,id FROM scoped_membership WHERE collection=?1 AND audience=?2
             {predicate} ORDER BY {order} {limit}"
    )
}

type Key = (i64, String);
fn fetch(conn: &Connection, sql: &str, params: impl rusqlite::Params) -> (Vec<Key>, i64) {
    let mut stmt = conn.prepare_cached(sql).unwrap();
    let keys = stmt
        .query_map(params, |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    let steps = i64::from(stmt.get_status(StatementStatus::VmStep));
    stmt.reset_status(StatementStatus::VmStep);
    (keys, steps)
}

fn jumps(db: &Path, writer: &Connection) {
    let ahead = ordered_sql(false, "", "LIMIT ?3 OFFSET ?4");
    let back = ordered_sql(true, "", "LIMIT ?3 OFFSET ?4");
    explain(
        writer,
        &ahead,
        params![DYNAMIC, AUDIENCE, WINDOW as i64, 10_000],
    );
    explain(
        writer,
        &back,
        params![DYNAMIC, AUDIENCE, WINDOW as i64, 10_000],
    );
    for collection in [DYNAMIC, MANUAL] {
        let total = count(writer, collection);
        let mut offsets = OFFSETS.to_vec();
        if collection == MANUAL {
            offsets.extend([total - WINDOW as i64, total - 1, total]);
            offsets.sort_unstable();
        }
        for offset in offsets {
            let mut elapsed = Vec::new();
            let mut visits = Vec::new();
            let mut vm = Vec::new();
            // Independent forward walk checks the reverse offset and row order.
            let expected = fetch(
                writer,
                &ahead,
                params![collection, AUDIENCE, WINDOW as i64, offset],
            )
            .0;
            let mut direction = "empty";
            for _ in 0..RUNS {
                let start = Instant::now();
                let mut conn = reader(db);
                let tx = conn.transaction().unwrap();
                let total_read = count(&tx, collection);
                let size = (total_read - offset).clamp(0, WINDOW as i64);
                let (rows, visited, steps) = if size == 0 {
                    (Vec::new(), 0, 0)
                } else {
                    let skip_from_end = total_read - offset - size;
                    let reverse = skip_from_end < offset;
                    let skip = offset.min(skip_from_end);
                    assert!(skip <= total_read / 2);
                    direction = if reverse { "end" } else { "start" };
                    let (mut rows, steps) = fetch(
                        &tx,
                        if reverse { &back } else { &ahead },
                        params![collection, AUDIENCE, size, skip],
                    );
                    if reverse {
                        rows.reverse();
                    }
                    (rows, skip + size, steps)
                };
                elapsed.push(start.elapsed().as_micros());
                assert_eq!(total_read, total);
                assert_eq!(rows, expected);
                visits.push(visited as u128);
                vm.push(steps as u128);
            }
            println!(
                "covering jump collection={collection} offset={offset} total={total} near={direction} n={RUNS} us {} visits {} vm_steps {}",
                summary(&mut elapsed),
                summary(&mut visits),
                summary(&mut vm)
            );
        }
    }
}

fn keysets(db: &Path, writer: &Connection) {
    for (reverse, edge) in [(false, "Home"), (true, "End")] {
        let head = ordered_sql(reverse, "", "LIMIT ?3");
        let predicate = if reverse {
            "AND (newest,id)<(?3,?4)"
        } else {
            "AND (newest,id)>(?3,?4)"
        };
        let resume = ordered_sql(reverse, predicate, "LIMIT ?5");
        let first = fetch(writer, &head, params![DYNAMIC, AUDIENCE, WINDOW as i64]).0;
        let expected = fetch(
            writer,
            &ordered_sql(reverse, "", "LIMIT ?3 OFFSET ?4"),
            params![DYNAMIC, AUDIENCE, WINDOW as i64, WINDOW as i64],
        )
        .0;
        let (key, id) = first.last().unwrap();
        explain(
            writer,
            &resume,
            params![DYNAMIC, AUDIENCE, key, id, WINDOW as i64],
        );
        let mut starts = Vec::new();
        let mut nexts = Vec::new();
        for _ in 0..RUNS {
            let start = Instant::now();
            let rows = fetch(
                &reader(db),
                &head,
                params![DYNAMIC, AUDIENCE, WINDOW as i64],
            )
            .0;
            starts.push(start.elapsed().as_micros());
            assert_eq!(rows, first);
            let start = Instant::now();
            let rows = fetch(
                &reader(db),
                &resume,
                params![DYNAMIC, AUDIENCE, key, id, WINDOW as i64],
            )
            .0;
            nexts.push(start.elapsed().as_micros());
            assert_eq!(rows, expected);
        }
        println!(
            "covering keyset {edge} n={RUNS} visits={WINDOW} edge_us {} next_us {}",
            summary(&mut starts),
            summary(&mut nexts)
        );
    }
}

fn writes(writer: &mut Connection) {
    let (item, mut old_key): (String, i64) = writer
        .query_row(
            "SELECT id,newest FROM scoped_membership WHERE collection=?1 AND audience=?2
         INTERSECT SELECT id,newest FROM scoped_membership WHERE collection=?3 AND audience=?2
         LIMIT 1",
            params![MANUAL, AUDIENCE, DYNAMIC],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    let expected_count = count(writer, DYNAMIC);
    let mut saves = Vec::new();
    let mut membership = Vec::new();
    for run in 0..RUNS {
        let new_key = if run % 2 == 0 { -(ROWS as i64) } else { 1 };
        let start = Instant::now();
        let tx = writer.transaction().unwrap();
        for collection in [DYNAMIC, MANUAL] {
            assert_eq!(tx.execute(
                "UPDATE scoped_membership SET newest=?1 WHERE collection=?2 AND audience=?3 AND newest=?4 AND id=?5",
                params![new_key,collection,AUDIENCE,old_key,item],
            ).unwrap(),1);
        }
        tx.execute("UPDATE projection_version SET result=result+1", [])
            .unwrap();
        tx.commit().unwrap();
        saves.push(start.elapsed().as_micros());
        assert_eq!(count(writer, DYNAMIC), expected_count);
        old_key = new_key;

        let start = Instant::now();
        let tx = writer.transaction().unwrap();
        assert_eq!(tx.execute(
            "DELETE FROM scoped_membership WHERE collection=?1 AND audience=?2 AND newest=?3 AND id=?4",
            params![DYNAMIC,AUDIENCE,old_key,item],
        ).unwrap(),1);
        tx.execute("UPDATE projection_version SET result=result+1", [])
            .unwrap();
        tx.commit().unwrap();
        membership.push(start.elapsed().as_micros());
        assert_eq!(count(writer, DYNAMIC), expected_count - 1);
        // Reset membership for the next sample. The restore also updates the count.
        let tx = writer.transaction().unwrap();
        tx.execute(
            "INSERT INTO scoped_membership VALUES(?1,?2,?3,?4)",
            params![DYNAMIC, AUDIENCE, old_key, item],
        )
        .unwrap();
        tx.execute("UPDATE projection_version SET result=result+1", [])
            .unwrap();
        tx.commit().unwrap();
        assert_eq!(count(writer, DYNAMIC), expected_count);
    }
    println!(
        "covering save n={RUNS} changed_memberships=2 old_new_keys=4 count_rows=0 us {}",
        summary(&mut saves)
    );
    println!(
        "covering membership removal n={RUNS} changed_memberships=1 count_rows=1 us {}",
        summary(&mut membership)
    );
}

pub(super) fn exercise(source: &Path, db: &Path) {
    let mut writer = Connection::open(db).unwrap();
    seed_membership(&mut writer, source, COUNTS, |_| {});
    let bytes = |pragma| {
        writer
            .pragma_query_value(None, pragma, |r| r.get::<_, i64>(0))
            .unwrap()
    };
    println!(
        "covering storage main_bytes={} live_page_bytes={} source_rows={ROWS} dynamic={} manual={}",
        bytes("page_count") * bytes("page_size"),
        (bytes("page_count") - bytes("freelist_count")) * bytes("page_size"),
        count(&writer, DYNAMIC),
        count(&writer, MANUAL)
    );
    explain(&writer, COUNT, params![DYNAMIC, AUDIENCE]);
    let mut counts = Vec::new();
    let expected_count = count(&writer, DYNAMIC);
    for _ in 0..RUNS {
        let start = Instant::now();
        let actual = count(&reader(db), DYNAMIC);
        counts.push(start.elapsed().as_micros());
        assert_eq!(expected_count, actual);
    }
    println!(
        "covering exact count n={RUNS} visits=1 us {}",
        summary(&mut counts)
    );
    keysets(db, &writer);
    jumps(db, &writer);
    writes(&mut writer);
}
