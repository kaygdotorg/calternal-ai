//! Migration boundaries exercised through the shared SQLite runner.
use calternal_store::migrations::{Schema, migrate};
use rusqlite::Connection;

const BASE: &[&str] = &["CREATE TABLE records(id INTEGER PRIMARY KEY, text TEXT NOT NULL)"];
const ADD: &str = "ALTER TABLE records ADD COLUMN archived INTEGER NOT NULL DEFAULT 0";

fn version(db: &Connection) -> i64 {
    db.pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap()
}

#[test]
#[should_panic(expected = "migration versions must be consecutive")]
fn rejects_two_steps_claiming_the_same_version() {
    Schema::new("test", 4, BASE, &[(5, ADD), (5, "SELECT 1")]);
}

#[test]
#[should_panic(expected = "migration versions must be consecutive")]
fn rejects_a_gap_after_the_baseline() {
    Schema::new("test", 4, BASE, &[(6, ADD)]);
}

#[test]
fn advances_in_order_and_preserves_rows() {
    let db = Connection::open_in_memory().unwrap();
    migrate(&db, Schema::new("test", 4, BASE, &[])).unwrap();
    db.execute("INSERT INTO records VALUES (1, 'kept')", [])
        .unwrap();
    let steps = &[
        (5, ADD),
        (6, "CREATE INDEX records_archived ON records(archived)"),
    ];
    migrate(&db, Schema::new("test", 4, BASE, steps)).unwrap();
    migrate(&db, Schema::new("test", 4, BASE, steps)).unwrap();
    assert_eq!(version(&db), 6);
    assert_eq!(
        db.query_row("SELECT text,archived FROM records", [], |r| Ok((
            r.get::<_, String>(0)?,
            r.get::<_, i64>(1)?
        )))
        .unwrap(),
        ("kept".into(), 0)
    );
}

#[test]
fn failed_step_rolls_back_schema_and_version() {
    let db = Connection::open_in_memory().unwrap();
    migrate(&db, Schema::new("test", 4, BASE, &[])).unwrap();
    let steps = &[(5, ADD), (6, "CREATE INDEX invalid ON missing_table(id)")];
    assert!(migrate(&db, Schema::new("test", 4, BASE, steps)).is_err());
    assert_eq!(version(&db), 4);
    assert!(db.prepare("SELECT archived FROM records").is_err());
}

#[test]
fn refuses_a_newer_database_without_creating_tables() {
    let db = Connection::open_in_memory().unwrap();
    db.pragma_update(None, "user_version", 9).unwrap();
    assert!(migrate(&db, Schema::new("test", 4, BASE, &[])).is_err());
    assert_eq!(version(&db), 9);
    assert!(db.prepare("SELECT * FROM records").is_err());
}

#[test]
fn an_explicitly_compatible_upgrade_admits_the_previous_schema_writer() {
    use calternal_store::migrations::migrate_release;
    let db = Connection::open_in_memory().unwrap();
    let old = Schema::new("test", 4, BASE, &[]);
    migrate(&db, old).unwrap();
    let steps = &[(5, ADD)];
    migrate_release(&db, Schema::new("test", 4, BASE, steps), true).unwrap();
    migrate(&db, old).unwrap();
    db.execute(
        "INSERT INTO records (id,text) VALUES (2,'from older writer')",
        [],
    )
    .unwrap();
    assert_eq!(version(&db), 5);
}

#[test]
fn incompatible_release_leaves_no_exception_for_an_older_writer() {
    use calternal_store::migrations::migrate_release;
    let db = Connection::open_in_memory().unwrap();
    let old = Schema::new("test", 4, BASE, &[]);
    migrate(&db, old).unwrap();
    let steps = &[(5, ADD)];
    migrate_release(&db, Schema::new("test", 4, BASE, steps), false).unwrap();
    assert!(migrate(&db, old).is_err());
}
