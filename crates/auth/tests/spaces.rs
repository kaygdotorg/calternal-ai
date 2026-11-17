//! Stored space invariants and source people-action projections, using only temp DBs.
use calternal_auth::{
    ErrorCode,
    db::open_conn,
    spaces::{DevelopmentIds, SpaceChange, Spaces},
};
use calternal_types::{AccountId, SpaceId};
use rusqlite::params;

struct Fixture {
    _dir: tempfile::TempDir,
    path: std::path::PathBuf,
    spaces: Spaces,
    ids: DevelopmentIds,
}
impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.db");
        let spaces = Spaces::open(&path).unwrap();
        let ids = spaces.seed_development().unwrap();
        Self {
            _dir: dir,
            path,
            spaces,
            ids,
        }
    }
    fn change(
        &self,
        actor: AccountId,
        s: SpaceId,
        change: SpaceChange,
    ) -> calternal_auth::Result<serde_json::Value> {
        self.spaces.change(actor, Some(s), change)
    }
    fn add(&self, name: &str, space: SpaceId, role: &str) -> AccountId {
        let c = open_conn(&self.path).unwrap();
        let id = AccountId::new();
        c.execute(
            "INSERT INTO users(id,name,username,created_at) VALUES(?1,?2,?2,0)",
            params![id.to_string(), name],
        )
        .unwrap();
        c.execute(
            "INSERT INTO space_memberships VALUES(?1,?2,?3)",
            params![space.to_string(), id.to_string(), role],
        )
        .unwrap();
        id
    }
}
fn forbidden<T: std::fmt::Debug>(r: calternal_auth::Result<T>) {
    assert_eq!(r.unwrap_err().code(), ErrorCode::Forbidden);
}

