use super::*;
use crate::{FaultPoint, FileFormat, Placement, Request, db::SPACE_SCHEMA};
use calternal_roots::Roots;
use calternal_types::{AccountId, ErrorCode};
use rusqlite::params;
use std::{cell::Cell, time::Duration};

fn memory() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", true).unwrap();
    calternal_store::migrations::migrate(&conn, SPACE_SCHEMA).unwrap();
    conn
}

fn content(dir: &tempfile::TempDir) -> Content {
    let path = dir.path();
    Content::open(
        Roots {
            appdata: path.join("appdata"),
            userdata_ssd: path.join("ssd"),
            userdata_hdd: path.join("hdd"),
        },
        path.join("secrets"),
        Duration::ZERO,
        crate::DEFAULT_RECEIPT_RETENTION,
        |_: FaultPoint| {},
    )
    .unwrap()
}

fn request(account: AccountId, id: &str) -> Request<'_> {
    Request {
        account,
        via: None,
        id,
        operation: "window.test",
        fingerprint: id,
    }
}

fn insert(conn: &Connection, id: Uuid, kind: &str, metadata: Value, second: u64, live: bool) {
    let path = if live {
        id.to_string()
    } else {
        format!(".trash/{id}")
    };
    let date = format!("2026-11-19T00:00:{second:02}.000Z");
    conn.execute(
        "INSERT INTO items VALUES (?1, ?2, ?3, 'revision', 1, ?4, ?5, ?5, NULL, 'materialized', NULL)",
        params![id.to_string(), kind, metadata.to_string(), date, path],
    ).unwrap();
}

fn range(from: u64, to: u64, anchor: WindowAnchor) -> WindowRange {
    WindowRange {
        bind: "notes".into(),
        from,
        to,
        parent: None,
        anchor: Some(anchor),
    }
}

fn result(reply: WindowReply) -> WindowResult {
    match reply {
        WindowReply::Result(result) => result,
        WindowReply::PlanError(error) => panic!("unexpected refusal: {error:?}"),
    }
}

fn refusal(reply: WindowReply, expected: WindowPlanReason) {
    match reply {
        WindowReply::PlanError(error) => assert_eq!(error.reason, expected),
        WindowReply::Result(_) => panic!("expected {expected:?}"),
    }
}

fn next(seed: &mut u64) -> u64 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    *seed
}

#[test]
fn randomized_windows_match_independent_scan_of_mixed_visibility() {
    let mut seed = 28199;
    let conn = memory();
    let mut members = Vec::new();
    let mut naive = Vec::new();
    for n in 1..=900 {
        let flags = next(&mut seed);
        let id = Uuid::from_u128(n);
        let second = flags % 60;
        let title = format!("TITLE {:02}", flags % 42);
        let live = flags % 5 != 0;
        let included = flags % 3 == 0;
        insert(
            &conn,
            id,
            "notes",
            json!({"title":title,"private":flags}),
            second,
            live,
        );
        if included {
            members.push(id);
        }
        if live {
            naive.push((id, second, title, included));
        }
    }
    let coll = Uuid::new_v4();
    insert(
        &conn,
        coll,
        "collections",
        json!({"item_kind":"notes","members":members}),
        0,
        true,
    );
    for (scope, audience, mode) in [
        (
            WindowScope::SpaceNotes,
            WindowAudience::Member,
            WindowCountMode::Exact,
        ),
        (
            WindowScope::NotesCollection(coll),
            WindowAudience::Guest,
            WindowCountMode::Exact,
        ),
        (
            WindowScope::NotesCollection(coll),
            WindowAudience::Public,
            WindowCountMode::None,
        ),
    ] {
        let mut expected = naive.clone();
        if scope == WindowScope::SpaceNotes {
            expected.sort_by_key(|(id, second, _, _)| (std::cmp::Reverse(*second), *id));
        } else {
            expected.retain(|row| row.3);
            expected.sort_by_key(|(id, _, title, _)| (title.to_lowercase(), *id));
        }
        let len = expected.len() as u64;
        let access = WindowAccess {
            scope,
            audience,
            count: mode,
            view: Uuid::new_v4(),
        };
        let key = scope.key();
        let snapshot = read::scope(&conn, &key).unwrap();
        assert_eq!(snapshot.total, len);
        for _ in 0..100 {
            let from = next(&mut seed) % (len + 35);
            let to = from + 1 + next(&mut seed) % 200;
            for anchor in [WindowAnchor::Start, WindowAnchor::End] {
                let request = range(from, to, anchor);
                if let Some(reason) = bad_plan(&request, &access, None) {
                    assert_eq!(reason, WindowPlanReason::EndUnavailable);
                    continue;
                }
                let out = result(
                    evaluate(Some(&conn), &key, &snapshot, &request, &access, None).unwrap(),
                );
                let (first, last) = if anchor == WindowAnchor::End {
                    (len.saturating_sub(to), len.saturating_sub(from))
                } else {
                    (len.min(from), len.min(to))
                };
                assert_eq!(out.offset, from);
                assert_eq!(out.anchor, anchor);
                let want = &expected[first as usize..last as usize];
                assert_eq!(out.rows.len(), want.len());
                for (row, (id, _, title, _)) in out.rows.iter().zip(want) {
                    assert_eq!(out.item_for_row(&row.id), Some(*id));
                    assert_eq!(row.fields, json!({"title":title}));
                    assert_ne!(row.id, id.to_string());
                }
                if access.count == WindowCountMode::Exact {
                    assert_eq!(out.count.unwrap().value, len);
                } else {
                    assert!(out.count.is_none());
                }
            }
        }
    }
}

