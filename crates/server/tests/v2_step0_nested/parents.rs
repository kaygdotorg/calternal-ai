//! Opaque checked parent refs over a disposable audience/parent covering index.
use super::fixture::{identity, run, summary};
use calternal_auth::spaces::DevelopmentIds;
use calternal_server::{api::Shared, auth::Identity};
use calternal_types::SpaceId;
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq)]
struct Row {
    id: String,
    kind: &'static str,
}

#[derive(Clone)]
struct Parent {
    space: SpaceId,
    viewer: Identity,
    view: Uuid,
    child_bind: &'static str,
    ancestry: Vec<Row>,
}

struct Windows {
    projection: Connection,
    authority: Arc<AtomicU64>,
    projected_at: u64,
    issued: HashMap<Uuid, Parent>,
}

const PAGE: &str = "SELECT item FROM child_window
   WHERE actor=?1 AND space=?2 AND parent=?3 AND bind=?4
   ORDER BY sort,item LIMIT ?5 OFFSET ?6";
const COUNT: &str =
    "SELECT n FROM child_count WHERE actor=?1 AND space=?2 AND parent=?3 AND bind=?4";

impl Windows {
    fn open(path: &std::path::Path) -> Self {
        let projection = Connection::open(path).unwrap();
        projection
            .execute_batch(
                "
        CREATE TABLE child_window (
          actor TEXT, space TEXT, parent TEXT, bind TEXT, sort INTEGER, item TEXT,
          PRIMARY KEY(actor,space,parent,bind,sort,item)
        ) STRICT, WITHOUT ROWID;
        CREATE TABLE child_count (
          actor TEXT, space TEXT, parent TEXT, bind TEXT, n INTEGER NOT NULL,
          PRIMARY KEY(actor,space,parent,bind)
        ) STRICT, WITHOUT ROWID;
        CREATE TRIGGER child_insert AFTER INSERT ON child_window BEGIN
          INSERT INTO child_count VALUES(NEW.actor,NEW.space,NEW.parent,NEW.bind,1)
          ON CONFLICT DO UPDATE SET n=n+1;
        END;
        CREATE TRIGGER child_delete AFTER DELETE ON child_window BEGIN
          UPDATE child_count SET n=n-1
          WHERE actor=OLD.actor AND space=OLD.space AND parent=OLD.parent AND bind=OLD.bind;
        END;
        ",
            )
            .unwrap();
        let authority = calternal_types::item_access_revision_handle();
        let projected_at = authority.load(Ordering::Acquire);
        assert_eq!(projected_at & 1, 0);
        Self {
            projection,
            authority,
            projected_at,
            issued: HashMap::new(),
        }
    }

    fn generation(&self) -> u64 {
        self.authority.load(Ordering::Acquire)
    }

    fn mint(&mut self, parent: Parent) -> Option<Uuid> {
        if parent.ancestry.is_empty() || parent.ancestry.len() > 3 {
            return None;
        }
        for (i, row) in parent.ancestry.iter().enumerate() {
            if parent.ancestry[..i].contains(row) {
                return None;
            }
        }
        let token = Uuid::new_v4();
        self.issued.insert(token, parent);
        Some(token)
    }

    async fn page(
        &self,
        state: &Shared,
        viewer: Identity,
        view: Uuid,
        bind: &str,
        token: Uuid,
        offset: i64,
    ) -> Option<(Vec<String>, i64)> {
        let parent = self.issued.get(&token)?;
        if parent.viewer.id != viewer.id || parent.view != view || parent.child_bind != bind {
            return None;
        }
        for ancestor in &parent.ancestry {
            if !visible(state, viewer, ancestor, parent.space).await {
                return None;
            }
        }
        if self.generation() != self.projected_at {
            return None;
        }
        let scope = params![
            viewer.id.to_string(),
            parent.space.to_string(),
            parent.ancestry.last()?.id,
            bind,
        ];
        let count = self
            .projection
            .query_row(COUNT, scope, |r| r.get::<_, i64>(0))
            .unwrap_or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => 0,
                _ => panic!("{e}"),
            });
        let rows = self
            .projection
            .prepare_cached(PAGE)
            .unwrap()
            .query_map(
                params![
                    viewer.id.to_string(),
                    parent.space.to_string(),
                    parent.ancestry.last()?.id,
                    bind,
                    2,
                    offset,
                ],
                |r| r.get(0),
            )
            .unwrap()
            .map(Result::unwrap)
            .collect();
        (self.generation() == self.projected_at).then_some((rows, count))
    }

    async fn index(&self, state: &Shared, parent: &Parent, children: &[Row]) {
        for (pos, row) in children.iter().enumerate() {
            if !visible(state, parent.viewer, row, parent.space).await {
                continue;
            }
            self.projection
                .execute(
                    "INSERT INTO child_window VALUES(?1,?2,?3,?4,?5,?6)",
                    params![
                        parent.viewer.id.to_string(),
                        parent.space.to_string(),
                        parent.ancestry.last().unwrap().id,
                        parent.child_bind,
                        pos as i64,
                        row.id
                    ],
                )
                .unwrap();
        }
    }
}

