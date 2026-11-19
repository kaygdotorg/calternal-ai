//! Migration and replay guarantees through the durable job store.
use super::*;

fn admit(jobs: &Jobs, space: &str, key: &str) -> String {
    jobs.admit(Admission {
        space: space.into(),
        plugin: "package".into(),
        account: AccountId::new(),
        session: Session::Static,
        operation: "package.run".into(),
        input: Value::Null,
        parent: None,
        generation: 1,
        request_key: key.into(),
    })
    .unwrap()
}

fn park(jobs: &Jobs, space: &str, id: &str) -> Result<Timer, Error> {
    let claim = jobs.claim(space, id)?.unwrap();
    jobs.park_signal(
        &claim.job,
        &claim.live,
        Park {
            checkpoint: json!({"phase": 1}),
            reason: "Test wait".into(),
            deadline_ms: now_ms() + 60_000,
            source: WaitSource::Approval,
        },
        None,
    )
    .map(Option::unwrap)
}

#[test]
fn jobs_v2_migrates_with_existing_rows_and_a_failed_batch_rolls_back() {
    let temp = tempfile::tempdir().unwrap();
    let space = calternal_types::SpaceId::new().to_string();
    let folder = temp.path().join("spaces").join(&space);
    std::fs::create_dir_all(&folder).unwrap();
    let conn = Connection::open(folder.join("jobs.db")).unwrap();
    // The baseline jobs schema stays the same, including its positional insert.
    let batches = [
        crate::plugins::job_sql::SCHEMA,
        crate::threads::turn_events::schema(),
    ];
    let baseline = calternal_store::migrations::Schema::new("jobs.db", 2, &batches, &[]);
    calternal_store::migrations::migrate(&conn, baseline).unwrap();
    let row = Uuid::now_v7().to_string();
    conn.execute(
        "INSERT INTO jobs VALUES(?1,'package',?2,'{\"kind\":\"static\"}','package.run','null',NULL,'admitted','null',0,0,'null',0,1,'old',NULL)",
        params![row, AccountId::new().to_string()],
    ).unwrap();
    let bad_batch = format!("{SCHEMA} INSERT INTO absent_table VALUES (1);");
    let changes = [(3, bad_batch.as_str())];
    let invalid = calternal_store::migrations::Schema::new("jobs.db", 2, &batches, &changes);
    assert!(calternal_store::migrations::migrate(&conn, invalid).is_err());
    assert_eq!(
        calternal_store::migrations::inspect(&conn, baseline).unwrap(),
        2
    );
    drop(conn);
    let jobs = Jobs::open(temp.path()).unwrap();
    let timer = park(&jobs, &space, &row).unwrap();
    assert!(jobs.wait_is_pending(&timer).unwrap());
}

#[test]
fn first_signal_survives_claim_and_restart_with_one_receipt() {
    let temp = tempfile::tempdir().unwrap();
    let space = calternal_types::SpaceId::new().to_string();
    let jobs = Jobs::open(temp.path()).unwrap();
    let id = admit(&jobs, &space, "park");
    let timer = park(&jobs, &space, &id).unwrap();
    let signal = || Decision {
        outcome: json!({"kind":"approval","approved":true}),
        receipt: Some(("decision", "approved")),
    };
    assert!(matches!(
        jobs.resolve_wait(&space, &timer.id, &id, Some(signal()))
            .unwrap(),
        Resolution::Woken
    ));
    assert!(matches!(
        jobs.resolve_wait(&space, &timer.id, &id, Some(signal()))
            .unwrap(),
        Resolution::Replay
    ));
    let claim = jobs.claim(&space, &id).unwrap().unwrap();
    assert!(jobs.claim(&space, &id).unwrap().is_none());
    assert_eq!(
        jobs.continuation(&claim.job).unwrap(),
        Some(signal().outcome)
    );
    drop(jobs);
    let jobs = Jobs::open(temp.path()).unwrap();
    assert_eq!(
        jobs.recover(|_, _, _| true).unwrap(),
        vec![(space.clone(), id.clone())]
    );
    let claim = jobs.claim(&space, &id).unwrap().unwrap();
    assert_eq!(
        jobs.continuation(&claim.job).unwrap(),
        Some(signal().outcome)
    );
}

#[test]
fn package_capacity_excludes_cancelled_waiters() {
    let temp = tempfile::tempdir().unwrap();
    let space = calternal_types::SpaceId::new().to_string();
    let jobs = Jobs::open(temp.path()).unwrap();
    let timers: Vec<_> = (0..PACKAGE_LIMIT)
        .map(|n| {
            let id = admit(&jobs, &space, &n.to_string());
            park(&jobs, &space, &id).unwrap()
        })
        .collect();
    let blocked = admit(&jobs, &space, "blocked");
    assert_eq!(
        park(&jobs, &space, &blocked).err().unwrap().code,
        calternal_types::ErrorCode::RateLimited
    );
    let first = &timers[0];
    jobs.cancel(&space, &first.job).unwrap();
    assert!(!jobs.wait_is_pending(first).unwrap());
    let replacement = admit(&jobs, &space, "replacement");
    assert!(park(&jobs, &space, &replacement).is_ok());
}