#[test]
fn lost_authority_or_membership_discards_rows_and_count() {
    let dir = tempfile::tempdir().unwrap();
    let content = content(&dir);
    let space = SpaceId::new();
    let actor = AccountId::new();
    content
        .register_kind(
            space,
            "notes",
            Placement::Visible {
                folder: "Notes".into(),
            },
            FileFormat::Markdown,
        )
        .unwrap();
    let item = content
        .write(
            space,
            None,
            "notes",
            None,
            json!({"title":"Visible"}),
            b"body",
            request(actor, "note"),
            |_| json!({}),
        )
        .unwrap();
    let created = content
        .create_notes_collection(
            space,
            "Shared".into(),
            vec![item.id],
            request(actor, "collection"),
        )
        .unwrap();
    let coll: Uuid = serde_json::from_value(created["id"].clone()).unwrap();
    let saved = content.live_collection(space, coll).unwrap();
    let access = WindowAccess {
        scope: WindowScope::NotesCollection(coll),
        audience: WindowAudience::Guest,
        count: WindowCountMode::Exact,
        view: Uuid::new_v4(),
    };
    let window = WindowRequest::Window {
        request: range(0, 10, WindowAnchor::End),
    };
    let calls = Cell::new(0);
    let denied = content
        .window(space, &window, None, || {
            calls.set(calls.get() + 1);
            if calls.get() == 1 {
                Ok(access.clone())
            } else {
                Err(missing())
            }
        })
        .unwrap_err();
    assert_eq!(denied.code, ErrorCode::NotFound);
    assert_eq!(calls.get(), 2);
    calls.set(0);
    let stale = content
        .window(space, &window, None, || {
            calls.set(calls.get() + 1);
            if calls.get() == 2 {
                content.set_collection_members(
                    space,
                    coll,
                    &saved.revision,
                    Vec::new(),
                    request(actor, "remove"),
                )?;
            }
            Ok(access.clone())
        })
        .unwrap_err();
    assert_eq!(stale.code, ErrorCode::NotFound);
    let empty = result(
        content
            .window(space, &window, None, || Ok(access.clone()))
            .unwrap(),
    );
    assert_eq!(empty.state, "empty");
    assert_eq!(empty.count.unwrap().value, 0);
}

