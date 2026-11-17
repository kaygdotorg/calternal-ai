//! Protected key fingerprints and irreversible erasure markers, never key bytes.
use crate::{Error, Result, spaces::Spaces};
use rusqlite::{OptionalExtension, params};

impl Spaces {
    /// Publish one owner's key fingerprint through the configured recovery barrier.
    /// A tombstone cannot be revived and a fingerprint cannot change. This cold
    /// lifecycle path takes one auth-worker round trip and one protected commit;
    /// ordinary secret use reads only its owner's local database.
    pub fn record_secret_key(
        &self,
        kind: &str,
        id: uuid::Uuid,
        fingerprint: [u8; 32],
        erasing: bool,
    ) -> Result<()> {
        if !matches!(kind, "account" | "space") {
            return Err(Error::internal("invalid secret owner"));
        }
        let kind = kind.to_owned();
        self.db.run_blocking(move |c| {
            let tx = c.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(Error::internal)?;
            let table = if kind=="account" {"users"} else {"spaces"};
            if !erasing && !tx.query_row(&format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE id=?1)"), [id.to_string()], |r| r.get::<_,bool>(0)).map_err(Error::internal)? {
                return Err(Error::internal("secret owner no longer exists"));
            }
            let old: Option<(Vec<u8>, bool)> = tx.query_row("SELECT fingerprint,erasing FROM owner_secret_keys WHERE kind=?1 AND owner_id=?2", params![kind,id.to_string()], |r| Ok((r.get(0)?,r.get(1)?))).optional().map_err(Error::internal)?;
            if let Some((recorded, was_erasing)) = old {
                if recorded != fingerprint || (was_erasing && !erasing) {
                    return Err(Error::internal("secret key state mismatch"));
                }
                if was_erasing != erasing {
                    tx.execute("UPDATE owner_secret_keys SET erasing=?3 WHERE kind=?1 AND owner_id=?2",
                        params![kind, id.to_string(), erasing]).map_err(Error::internal)?;
                }
            } else {
                tx.execute("INSERT INTO owner_secret_keys VALUES(?1,?2,?3,?4)",
                    params![kind,id.to_string(),&fingerprint[..],erasing]).map_err(Error::internal)?;
            }
            crate::spaces::commit(tx)
        })
    }

    /// Read all protected owner key metadata during startup reconciliation only.
    /// Cost: one auth-worker scan; output has one row per owner that held secrets.
    /// Tombstones persist because restored ciphertext must never revive an owner.
    pub fn secret_key_records(&self) -> Result<Vec<(String, uuid::Uuid, Vec<u8>, bool)>> {
        self.db.run_blocking(|c| {
            let mut s = c
                .prepare("SELECT kind,owner_id,fingerprint,erasing FROM owner_secret_keys")
                .map_err(Error::internal)?;
            let rows = s
                .query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, Vec<u8>>(2)?,
                        r.get::<_, bool>(3)?,
                    ))
                })
                .map_err(Error::internal)?;
            rows.map(|r| {
                let (k, id, f, e) = r.map_err(Error::internal)?;
                Ok((
                    k,
                    uuid::Uuid::parse_str(&id).map_err(Error::internal)?,
                    f,
                    e,
                ))
            })
            .collect()
        })
    }
}
