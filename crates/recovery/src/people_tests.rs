use super::*;
use calternal_auth::{
    Auth, AuthConfig,
    people::GroupChange,
    spaces::{MemberTarget, SpaceChange},
};
use calternal_types::{GroupId, InviteId};
use std::{os::unix::fs::PermissionsExt, sync::Arc};

#[tokio::test]
async fn every_people_write_publishes_current_authority_before_acknowledgement() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("protected");
    std::fs::create_dir(&root).unwrap();
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
    let install = Uuid::now_v7();
    let c = Arc::new(Coordinator::open(&root, install).unwrap());
    let auth = Auth::open(
        dir.path(),
        AuthConfig::new("http://localhost:18080").unwrap(),
    )
    .unwrap();
    auth.configure_recovery(c.clone()).await.unwrap();
    let operator = auth.create("Operator").await.unwrap().id;
    let person = auth.create("Person").await.unwrap().id;

    let db = dir.path().join("system/auth.db");
    tokio::task::spawn_blocking(move || {
        let spaces = auth.spaces();
        let space = spaces.create(operator, "Reviewed space").unwrap().id;
        let mut sequence = c.validate_and_read().unwrap().sequence;
        let mut witnessed = || {
            let image = c.validate_and_read().unwrap();
            assert_eq!(image.sequence, sequence + 2);
            sequence = image.sequence;
            assert_eq!(
                image.tables,
                capture(&calternal_auth::db::open_conn(&db).unwrap(), install, 1)
                    .unwrap()
                    .tables
            );
            image
        };

        let group = spaces
            .change_group(
                operator,
                GroupChange::Create {
                    name: "Plan".into(),
                },
                ("group-new", "groups.create", "x"),
            )
            .unwrap();
        let group = GroupId::parse(group["group_id"].as_str().unwrap()).unwrap();
        witnessed();
        spaces
            .change_group(
                operator,
                GroupChange::Rename {
                    group,
                    name: "Updated plan".into(),
                },
                ("group-name", "groups.rename", "x"),
            )
            .unwrap();
        witnessed();
        spaces
            .change_group(
                operator,
                GroupChange::AddMember {
                    group,
                    account: person,
                },
                ("group-add", "groups.add_member", "x"),
            )
            .unwrap();
        witnessed();
        for change in [
            SpaceChange::Add {
                target: MemberTarget::Group(group),
                role: "reader".into(),
            },
            SpaceChange::SetGroupRole {
                target: group,
                role: "editor".into(),
            },
            SpaceChange::RemoveGroup { target: group },
            SpaceChange::Add {
                target: MemberTarget::Account(person),
                role: "reader".into(),
            },
            SpaceChange::SetRole {
                target: person,
                role: "editor".into(),
            },
            SpaceChange::Remove { target: person },
        ] {
            spaces.change(operator, Some(space), change).unwrap();
            witnessed();
        }
        spaces
            .change_group(
                operator,
                GroupChange::RemoveMember {
                    group,
                    account: person,
                },
                ("group-remove", "groups.remove_member", "x"),
            )
            .unwrap();
        witnessed();
        spaces
            .change_group(
                operator,
                GroupChange::Delete(group),
                ("group-delete", "groups.delete", "x"),
            )
            .unwrap();
        witnessed();
        spaces
            .set_profile(
                person,
                "Changed name".into(),
                ("profile", "account.set_profile", "x"),
            )
            .unwrap();
        witnessed();
        let invite = auth
            .create_invite(
                operator,
                space,
                "reader".into(),
                None,
                ("invite-new", "invites.create", "x"),
            )
            .unwrap();
        let id = InviteId::parse(invite["invite_id"].as_str().unwrap()).unwrap();
        let image = witnessed();
        let invites = auth.list_invites(operator, space, None).unwrap();
        let token = invites["invites"][0]["link"]
            .as_str()
            .unwrap()
            .rsplit('/')
            .next()
            .unwrap();
        assert!(
            image.tables["invites"].rows[0]
                .iter()
                .any(|cell| matches!(cell, Cell::Blob(bytes) if bytes.len() > token.len()))
        );
        assert!(!serde_json::to_string(&image).unwrap().contains(token));
        auth.revoke_invite(
            operator,
            space,
            id,
            ("invite-revoke", "invites.revoke", "x"),
        )
        .unwrap();
        witnessed();
        auth.create_invite(
            operator,
            space,
            "editor".into(),
            None,
            ("invite-other", "invites.create", "x"),
        )
        .unwrap();
        witnessed();
        let invites = auth.list_invites(operator, space, None).unwrap();
        let token = invites["invites"]
            .as_array()
            .unwrap()
            .iter()
            .find_map(|row| row["link"].as_str())
            .unwrap()
            .rsplit('/')
            .next()
            .unwrap();
        auth.redeem_invite(person, token.into(), ("invite-use", "invites.redeem", "x"))
            .unwrap();
        witnessed();
        auth.disable_person(operator, person, ("person-disable", "people.disable", "x"))
            .unwrap();
        witnessed();
    })
    .await
    .unwrap();
}
