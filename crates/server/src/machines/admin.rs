//! Operator actions on `<data>/system/machines.db`, behind `calternal machines …`.
//!
//! They write the database directly, so they work whether or not the server is
//! running; a running server picks the change up within one sweep tick through
//! `PRAGMA data_version` (revocation then takes effect as in
//! [`super::MachineService::revoke`]). Each call opens the file once and commits
//! at most once.

use std::path::Path;
use std::time::Duration;

use calternal_types::AccountId;
use calternal_machine::protocol::Owner;
use uuid::Uuid;

use super::{db, sql};
use calternal_types::Error;

fn open(data_dir: &Path) -> Result<rusqlite::Connection, Error> {
    db::open(&data_dir.join("system/machines.db")).map_err(sql)
}

/// Issue an enrolment code for `owner` (an account, or the pool) valid for `ttl`.
/// Returns the code and its expiry in Unix milliseconds.
pub fn issue_code(
    data_dir: &Path,
    owner: Owner,
    ttl: Duration,
) -> Result<(String, i64), Error> {
    db::new_code(&open(data_dir)?, owner, ttl)
}

/// Revoke any machine; false when it does not exist.
pub fn revoke(data_dir: &Path, machine: Uuid) -> Result<bool, Error> {
    db::revoke(&open(data_dir)?, machine).map_err(sql)
}

/// Grant or withdraw an account's use of pool machines.
pub fn pool_grant(data_dir: &Path, account: AccountId, granted: bool) -> Result<(), Error> {
    db::set_pool_grant(&open(data_dir)?, account, granted).map_err(sql)
}

/// Every enrolled machine, revoked ones included.
pub fn list(data_dir: &Path) -> Result<Vec<db::MachineRow>, Error> {
    db::machines(&open(data_dir)?).map_err(sql)
}
