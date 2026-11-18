//! Forward-only SQLite schema changes shared by the durable stores.

use rusqlite::{
    Connection, OptionalExtension, Result, Transaction, TransactionBehavior, ffi, params,
};

const COMPATIBLE: &str = "_calternal_schema_compatibility";

/// A store's first shipped schema and the consecutive changes after it.
#[derive(Clone, Copy)]
pub struct Schema<'a> {
    name: &'a str,
    baseline: i64,
    initial: &'a [&'a str],
    changes: &'a [(i64, &'a str)],
}

impl<'a> Schema<'a> {
    /// Declare the schema at its first supported version. Opening an unversioned
    /// database applies every initial batch. Subsequent entries pair an explicit
    /// target version with its SQL. Reject duplicates and gaps, starting at
    /// baseline plus one. Use SQL without transaction controls. Validation is
    /// allocation-free and runs at compile time for a constant descriptor.
    pub const fn new(
        name: &'a str,
        baseline: i64,
        initial: &'a [&'a str],
        changes: &'a [(i64, &'a str)],
    ) -> Self {
        assert!(baseline > 0, "schema versions start at one");
        let mut index = 0;
        let mut expected = baseline;
        while index < changes.len() {
            expected = expected.checked_add(1).expect("migration version overflow");
            assert!(
                changes[index].0 == expected,
                "migration versions must be consecutive"
            );
            index += 1;
        }
        Self {
            name,
            baseline,
            initial,
            changes,
        }
    }

    /// Version written after all declared batches. No I/O.
    pub const fn version(self) -> i64 {
        match self.changes.last() {
            Some(&(version, _)) => version,
            None => self.baseline,
        }
    }
}

/// Apply a store's pending changes in one immediate transaction. Existing
/// current stores take one pragma read and no write lock. An older supported
/// database is rechecked under the write lock. Unknown versions and failing
/// batches leave the schema and version unchanged. Run on the store's writer.
pub fn migrate(conn: &Connection, schema: Schema<'_>) -> Result<()> {
    apply(conn, schema, false)
}

/// Apply the declared steps under the release coordinator's snapshot and locks.
/// Additive approval is granted by the cross-release gate. It records the exact
/// previous schema as readable by its writer, atomically with the migration.
pub fn migrate_release(conn: &Connection, schema: Schema<'_>, additive: bool) -> Result<()> {
    apply(conn, schema, additive)
}

/// Retire a rollback marker that would admit a writer older than the newly
/// written representation. Call in the same transaction as the new data.
pub fn require_writer_version(tx: &Transaction<'_>, minimum: i64) -> Result<()> {
    let exists = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name=?1 AND type='table')",
        [COMPATIBLE],
        |row| row.get::<_, bool>(0),
    )?;
    if exists {
        tx.execute(
            &format!("DELETE FROM {COMPATIBLE} WHERE older < ?1"),
            [minimum],
        )?;
    }
    Ok(())
}

/// Read the installed version and reject an unsupported source. One pragma on
/// the normal path. Used before a deployment chooses files to snapshot.
pub fn inspect(conn: &Connection, schema: Schema<'_>) -> Result<i64> {
    let have = version(conn)?;
    check(conn, have, schema)?;
    Ok(have)
}

fn apply(conn: &Connection, schema: Schema<'_>, additive: bool) -> Result<()> {
    if inspect(conn, schema)? >= schema.version() {
        return Ok(());
    }
    let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;
    let previous = inspect(&tx, schema)?;
    if previous >= schema.version() {
        return Ok(());
    }
    let mut have = previous;
    if have == 0 {
        for batch in schema.initial {
            tx.execute_batch(batch)?;
        }
        have = schema.baseline;
        tx.pragma_update(None, "user_version", have)?;
    }
    for &(version, batch) in &schema.changes[(have - schema.baseline) as usize..] {
        tx.execute_batch(batch)?;
        have = version;
        tx.pragma_update(None, "user_version", have)?;
    }
    if additive && previous > 0 {
        tx.execute_batch(&format!(
            "CREATE TABLE IF NOT EXISTS {COMPATIBLE}(
             singleton INTEGER PRIMARY KEY CHECK(singleton=1),
             format TEXT NOT NULL, older INTEGER NOT NULL, newer INTEGER NOT NULL) STRICT;"
        ))?;
        tx.execute(
            &format!("INSERT OR REPLACE INTO {COMPATIBLE} VALUES(1,?1,?2,?3)"),
            params![schema.name, previous, have],
        )?;
    }
    tx.commit()
}

fn version(conn: &Connection) -> Result<i64> {
    conn.pragma_query_value(None, "user_version", |row| row.get(0))
}

fn check(conn: &Connection, have: i64, schema: Schema<'_>) -> Result<()> {
    if have == 0 || (schema.baseline..=schema.version()).contains(&have) {
        return Ok(());
    }
    if have > schema.version() && compatible(conn, have, schema)? {
        return Ok(());
    }
    Err(rusqlite::Error::SqliteFailure(
        ffi::Error::new(ffi::SQLITE_MISMATCH),
        Some(format!(
            "{} schema version {have} is outside the supported range {}–{}",
            schema.name,
            schema.baseline,
            schema.version(),
        )),
    ))
}

fn compatible(conn: &Connection, have: i64, schema: Schema<'_>) -> Result<bool> {
    let has_table = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name=?1 AND type='table')",
        [COMPATIBLE],
        |row| row.get::<_, bool>(0),
    )?;
    if !has_table {
        return Ok(false);
    }
    conn.query_row(
        &format!(
            "SELECT 1 FROM {COMPATIBLE} WHERE singleton=1 AND format=?1 AND older=?2 AND newer=?3"
        ),
        params![schema.name, schema.version(), have],
        |_| Ok(true),
    )
    .optional()
    .map(|found| found.unwrap_or(false))
}
