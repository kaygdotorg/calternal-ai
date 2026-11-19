//! Private per-package state with compare-and-set and bounded retry receipts.

use crate::{Content, Error, Result, SpaceId, types::internal};
use rusqlite::{OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const MAX_KEYS: i64 = 128;
const MAX_RECEIPTS: i64 = 16_384;
const MAX_STATE_BYTES: usize = 64 * 1024;
pub(crate) const MIGRATION: &str = include_str!("plugin_state.sql");

/// Versioned state owned by one package in a space.
#[derive(Debug, Serialize, Deserialize)]
pub struct PluginStateRecord {
    /// Host revision used for compare-and-set, as an exact decimal number.
    pub revision: String,
    /// Package-defined positive state-format version.
    pub schema: u32,
    /// Bounded package-owned JSON.
    pub value: Value,
}

/// Replace one package-owned state record under a revision fence.
pub struct PluginStateWrite<'a> {
    /// Own package ID resolved from the invocation, never from worker input.
    pub package: &'a str,
    /// State key in the package's namespace.
    pub key: &'a str,
    /// Previously observed revision. None asserts absence.
    pub base: Option<i64>,
    /// Package-defined state-format version.
    pub schema: u32,
    /// Replacement JSON value.
    pub value: &'a Value,
    /// Stable request identity derived from the admitted invocation.
    pub request: &'a str,
    /// Digest of the requested key, base, format, and replacement.
    pub fingerprint: &'a str,
    /// Canonical outer operation fingerprint, when a caller receipt is required.
    pub outer_fingerprint: Option<&'a str>,
}

fn validate_key(key: &str) -> Result<()> {
    if !(1..=128).contains(&key.len())
        || !key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b':' | b'_' | b'-'))
    {
        return Err(Error::invalid_input("invalid package state key"));
    }
    Ok(())
}

impl Content {
    /// Read one bounded own-package state value. The caller checks package and
    /// space authorization. One indexed lookup on a warm space.
    pub fn plugin_state(
        &self,
        space: SpaceId,
        package: &str,
        key: &str,
    ) -> Result<Option<PluginStateRecord>> {
        validate_key(key)?;
        self.with_space(space, |space| {
            let row: Option<(i64, u32, String)> = space
                .conn
                .query_row(
                    "SELECT revision,schema,value FROM plugin_state WHERE package=?1 AND key=?2",
                    params![package, key],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()
                .map_err(internal)?;
            row.map(|(revision, schema, value)| {
                Ok(PluginStateRecord {
                    revision: revision.to_string(),
                    schema,
                    value: serde_json::from_str(&value).map_err(internal)?,
                })
            })
            .transpose()
        })
    }

    /// Resolve a committed request's revision before re-invoking its worker.
    /// One indexed space-local read. A different outer request is rejected.
    pub fn plugin_state_receipt(
        &self,
        space: SpaceId,
        package: &str,
        request: &str,
        outer_fingerprint: &str,
    ) -> Result<Option<String>> {
        let retention = i64::try_from(self.receipt_retention().as_millis()).unwrap_or(i64::MAX);
        let cutoff = calternal_types::now_ms().saturating_sub(retention);
        self.with_space(space, |space| {
            let row: Option<(Option<String>, i64)> = space
                .conn
                .query_row(
                    "SELECT outer_fingerprint,revision FROM plugin_state_receipts
                 WHERE package=?1 AND request=?2 AND committed_ms>=?3",
                    params![package, request, cutoff],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()
                .map_err(internal)?;
            match row {
                Some((Some(saved), revision)) if saved == outer_fingerprint => {
                    Ok(Some(revision.to_string()))
                }
                Some(_) => Err(Error::request_reused()),
                None => Ok(None),
            }
        })
    }

    /// Compare-and-set one record and commit its retry receipt atomically.
    /// Fixed state and receipt bounds prevent unbounded reader allocation.
    /// All validation runs before commit. The caller checks package authority.
    /// Cost: indexed queries and one FULL transaction under the space lock.
    pub fn write_plugin_state(
        &self,
        space: SpaceId,
        write: PluginStateWrite<'_>,
    ) -> Result<String> {
        validate_key(write.key)?;
        let value = serde_json::to_string(write.value).map_err(internal)?;
        if value.len() > MAX_STATE_BYTES || write.schema == 0 || write.request.len() > 512 {
            return Err(Error::invalid_input(
                "package state exceeds supported bounds",
            ));
        }
        let now = calternal_types::now_ms();
        let cutoff = now.saturating_sub(
            i64::try_from(self.receipt_retention().as_millis()).unwrap_or(i64::MAX),
        );
        self.with_space(space, |space| {
            let conn = &mut space.conn;
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(internal)?;
            tx.execute("DELETE FROM plugin_state_receipts WHERE package=?1 AND committed_ms<?2",
                       params![write.package,cutoff]).map_err(internal)?;
            let replay: Option<(String,i64)> = tx.query_row(
                "SELECT fingerprint,revision FROM plugin_state_receipts WHERE package=?1 AND request=?2",
                params![write.package,write.request], |row| Ok((row.get(0)?,row.get(1)?)),
            ).optional().map_err(internal)?;
            if let Some((fingerprint, revision)) = replay {
                if fingerprint != write.fingerprint {
                    return Err(Error::conflict("package state request already used"));
                }
                return Ok(revision.to_string());
            }
            let current: Option<i64> = tx.query_row(
                "SELECT revision FROM plugin_state WHERE package=?1 AND key=?2",
                params![write.package,write.key], |row| row.get(0),
            ).optional().map_err(internal)?;
            if current != write.base {
                return Err(Error::conflict("package state revision changed"));
            }
            let receipts: i64 = tx.query_row("SELECT count(*) FROM plugin_state_receipts WHERE package=?1",
                                [write.package], |row| row.get(0)).map_err(internal)?;
            let full_keys = current.is_none() && tx.query_row("SELECT count(*)>=?2 FROM plugin_state WHERE package=?1",
                                params![write.package,MAX_KEYS], |row| row.get::<_,bool>(0)).map_err(internal)?;
            if receipts >= MAX_RECEIPTS || full_keys {
                return Err(Error::unavailable("package state capacity is full"));
            }
            let revision = current.unwrap_or(0).checked_add(1)
                .ok_or_else(|| Error::unavailable("package state revision overflow"))?;
            tx.execute(
                "INSERT INTO plugin_state VALUES(?1,?2,?3,?4,?5) ON CONFLICT(package,key)
                 DO UPDATE SET revision=excluded.revision,schema=excluded.schema,value=excluded.value",
                params![write.package,write.key,revision,write.schema,value],
            ).map_err(internal)?;
            tx.execute("INSERT INTO plugin_state_receipts VALUES(?1,?2,?3,?4,?5,?6)",
                       params![write.package,write.request,write.fingerprint,write.outer_fingerprint,revision,now]).map_err(internal)?;
            tx.commit().map_err(internal)?;
            Ok(revision.to_string())
        })
    }
}
