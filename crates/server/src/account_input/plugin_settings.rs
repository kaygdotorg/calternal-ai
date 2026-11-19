//! Owner-local plugin overrides, durable receipts and private invalidation.
use super::{Inputs, internal};
use calternal_core::PluginSettingValue;
use calternal_types::{AccountId, Error, SpaceId};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde_json::Value;
use std::time::Duration;

pub(super) const SCHEMA: &str = "
CREATE TABLE plugin_settings(
  space TEXT NOT NULL, plugin TEXT NOT NULL, key TEXT NOT NULL, value TEXT NOT NULL,
  PRIMARY KEY(space, plugin, key)
) STRICT;
CREATE TABLE plugin_setting_receipts(
  space TEXT NOT NULL, request TEXT NOT NULL, fingerprint TEXT NOT NULL,
  reply TEXT NOT NULL, expires_ms INTEGER NOT NULL, PRIMARY KEY(space, request)
) STRICT;
CREATE INDEX plugin_setting_expiry ON plugin_setting_receipts(expires_ms);
";

const MAX_VALUES: i64 = 1024;
const MAX_RECEIPTS: i64 = 4096;
const MAX_VALUE_BYTES: usize = 4096;

/// One compare-and-set request for a personal key, with its stable receipt.
pub(crate) struct PersonalEdit<'a> {
    /// Explicit space UUID after the caller passes authorization.
    pub space: SpaceId,
    /// Installed package ID.
    pub plugin: &'a str,
    /// Manifest's personal setting key.
    pub key: &'a str,
    /// Last value read by the caller.
    pub base: &'a Value,
    /// New override, or null to clear.
    pub value: &'a Value,
    /// Declared package default used for a cleared value.
    pub default: &'a Value,
    /// Request identifier supplied by the current account.
    pub request: &'a str,
    /// Fingerprint of the operation and input.
    pub fingerprint: &'a str,
    /// Configured decision-receipt window.
    pub retention: Duration,
}

fn stored(c: &Connection, space: SpaceId, plugin: &str, key: &str) -> Result<Value, Error> {
    let text = c
        .query_row(
            "SELECT value FROM plugin_settings WHERE space=?1 AND plugin=?2 AND key=?3",
            params![space.to_string(), plugin, key],
            |r| r.get::<_, String>(0),
        )
        .optional()
        .map_err(internal)?;
    text.map(|text| serde_json::from_str(&text).map_err(internal))
        .transpose()
        .map(Option::unwrap_or_default)
}

impl Inputs {
    /// Read one private override. Does not create an input DB on a default-only read.
    /// Cost: one file existence check and one indexed read when the DB exists.
    pub(crate) fn plugin_value(
        &self,
        account: AccountId,
        space: SpaceId,
        plugin: &str,
        key: &str,
    ) -> Result<Value, Error> {
        let _step = self.step(account)?;
        if !self
            .root
            .join("accounts")
            .join(account.to_string())
            .join("input.db")
            .is_file()
        {
            return Ok(Value::Null);
        }
        stored(&self.open(account)?, space, plugin, key)
    }

    /// Write a bounded personal override, reply and private event atomically.
    /// Conflicting edits keep the saved value. A replay does not publish a second event.
    /// Cost: one account-local FULL transaction with indexed reads and writes.
    pub(crate) fn edit_plugin_value(
        &self,
        account: AccountId,
        edit: PersonalEdit<'_>,
    ) -> Result<PluginSettingValue, Error> {
        let text = edit.value.to_string();
        if text.len() > MAX_VALUE_BYTES {
            return Err(Error::invalid_input("setting exceeds 4 KiB"));
        }
        let _step = self.step(account)?;
        let mut c = self.open(account)?;
        let tx = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(internal)?;
        let space = edit.space.to_string();
        let now = (self.clock)();
        if let Some((fingerprint, reply)) = tx
            .query_row(
                "SELECT fingerprint,reply FROM plugin_setting_receipts
             WHERE space=?1 AND request=?2 AND expires_ms>?3",
                params![space, edit.request, now],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(internal)?
        {
            if fingerprint != edit.fingerprint {
                return Err(Error::request_reused());
            }
            return serde_json::from_str(&reply).map_err(internal);
        }
        let current = stored(&tx, edit.space, edit.plugin, edit.key)?;
        if current != *edit.base {
            return Err(Error::conflict(
                "setting changed; read its current value before editing",
            ));
        }
        tx.execute(
            "DELETE FROM plugin_setting_receipts WHERE expires_ms<=?1",
            [now],
        )
        .map_err(internal)?;
        let receipts: i64 = tx
            .query_row("SELECT count(*) FROM plugin_setting_receipts", [], |r| {
                r.get(0)
            })
            .map_err(internal)?;
        if receipts >= MAX_RECEIPTS {
            return Err(Error::rate_limited(
                "personal setting receipt capacity reached",
            ));
        }
        if edit.value.is_null() {
            tx.execute(
                "DELETE FROM plugin_settings WHERE space=?1 AND plugin=?2 AND key=?3",
                params![space, edit.plugin, edit.key],
            )
            .map_err(internal)?;
        } else {
            if current.is_null() {
                let count: i64 = tx
                    .query_row("SELECT count(*) FROM plugin_settings", [], |r| r.get(0))
                    .map_err(internal)?;
                if count >= MAX_VALUES {
                    return Err(Error::conflict("personal setting capacity reached"));
                }
            }
            tx.execute(
                "INSERT INTO plugin_settings VALUES(?1,?2,?3,?4)
                 ON CONFLICT(space,plugin,key) DO UPDATE SET value=excluded.value",
                params![space, edit.plugin, edit.key, text],
            )
            .map_err(internal)?;
        }
        let reply = PluginSettingValue {
            effective: if edit.value.is_null() {
                edit.default.clone()
            } else {
                edit.value.clone()
            },
            value: edit.value.clone(),
            alternatives: Vec::new(),
            writable: true,
            repair: None,
        };
        let expiry =
            now.saturating_add(i64::try_from(edit.retention.as_millis()).unwrap_or(i64::MAX));
        tx.execute(
            "INSERT INTO plugin_setting_receipts VALUES(?1,?2,?3,?4,?5)",
            params![
                space,
                edit.request,
                edit.fingerprint,
                serde_json::to_string(&reply).map_err(internal)?,
                expiry,
            ],
        )
        .map_err(internal)?;
        self.change(&tx)?;
        tx.commit().map_err(internal)?;
        self.wakes.committed(account);
        Ok(reply)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v2_account_input_keeps_its_binding_when_settings_are_added() {
        let c = Connection::open_in_memory().unwrap();
        let v2 = calternal_store::migrations::Schema::new(
            "input.db",
            2,
            &[super::super::db::SCHEMA],
            &[],
        );
        calternal_store::migrations::migrate(&c, v2).unwrap();
        c.execute("INSERT INTO binding VALUES('existing','epoch')", [])
            .unwrap();
        calternal_store::migrations::migrate(&c, super::super::db::UPGRADE_SCHEMA).unwrap();
        assert_eq!(
            c.query_row(
                "SELECT epoch FROM binding WHERE account='existing'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            "epoch"
        );
        assert_eq!(
            stored(&c, SpaceId::new(), "package", "key").unwrap(),
            Value::Null
        );
    }
}
