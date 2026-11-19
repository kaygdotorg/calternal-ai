use super::*;
use uuid::Uuid;

fn input(thread: i64, stamp: i64, id: u128) -> Input {
    Input {
        record: MessageRecord {
            id: Uuid::from_u128(id),
            stamp,
            offset_seconds: 0,
            record: 5..20,
            body: 10..20,
        },
        thread,
        words: vec!["canvas".into()],
    }
}

#[test]
fn thread_removal_uses_an_index_with_the_thread_prefix() {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(SCHEMA).unwrap();
    db.execute_batch(SECONDARY).unwrap();
    let plan: String = db
        .query_row(
            "EXPLAIN QUERY PLAN DELETE FROM locations WHERE thread=?1 RETURNING position",
            [7],
            |row| row.get(3),
        )
        .unwrap();
    assert!(
        ["locations_id", "locations_thread"].iter().any(|index| {
            plan.contains(&format!("SEARCH locations USING INDEX {index} (thread=?)"))
                || plan.contains(&format!(
                    "SEARCH locations USING COVERING INDEX {index} (thread=?)"
                ))
        }),
        "{plan}"
    );
}

#[test]
fn late_runs_sort_by_saved_time_and_deletion_updates_shared_block() {
    let mut db = Connection::open_in_memory().unwrap();
    db.execute_batch(SCHEMA).unwrap();
    let tx = db.transaction().unwrap();
    let mut state = State {
        generation: GENERATION.to_owned(),
        epoch: "fixture".into(),
        watermark: 0,
        after: None,
        ready: false,
        next_block: 0,
    };
    write_run(&tx, &mut state, &mut [input(1, 100, 2), input(2, 100, 3)]).unwrap();
    write_run(&tx, &mut state, &mut [input(3, 99, 4)]).unwrap();
    tx.commit().unwrap();
    db.execute_batch(SECONDARY).unwrap();
    let keys = |db: &Connection| {
        db.prepare("SELECT stamp,newest FROM postings INDEXED BY postings_newest WHERE term=1 ORDER BY stamp DESC,newest DESC,block")
            .unwrap()
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, super::super::uuid(r, 1)?.as_u128())))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap()
    };
    assert_eq!(keys(&db), [(100, 3), (99, 4)]);
    let tx = db.transaction().unwrap();
    remove(&tx, 2).unwrap();
    tx.commit().unwrap();
    assert_eq!(keys(&db), [(100, 2), (99, 4)]);
    let tx = db.transaction().unwrap();
    remove(&tx, 1).unwrap();
    tx.commit().unwrap();
    assert_eq!(keys(&db), [(99, 4)]);
}

#[test]
fn append_backlog_advances_in_batches_without_rebuilding_the_thread() {
    let primary = Connection::open_in_memory().unwrap();
    calternal_store::migrations::migrate(&primary, crate::SPACE_SCHEMA).unwrap();
    primary.execute_batch(
        "INSERT INTO items VALUES
         ('t','threads','{}','v66',66,'now','thread.md','thread.md',NULL,'materialized',NULL);
         INSERT INTO versions VALUES ('t',1,'v1','{}',X'FF','now','owner');
         WITH RECURSIVE n(i) AS (SELECT 2 UNION ALL SELECT i+1 FROM n WHERE i<66)
         INSERT INTO versions SELECT 't',i,'v'||i,'{}',CAST(printf('%02d',i) AS BLOB),'now','owner' FROM n;
         INSERT INTO thread_revision_patches
         SELECT 't',seq,1,2*seq-3,2,zeroblob(32),zeroblob(32),printf('%064d',0),printf('%064d',0),seq
         FROM versions WHERE seq>1;",
    ).unwrap();
    let head = source::get(&primary, "t").unwrap().unwrap();
    let mut db = Connection::open_in_memory().unwrap();
    db.execute_batch(SCHEMA).unwrap();
    db.execute("INSERT INTO threads VALUES(?1,'t','v1')", [head.slot])
        .unwrap();
    let tx = db.transaction().unwrap();
    let mut state = State {
        generation: GENERATION.to_owned(),
        epoch: "fixture".into(),
        watermark: 0,
        after: None,
        ready: true,
        next_block: 0,
    };
    let decode = |bytes: &[u8], fragment: bool| {
        assert!(fragment, "a backlog must never parse the base snapshot");
        let id = std::str::from_utf8(bytes).unwrap().parse::<u128>().unwrap();
        Ok(vec![MessageRecord {
            id: Uuid::from_u128(id),
            stamp: id as i64,
            offset_seconds: 0,
            record: 0..bytes.len(),
            body: 0..bytes.len(),
        }])
    };
    assert!(!replace(&primary, &tx, &mut state, "t", Some(&head), decode).unwrap());
    let status = || {
        tx.query_row(
            "SELECT revision,(SELECT count(*) FROM locations) FROM threads WHERE id='t'",
            [],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, usize>(1)?)),
        )
        .unwrap()
    };
    assert_eq!(status(), ("v65".into(), 64));
    assert!(replace(&primary, &tx, &mut state, "t", Some(&head), decode).unwrap());
    assert_eq!(status(), ("v66".into(), 65));
}
