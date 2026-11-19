use super::*;
use crate::spaces::{DEFAULT_RECEIPT_RETENTION, Spaces};
use crate::test_support::TempDir;
use std::sync::mpsc;
use uuid::Uuid;

fn enroll(spaces: &Spaces) -> Uuid {
    let machines = spaces.machines();
    let (code, _) = machines.issue_code(None, Duration::from_secs(30)).unwrap();
    let code = crate::token::normalize_setup_token(&code).unwrap();
    machines
        .redeem_code(
            crate::token::hash32(&code),
            vec![7; 32],
            "fixture".into(),
            "linux".into(),
            "x86_64".into(),
            false,
        )
        .unwrap()
        .unwrap()
        .id
}

#[test]
fn affected_space_is_fenced_before_session_write_commits() {
    let dir = TempDir::new();
    let db = Db::open(&dir.path().join("auth.db")).unwrap();
    let spaces = Spaces::from_db(db.clone(), DEFAULT_RECEIPT_RETENTION);
    let ids = spaces.seed_development().unwrap();
    let own_machine = enroll(&spaces);
    let affected = spaces.grant_revision(ids.personal, own_machine).unwrap();
    let guest = ids.guest.to_string();
    let other_space = db
        .read_blocking(move |c| {
            let id: String = c
                .query_row(
                    "SELECT id FROM spaces WHERE personal_for=?1",
                    [guest],
                    |r| r.get(0),
                )
                .map_err(sql)?;
            crate::SpaceId::parse(&id).map_err(Error::internal)
        })
        .unwrap();
    let unrelated = spaces.grant_revision(other_space, enroll(&spaces)).unwrap();
    let before = affected.current().unwrap();
    let other_before = unrelated.current().unwrap();
    let (changed, in_transaction) = mpsc::channel();
    let (release, resume) = mpsc::channel();
    let writer_db = db.clone();
    let writer = std::thread::spawn(move || {
        writer_db
            .run_blocking(move |connection| {
                let tx = connection.transaction().map_err(sql)?;
                tx.execute(
                "INSERT INTO installations(id,user_id,kind,name,token_hash,created_at,last_seen_at)
                 VALUES ('fixture',?1,'browser','Fixture',x'01',0,0)",
                [ids.owner.to_string()],
            )
            .map_err(sql)?;
                changed.send(()).unwrap();
                resume.recv_timeout(Duration::from_secs(5)).unwrap();
                tx.commit().map_err(sql)
            })
            .unwrap()
    });
    in_transaction.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(affected.current(), None);
    assert_eq!(unrelated.current(), Some(other_before));
    release.send(()).unwrap();
    writer.join().unwrap();
    db.read_blocking(|_| Ok(())).unwrap();
    assert_ne!(affected.current(), Some(before));
    assert_eq!(unrelated.current(), Some(other_before));
}

#[test]
fn role_changes_advance_only_the_selected_space() {
    let dir = TempDir::new();
    let spaces = Spaces::open(&dir.path().join("auth.db")).unwrap();
    let ids = spaces.seed_development().unwrap();
    let machine = enroll(&spaces);
    let shared = spaces.grant_revision(ids.shared, machine).unwrap();
    let personal = spaces.grant_revision(ids.personal, machine).unwrap();
    let before = shared.current().unwrap();
    let other_before = personal.current().unwrap();

    spaces
        .set_role(ids.owner, ids.shared, ids.guest, "editor")
        .unwrap();
    assert!(
        spaces
            .capabilities(ids.guest, ids.shared)
            .unwrap()
            .is_some()
    );
    assert_ne!(shared.current().unwrap(), before);
    assert_eq!(personal.current(), Some(other_before));
}

#[test]
fn machine_revocation_retires_bound_assignments() {
    let dir = TempDir::new();
    let spaces = Spaces::open(&dir.path().join("auth.db")).unwrap();
    let ids = spaces.seed_development().unwrap();
    let machine = enroll(&spaces);
    let shared = spaces.grant_revision(ids.shared, machine).unwrap();
    let personal = spaces
        .grant_revision(ids.personal, enroll(&spaces))
        .unwrap();
    let other_before = personal.current().unwrap();

    assert!(spaces.machines().revoke(machine).unwrap());
    assert_eq!(shared.current(), None);
    assert_eq!(personal.current(), Some(other_before));
    assert!(spaces.grant_revision(ids.shared, machine).is_err());
}

#[test]
fn custom_role_mutations_fence_spaces_where_the_role_is_in_use() {
    let dir = TempDir::new();
    let db = Db::open(&dir.path().join("auth.db")).unwrap();
    let spaces = Spaces::from_db(db.clone(), DEFAULT_RECEIPT_RETENTION);
    let ids = spaces.seed_development().unwrap();
    let machine = enroll(&spaces);
    let shared = spaces.grant_revision(ids.shared, machine).unwrap();
    let personal = spaces.grant_revision(ids.personal, machine).unwrap();
    let before = shared.current().unwrap();
    let other_before = personal.current().unwrap();

    db.run_blocking(|c| {
        c.execute(
            "DELETE FROM space_role_capabilities WHERE role='reader' AND capability='space.read'",
            [],
        )
        .map_err(sql)?;
        Ok(())
    })
    .unwrap();
    db.read_blocking(|_| Ok(())).unwrap();
    assert_ne!(shared.current(), Some(before));
    assert_eq!(personal.current(), Some(other_before));
}