// Source people.rs first_owner_has_capabilities_and_cannot_demote_the_last_admin:
// space ownership replaces the server-level last-manager rule.
#[test]
fn first_owner_has_capabilities_and_cannot_be_demoted_or_removed() {
    let f = Fixture::new();
    let s = f.ids.shared;
    let o = f.ids.owner;
    let caps = f.spaces.capabilities(o, s).unwrap().unwrap();
    for cap in [
        "members.manage",
        "managers.manage",
        "space.transfer",
        "space.delete",
    ] {
        assert!(caps.iter().any(|c| c.name() == cap));
    }
    for change in [
        SpaceChange::Leave,
        SpaceChange::Remove { target: o },
        SpaceChange::SetRole {
            target: o,
            role: "reader".into(),
        },
    ] {
        forbidden(f.change(o, s, change));
    }
    assert_eq!(
        f.spaces
            .members(o, s)
            .unwrap()
            .iter()
            .filter(|m| m.role == "owner")
            .count(),
        1
    );
}
// Source people.rs member_cannot_change_roles_and_new_capability_sets_work:
// preserve arbitrary capability-set lookup, generic refusals, and retry revocation.
#[test]
fn member_cannot_change_roles_and_new_capability_sets_work() {
    let f = Fixture::new();
    let s = f.ids.shared;
    let o = f.ids.owner;
    let g = f.ids.guest;
    for sid in [s, SpaceId::new()] {
        forbidden(f.change(
            g,
            sid,
            SpaceChange::SetRole {
                target: o,
                role: "reader".into(),
            },
        ));
    }
    let c = open_conn(&f.path).unwrap();
    c.execute_batch("INSERT INTO space_roles VALUES('helper'); INSERT INTO space_role_capabilities VALUES('helper','members.manage');").unwrap();
    c.execute(
        "UPDATE space_memberships SET role='helper' WHERE space_id=?1 AND account_id=?2",
        params![s.to_string(), g.to_string()],
    )
    .unwrap();
    let target = f.add("target", s, "reader");
    let change = || SpaceChange::SetRole {
        target,
        role: "editor".into(),
    };
    let first = f
        .spaces
        .change_with_receipt(
            g,
            Some(s),
            change(),
            Some(("retry", "spaces.set_role", "fp")),
        )
        .unwrap();
    assert_eq!(
        first,
        f.spaces
            .change_with_receipt(
                g,
                Some(s),
                change(),
                Some(("retry", "spaces.set_role", "fp"))
            )
            .unwrap()
    );
    f.change(
        o,
        s,
        SpaceChange::SetRole {
            target: g,
            role: "reader".into(),
        },
    )
    .unwrap();
    forbidden(f.spaces.change_with_receipt(
        g,
        Some(s),
        change(),
        Some(("retry", "spaces.set_role", "fp")),
    ));
}
// Source people.rs two_admins_racing_to_demote_themselves_leave_one_admin and
// security_regressions.rs:276: two writers may never eliminate protected authority.
#[test]
fn two_concurrent_transfers_have_one_winner() {
    let f = Fixture::new();
    let a = f.add("a", f.ids.shared, "reader");
    let b = f.add("b", f.ids.shared, "reader");
    let left = Spaces::open(&f.path).unwrap();
    let right = Spaces::open(&f.path).unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let o = f.ids.owner;
    let s = f.ids.shared;
    let b1 = barrier.clone();
    let t = std::thread::spawn(move || {
        b1.wait();
        left.change(o, Some(s), SpaceChange::Transfer { target: a })
    });
    barrier.wait();
    let other = right.change(o, Some(s), SpaceChange::Transfer { target: b });
    let first = t.join().unwrap();
    assert_eq!(
        [first.is_ok(), other.is_ok()]
            .into_iter()
            .filter(|ok| *ok)
            .count(),
        1
    );
    let new = if first.is_ok() { a } else { b };
    let members = f.spaces.members(new, s).unwrap();
    assert_eq!(members.iter().filter(|m| m.role == "owner").count(), 1);
    assert_eq!(
        members.iter().find(|m| m.account_id == o).unwrap().role,
        "manager"
    );
}
#[test]
fn transfer_racing_target_removal_preserves_owner_membership() {
    for i in 0..12 {
        let f = Fixture::new();
        let a = f.add(&format!("a{i}"), f.ids.shared, "editor");
        let other = Spaces::open(&f.path).unwrap();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let b = barrier.clone();
        let s = f.ids.shared;
        let o = f.ids.owner;
        let t = std::thread::spawn(move || {
            b.wait();
            other.change(o, Some(s), SpaceChange::Transfer { target: a })
        });
        barrier.wait();
        let removed = f.change(o, s, SpaceChange::Remove { target: a });
        let transferred = t.join().unwrap();
        assert_ne!(removed.is_ok(), transferred.is_ok());
        let owner = if transferred.is_ok() { a } else { o };
        assert_eq!(
            f.spaces
                .members(owner, s)
                .unwrap()
                .iter()
                .filter(|m| m.role == "owner")
                .count(),
            1
        );
        assert!(
            open_conn(&f.path)
                .unwrap()
                .prepare("PRAGMA foreign_key_check")
                .unwrap()
                .query([])
                .unwrap()
                .next()
                .unwrap()
                .is_none()
        );
    }
}
#[test]
fn managers_manage_only_readers_and_editors() {
    let f = Fixture::new();
    let s = f.ids.shared;
    let o = f.ids.owner;
    let m = f.add("manager", s, "manager");
    let m2 = f.add("manager2", s, "manager");
    let r = f.add("reader", s, "reader");
    for target in [o, m, m2] {
        for change in [
            SpaceChange::Remove { target },
            SpaceChange::SetRole {
                target,
                role: "reader".into(),
            },
        ] {
            forbidden(f.change(m, s, change));
        }
    }
    forbidden(f.change(
        m,
        s,
        SpaceChange::SetRole {
            target: r,
            role: "manager".into(),
        },
    ));
    forbidden(f.change(m, s, SpaceChange::Transfer { target: r }));
    forbidden(f.change(m, s, SpaceChange::Delete));
    f.change(
        m,
        s,
        SpaceChange::SetRole {
            target: r,
            role: "editor".into(),
        },
    )
    .unwrap();
    f.change(m, s, SpaceChange::Remove { target: r }).unwrap();
    f.change(
        o,
        s,
        SpaceChange::SetRole {
            target: m,
            role: "reader".into(),
        },
    )
    .unwrap();
    f.change(
        o,
        s,
        SpaceChange::SetRole {
            target: m2,
            role: "reader".into(),
        },
    )
    .unwrap(); // last Manager may be demoted
    assert_eq!(
        f.spaces
            .members(o, s)
            .unwrap()
            .iter()
            .filter(|m| m.role == "manager")
            .count(),
        0
    );
}
#[test]
fn owner_role_requires_explicit_transfer() {
    let f = Fixture::new();
    forbidden(f.change(
        f.ids.owner,
        f.ids.shared,
        SpaceChange::Transfer {
            target: AccountId::new(),
        },
    ));
    assert_eq!(
        f.change(
            f.ids.owner,
            f.ids.shared,
            SpaceChange::SetRole {
                target: f.ids.guest,
                role: "owner".into()
            }
        )
        .unwrap_err()
        .code(),
        ErrorCode::InvalidInput
    );
    f.change(
        f.ids.owner,
        f.ids.shared,
        SpaceChange::Transfer {
            target: f.ids.guest,
        },
    )
    .unwrap();
    forbidden(f.change(
        f.ids.owner,
        f.ids.shared,
        SpaceChange::Transfer {
            target: f.ids.owner,
        },
    ));
    f.change(f.ids.owner, f.ids.shared, SpaceChange::Leave)
        .unwrap();
    forbidden(f.spaces.members(f.ids.owner, f.ids.shared));
}
#[test]
fn database_requires_owner_membership_and_existing_accounts() {
    let f = Fixture::new();
    let mut c = open_conn(&f.path).unwrap();
    for statement in [
        format!(
            "DELETE FROM space_memberships WHERE space_id='{}' AND account_id='{}'",
            f.ids.shared, f.ids.owner
        ),
        format!(
            "UPDATE spaces SET owner_account='{}' WHERE id='{}'",
            f.ids.guest, f.ids.personal
        ),
        format!(
            "INSERT INTO space_memberships VALUES('{}','{}','reader')",
            f.ids.shared,
            AccountId::new()
        ),
        format!(
            "INSERT INTO space_memberships VALUES('{}','{}','reader')",
            SpaceId::new(),
            f.ids.owner
        ),
    ] {
        let tx = c
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .unwrap();
        let result = tx.execute_batch(&statement);
        if result.is_ok() {
            assert!(tx.commit().is_err());
        }
    }
    assert_eq!(
        f.spaces.members(f.ids.owner, f.ids.shared).unwrap().len(),
        2
    );
}
#[test]
fn identifiers_are_fresh_v7_and_memberships_are_space_scoped() {
    let f = Fixture::new();
    let ids = [
        f.ids.owner.as_uuid(),
        f.ids.guest.as_uuid(),
        f.ids.personal.as_uuid(),
        f.ids.shared.as_uuid(),
    ];
    for id in ids {
        assert_eq!(id.get_version_num(), 7);
    }
    assert_eq!(
        ids.into_iter()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        4
    );
    let second = f
        .spaces
        .change(
            f.ids.guest,
            None,
            SpaceChange::Create {
                name: "other".into(),
            },
        )
        .unwrap();
    let s = SpaceId::parse(second["space"]["id"].as_str().unwrap()).unwrap();
    assert!(
        f.spaces
            .capabilities(f.ids.guest, s)
            .unwrap()
            .unwrap()
            .contains(calternal_types::Capability::from_name("space.delete").unwrap())
    );
    assert!(
        !f.spaces
            .capabilities(f.ids.guest, f.ids.shared)
            .unwrap()
            .unwrap()
            .contains(calternal_types::Capability::from_name("space.delete").unwrap())
    );
    forbidden(f.change(f.ids.guest, f.ids.shared, SpaceChange::Delete));
    forbidden(f.spaces.members(f.ids.guest, f.ids.personal));
    forbidden(f.spaces.members(f.ids.guest, SpaceId::new()));
}
#[test]
fn server_admin_gains_no_content_or_membership_access() {
    let f = Fixture::new();
    let c = open_conn(&f.path).unwrap();
    c.execute(
        "UPDATE users SET role='admin' WHERE id=?1",
        [f.ids.guest.to_string()],
    )
    .unwrap();
    assert!(
        f.spaces
            .capabilities(f.ids.guest, f.ids.personal)
            .unwrap()
            .is_none()
    );
    forbidden(f.spaces.members(f.ids.guest, f.ids.personal));
    assert_eq!(f.spaces.list(f.ids.guest).unwrap().len(), 1);
}
#[test]
fn deletion_removes_only_directory_records_and_does_not_resurrect_on_restart() {
    let f = Fixture::new();
    let content = f._dir.path().join("fixture.md");
    std::fs::write(&content, "content remains").unwrap();
    f.change(f.ids.owner, f.ids.shared, SpaceChange::Delete)
        .unwrap();
    assert!(content.exists());
    assert!(
        f.spaces
            .capabilities(f.ids.guest, f.ids.shared)
            .unwrap()
            .is_none()
    );
    let reopened = Spaces::open(&f.path).unwrap();
    let ids = reopened.seed_development().unwrap();
    assert_eq!(ids.shared, f.ids.shared);
    assert!(reopened.list(ids.guest).unwrap().is_empty());
}
#[test]
fn revocations_and_transfer_survive_restart() {
    let f = Fixture::new();
    f.change(
        f.ids.owner,
        f.ids.shared,
        SpaceChange::Transfer {
            target: f.ids.guest,
        },
    )
    .unwrap();
    f.change(
        f.ids.guest,
        f.ids.shared,
        SpaceChange::Remove {
            target: f.ids.owner,
        },
    )
    .unwrap();
    let reopened = Spaces::open(&f.path).unwrap();
    reopened.seed_development().unwrap();
    assert!(
        reopened
            .capabilities(f.ids.owner, f.ids.shared)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        reopened.members(f.ids.guest, f.ids.shared).unwrap()[0].role,
        "owner"
    );
}
#[test]
fn request_receipts_are_actor_private_and_bind_operation_and_input() {
    let f = Fixture::new();
    let create = || SpaceChange::Create {
        name: "space".into(),
    };
    let a = f
        .spaces
        .change_with_receipt(
            f.ids.owner,
            None,
            create(),
            Some(("same", "spaces.create", "one")),
        )
        .unwrap();
    let b = f
        .spaces
        .change_with_receipt(
            f.ids.guest,
            None,
            create(),
            Some(("same", "spaces.create", "one")),
        )
        .unwrap();
    assert_ne!(a["space"]["id"], b["space"]["id"]);
    assert_eq!(
        f.spaces
            .change_with_receipt(
                f.ids.owner,
                None,
                create(),
                Some(("same", "spaces.create", "changed"))
            )
            .unwrap_err()
            .code(),
        ErrorCode::Conflict
    );
}

