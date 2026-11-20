use super::*;

#[test]
fn registered_scalar_order_and_checked_parent_scope_follow_child_moves() {
    let dir = tempfile::tempdir().unwrap();
    let content = content(&dir);
    let space = SpaceId::new();
    let account = AccountId::new();
    let kind = "reading-reminder";
    content
        .register_kind(
            space,
            kind,
            Placement::Visible {
                folder: "Reading".into(),
            },
            FileFormat::Markdown,
        )
        .unwrap();
    let write = |metadata, previous: Option<&crate::Item>, id| {
        content
            .write(
                space,
                previous.map(|item| item.id),
                kind,
                previous.map(|item| item.revision.as_str()),
                metadata,
                b"reminder",
                request(account, id),
                |_| json!({}),
            )
            .unwrap()
    };
    let parent = write(json!({"name":"Parent"}), None, "parent");
    let other = write(json!({"name":"Unrelated parent"}), None, "second");
    let third = write(
        json!({"name":"Third", "due":"2026-12-01", "belongs_to":parent.id}),
        None,
        "third",
    );
    let first = write(
        json!({"name":"First", "due":"2026-11-21", "belongs_to":parent.id}),
        None,
        "first",
    );
    let unrelated = write(
        json!({"name":"Excluded", "due":"2026-11-20", "belongs_to":other.id}),
        None,
        "excluded",
    );

    // Installing the declaration also projects retained heads that were
    // written while the kind only had its default newest order.
    let order = WindowKindOrder {
        title_field: "name".into(),
        sort_field: "due".into(),
        parent_field: Some("belongs_to".into()),
    };
    content.configure_window_kind(space, kind, &order).unwrap();
    let view = Uuid::new_v4();
    let scope = |parent, reference: &str, descending| WindowScope::Children {
        kind: kind.into(),
        parent,
        reference: reference.into(),
        order: WindowOrder {
            field: "due".into(),
            descending,
        },
    };
    let run = |scope: WindowScope, anchor, from, to, previous, reference: Option<&str>| {
        project(&content, space);
        let mut payload = range(from, to, anchor);
        payload.parent = reference.map(str::to_owned);
        content
            .window(
                space,
                &WindowRequest::Window { request: payload },
                previous,
                || {
                    Ok(WindowAccess {
                        scope: scope.clone(),
                        audience: WindowAudience::Member,
                        count: WindowCountMode::Exact,
                        view,
                    })
                },
            )
            .unwrap()
    };
    let child = scope(parent.id, "checked-parent", false);
    let p1 = result(run(
        child.clone(),
        WindowAnchor::Start,
        0,
        1,
        None,
        Some("checked-parent"),
    ));
    assert_eq!(p1.rows[0].item, first.id);
    assert_eq!(p1.rows[0].fields, json!({"title":"First"}));
    assert_eq!(p1.count.as_ref().unwrap().value, 2);
    assert_eq!(
        serde_json::to_value(&p1).unwrap()["parent"],
        "checked-parent"
    );
    let p2 = result(run(
        child.clone(),
        WindowAnchor::Start,
        1,
        2,
        Some(&p1),
        Some("checked-parent"),
    ));
    assert_eq!(p2.rows[0].item, third.id);
    refusal(
        run(
            child.clone(),
            WindowAnchor::Start,
            0,
            2,
            None,
            Some("other-view"),
        ),
        WindowPlanReason::UnsupportedPlan,
    );
    let reverse = scope(parent.id, "checked-parent", true);
    let p1_reverse = result(run(
        reverse.clone(),
        WindowAnchor::Start,
        0,
        1,
        None,
        Some("checked-parent"),
    ));
    assert_eq!(p1_reverse.rows[0].item, third.id);
    let p2_reverse = result(run(
        reverse.clone(),
        WindowAnchor::Start,
        1,
        2,
        Some(&p1_reverse),
        Some("checked-parent"),
    ));
    assert_eq!(p2_reverse.rows[0].item, first.id);
    let end_reverse = result(run(
        reverse,
        WindowAnchor::End,
        0,
        1,
        None,
        Some("checked-parent"),
    ));
    assert_eq!(end_reverse.rows[0].item, first.id);

    let root = WindowScope::Kind {
        kind: kind.into(),
        fields: vec![],
        order: WindowOrder {
            field: "due".into(),
            descending: false,
        },
    };
    let all = result(run(root, WindowAnchor::Start, 0, 3, None, None));
    assert_eq!(
        all.rows.iter().map(|row| row.item).collect::<Vec<_>>(),
        [unrelated.id, first.id, third.id]
    );
    assert_eq!(all.count.unwrap().value, 5);

    let moved = write(
        json!({"name":"Moved","due":"2026-11-21","belongs_to":other.id}),
        Some(&first),
        "move",
    );
    let prior = result(run(
        child,
        WindowAnchor::Start,
        0,
        2,
        Some(&p1),
        Some("checked-parent"),
    ));
    assert_eq!(prior.rows.len(), 1);
    assert_eq!(prior.count.unwrap().value, 1);
    let destination = scope(other.id, "other-parent", false);
    let next = result(run(
        destination,
        WindowAnchor::Start,
        0,
        2,
        None,
        Some("other-parent"),
    ));
    assert_eq!(next.rows[1].item, moved.id);
    assert_eq!(next.count.unwrap().value, 2);
}

