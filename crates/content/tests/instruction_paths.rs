//! Generic kinds cannot adopt or overwrite the Manager-owned instructions file.
use calternal_content::{Content, FileFormat, Placement, Request};
use calternal_types::{AccountId, ErrorCode, SPACE_INSTRUCTIONS_PATH, SpaceId};
use serde_json::json;
use std::time::Duration;
#[test]
fn kinds_cannot_claim_write_or_replay_the_reserved_instruction_path() {
    let dir = tempfile::tempdir().unwrap();
    let roots = calternal_roots::Roots {
        appdata: dir.path().join("appdata"),
        userdata_ssd: dir.path().join("ssd"),
        userdata_hdd: dir.path().join("hdd"),
    };
    let content = Content::open(
        roots,
        dir.path().join("secrets"),
        Duration::ZERO,
        calternal_content::DEFAULT_RECEIPT_RETENTION,
        |_| {},
    )
    .unwrap();
    let (space, account) = (SpaceId::new(), AccountId::new());
    for (index, parts) in [vec!["AGENTS.md"], vec!["agents.md", "child"]]
        .into_iter()
        .enumerate()
    {
        let denied = content.register_kind(
            space,
            &format!("blocked-{index}"),
            Placement::VisibleTree {
                folder: "AI".into(),
                subdirectories: parts.into_iter().map(str::to_owned).collect(),
            },
            FileFormat::Markdown,
        );
        assert!(denied.is_err());
    }
    content
        .register_kind(
            space,
            "scratch",
            Placement::Visible {
                folder: "AI".into(),
            },
            FileFormat::Markdown,
        )
        .unwrap();
    let tree = dir.path().join("ssd/spaces").join(space.to_string());
    std::fs::create_dir_all(tree.join("AI")).unwrap();
    let reserved = tree.join(SPACE_INSTRUCTIONS_PATH);
    calternal_fsio::write_atomic_mode(&reserved, b"Manager instructions", 0o600).unwrap();
    for title in ["AGENTS", "agents", "Agents"] {
        let denied = content.write(
            space,
            None,
            "scratch",
            None,
            json!({"title":title}),
            b"Editor content",
            Request {
                account,
                via: None,
                id: title,
                operation: "scratch.write",
                fingerprint: title,
            },
            |_| json!({}),
        );
        assert_eq!(denied.unwrap_err().code, ErrorCode::Forbidden);
    }
    assert_eq!(std::fs::read(&reserved).unwrap(), b"Manager instructions");
    let ordinary = content
        .write(
            space,
            None,
            "scratch",
            None,
            json!({"title":"Ordinary"}),
            b"ordinary",
            Request {
                account,
                via: None,
                id: "ordinary",
                operation: "scratch.write",
                fingerprint: "ordinary",
            },
            |i| json!({"id":i.id}),
        )
        .unwrap();
    // Finish the baseline placement before deliberately corrupting its intent;
    // the background worker must not race the test-only direct SQLite mutation.
    content.materialize(space, ordinary.id).unwrap();
    let db = rusqlite::Connection::open(
        dir.path()
            .join("appdata/spaces")
            .join(space.to_string())
            .join("content.db"),
    )
    .unwrap();
    // An invalid replay intent must fail closed, even before a process restart.
    db.execute(
        "UPDATE items SET path=?1,mat_state='pending' WHERE id=?2",
        rusqlite::params![SPACE_INSTRUCTIONS_PATH, ordinary.id.to_string()],
    )
    .unwrap();
    assert!(content.materialize(space, ordinary.id).is_err());
    db.execute(
        "UPDATE items SET path='AI/Ordinary.md',old_path=?1,mat_state='pending' WHERE id=?2",
        rusqlite::params![SPACE_INSTRUCTIONS_PATH, ordinary.id.to_string()],
    )
    .unwrap();
    assert!(content.materialize(space, ordinary.id).is_err());
    assert_eq!(std::fs::read(&reserved).unwrap(), b"Manager instructions");
}