#[test]
fn identity_namespaces_cannot_collide_even_in_direct_database_writes() {
    let f = Fixture::new();
    let c = open_conn(&f.path).unwrap();
    assert!(
        c.execute(
            "INSERT INTO users(id,name,username,created_at) VALUES(?1,'x','collision',0)",
            [f.ids.shared.to_string()]
        )
        .is_err()
    );
    assert!(
        c.execute(
            "INSERT INTO spaces(id,name,owner_account,created) VALUES(?1,'x',?2,0)",
            [f.ids.guest.to_string(), f.ids.owner.to_string()]
        )
        .is_err()
    );
}

#[test]
fn a_custom_manager_management_capability_cannot_replace_ownership() {
    let f = Fixture::new();
    let s = f.ids.shared;
    let guest = f.ids.guest;
    let c = open_conn(&f.path).unwrap();
    c.execute_batch("INSERT INTO space_roles VALUES('custom'); INSERT INTO space_role_capabilities SELECT 'custom',capability FROM space_role_capabilities WHERE role='owner';").unwrap();
    c.execute(
        "UPDATE space_memberships SET role='custom' WHERE space_id=?1 AND account_id=?2",
        params![s.to_string(), guest.to_string()],
    )
    .unwrap();
    let target = f.add("reader", s, "reader");
    let manager = f.add("manager", s, "manager");
    for change in [
        SpaceChange::SetRole {
            target,
            role: "manager".into(),
        },
        SpaceChange::Remove { target: manager },
        SpaceChange::Transfer { target },
        SpaceChange::Delete,
    ] {
        forbidden(f.change(guest, s, change));
    }
}