#[test]
fn scope_33_rolls_back_entire_membership_commit() {
    let dir = tempfile::tempdir().unwrap();
    let content = content(&dir);
    let space = SpaceId::new();
    let actor = AccountId::new();
    content
        .register_kind(
            space,
            "notes",
            Placement::Visible {
                folder: "Notes".into(),
            },
            FileFormat::Markdown,
        )
        .unwrap();
    let item = content
        .write(
            space,
            None,
            "notes",
            None,
            json!({"title":"Capped"}),
            b"body",
            request(actor, "note"),
            |_| json!({}),
        )
        .unwrap();
    let catalogue: Value = serde_json::from_str(CATALOGUE).unwrap();
    let cap = catalogue["query"]["live_limits"]["effective_scopes_per_item"]
        .as_u64()
        .unwrap();
    for n in 1..cap {
        let id = format!("collection-{n}");
        content
            .create_notes_collection(space, id.clone(), vec![item.id], request(actor, &id))
            .unwrap();
    }
    let refused = content
        .create_notes_collection(
            space,
            "too many".into(),
            vec![item.id],
            request(actor, "refused"),
        )
        .unwrap_err();
    assert_eq!(refused.code, ErrorCode::EffectiveScopeLimitExceeded);
    assert_eq!(refused.code.http_status(), 422);
    assert_eq!(
        json!(refused.code),
        catalogue["query"]["scope_limit_refusal"]["code"]
    );
    assert!(content.receipt(space, actor, "refused").unwrap().is_none());
    assert_eq!(
        content.sharing_collections(space, item.id).unwrap().len() as u64,
        cap - 1
    );
    let journal = content.journal(space).unwrap();
    assert_eq!(journal.len() as u64, cap);
    content
        .with_space(space, |space| {
            let total: i64 = space
                .conn
                .query_row("SELECT sum(total) FROM window_scopes", [], |row| row.get(0))
                .map_err(internal)?;
            assert_eq!(total as u64, cap);
            Ok(())
        })
        .unwrap();
}

#[test]
fn typed_plan_refusals_preserve_catalogue_shape() {
    let request = json!({"type":"window","request":{"bind":"notes","from":0,"to":10}});
    assert!(serde_json::from_value::<WindowRequest>(request.clone()).is_ok());
    for field in ["anchor", "parent"] {
        let mut invalid = request.clone();
        invalid["request"][field] = Value::Null;
        assert!(serde_json::from_value::<WindowRequest>(invalid).is_err());
    }
    let scope = WindowScope::NotesCollection(Uuid::new_v4()).key();
    let snapshot = read::Scope {
        total: 2_000_000,
        version: "data".into(),
    };
    let access = WindowAccess {
        scope: WindowScope::SpaceNotes,
        audience: WindowAudience::Member,
        count: WindowCountMode::Exact,
        view: Uuid::new_v4(),
    };
    let reply = evaluate(
        None,
        &scope,
        &snapshot,
        &range(350_000, 350_050, WindowAnchor::Start),
        &access,
        None,
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(&reply).unwrap(),
        json!({"status": 422, "code":"unsupported_query_plan","reason":"skip_limit"})
    );
    let mut none = access.clone();
    none.count = WindowCountMode::None;
    assert_eq!(
        bad_plan(&range(0, 1, WindowAnchor::End), &none, None),
        Some(WindowPlanReason::EndUnavailable)
    );
    assert_eq!(
        bad_plan(&range(500_001, 500_002, WindowAnchor::Start), &none, None),
        Some(WindowPlanReason::UnsupportedPlan)
    );
    let oversized = limits().live_limits.materialized_rows_per_window + 1;
    refusal(
        evaluate(
            None,
            &scope,
            &snapshot,
            &range(0, oversized, WindowAnchor::Start),
            &access,
            None,
        )
        .unwrap(),
        WindowPlanReason::MaterializationLimit,
    );
}

