use super::*;
use crate::requests_cases::Machine;
use std::time::SystemTime;

const CUTS: &[(&str, bool, bool, bool)] = &[
    // Fault, outbox occurrence committed, provider job committed, job claimed.
    ("automation_before_queue_commit", false, false, false),
    ("automation_after_queue_commit", true, false, false),
    ("automation_after_claim", true, false, false),
    ("automation_thread_after_commit", true, false, false),
    ("automation_thread_after_journal_commit", true, false, false),
    ("automation_prompt_after_commit", true, false, false),
    ("automation_prompt_after_journal_commit", true, false, false),
    ("automation_after_thread", true, false, false),
    ("automation_after_prompt", true, false, false),
    ("automation_after_job", true, true, false),
    ("automation_after_admission", true, true, false),
    ("automation_after_release", true, true, false),
    ("automation_after_turn_claim", true, true, true),
    ("automation_after_machine_lease", true, true, true),
    ("automation_after_lease_checkpoint", true, true, true),
    ("automation_after_machine_placement", true, true, true),
    ("automation_after_placement", true, true, true),
    ("automation_after_token", true, true, true),
    ("automation_before_exec", true, true, true),
    ("automation_after_exec", true, true, true),
    ("automation_after_machine_output", true, true, true),
];

#[derive(Debug)]
struct Run {
    id: String,
    thread: Option<String>,
    job: Option<String>,
}

fn occurrence(db: &Connection, space: &str, automation: &str, due: i64) -> Option<Run> {
    let mut stmt = db
        .prepare(
            "SELECT id, thread, job FROM runs WHERE space=?1 AND automation=?2 AND scheduled=?3",
        )
        .unwrap();
    let mut matches = stmt.query(params![space, automation, due]).unwrap();
    let row = matches.next().unwrap()?;
    let run = Run {
        id: row.get(0).unwrap(),
        thread: row.get(1).unwrap(),
        job: row.get(2).unwrap(),
    };
    assert!(matches.next().unwrap().is_none(), "duplicate occurrence");
    Some(run)
}