// Source security_regressions.rs:325: administrative directory capabilities
// cannot expose a person outside the effective visibility boundary. Group
// management remains separate; it never manufactures a space membership.
#[test]
fn directory_group_management_cannot_expose_another_spaces_members() {
    let f = Fixture::new();
    let c = open_conn(&f.path).unwrap();
    c.execute_batch("INSERT INTO roles VALUES('group-manager'); INSERT INTO role_capabilities VALUES('group-manager','groups.manage');").unwrap();
    c.execute(
        "UPDATE users SET role='group-manager' WHERE id=?1",
        [f.ids.guest.to_string()],
    )
    .unwrap();
    c.execute(
        "INSERT INTO groups(id,name) VALUES('directory','Directory')",
        [],
    )
    .unwrap();
    c.execute(
        "INSERT INTO group_members VALUES('directory',?1,1)",
        [f.ids.guest.to_string()],
    )
    .unwrap();
    c.execute(
        "INSERT INTO group_members VALUES('directory',?1,0)",
        [f.ids.owner.to_string()],
    )
    .unwrap();
    forbidden(f.spaces.members(f.ids.guest, f.ids.personal));
    assert_eq!(f.spaces.list(f.ids.guest).unwrap().len(), 1);
    assert!(
        f.spaces
            .capabilities(f.ids.guest, f.ids.personal)
            .unwrap()
            .is_none()
    );
}