#[test]
fn guest_titles_ignore_hidden_edits_and_certified_estimates_are_member_only() {
    let conn = memory();
    let note = Uuid::new_v4();
    let coll = Uuid::new_v4();
    insert(
        &conn,
        note,
        "notes",
        json!({"title":"Public","hidden":"Secret"}),
        1,
        true,
    );
    insert(
        &conn,
        coll,
        "collections",
        json!({"item_kind":"notes","members":[note]}),
        0,
        true,
    );
    let scope = WindowScope::NotesCollection(coll);
    let key = scope.key();
    let old = read::scope(&conn, &key).unwrap();
    conn.execute("UPDATE items SET metadata=json_set(metadata,'$.hidden','New secret'),updated_at='2026-11-19T00:00:10.000Z' WHERE id=?1", [note.to_string()]).unwrap();
    let new = read::scope(&conn, &key).unwrap();
    assert_eq!(old.version, new.version);
    let request = range(0, 10, WindowAnchor::Start);
    let mut access = WindowAccess {
        scope,
        audience: WindowAudience::Member,
        count: WindowCountMode::None,
        view: Uuid::new_v4(),
    };
    let version = result_version(&key, &new, &request, &access);
    access.count = WindowCountMode::Estimate {
        value: 4,
        result_version: version,
    };
    let reply = evaluate(Some(&conn), &key, &new, &request, &access, None).unwrap();
    assert_eq!(
        serde_json::to_value(reply).unwrap()["count"],
        json!({"mode":"estimate","value":4})
    );
    access.audience = WindowAudience::Guest;
    assert_eq!(
        bad_plan(&request, &access, None),
        Some(WindowPlanReason::UnsupportedPlan)
    );
}

#[test]
fn deep_starts_require_a_current_boundary_and_end_offsets_exceed_the_start_ceiling() {
    let conn = memory();
    let items = [Uuid::from_u128(1), Uuid::from_u128(2)];
    for id in items {
        insert(&conn, id, "notes", json!({"title":"Same"}), 1, true);
    }
    let access = WindowAccess {
        scope: WindowScope::SpaceNotes,
        audience: WindowAudience::OwnSpaceMember,
        count: WindowCountMode::None,
        view: Uuid::new_v4(),
    };
    let max = limits().window.max_offset;
    let key = access.scope.key();
    let mut snapshot = read::scope(&conn, &key).unwrap();
    // Model the final two entries of a dense scope. Only the evaluated range
    // and its real covering seek key are used to check the deep continuation.
    snapshot.total = max + 2;
    let deep = range(max + 1, max + 2, WindowAnchor::Start);
    let prior_range = range(max, max + 1, WindowAnchor::Start);
    let prior = WindowResult {
        bind: prior_range.bind.clone(),
        state: "ready",
        result_version: result_version(&key, &snapshot, &prior_range, &access),
        window_ref: Uuid::new_v4(),
        offset: max,
        rows: vec![WindowRow {
            id: "first".into(),
            fields: json!({"title":"Same"}),
            item: items[0],
        }],
        audience: access.audience,
        anchor: WindowAnchor::Start,
        count: None,
        boundary: Some(
            read::keys(
                &conn,
                &key,
                read::Plan::new(2, 0, 1, WindowAnchor::Start, false, None).unwrap(),
            )
            .unwrap()
            .pop()
            .unwrap(),
        ),
    };
    let next =
        result(evaluate(Some(&conn), &key, &snapshot, &deep, &access, Some(&prior)).unwrap());
    assert_eq!(next.rows[0].item, items[1]);
    assert_eq!(next.offset, max + 1);
    assert!(next.count.is_none());

    let different_view = WindowAccess {
        view: Uuid::new_v4(),
        ..access.clone()
    };
    refusal(
        evaluate(
            Some(&conn),
            &key,
            &snapshot,
            &deep,
            &different_view,
            Some(&prior),
        )
        .unwrap(),
        WindowPlanReason::UnsupportedPlan,
    );
    let exact = WindowAccess {
        count: WindowCountMode::Exact,
        ..access
    };
    let end = range(max + 1, max + 2, WindowAnchor::End);
    let result = result(evaluate(Some(&conn), &key, &snapshot, &end, &exact, None).unwrap());
    assert_eq!(result.offset, max + 1);
    assert_eq!(result.rows[0].item, items[0]);
    assert_eq!(result.count.unwrap().value, max + 2);
}
