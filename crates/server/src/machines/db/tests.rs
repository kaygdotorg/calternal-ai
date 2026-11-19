use super::*;
use calternal_store::migrations::{Schema, migrate_release};

#[test]
fn shared_execution_migrates_from_harness_leases() {
    let conn = Connection::open_in_memory().unwrap();
    let previous = Schema::new(
        "machines.db",
        6,
        &[SCHEMA],
        &[(7, TITLE_LEASES), (8, ADD_HARNESS)],
    );
    migrate_release(&conn, previous, false).unwrap();
    let space = SpaceId::new();
    let actor = AccountId::new();
    let lease = Uuid::new_v4();
    conn.execute(
        "INSERT INTO leases (id, space, account, thread, fence, state, created_ms, updated_ms, worker) VALUES (?1, ?2, ?3, ?4, 0, 'waiting', 1, 1, 0)",
        params![lease.to_string(), space.to_string(), actor.to_string(), Uuid::new_v4().to_string()],
    ).unwrap();
    migrate_release(&conn, upgrade_schema(), true).unwrap();
    assert_eq!(
        calternal_store::migrations::inspect(&conn, previous).unwrap(),
        upgrade_schema().version()
    );
    assert_eq!(
        super::lease(&conn, lease).unwrap().unwrap().shared_sponsor,
        None
    );
    let stored: i64 = conn
        .query_row("SELECT count(*) FROM space_execution", [], |row| row.get(0))
        .unwrap();
    assert_eq!(stored, 0);
}

#[test]
fn harness_lease_migration_preserves_existing_fences_and_writes_scoped_profiles() {
    use calternal_machine::protocol::{HarnessSandbox, ProviderLoginScope};

    let conn = Connection::open_in_memory().unwrap();
    let previous = Schema::new("machines.db", 6, &[SCHEMA], &[(7, TITLE_LEASES)]);
    migrate_release(&conn, previous, false).unwrap();
    let (id, space, account, thread) = (
        Uuid::new_v4(),
        SpaceId::new(),
        AccountId::new(),
        Uuid::new_v4(),
    );
    conn.execute(
        "INSERT INTO leases
         (id, space, account, thread, image, worker, fence, state, created_ms, updated_ms)
         VALUES (?1, ?2, ?3, ?4, 'fixture-image', 0, 11, 'running', 1, 1)",
        params![
            id.to_string(),
            space.to_string(),
            account.to_string(),
            thread.to_string(),
        ],
    )
    .unwrap();

    migrate_release(&conn, upgrade_schema(), true).unwrap();
    let prior = lease(&conn, id).unwrap().unwrap();
    assert_eq!(prior.fence, 11);
    assert_eq!(prior.image.as_deref(), Some("fixture-image"));
    assert_eq!(prior.harness, None);
    let harness = HarnessSandbox {
        provider_id: "fixture-harness".into(),
        login_scope: ProviderLoginScope::Space(space),
        workspace_read_only: true,
        disposable_profile: false,
        legacy_profile: false,
        guest_image_digest: None,
    };
    let managed = LeaseRow {
        id: Uuid::new_v4(),
        thread: Uuid::new_v4(),
        harness: Some(harness.clone()),
        ..prior
    };
    put_lease(&conn, &managed).unwrap();
    assert_eq!(
        lease(&conn, managed.id).unwrap().unwrap().harness,
        Some(harness)
    );
}

#[test]
fn title_leases_migrate_with_durable_classification_for_both_writers() {
    let mut conn = Connection::open_in_memory().unwrap();
    let previous = Schema::new("machines.db", 6, &[SCHEMA], &[]);
    migrate_release(&conn, previous, false).unwrap();
    let existing = LeaseRow {
        id: Uuid::new_v4(),
        space: SpaceId::new(),
        account: AccountId::new(),
        shared_sponsor: None,
        shared_tools: None,
        thread: Uuid::new_v4(),
        worker: false,
        background_title: false,
        image: None,
        harness: None,
        machine: None,
        last_machine: None,
        pinned_machine: None,
        fence: 0,
        state: "waiting".into(),
        reason: None,
        created_ms: 1,
    };
    conn.execute(
        "INSERT INTO leases
         (id, space, account, thread, worker, fence, state, created_ms, updated_ms)
         VALUES (?1, ?2, ?3, ?4, 0, 0, 'waiting', 1, 1)",
        params![
            existing.id.to_string(),
            existing.space.to_string(),
            existing.account.to_string(),
            existing.thread.to_string()
        ],
    )
    .unwrap();

    let schema = upgrade_schema();
    migrate_release(&conn, schema, true).unwrap();
    assert_eq!(
        calternal_store::migrations::inspect(&conn, previous).unwrap(),
        schema.version()
    );
    assert!(!lease(&conn, existing.id).unwrap().unwrap().background_title);

    let title = LeaseRow {
        id: Uuid::new_v4(),
        thread: Uuid::new_v4(),
        background_title: true,
        ..existing
    };
    put_title(&mut conn, &title).unwrap();
    let reread = lease(&conn, title.id).unwrap().unwrap();
    assert!(reread.background_title);
    assert!(!reread.worker);
    let legacy_worker: bool = conn
        .query_row(
            "SELECT worker FROM leases WHERE id=?1",
            [title.id.to_string()],
            |r| r.get(0),
        )
        .unwrap();
    assert!(legacy_worker);
}
