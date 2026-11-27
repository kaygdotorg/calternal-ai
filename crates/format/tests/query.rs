//! Collection grouping vectors.

use calternal_format::{
    EvalContext, Fields, InstallOptions, Page, Plugin, PluginFile, Row, Value, evaluate_bounded,
    evaluate_query, evaluate_query_bounded, parse_plugin,
};

fn grouping() -> Plugin {
    let files = [
        PluginFile {
            path: "plugin.md",
            text: Some("---\nplugin: grouping\nname: Grouping\nformat: 1\nversion: 1.0.0\n---\n"),
        },
        PluginFile {
            path: "kinds/entries.md",
            text: Some(
                "---\nkind: entries\nname: Entry\nformat: 1\nfolder: Entries\neditable: outside\nfields:\n  title: text, required\n  tags: tags\n---\nEntry.\n",
            ),
        },
        PluginFile {
            path: "collections/tags.md",
            text: Some(
                "---\ncollection: tags\ntitle: Tags\nformat: 1\nfrom: entries\ngroup: tags\nviews:\n  list: title\n---\n",
            ),
        },
    ];
    parse_plugin(&files, &InstallOptions::default()).unwrap()
}

#[test]
fn group_memberships_are_distinct_and_match_the_row() {
    let plugin = grouping();
    let one = Value::Text("one".into());
    let two = Value::Text("two".into());
    let fields = Fields::from([(
        "tags".into(),
        Value::List(vec![one.clone(), two.clone(), one.clone()]),
    )]);
    let metadata = Fields::new();
    let rows = [Row {
        fields: &fields,
        metadata: &metadata,
    }];
    let result = evaluate_query(
        &plugin.collections["tags"],
        &plugin.kinds["entries"],
        &rows,
        Page::default(),
        &EvalContext::default(),
    )
    .unwrap();
    assert_eq!(result.rows[0].groups, [one.clone(), two.clone()]);
    assert_eq!(
        result
            .groups
            .iter()
            .map(|group| (&group.key, group.matched))
            .collect::<Vec<_>>(),
        [(&one, 1), (&two, 1)]
    );
}

#[test]
fn duplicate_group_memberships_spend_work_before_lookup() {
    let plugin = grouping();
    let collection = &plugin.collections["tags"];
    let kind = &plugin.kinds["entries"];
    let mut ungrouped = collection.clone();
    ungrouped.group = None;
    let fields = Fields::from([(
        "tags".into(),
        Value::List(vec![Value::Text("repeat".into()); 100]),
    )]);
    let metadata = Fields::new();
    let row = Row {
        fields: &fields,
        metadata: &metadata,
    };
    let context = EvalContext::default();
    let run = |collection: &calternal_format::Collection, steps: &mut usize| {
        evaluate_query_bounded(
            collection,
            kind,
            &[row],
            Page::default(),
            &context,
            &|_, _| true,
            steps,
        )
    };
    let mut routine = 50_000;
    run(&ungrouped, &mut routine).unwrap();
    let mut expression = 50_000;
    evaluate_bounded(
        collection.group.as_ref().unwrap(),
        row,
        &context,
        &mut expression,
    )
    .unwrap();
    let mut steps = (50_000 - routine) + (50_000 - expression) + 10;
    assert!(
        run(collection, &mut steps)
            .unwrap_err()
            .message
            .contains("too costly")
    );
}