fn jobs(f: &Fixture, space: &str) -> Vec<(String, String, u32)> {
    let file = f.appdata().join("spaces").join(space).join("jobs.db");
    if !file.is_file() {
        return vec![];
    }
    let conn = Connection::open(file).unwrap();
    let mut query = conn
        .prepare("SELECT id,state,runs FROM jobs WHERE json_type(input,'$.thread_request.automation_run')='object'")
        .unwrap();
    query
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

async fn verify_cut(point: &str, committed: bool, job_committed: bool, claimed: bool) {
    let mut f = Fixture::new().await;
    let space = f.spaces[0].clone();
    let mut machine = Machine::enroll(&f).await;
    let now_minute = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        / 60;
    let due = (now_minute * 60_000) as i64;
    let clock_minutes = now_minute % 1440;
    let words = format!("daily at {}:{:02}", clock_minutes / 60, clock_minutes % 60);
    let preview = data(
        f.op(
            1,
            "automations.preview_schedule",
            json!({"space_id":space,"schedule_text":words}),
        )
        .await,
    );
    let created = data(
        f.op(
            1,
            "automations.create",
            json!({
                "space_id":space,"name":"Crash boundary","prompt":"Read the run marker.",
                "schedule_text":words,"schedule":preview["schedule"],"timezone":preview["timezone"],
                "machine_id":machine.id,
            }),
        )
        .await,
    );
    let automation = created["automation"]["id"].as_str().unwrap();
    f.stop();
    let db_path = f.appdata().join("system/automations.db");
    Connection::open(&db_path)
        .unwrap()
        .execute(
            "UPDATE schedules SET next=?3, checked=?4 WHERE space=?1 AND id=?2",
            params![
                space,
                automation,
                due,
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as i64
                    + 1800
            ],
        )
        .unwrap();

    let fault_file = f.dir.path().join("content-fault");
    let stderr = f.dir.path().join("server.stderr");
    std::fs::write(f.dir.path().join("capture-setup-logs"), "").unwrap();
    std::fs::write(&fault_file, point).unwrap();
    f.start().await;
    let marker = format!("injected fault at {point}; aborting");
    let wait_for_fault = async {
        loop {
            if std::fs::read_to_string(&stderr).is_ok_and(|log| log.contains(&marker)) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    };
    let drive = async {
        machine.connect(&f).await;
        while let Ok(frames) = machine.try_sync(&f, json!([]), 2).await {
            if point == "automation_after_machine_output"
                && let Some(exec) = frames.iter().find(|frame| frame["type"] == "exec")
            {
                // The marker proves the server saw output from a delivered exec.
                let chunks = json!([{"kind":"data","seq":1,"stream":"stderr","data":"b2sK"}]);
                if let Ok(answer) = machine.try_output(&f, exec, chunks).await {
                    answer.ok();
                }
            }
        }
        std::future::pending::<()>().await;
    };
    tokio::time::timeout(Duration::from_secs(12), async {
        tokio::select! {
            () = wait_for_fault => {},
            () = drive => unreachable!(),
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{point}: server never reached the fault"));
    assert!(
        !f.child.take().unwrap().wait().unwrap().success(),
        "{point}: fault did not abort"
    );
    std::fs::remove_file(fault_file).unwrap();
    let conn = Connection::open(&db_path).unwrap();
    let before = occurrence(&conn, &space, automation, due);
    assert_eq!(before.is_some(), committed, "{point}");
    let before_jobs = jobs(&f, &space);
    assert_eq!(before_jobs.len(), usize::from(job_committed), "{point}");
    if let Some((_, _, attempts)) = before_jobs.first() {
        assert_eq!(*attempts, u32::from(claimed), "{point}");
    }
    // Advance the retry timers in the stopped disposable fixture. The real
    // scheduler path still checks the same expired claim/cursor on restart.
    conn.execute(
        "UPDATE schedules SET checked=0 WHERE space=?1 AND id=?2",
        params![space, automation],
    )
    .unwrap();
    conn.execute(
        "UPDATE runs SET claimed_until=0 WHERE space=?1 AND automation=?2",
        params![space, automation],
    )
    .unwrap();
    drop(conn);

    f.start().await;
    if !claimed {
        machine.connect(&f).await;
        machine.next_token(&f).await;
    }
    tokio::time::timeout(Duration::from_secs(12), async {
        loop {
            let last = data(
                f.op(
                    1,
                    "automations.last_run",
                    json!({"space_id":space,"id":automation}),
                )
                .await,
            );
            if last["last"]["status"] == if claimed { "interrupted" } else { "queued" } {
                break;
            }
            tokio::time::sleep(Duration::from_millis(40)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{point}: scheduler did not recover the occurrence"));
    let conn = Connection::open(&db_path).unwrap();
    let saved = occurrence(&conn, &space, automation, due).expect(point);
    if let Some(before) = before {
        assert_eq!(before.id, saved.id, "{point}: changed occurrence identity");
    }
    assert_eq!(saved.thread.as_deref(), Some(saved.id.as_str()), "{point}");
    let after_jobs = jobs(&f, &space);
    assert_eq!(after_jobs.len(), 1, "{point}: extra provider job");
    assert_eq!(
        saved.job.as_deref(),
        Some(after_jobs[0].0.as_str()),
        "{point}"
    );
    if let Some((job, _, _)) = before_jobs.first() {
        assert_eq!(job, &after_jobs[0].0, "{point}: replaced provider job");
    }
    assert_eq!(
        after_jobs[0].2, 1,
        "{point}: duplicate attempt after restart"
    );
    let machine_db = Connection::open(f.appdata().join("system/machines.db")).unwrap();
    let lease_count: i64 = machine_db
        .query_row(
            "SELECT COUNT(*) FROM leases WHERE space=?1 AND thread=?2",
            params![space, saved.id],
            |row| row.get(0),
        )
        .unwrap();
    let expected_leases = i64::from(point != "automation_after_turn_claim");
    assert_eq!(
        lease_count, expected_leases,
        "{point}: unexpected machine leases"
    );
    let threads = data(f.op(1, "threads.list", json!({"space_id":space})).await);
    assert_eq!(
        threads["threads"].as_array().unwrap().len(),
        1,
        "{point}: extra thread"
    );
    let thread = data(f.op(1, "threads.get", json!({"id":saved.id})).await);
    let messages = thread["thread"]["messages"].as_array().unwrap();
    assert_eq!(
        messages
            .iter()
            .filter(|message| message["role"] == "automation")
            .count(),
        1,
        "{point}: duplicate prompt"
    );
}

#[tokio::test]
async fn scheduled_occurrence_is_created_once_at_each_handoff_crash() {
    for &(point, committed, job_committed, claimed) in CUTS {
        verify_cut(point, committed, job_committed, claimed).await;
    }
}
