//! Locked-perf probe of declared parent edges and personal layout transactions.
#[path = "v2_step0_nested/bounded.rs"]
mod bounded;
#[path = "../../../spikes/plugin-api-v2/fixture.rs"]
mod fixture;
#[path = "v2_step0_nested/layout.rs"]
mod layout;
#[path = "v2_step0_nested/parents.rs"]
mod parents;

use fixture::{identity, run, summary};
use layout::{Cut, Edit, LayoutWindows, Scope};
use rusqlite::params;
use serde_json::json;
use std::time::{Duration, Instant};
use uuid::Uuid;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "disposable step 0 perf fixture"]
async fn parent_windows_and_personal_sections() {
    let dir = tempfile::tempdir().unwrap();
    let roots = fixture::roots(dir.path());
    let (state, ids) = tokio::task::block_in_place(|| fixture::open(&roots, &fixture::packages()));
    let runs = std::env::var("STEP0_RUNS").map_or(100, |n| n.parse().unwrap());
    parents::probe(&state, &ids, &roots, runs).await;

    let path = roots.appdata.join("system/auth.db");
    let mut conn = layout::open(&path);
    let owner_id = ids.owner.to_string();
    let member_id = ids.guest.to_string();
    let shared = ids.shared.to_string();
    let owner = Scope {
        space: &shared,
        actor: &owner_id,
        app: "notes",
    };
    let member = Scope {
        actor: &member_id,
        ..owner
    };
    let size = std::env::var("STEP0_LAYOUT_SIZE").map_or(256, |n| n.parse().unwrap());
    let foreign_item = run(
        &state,
        identity(ids.owner),
        "notes.create",
        json!({
            "space_id":ids.personal,"title":"Foreign to layout","markdown":"hidden"
        }),
    )
    .await
    .unwrap();
    let foreign_item = foreign_item["note"]["id"].as_str().unwrap();
    let mut ids_fixture = Vec::new();
    for i in 0..size {
        let item = run(
            &state,
            identity(ids.owner),
            "notes.create",
            json!({
                "space_id":ids.shared,"title":format!("layout note {i}"),"markdown":"layout"
            }),
        )
        .await
        .unwrap();
        ids_fixture.push(item["note"]["id"].as_str().unwrap().to_owned());
    }
    for id in &ids_fixture {
        conn.execute(
            "INSERT INTO spike_layout_item VALUES(?1,?2)",
            params![shared, id],
        )
        .unwrap();
    }
    let first = ids_fixture[0].as_str();
    let second = ids_fixture[1].as_str();
    let apply = |scope: Scope<'_>, conn: &mut rusqlite::Connection, command| {
        let pin = scope.pin(conn);
        scope.edit(conn, pin, command, Cut::None).unwrap()
    };
    apply(owner, &mut conn, Edit::Create("a", "One"));
    apply(owner, &mut conn, Edit::Create("b", "Two"));
    apply(member, &mut conn, Edit::Create("a", "Reader only"));
    for id in &ids_fixture {
        apply(owner, &mut conn, Edit::To(id, "a"));
    }
    apply(member, &mut conn, Edit::To(second, "a"));
    let member_before = member.snapshot(&mut conn).unwrap();
    let initial = owner.snapshot(&mut conn).unwrap();
    let mut windows = LayoutWindows::default();
    let view = Uuid::new_v4();
    let sections = windows.sections(&mut conn, owner, view, 0, 10).unwrap();
    let member_sections = windows.sections(&mut conn, member, view, 0, 10).unwrap();
    assert_eq!(
        sections.iter().map(|s| &s.0).collect::<Vec<_>>(),
        initial.iter().map(|s| &s.0).collect::<Vec<_>>()
    );
    assert_eq!(member_sections[0].0, member_before[0].0);
    assert_eq!(
        &windows
            .placements(&mut conn, owner, view, sections[0].1, 1..4)
            .unwrap(),
        &initial[0].1[1..4]
    );
    assert_eq!(
        windows.placements(&mut conn, owner, view, sections[1].1, 0..3),
        Some(vec![])
    );
    assert!(
        windows
            .placements(&mut conn, member, view, sections[0].1, 0..3)
            .is_none()
    );
    assert!(
        windows
            .placements(&mut conn, owner, view, Uuid::new_v4(), 0..3)
            .is_none()
    );
    let pin = owner.pin(&conn);
    assert!(
        owner
            .edit(&mut conn, pin, Edit::To(foreign_item, "a"), Cut::None)
            .is_none()
    );
    let guest = Scope {
        space: &ids.personal.to_string(),
        ..member
    };
    assert!(guest.snapshot(&mut conn).is_none());

    eprintln!("layout fixture placements={size}");
    for (name, command) in [
        ("host.layout.section.create", Edit::Create("c", "Created")),
        ("host.layout.section.rename", Edit::Rename("a", "Renamed")),
        ("host.layout.section.remove", Edit::Remove("a")),
        ("host.layout.move_to", Edit::To(first, "b")),
        ("host.layout.move_up", Edit::Up(second)),
        ("host.layout.move_down", Edit::Down(first)),
    ] {
        let receipts = receipt_count(&conn);
        for (cut, code) in [
            (Cut::BeforeCommit, 71),
            (Cut::AfterCommit, 72),
            (Cut::DuringReply, 73),
        ] {
            let pin = owner.pin(&conn);
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--ignored", "--exact", "crash_layout_child", "--nocapture"])
                .env("STEP0_LAYOUT_DB", &path)
                .env(
                    "STEP0_LAYOUT_CRASH",
                    serde_json::to_string(&(owner, pin, command, cut)).unwrap(),
                )
                .output()
                .unwrap();
            assert_eq!(
                output.status.code(),
                Some(code),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            if code == 71 {
                assert_eq!(receipt_count(&conn), receipts);
                assert_eq!(owner.pin(&conn), pin);
                assert_eq!(owner.snapshot(&mut conn).unwrap(), initial);
            } else {
                assert_eq!(receipt_count(&conn), receipts + 1);
                assert_eq!(owner.pin(&conn), pin + 1);
                assert_ne!(owner.snapshot(&mut conn).unwrap(), initial);
                let token: String = conn.query_row(
                    "SELECT token FROM spike_undo WHERE space=?1 AND actor=?2 AND app=?3 AND rev=?4",
                    params![owner.space, owner.actor, owner.app, pin + 1], |r| r.get(0)
                ).unwrap();
                assert!(owner.undo(&mut conn, Uuid::parse_str(&token).unwrap()));
                assert_eq!(owner.snapshot(&mut conn).unwrap(), initial);
                if code == 73 {
                    assert!(String::from_utf8_lossy(&output.stdout).ends_with(r#"{"undo":""#));
                }
            }
        }
        let mut writes = Vec::new();
        let mut undos = Vec::new();
        for i in 0..runs {
            let pin = owner.pin(&conn);
            let receipts = receipt_count(&conn);
            assert!(owner.edit(&mut conn, pin, command, Cut::Rollback).is_none());
            assert_eq!(owner.pin(&conn), pin);
            assert_eq!(receipt_count(&conn), receipts);
            assert_eq!(owner.snapshot(&mut conn).unwrap(), initial);

            let start = Instant::now();
            let token = owner.edit(&mut conn, pin, command, Cut::None).unwrap();
            writes.push(start.elapsed());
            if i == 0 {
                // Reopen after durable commit and discard any publication state.
                conn = rusqlite::Connection::open(&path).unwrap();
                conn.pragma_update(None, "synchronous", "FULL").unwrap();
            }
            assert_eq!(receipt_count(&conn), receipts + 1);
            let after = owner.snapshot(&mut conn).unwrap();
            assert_ne!(after, initial);
            match command {
                Edit::Create(..) => assert_eq!(after[2].0.1, "Created"),
                Edit::Rename(..) => assert_eq!(after[0].0.1, "Renamed"),
                Edit::Remove(..) => assert_eq!(after, initial[1..]),
                Edit::To(..) => assert_eq!(after[1].1[0].0, first),
                Edit::Up(..) | Edit::Down(..) => assert_eq!(after[0].1[0].0, second),
            }
            assert!(!member.undo(&mut conn, token));
            assert_eq!(member.snapshot(&mut conn).unwrap(), member_before);
            let start = Instant::now();
            assert!(owner.undo(&mut conn, token));
            undos.push(start.elapsed());
            assert_eq!(owner.snapshot(&mut conn).unwrap(), initial);
            assert_eq!(receipt_count(&conn), receipts);
        }
        summary(name, &mut writes);
        summary(&format!("{name} undo"), &mut undos);
        if writes[(writes.len() * 95).div_ceil(100) - 1] > Duration::from_millis(40)
            || undos[(undos.len() * 95).div_ceil(100) - 1] > Duration::from_millis(40)
        {
            eprintln!("gate_miss {name}: p95 exceeds 40 ms");
        }
    }

    for _ in 0..runs {
        let prior = apply(owner, &mut conn, Edit::Rename("a", "prior"));
        let _later = apply(owner, &mut conn, Edit::Rename("a", "later"));
        let before = owner.snapshot(&mut conn).unwrap();
        assert!(!owner.undo(&mut conn, prior));
        assert_eq!(owner.snapshot(&mut conn).unwrap(), before);

        let auth = fixture::spaces(&roots);
        let change = apply(member, &mut conn, Edit::Rename("a", "Live member"));
        let pin = member.pin(&conn);
        let member_ref = windows.sections(&mut conn, member, view, 0, 10).unwrap()[0].1;
        assert!(
            windows
                .placements(&mut conn, member, view, member_ref, 0..3)
                .is_some()
        );
        tokio::task::block_in_place(|| auth.remove(ids.owner, ids.shared, ids.guest)).unwrap();
        assert!(member.snapshot(&mut conn).is_none());
        assert!(windows.sections(&mut conn, member, view, 0, 10).is_none());
        assert!(
            windows
                .placements(&mut conn, member, view, member_ref, 0..3)
                .is_none()
        );
        assert!(
            member
                .edit(
                    &mut conn,
                    pin,
                    Edit::Rename("a", "must not write"),
                    Cut::None
                )
                .is_none()
        );
        assert!(!member.undo(&mut conn, change));
        assert_eq!(member.pin(&conn), pin);
        conn.execute(
            "INSERT INTO space_memberships VALUES(?1,?2,'reader')",
            params![shared, member_id],
        )
        .unwrap();
        assert!(member.undo(&mut conn, change));
        assert_eq!(member.snapshot(&mut conn).unwrap(), member_before);
    }
}

fn receipt_count(conn: &rusqlite::Connection) -> i64 {
    conn.query_row("SELECT count(*) FROM spike_undo", [], |r| r.get(0))
        .unwrap()
}

#[test]
#[ignore = "process exit helper for the injected layout crashes"]
fn crash_layout_child() {
    let Ok(path) = std::env::var("STEP0_LAYOUT_DB") else {
        return;
    };
    let raw = std::env::var("STEP0_LAYOUT_CRASH").unwrap();
    let (scope, pin, edit, cut): (Scope<'_>, i64, Edit<'_>, Cut) =
        serde_json::from_str(&raw).unwrap();
    let mut conn = rusqlite::Connection::open(path).unwrap();
    conn.pragma_update(None, "synchronous", "FULL").unwrap();
    scope
        .edit(&mut conn, pin, edit, cut)
        .expect("child had no authority");
    panic!("injected process exit did not happen");
}