#[test]
fn a_manual_collection_selects_one_order_without_disclosing_hidden_sort_fields() {
    let dir = tempfile::tempdir().unwrap();
    let content = content(&dir);
    let space = SpaceId::new();
    let account = AccountId::new();
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
    let create = |id: &str, title| {
        content
            .write(
                space,
                None,
                "notes",
                None,
                json!({"title":title}),
                b"note",
                request(account, id),
                |_| json!({}),
            )
            .unwrap()
    };
    let old = create("older", "A");
    let new = create("newer", "Z");
    content
        .with_space(space, |db| {
            for (id, date) in [
                (old.id, "2026-11-19T00:00:01.000Z"),
                (new.id, "2026-11-19T00:00:02.000Z"),
            ] {
                db.conn
                    .execute(
                        "UPDATE items SET updated_at=?1 WHERE id=?2",
                        params![date, id.to_string()],
                    )
                    .map_err(internal)?;
            }
            Ok(())
        })
        .unwrap();
    let id = serde_json::from_value(
        content
            .create_notes_collection(
                space,
                "Test".into(),
                vec![new.id, old.id],
                request(account, "collection"),
            )
            .unwrap()["id"]
            .clone(),
    )
    .unwrap();
    let view = Uuid::new_v4();
    let read = |scope: WindowScope, audience, count: WindowCountMode| {
        project(&content, space);
        content
            .window(
                space,
                &WindowRequest::Window {
                    request: range(0, 2, WindowAnchor::Start),
                },
                None,
                || {
                    Ok(WindowAccess {
                        scope: scope.clone(),
                        audience,
                        count: count.clone(),
                        view,
                    })
                },
            )
            .unwrap()
    };
    let title = WindowScope::NotesCollection(id);
    let newest = WindowScope::NotesCollectionNewest(id);
    let shared = result(read(
        title.clone(),
        WindowAudience::Guest,
        WindowCountMode::None,
    ));
    assert_eq!(shared.rows[0].item, old.id);
    assert!(shared.count.is_none());
    content
        .configure_notes_collection_order(space, id, true)
        .unwrap();
    let member = result(read(
        newest.clone(),
        WindowAudience::Member,
        WindowCountMode::Exact,
    ));
    assert_eq!(member.rows[0].item, new.id);
    assert_eq!(member.count.unwrap().value, 2);
    refusal(
        read(newest, WindowAudience::Guest, WindowCountMode::None),
        WindowPlanReason::UnsupportedPlan,
    );
    refusal(
        read(title.clone(), WindowAudience::Guest, WindowCountMode::None),
        WindowPlanReason::UnsupportedPlan,
    );
    content
        .configure_notes_collection_order(space, id, false)
        .unwrap();
    assert_eq!(
        result(read(title, WindowAudience::Guest, WindowCountMode::None)).rows[0].item,
        old.id
    );
}
