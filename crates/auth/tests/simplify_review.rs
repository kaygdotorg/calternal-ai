//! Independent regression probes for directory receipt scope.
use calternal_auth::{
    db::open_conn,
    spaces::{SpaceChange, Spaces},
};

#[test]
fn directory_request_ids_are_independent_between_spaces() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("auth.db");
    let spaces = Spaces::open(&path).unwrap();
    let ids = spaces.seed_development().unwrap();
    open_conn(&path)
        .unwrap()
        .execute(
            "INSERT INTO space_memberships VALUES(?1,?2,'reader')",
            [ids.personal.to_string(), ids.guest.to_string()],
        )
        .unwrap();
    spaces
        .change_with_receipt(
            ids.owner,
            Some(ids.shared),
            SpaceChange::SetRole {
                target: ids.guest,
                role: "editor".into(),
            },
            Some(("same-id", "spaces.set_role", "shared-input")),
        )
        .unwrap();
    let second = spaces.change_with_receipt(
        ids.owner,
        Some(ids.personal),
        SpaceChange::SetRole {
            target: ids.guest,
            role: "editor".into(),
        },
        Some(("same-id", "spaces.set_role", "personal-input")),
    );
    assert!(
        second.is_ok(),
        "another space's receipt blocked this request: {second:?}"
    );
}
