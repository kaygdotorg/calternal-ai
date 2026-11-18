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
