//! Bounded network activity records. Only normalized hostnames and port
//! numbers cross this boundary, alongside timing, authorization and byte counts.

use calternal_types::{AccountId, Error, SpaceId};
use rusqlite::{TransactionBehavior, params};
use std::{path::Path, time::Duration};

use super::db;

const DAYS: i64 = 7 * 24 * 60 * 60;
const ROWS: i64 = 10_000;

pub(super) struct Attempt<'a> {
    pub(super) account: AccountId,
    pub(super) package: &'a str,
    pub(super) revision: u32,
    pub(super) approval: u32,
    pub(super) host: &'a str,
    pub(super) port: u16,
}

fn path(appdata: &Path, space: SpaceId) -> std::path::PathBuf {
    appdata
        .join("spaces")
        .join(space.to_string())
        .join("network.db")
}

fn epoch() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}

/// Persist an admitted proxy hop before the upstream request starts. A crash
/// can leave a pending outcome. One immediate commit also enforces retention.
pub(super) fn begin(appdata: &Path, space: SpaceId, request: Attempt<'_>) -> Result<i64, Error> {
    if request.host.is_empty()
        || request.host.len() > 253
        || !request
            .host
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-.".contains(&c))
    {
        return Err(Error::invalid_input("network destination is invalid"));
    }
    let mut db = db::open(&path(appdata, space), true)?
        .ok_or_else(|| Error::unavailable("Plugin network audit is unavailable."))?;
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(db::storage)?;
    let now = epoch();
    tx.execute("DELETE FROM activity WHERE started < ?1", [now - DAYS])
        .map_err(db::storage)?;
    tx.execute(
        "INSERT INTO activity(started, actor, package, package_revision, approval,
                              class, origin, host, port, protocol, outcome)
         VALUES (?1, ?2, ?3, ?4, ?5, 'content_fetch', 'server', ?6, ?7, 'https', 'pending')",
        params![
            now,
            request.account.to_string(),
            request.package,
            request.revision,
            request.approval,
            request.host,
            request.port,
        ],
    )
    .map_err(db::storage)?;
    let id = tx.last_insert_rowid();
    tx.execute(
        "DELETE FROM activity WHERE id <= ?1",
        [id.saturating_sub(ROWS)],
    )
    .map_err(db::storage)?;
    tx.commit().map_err(db::storage)?;
    Ok(id)
}

#[derive(Clone, Copy)]
pub(super) enum Outcome {
    Ready,
    Redirect,
    Refused,
    UpstreamFailure,
}

/// Finish one admitted hop. The status is a host constant and byte counts
/// exclude HTTP headers. One indexed update, with no payload retained.
pub(super) fn finish(
    appdata: &Path,
    space: SpaceId,
    id: i64,
    outcome: Outcome,
    elapsed: Duration,
    bytes: u64,
) -> Result<(), Error> {
    let Some(db) = db::open(&path(appdata, space), false)? else {
        return Err(Error::unavailable("Plugin network audit is unavailable."));
    };
    let outcome = match outcome {
        Outcome::Ready => "ready",
        Outcome::Redirect => "redirect",
        Outcome::Refused => "refused",
        Outcome::UpstreamFailure => "upstream_failure",
    };
    db.execute(
        "UPDATE activity SET outcome = ?1, elapsed_ms = ?2, received_bytes = ?3 WHERE id = ?4",
        params![
            outcome,
            elapsed.as_millis().min(i64::MAX as u128) as i64,
            i64::try_from(bytes).unwrap_or(i64::MAX),
            id
        ],
    )
    .map_err(db::storage)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_keeps_only_host_and_outcome_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let space = SpaceId::parse("01932910-0000-7000-8000-000000000002").unwrap();
        let account = AccountId::parse("01932910-0000-7000-8000-000000000001").unwrap();
        let attempt = |host| Attempt {
            account,
            package: "fixture",
            revision: 1,
            approval: 2,
            host,
            port: 443,
        };
        assert!(begin(dir.path(), space, attempt("example.com/path?secret")).is_err());
        let id = begin(dir.path(), space, attempt("example.com")).unwrap();
        finish(
            dir.path(),
            space,
            id,
            Outcome::Ready,
            Duration::from_millis(21),
            7,
        )
        .unwrap();
        let db = db::open(&path(dir.path(), space), false).unwrap().unwrap();
        let row: (String, String, i64, i64) = db
            .query_row(
                "SELECT host, outcome, elapsed_ms, received_bytes FROM activity WHERE id = ?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(row, ("example.com".into(), "ready".into(), 21, 7));
    }
}
