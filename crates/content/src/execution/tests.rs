use crate::db::SPACE_SCHEMA;
use rusqlite::{Connection, params};
use tempfile::tempdir;

use super::*;

/// Open a fresh space store: the execution tables are in the baseline.
pub(super) fn upgrade(db: &Connection) {
    db.pragma_update(None, "foreign_keys", "ON").unwrap();
    calternal_store::migrations::migrate(db, SPACE_SCHEMA).unwrap();
}

pub(super) fn seed_item(db: &Connection, item: &ItemRef) {
    db.execute(
        "INSERT INTO items(id,kind,metadata,revision,seq,updated_at,path,folded_path,mat_state)
         VALUES(?1,'example','{}','initial',1,'today','demo.md','demo.md','ok')",
        [&item.0],
    )
    .unwrap();
}

pub(super) fn pin(item: ItemRef, actor: AccountId) -> PinnedExecution {
    PinnedExecution {
        item: item.clone(),
        kind: "example".into(),
        actor,
        installation: InstallationId::new(),
        installation_generation: 1,
        installation_expiry_ms: i64::MAX,
        operation: "example.run".into(),
        declaration: "example-job".into(),
        package: PackageId("example".into()),
        package_digest: "b".repeat(64),
        harness: HarnessRef("checked-harness".into()),
        contribution: ContributionIdentity {
            package: PackageId("example-harness".into()),
            contribution: calternal_types::ContributionId("run".into()),
        },
        contribution_digest: "c".repeat(64),
        entry_digest: "d".repeat(64),
        image_digest: "e".repeat(64),
        placement: Some(PlacementRef("checked-placement".into())),
        machine: Uuid::now_v7(),
        profile: "checked-profile".into(),
        profile_generation: 1,
        audience_generation: 1,
        effect_ceiling: json!({}),
        input: json!({"item": item}),
        contribution_input: json!({"text": "a"}),
    }
}

#[test]
fn baseline_stores_one_head_and_one_successor() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("content.db");
    let mut db = Connection::open(&path).unwrap();
    upgrade(&db);
    let item = ItemRef(Uuid::now_v7().to_string());
    seed_item(&db, &item);
    let tx = db.transaction().unwrap();
    let insert = |intent: &str, slot: i64| {
        tx.execute(
            "INSERT INTO execution_intents(intent,job,item,slot,actor,installation,pin)
             VALUES(?1,?1,?2,?3,'test-actor','test-installation','{}')",
            params![intent, item.0, slot],
        )
    };
    insert("head", 0).unwrap();
    insert("successor", 1).unwrap();
    assert!(insert("competing-head", 0).is_err());
    assert!(insert("competing-successor", 1).is_err());
    assert!(insert("additional-slot", 2).is_err());
    tx.commit().unwrap();
    drop(db);
    let db = Connection::open(path).unwrap();
    let ids = db
        .prepare("SELECT intent FROM execution_intents ORDER BY slot")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(ids, ["head", "successor"]);
}

#[test]
fn stop_fences_only_its_head_and_teardown_gates_successor() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("content.db");
    let mut db = Connection::open(&path).unwrap();
    upgrade(&db);
    let item = ItemRef(Uuid::now_v7().to_string());
    seed_item(&db, &item);
    let mut pin = pin(item.clone(), AccountId::new());
    let tx = db.transaction().unwrap();
    let head = admit(&tx, &pin, 64).unwrap();
    let claimed = claim(&tx, &head.job, |_| Ok(true)).unwrap().unwrap();
    assert_eq!(claimed.pin, pin);
    assert_eq!(claimed.intent, head.intent);
    let started = start(&tx, &head.job, &pin.harness, |_| Ok(true)).unwrap();
    assert_eq!(
        start(&tx, &head.job, &pin.harness, |_| Ok(true)).unwrap(),
        started
    );
    let attempt = started.attempt;
    assert!(stop_pending(&tx, &head.intent, |_, _| Ok(())).is_err());

    let first_actor = pin.actor;
    let first_installation = pin.installation;
    pin.actor = AccountId::new();
    pin.installation = InstallationId::new();
    let successor = admit(&tx, &pin, 64).unwrap();
    assert_eq!(
        admit(&tx, &pin, 64).unwrap_err().details,
        Some(json!({"reason": "pending_limit"}))
    );
    assert!(claim(&tx, &successor.job, |_| Ok(true)).unwrap().is_none());
    assert!(
        stop(&tx, &attempt, "stop", |_, _| Err(Error::forbidden(
            "foreign"
        )))
        .is_err()
    );
    assert!(is_current(&tx, &attempt, |_| Ok(true)).unwrap());
    assert!(
        !stop(&tx, &attempt, "stop", |target, starter| {
            assert_eq!(target, &item);
            assert_eq!(starter, first_actor);
            Ok(())
        })
        .unwrap()
        .is_empty()
    );
    assert_eq!(
        fence_installation(&tx, first_actor, first_installation).unwrap(),
        0
    );
    assert!(!is_current(&tx, &attempt, |_| Ok(true)).unwrap());
    assert!(promote(&tx, &item, |_| Ok(true)).unwrap().is_none());
    tx.commit().unwrap();
    drop(db);

    let mut db = Connection::open(path).unwrap();
    db.pragma_update(None, "foreign_keys", "ON").unwrap();
    let tx = db.transaction().unwrap();
    assert_eq!(cleanup(&tx, 64).unwrap(), [attempt.clone()]);
    let mut stale = attempt.clone();
    stale.generation += 1;
    assert!(!settle(&tx, &stale).unwrap());
    assert!(promote(&tx, &item, |_| Ok(true)).unwrap().is_none());
    assert!(settle(&tx, &attempt).unwrap());
    assert!(!settle(&tx, &attempt).unwrap());
    assert_eq!(
        promote(&tx, &item, |_| Ok(true)).unwrap(),
        Some(successor.job.clone())
    );
    assert!(claim(&tx, &successor.job, |_| Ok(false)).unwrap().is_none());
    assert!(claim(&tx, &successor.job, |_| Ok(true)).unwrap().is_none());
}

