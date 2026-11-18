//! Concurrent scheduler writers sharing a file or using one file per space.
use super::*;
use std::{path::Path, sync::Barrier, thread, time::Instant};

const OCCURRENCES: i64 = 32;

#[derive(Default)]
struct Samples {
    waits: Vec<u128>,
    commits: Vec<u128>,
    admission_busy: usize,
    finish_busy: usize,
}

fn open(path: &Path) -> Connection {
    let conn = Connection::open(path).unwrap();
    conn.busy_timeout(Duration::from_secs(3)).unwrap();
    conn.pragma_update(None, "synchronous", "FULL").unwrap();
    migrate(&conn, SCHEMA).unwrap();
    conn
}

fn writer(conn: &mut Connection) -> Option<rusqlite::Transaction<'_>> {
    match conn.transaction_with_behavior(TransactionBehavior::Immediate) {
        Ok(tx) => Some(tx),
        Err(error) if error.sqlite_error_code() == Some(rusqlite::ErrorCode::DatabaseBusy) => None,
        Err(error) => panic!("scheduler writer: {error}"),
    }
}

fn exercise(
    conn: &mut Connection,
    space: SpaceId,
    automations: &[Uuid],
    wave: Option<&Barrier>,
) -> Samples {
    let mut samples = Samples::default();
    for (tick, automation) in (0..OCCURRENCES).zip(automations) {
        if let Some(wave) = wave {
            wave.wait();
        }
        let clock = tick * 60_000;
        let begin = Instant::now();
        let tx = writer(conn);
        samples.waits.push(begin.elapsed().as_micros());
        let Some(tx) = tx else {
            samples.admission_busy += 1;
            continue;
        };
        let run = queue(
            &tx,
            space,
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
        samples.commits.push(begin.elapsed().as_micros());

        let Some(tx) = writer(conn) else {
            samples.finish_busy += 1;
            continue;
        };
        tx.execute(
            "UPDATE runs SET finished=?2 WHERE id=?1",
            params![run.id.to_string(), clock],
        )
        .unwrap();
        tx.commit().unwrap();
    }
    samples
}

fn burst(spaces: usize, per_space: bool, synchronized: bool) {
    let root = tempfile::tempdir().unwrap();
    let ready = Barrier::new(spaces + 1);
    let wave = synchronized.then(|| Barrier::new(spaces));
    let keys: Vec<_> = (0..spaces)
        .map(|index| {
            let name = if per_space {
                format!("{index}.db")
            } else {
                "shared.db".into()
            };
            let path = root.path().join(name);
            let space = SpaceId::new();
            let mut conn = open(&path);
            let tx = conn.transaction().unwrap();
            let automations: Vec<_> = (0..OCCURRENCES)
                .map(|_| {
                    let automation = Uuid::now_v7();
                    tx.execute(
                        "INSERT INTO schedules(space,id,revision,creator,epoch,enabled,next)
                         VALUES(?1,?2,'revision','creator','epoch',1,0)",
                        params![space.to_string(), automation.to_string()],
                    )
                    .unwrap();
                    automation
                })
                .collect();
            tx.commit().unwrap();
            (path, space, automations)
        })
        .collect();
    thread::scope(|scope| {
        let writers: Vec<_> = keys
            .iter()
            .map(|(path, space, automations)| {
                let ready = &ready;
                let wave = wave.as_ref();
                scope.spawn(move || {
                    let mut conn = open(path);
                    ready.wait();
                    exercise(&mut conn, *space, automations, wave)
                })
            })
            .collect();
        let started = Instant::now();
        ready.wait();
        let mut combined = Samples::default();
        for writer in writers {
            let sample = writer.join().unwrap();
            combined.waits.extend(sample.waits);
            combined.commits.extend(sample.commits);
            combined.admission_busy += sample.admission_busy;
            combined.finish_busy += sample.finish_busy;
        }
        combined.waits.sort_unstable();
        combined.commits.sort_unstable();
        let percentile = |data: &[u128], nth: usize| data[(data.len() * nth).div_ceil(100) - 1];
        println!(
            concat!(
                "spaces={} per_space={} synchronized={} samples={} wall_ms={} ",
                "admit_busy={} finish_busy={} wait_us_p50={} wait_p95={} wait_p99={} ",
                "wait_max={} commit_us_p95={} commit_p99={}"
            ),
            spaces,
            per_space,
            synchronized,
            combined.waits.len(),
            started.elapsed().as_millis(),
            combined.admission_busy,
            combined.finish_busy,
            percentile(&combined.waits, 50),
            percentile(&combined.waits, 95),
            percentile(&combined.waits, 99),
            combined.waits.last().unwrap(),
            percentile(&combined.commits, 95),
            percentile(&combined.commits, 99),
        );
    });
}

#[test]
#[ignore = "Measure on a perf VM under perf-lock.sh using a dev-built binary."]
fn scheduler_writer_contention() {
    assert_eq!(std::env::var("CALTERNAL_PERF_HELD").as_deref(), Ok("1"));
    for (spaces, synchronized) in [(1, true), (4, true), (16, true), (4, false), (16, false)] {
        for _ in 0..3 {
            burst(spaces, false, synchronized);
            burst(spaces, true, synchronized);
        }
    }
}
