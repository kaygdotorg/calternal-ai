//! Short bursts of independent spaces writing the same scheduler file.
use super::*;
use std::{path::Path, sync::Barrier, thread, time::Instant};

const OCCURRENCES: i64 = 32;

fn open(path: &Path) -> Connection {
    let conn = Connection::open(path).unwrap();
    conn.busy_timeout(Duration::from_secs(3)).unwrap();
    conn.pragma_update(None, "synchronous", "FULL").unwrap();
    migrate(&conn, SCHEMA).unwrap();
    conn
}

fn burst(spaces: usize, per_space: bool) {
    let root = tempfile::tempdir().unwrap();
    let ready = Barrier::new(spaces + 1);
    let keys: Vec<_> = (0..spaces)
        .map(|i| {
            let path = root.path().join(if per_space {
                format!("{i}.db")
            } else {
                "shared.db".into()
            });
            let space = SpaceId::parse(&Uuid::now_v7().to_string()).unwrap();
            let automation = Uuid::now_v7();
            open(&path)
                .execute(
                    "INSERT INTO schedules(space,id,revision,creator,epoch,enabled,next)
                VALUES(?1,?2,'revision','creator','epoch',1,0)",
                    params![space.to_string(), automation.to_string()],
                )
                .unwrap();
            (path, space, automation)
        })
        .collect();
    thread::scope(|scope| {
        let readers: Vec<_> = keys
            .iter()
            .map(|(path, space, automation)| {
                let ready = &ready;
                scope.spawn(move || {
                    let mut conn = open(path);
                    let mut waits = Vec::new();
                    let mut totals = Vec::new();
                    ready.wait();
                    for tick in 0..OCCURRENCES {
                        let clock = tick * 60_000;
                        let begin = Instant::now();
                        let tx = conn
                            .transaction_with_behavior(TransactionBehavior::Immediate)
                            .unwrap();
                        waits.push(begin.elapsed().as_micros());
                        let run = queue(
                            &tx,
                            *space,
                            *automation,
                            "revision".into(),
                            Some(clock),
                            None,
                            clock,
                        )
                        .unwrap()
                        .unwrap();
                        tx.execute(
                            "UPDATE schedules SET next=?3,last_due=?4 WHERE space=?1 AND id=?2",
                            params![
                                space.to_string(),
                                automation.to_string(),
                                clock + 60_000,
                                clock
                            ],
                        )
                        .unwrap();
                        tx.commit().unwrap();
                        totals.push(begin.elapsed().as_micros());

                        let tx = conn
                            .transaction_with_behavior(TransactionBehavior::Immediate)
                            .unwrap();
                        tx.execute(
                            "UPDATE runs SET finished=?2 WHERE id=?1",
                            params![run.id.to_string(), clock],
                        )
                        .unwrap();
                        tx.commit().unwrap();
                    }
                    (waits, totals)
                })
            })
            .collect();
        let started = Instant::now();
        ready.wait();
        let mut waits = Vec::new();
        let mut totals = Vec::new();
        for reader in readers {
            let (wait, total) = reader.join().unwrap();
            waits.extend(wait);
            totals.extend(total);
        }
        let ms = started.elapsed().as_millis();
        waits.sort_unstable();
        totals.sort_unstable();
        let sample =
            |data: &[u128], percentile: usize| data[(data.len() * percentile).div_ceil(100) - 1];
        println!(
            "spaces={spaces} per_space={per_space} samples={} wall_ms={ms} wait_us_p50={} p95={} p99={} max={}              commit_us_p50={} p95={} p99={}",
            waits.len(),
            sample(&waits, 50),
            sample(&waits, 95),
            sample(&waits, 99),
            waits.last().unwrap(),
            sample(&totals, 50),
            sample(&totals, 95),
            sample(&totals, 99),
        );
    });
}

#[test]
#[ignore = "Measure on a perf VM under perf-lock.sh using a dev-built binary."]
fn scheduler_writer_contention() {
    assert_eq!(std::env::var("CALTERNAL_PERF_HELD").as_deref(), Ok("1"));
    for spaces in [1, 4, 16] {
        for _ in 0..3 {
            burst(spaces, false);
            burst(spaces, true);
        }
    }
}