#[test]
fn lost_admission_and_stop_acks_replay_the_content_writer_receipts() {
    let dir = tempdir().unwrap();
    let roots = calternal_roots::Roots {
        appdata: dir.path().join("appdata"),
        userdata_ssd: dir.path().join("ssd"),
        userdata_hdd: dir.path().join("hdd"),
    };
    let space = calternal_types::SpaceId::new();
    let actor = AccountId::new();
    let request = crate::Request {
        account: actor,
        via: None,
        id: "checked-start",
        operation: "start_job",
        fingerprint: "checked-input",
    };
    let open = |fault| {
        crate::Content::open(
            roots.clone(),
            dir.path().join("secrets"),
            std::time::Duration::from_secs(60),
            crate::DEFAULT_RECEIPT_RETENTION,
            fault,
        )
        .unwrap()
    };
    let mut lost_reply = None;
    let content = open(|_| {});
    content
        .register_kind(
            space,
            "example",
            crate::Placement::Visible {
                folder: "Examples".into(),
            },
            crate::FileFormat::Markdown,
        )
        .unwrap();
    let item = content
        .write(
            space,
            None,
            "example",
            None,
            json!({"title": "Example"}),
            b"body",
            crate::Request {
                id: "create",
                operation: "item_create",
                fingerprint: "create",
                ..request
            },
            |item| json!({"item": item.id}),
        )
        .unwrap();
    drop(content);
    let pin = pin(ItemRef(item.id.to_string()), actor);
    let content = open(|point| {
        if point == crate::FaultPoint::AfterCommit {
            panic!("lost acknowledgement");
        }
    });
    let cut = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        content.execution_command(
            space,
            &|_| {},
            request,
            || Ok(()),
            |tx| {
                let admitted = admit(tx, &pin, 64)?;
                let reply = json!({"job_id": admitted.job});
                lost_reply = Some(reply.clone());
                Ok(reply)
            },
        )
    }));
    assert!(cut.is_err());
    drop(content);

    let content = open(|_| {});
    let replay = content.execution_command(
        space,
        &|_| {},
        request,
        || Ok(()),
        |_| panic!("the committed effect cannot repeat"),
    );
    let replay = replay.unwrap();
    assert_eq!(Some(&replay), lost_reply.as_ref());
    content
        .with_space(space, |s| {
            let count = s
                .conn
                .query_row("SELECT count(*) FROM execution_intents", [], |r| {
                    r.get::<_, i64>(0)
                })
                .map_err(internal)?;
            assert_eq!(count, 1);
            Ok(())
        })
        .unwrap();
    let reused = crate::Request {
        fingerprint: "changed-input",
        ..request
    };
    let err = content
        .execution_command(
            space,
            &|_| {},
            reused,
            || Ok(()),
            |_| panic!("conflicting replay"),
        )
        .unwrap_err();
    assert_eq!(err, Error::request_reused());
    assert!(
        content
            .execution_command(
                space,
                &|_| {},
                request,
                || Err(Error::forbidden("revoked")),
                |_| { panic!("revoked replay") }
            )
            .is_err()
    );

    let job: JobId = serde_json::from_value(replay["job_id"].clone()).unwrap();
    let attempt = content
        .with_space(space, |s| {
            let tx = s.conn.transaction().map_err(internal)?;
            assert_eq!(claim(&tx, &job, |_| Ok(true))?.unwrap().pin, pin);
            let attempt = start(&tx, &job, &pin.harness, |_| Ok(true))?.attempt;
            tx.commit().map_err(internal)?;
            Ok(attempt)
        })
        .unwrap();
    drop(content);
    let stop_request = crate::Request {
        operation: "host.execution.stop",
        id: "confirmed-stop",
        ..request
    };
    let content = open(|point| {
        if point == crate::FaultPoint::AfterCommit {
            panic!("lost Stop acknowledgement");
        }
    });
    let stopped = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        content.execution_command(space, &|_| {}, stop_request, || Ok(()), |tx| {
            let transition = stop(tx, &attempt, "confirmed-stop", |item, starter| {
                assert_eq!(item, &pin.item);
                assert_eq!(starter, actor);
                Ok(())
            })?;
            let reply = json!({"execution": attempt.execution, "phase": "stopped", "transition_cursor": transition});
            lost_reply = Some(reply.clone());
            Ok(reply)
        })
    }));
    assert!(stopped.is_err());
    drop(content);
    let content = open(|_| {});
    let replay = content.execution_command(
        space,
        &|_| {},
        stop_request,
        || Ok(()),
        |_| panic!("the committed Stop cannot repeat"),
    );
    assert_eq!(Some(replay.unwrap()), lost_reply);
    content
        .with_space(space, |s| {
            let tx = s.conn.transaction().map_err(internal)?;
            assert!(!is_current(&tx, &attempt, |_| panic!(
                "retired attempt reached dispatch"
            ))?);
            assert_eq!(cleanup(&tx, 64)?, [attempt]);
            Ok(())
        })
        .unwrap();
}

