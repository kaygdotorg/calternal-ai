//! Query budgets cover repeated declaration data as well as stored data.

use super::*;
use crate::space::Head;
use calternal_types::{AccountId, SpaceId};

#[test]
fn query_charges_declared_viewer_defaults_before_every_candidate_copy() {
    let root = tempfile::tempdir().unwrap();
    let mut space = Space::open(root.path(), SpaceId::new(), 86_400_000).unwrap();
    let manifest = b"---\nplugin: sample\nname: Sample\nversion: 1.0.0\nformat: 1\n---\n";
    let kind = format!(
        "---\nkind: entries\nname: Entries\nformat: 1\nfolder: Entries\neditable: outside\nfields:\n  title: text, required\nstate:\n  draft: long text, default \"{}\"\n---\nAn entry.\n",
        "x".repeat(16_000),
    );
    space
        .install(
            BTreeMap::from([
                ("plugin.md".into(), manifest.to_vec()),
                ("kinds/entries.md".into(), kind.into_bytes()),
            ]),
            false,
            None,
        )
        .unwrap();
    let owner = AccountId::new();
    let make_head = |id| Head {
        id,
        path: "unused".into(),
        kind: "sample.entries".into(),
        author: owner.to_string(),
        version: 1,
        edited: 0,
        prefix: 128,
        length: 128,
        hash: [0; 32],
    };
    let input = || Query {
        from: Some("sample.entries".into()),
        collection: None,
        filter: None,
        sort: Vec::new(),
        search: None,
        cursor: None,
        limit: Some(1),
        records: false,
    };
    let actor = Actor::account(owner);
    let first = Uuid::now_v7();
    space.rows.insert(
        first,
        Row {
            head: make_head(first),
            fields: Default::default(),
        },
    );
    let single = query(&space, &actor, input()).unwrap();
    assert_eq!(
        single["rows"][0]["state"]["draft"].as_str().unwrap().len(),
        16_000
    );

    for _ in 0..1_200 {
        let id = Uuid::now_v7();
        space.rows.insert(
            id,
            Row {
                head: make_head(id),
                fields: Default::default(),
            },
        );
    }
    let Err(error) = query(&space, &actor, input()) else {
        panic!("repeated viewer defaults exceeded the work budget")
    };
    assert_eq!(error.code, ErrorCode::TooLarge);
}