async fn visible(state: &Shared, viewer: Identity, row: &Row, space: SpaceId) -> bool {
    calternal_server::resolve::authorize_item_access(
        state,
        space,
        row.kind,
        Uuid::parse_str(&row.id).unwrap(),
        viewer.id,
        calternal_core::SharePermission::View,
    )
    .await
    .is_ok()
}

async fn note(state: &Shared, owner: Identity, space: SpaceId, title: &str) -> Row {
    let v = run(
        state,
        owner,
        "notes.create",
        json!({"space_id":space,"title":title,"markdown":"body"}),
    )
    .await
    .unwrap();
    Row {
        kind: "notes",
        id: v["note"]["id"].as_str().unwrap().into(),
    }
}

async fn thread(state: &Shared, owner: Identity, space: SpaceId, parent: Option<&Row>) -> Row {
    let mut input = json!({"space_id":space,"title":"Thread fixture"});
    if let Some(parent) = parent {
        input["parent_thread"] = json!(parent.id);
    }
    let v = run(state, owner, "threads.create", input).await.unwrap();
    Row {
        kind: "threads",
        id: v["thread"]["id"].as_str().unwrap().into(),
    }
}

async fn grant(state: &Shared, owner: Identity, guest: Identity, item: &Row) -> Value {
    run(state, owner, "sharing.grant", json!({
        "item":{"kind":item.kind,"id":item.id},"audience":{"account_id":guest.id},"permission":"view"
    })).await.unwrap()["grant_id"].clone()
}

async fn check_turn_order(
    state: &Shared,
    guest: Identity,
    space: SpaceId,
    thread_parent: &Row,
    turns: &mut [Row],
) {
    let base = run(
        state,
        guest,
        "threads.list",
        json!({"space_id":space,"parent_thread":thread_parent.id,"limit":2}),
    )
    .await
    .unwrap();
    assert_eq!(base["threads"].as_array().unwrap().len(), 2);
    let tail = run(
        state, guest, "threads.list",
        json!({"space_id":space,"parent_thread":thread_parent.id,"limit":2,"cursor":base["next_cursor"]}),
    ).await.unwrap();
    assert_eq!(tail["threads"].as_array().unwrap().len(), 1);
    let actual_order = base["threads"]
        .as_array()
        .unwrap()
        .iter()
        .chain(tail["threads"].as_array().unwrap())
        .map(|row| row["id"].as_str().unwrap())
        .collect::<Vec<_>>();
    turns.sort_by_key(|row| actual_order.iter().position(|id| *id == row.id).unwrap());
}

