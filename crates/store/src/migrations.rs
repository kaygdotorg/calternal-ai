//! Forward-only SQLite schema changes shared by the durable stores.

use rusqlite::{Connection, Result, Transaction, TransactionBehavior, ffi};

/// A store's first shipped schema and the consecutive changes after it.
#[derive(Clone, Copy)]
pub struct Schema<'a> {
    name: &'a str,
    baseline: i64,
    initial: &'a [&'a str],
    changes: &'a [&'a str],
}

impl<'a> Schema<'a> {
    /// Declare the schema at its first supported version. Opening an unversioned
    /// database applies every initial batch. Each subsequent batch adds one version.
    /// Use SQL without transaction controls. Cost is constant and allocation-free.
    pub const fn new(
        name: &'a str,
        baseline: i64,
        initial: &'a [&'a str],
        changes: &'a [&'a str],
    ) -> Self {
        assert!(baseline > 0, "schema versions start at one");
        Self {
            name,
            baseline,
            initial,
            changes,
        }
    }

    /// Version written after all declared batches. No I/O.
    pub const fn version(self) -> i64 {
        self.baseline + self.changes.len() as i64
    }
}

/// Apply a store's pending changes in one immediate transaction. Existing
/// current stores take one pragma read and no write lock. An older supported
/// database is rechecked under the write lock. Unknown versions and failing
/// batches leave the schema and version unchanged. Run on the store's writer.
pub fn migrate(conn: &Connection, schema: Schema<'_>) -> Result<()> {
    let mut have = version(conn)?;
    check(have, schema)?;
    if have == schema.version() {
        return Ok(());
    }
    let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;
    have = version(&tx)?;
    check(have, schema)?;
    if have == 0 {
        for batch in schema.initial {
            tx.execute_batch(batch)?;
        }
        have = schema.baseline;
        tx.pragma_update(None, "user_version", have)?;
    }
    for batch in &schema.changes[(have - schema.baseline) as usize..] {
        tx.execute_batch(batch)?;
        have += 1;
        tx.pragma_update(None, "user_version", have)?;
    }
    tx.commit()
}

fn version(conn: &Connection) -> Result<i64> {
    conn.pragma_query_value(None, "user_version", |row| row.get(0))
}

fn check(have: i64, schema: Schema<'_>) -> Result<()> {
    if have != 0 && !(schema.baseline..=schema.version()).contains(&have) {
        return Err(rusqlite::Error::SqliteFailure(
            ffi::Error::new(ffi::SQLITE_MISMATCH),
            Some(format!(
                "{} schema version {have} is outside the supported range {}–{}",
                schema.name,
                schema.baseline,
                schema.version(),
            )),
        ));
    }
    Ok(())
}
