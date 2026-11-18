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
fn advances_in_order_and_preserves_rows() {
    let db = Connection::open_in_memory().unwrap();
    migrate(&db, Schema::new("test", 4, BASE, &[])).unwrap();
    db.execute("INSERT INTO records VALUES (1, 'kept')", [])
        .unwrap();
    let steps = &[ADD, "CREATE INDEX records_archived ON records(archived)"];
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
    let steps = &[ADD, "CREATE INDEX invalid ON missing_table(id)"];
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