pub(super) async fn probe(
    state: &Shared,
    ids: &DevelopmentIds,
    roots: &calternal_roots::Roots,
    runs: usize,
) {
    let owner = identity(ids.owner);
    let guest = identity(ids.guest);
    let view = Uuid::new_v4();
    let root = note(state, owner, ids.personal, "Parent").await;
    let a = note(state, owner, ids.personal, "Child of note").await;
    let b = note(state, owner, ids.personal, "Sibling").await;
    let c = note(state, owner, ids.personal, "Sibling 2").await;
    let hidden = note(state, owner, ids.personal, "HIDDEN").await;
    let grand = note(state, owner, ids.personal, "Grandchild").await;
    let great = note(state, owner, ids.personal, "Great grandchild").await;
    let parent_grant = grant(state, owner, guest, &root).await;
    let child_grant = grant(state, owner, guest, &c).await;
    for item in [&a, &b, &grand, &great] {
        grant(state, owner, guest, item).await;
    }

    let thread_parent = thread(state, owner, ids.shared, None).await;
    let mut turns = Vec::new();
    for _ in 0..3 {
        turns.push(thread(state, owner, ids.shared, Some(&thread_parent)).await);
    }
    let turn_child = turns[0].clone();
    let turn_grand = thread(state, owner, ids.shared, Some(&turn_child)).await;
    let turn_great = thread(state, owner, ids.shared, Some(&turn_grand)).await;
    check_turn_order(state, guest, ids.shared, &thread_parent, &mut turns).await;
    let other_parent = thread(state, owner, ids.shared, None).await;
    let mut other_children = Vec::new();
    for _ in 0..3 {
        other_children.push(thread(state, owner, ids.shared, Some(&other_parent)).await);
    }
    check_turn_order(state, guest, ids.shared, &other_parent, &mut other_children).await;
    let scope = Parent {
        viewer: guest,
        view,
        space: ids.personal,
        child_bind: "notes.children",
        ancestry: vec![root.clone()],
    };
    let turn_scope = Parent {
        space: ids.shared,
        child_bind: "turns.children",
        ancestry: vec![thread_parent.clone()],
        ..scope.clone()
    };
    let mut grand_scope = scope.clone();
    grand_scope.child_bind = "notes.grandchildren";
    grand_scope.ancestry.push(a.clone());
    let mut great_scope = grand_scope.clone();
    great_scope.child_bind = "notes.great";
    great_scope.ancestry.push(grand.clone());

    let other_turn_scope = Parent {
        ancestry: vec![other_parent],
        ..turn_scope.clone()
    };
    let mut second_turn = turn_scope.clone();
    second_turn.child_bind = "turns.grandchildren";
    second_turn.ancestry.push(turn_child);
    let mut third_turn = second_turn.clone();
    third_turn.child_bind = "turns.great";
    third_turn.ancestry.push(turn_grand.clone());

    let mut windows = Windows::open(&roots.appdata.join("parent-projection.db"));
    windows
        .index(state, &scope, &[a.clone(), hidden, b.clone(), c.clone()])
        .await;
    windows.index(state, &turn_scope, &turns).await;
    windows
        .index(state, &other_turn_scope, &other_children)
        .await;
    windows
        .index(state, &second_turn, std::slice::from_ref(&turn_grand))
        .await;
    windows
        .index(state, &third_turn, std::slice::from_ref(&turn_great))
        .await;
    windows
        .index(state, &grand_scope, std::slice::from_ref(&grand))
        .await;
    windows
        .index(state, &great_scope, std::slice::from_ref(&great))
        .await;
    assert_eq!(windows.generation(), windows.projected_at);
    let note_ref = windows.mint(scope.clone()).unwrap();
    let turn_ref = windows.mint(turn_scope).unwrap();
    let other_turn_ref = windows.mint(other_turn_scope).unwrap();
    let grand_ref = windows.mint(grand_scope.clone()).unwrap();
    let great_ref = windows.mint(great_scope.clone()).unwrap();
    let second_turn_ref = windows.mint(second_turn).unwrap();
    let third_turn_ref = windows.mint(third_turn).unwrap();
    great_scope.ancestry.push(great.clone());
    assert!(windows.mint(great_scope).is_none());
    grand_scope.ancestry.push(root.clone());
    assert!(windows.mint(grand_scope).is_none());

    let content = Connection::open(
        roots
            .appdata
            .join(format!("spaces/{}/content.db", ids.shared)),
    )
    .unwrap();
    let baseline = plan(
        &content,
        "SELECT id FROM items WHERE kind='threads' AND updated_at<=?1 AND (updated_at<?1 OR id>?2)
         AND (?3 IS NOT NULL OR json_extract(metadata,'$.deleted') IS NOT 1)
         AND (?3 IS NOT NULL OR ?4 OR json_extract(metadata,'$.archived') IS NOT 1)
         AND (?3 IS NULL OR json_extract(metadata,'$.parent_thread')=?3)
         ORDER BY updated_at DESC,id LIMIT ?5",
        params!["~", "", thread_parent.id, false, 3],
    );
    let covering = plan(
        &windows.projection,
        PAGE,
        params!["a", "s", "p", "notes.children", 2, 0],
    );
    assert!(
        covering.contains("actor=? AND space=? AND parent=? AND bind=?"),
        "{covering}"
    );
    assert!(!covering.contains("TEMP B-TREE"), "{covering}");
    eprintln!("Turns baseline {baseline}; parent covering {covering}");

    let mut reads = vec![];
    for _ in 0..runs {
        let start = Instant::now();
        let a0 = windows
            .page(state, guest, view, "turns.children", turn_ref, 0)
            .await
            .unwrap();
        let b0 = windows
            .page(state, guest, view, "turns.children", other_turn_ref, 0)
            .await
            .unwrap();
        let a2 = windows
            .page(state, guest, view, "turns.children", turn_ref, 2)
            .await
            .unwrap();
        let b2 = windows
            .page(state, guest, view, "turns.children", other_turn_ref, 2)
            .await
            .unwrap();
        reads.push(start.elapsed());
        assert_eq!(a0, (vec![turns[0].id.clone(), turns[1].id.clone()], 3));
        assert_eq!(a2, (vec![turns[2].id.clone()], 3));
        assert_eq!(
            b0,
            (
                vec![other_children[0].id.clone(), other_children[1].id.clone()],
                3
            )
        );
        assert_eq!(b2, (vec![other_children[2].id.clone()], 3));
        assert_eq!(
            windows
                .page(state, guest, view, "notes.children", note_ref, 0)
                .await,
            Some((vec![a.id.clone(), b.id.clone()], 3))
        );
        assert_eq!(
            windows
                .page(state, guest, view, "notes.children", note_ref, 2)
                .await,
            Some((vec![c.id.clone()], 3))
        );
        assert_eq!(
            windows
                .page(state, guest, view, "notes.grandchildren", grand_ref, 0)
                .await,
            Some((vec![grand.id.clone()], 1))
        );
        assert_eq!(
            windows
                .page(state, guest, view, "notes.great", great_ref, 0)
                .await,
            Some((vec![great.id.clone()], 1))
        );
        assert_eq!(
            windows
                .page(
                    state,
                    guest,
                    view,
                    "turns.grandchildren",
                    second_turn_ref,
                    0
                )
                .await,
            Some((vec![turn_grand.id.clone()], 1))
        );
        assert_eq!(
            windows
                .page(state, guest, view, "turns.great", third_turn_ref, 0)
                .await,
            Some((vec![turn_great.id.clone()], 1))
        );
        for bad in [
            windows
                .page(state, guest, view, "turns.children", note_ref, 0)
                .await,
            windows
                .page(state, guest, Uuid::new_v4(), "notes.children", note_ref, 0)
                .await,
            windows
                .page(state, owner, view, "notes.children", note_ref, 0)
                .await,
            windows
                .page(state, guest, view, "notes.children", Uuid::new_v4(), 0)
                .await,
        ] {
            assert!(bad.is_none());
        }
    }
    summary("interleaved parent windows (four reads)", &mut reads);
    run(
        state,
        owner,
        "sharing.revoke",
        json!({
            "item":{"kind":c.kind,"id":c.id},"grant_id":child_grant
        }),
    )
    .await
    .unwrap();
    assert!(
        windows
            .page(state, guest, view, "notes.children", note_ref, 0)
            .await
            .is_none()
    );
    windows
        .projection
        .execute("DELETE FROM child_window WHERE item=?1", [&c.id])
        .unwrap();
    windows.projected_at = windows.generation();
    assert_eq!(
        windows
            .page(state, guest, view, "notes.children", note_ref, 2)
            .await,
        Some((vec![], 2))
    );
    run(
        state,
        owner,
        "sharing.revoke",
        json!({
            "item":{"kind":root.kind,"id":root.id},"grant_id":parent_grant
        }),
    )
    .await
    .unwrap();
    for _ in 0..runs {
        for (bind, token) in [
            ("notes.children", note_ref),
            ("notes.grandchildren", grand_ref),
            ("notes.great", great_ref),
        ] {
            assert!(
                windows
                    .page(state, guest, view, bind, token, 0)
                    .await
                    .is_none()
            );
        }
    }
    tokio::task::block_in_place(|| {
        super::fixture::spaces(roots)
            .remove(ids.owner, ids.shared, ids.guest)
            .unwrap();
    });
    for _ in 0..runs {
        for (bind, token) in [
            ("turns.children", turn_ref),
            ("turns.children", other_turn_ref),
            ("turns.grandchildren", second_turn_ref),
            ("turns.great", third_turn_ref),
        ] {
            assert!(
                windows
                    .page(state, guest, view, bind, token, 0)
                    .await
                    .is_none()
            );
        }
    }
    // Reset the original development membership for the next independent arm.
    Connection::open(roots.appdata.join("system/auth.db"))
        .unwrap()
        .execute(
            "INSERT INTO space_memberships VALUES(?1,?2,'reader')",
            params![ids.shared.to_string(), ids.guest.to_string()],
        )
        .unwrap();
}

fn plan(conn: &Connection, sql: &str, values: impl rusqlite::Params) -> String {
    conn.prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
        .unwrap()
        .query_map(values, |r| r.get::<_, String>(3))
        .unwrap()
        .map(Result::unwrap)
        .collect::<Vec<_>>()
        .join("; ")
}