#[test]
fn archive_matches_and_checks_both_starters_before_retiring_work() {
    let mut db = Connection::open_in_memory().unwrap();
    upgrade(&db);
    let item = ItemRef(Uuid::now_v7().to_string());
    seed_item(&db, &item);
    let first = pin(item.clone(), AccountId::new());
    let second = pin(item.clone(), AccountId::new());
    let tx = db.transaction().unwrap();
    let head = admit(&tx, &first, 64).unwrap();
    claim(&tx, &head.job, |_| Ok(true)).unwrap().unwrap();
    let attempt = start(&tx, &head.job, &first.harness, |_| Ok(true))
        .unwrap()
        .attempt;
    let old_selection = archive_targets(&tx, &item).unwrap();
    let successor = admit(&tx, &second, 64).unwrap();
    assert!(fence_archive(&tx, &item, &old_selection, |_| panic!("stale archive")).is_err());
    let pinned = archive_targets(&tx, &item).unwrap();
    assert_eq!(pinned[0].starter, first.actor);
    assert_eq!(pinned[1].starter, second.actor);
    assert_eq!(pinned[1].intent, successor.intent);
    assert!(
        fence_archive(&tx, &item, &pinned, |_| Err(Error::forbidden(
            "other starter"
        )))
        .is_err()
    );
    assert!(is_current(&tx, &attempt, |_| Ok(true)).unwrap());

    fence_archive(&tx, &item, &pinned, |_| Ok(())).unwrap();
    assert!(!is_current(&tx, &attempt, |_| Ok(true)).unwrap());
    assert!(archive_targets(&tx, &item).unwrap().is_empty());
    assert_eq!(cleanup(&tx, 64).unwrap(), [attempt]);
    assert!(claim(&tx, &successor.job, |_| Ok(true)).unwrap().is_none());
}

#[test]
fn installation_retirement_leaves_other_actors_and_devices_live() {
    let mut db = Connection::open_in_memory().unwrap();
    upgrade(&db);
    let item = ItemRef(Uuid::now_v7().to_string());
    seed_item(&db, &item);
    let first = pin(item.clone(), AccountId::new());
    let second = pin(item.clone(), AccountId::new());
    let tx = db.transaction().unwrap();
    let head = admit(&tx, &first, 64).unwrap();
    let successor = admit(&tx, &second, 64).unwrap();
    assert_eq!(
        fence_installation(&tx, second.actor, InstallationId::new()).unwrap(),
        0
    );
    assert_eq!(
        fence_installation(&tx, second.actor, second.installation).unwrap(),
        1
    );
    assert_eq!(
        fence_installation(&tx, second.actor, second.installation).unwrap(),
        0
    );
    assert!(
        claim(&tx, &successor.job, |_| panic!(
            "retired successor dispatched"
        ))
        .unwrap()
        .is_none()
    );
    assert_eq!(
        claim(&tx, &head.job, |_| Ok(true)).unwrap().unwrap().pin,
        first
    );
    assert!(cleanup(&tx, 64).unwrap().is_empty());
}
