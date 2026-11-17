//! Machine authority: which machines may run code for whom.
//!
//! Enrolment codes, enrolled machines (with revocation) and pool grants decide who
//! may run code where, so they are security changes: they live in `auth.db` and
//! commit through the same recovery barrier as accounts and memberships (intent,
//! commit and witness, then acknowledgement). Placement state (leases, draining)
//! is not authority and stays with the host's machine service.
//!
//! An owner of `None` is the operator's pool. Every call runs one job on the auth
//! worker; callers must be on a blocking thread, never an async worker.

use crate::spaces::commit;
use crate::token::{hash32, new_setup_token, normalize_setup_token};
use crate::{Error, Result};
use calternal_types::AccountId;
use rusqlite::{OptionalExtension, TransactionBehavior, params};
use std::time::Duration;
use uuid::Uuid;

/// One enrolled machine as authority sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineRecord {
    /// Machine UUID, assigned at enrolment.
    pub id: Uuid,
    /// Owning account, or `None` for an operator pool machine.
    pub owner: Option<AccountId>,
    /// Raw Ed25519 public key the machine signs sessions with.
    pub public_key: Vec<u8>,
    /// Label chosen at enrolment.
    pub name: String,
    /// Operating system reported at enrolment.
    pub os: String,
    /// CPU architecture reported at enrolment.
    pub arch: String,
    /// A revoked machine never authenticates again.
    pub revoked: bool,
}

/// Machine authority over one `auth.db` handle; clones share its worker and
/// recovery barrier. Obtain it from [`crate::spaces::Spaces::machines`].
#[derive(Clone)]
pub struct Machines {
    db: crate::db::Db,
}

fn sql(e: impl std::fmt::Display) -> Error {
    Error::internal(e)
}

fn owner_text(owner: Option<AccountId>) -> Option<String> {
    owner.map(|a| a.to_string())
}

fn record(
    r: &rusqlite::Row<'_>,
) -> rusqlite::Result<(
    String,
    Option<String>,
    Vec<u8>,
    String,
    String,
    String,
    bool,
)> {
    Ok((
        r.get(0)?,
        r.get(1)?,
        r.get(2)?,
        r.get(3)?,
        r.get(4)?,
        r.get(5)?,
        r.get(6)?,
    ))
}

fn decode(
    (id, owner, public_key, name, os, arch, revoked): (
        String,
        Option<String>,
        Vec<u8>,
        String,
        String,
        String,
        bool,
    ),
) -> Result<MachineRecord> {
    Ok(MachineRecord {
        id: Uuid::parse_str(&id).map_err(sql)?,
        owner: owner
            .map(|o| AccountId::parse(&o).map_err(sql))
            .transpose()?,
        public_key,
        name,
        os,
        arch,
        revoked,
    })
}

impl Machines {
    pub(crate) fn new(db: crate::db::Db) -> Self {
        Self { db }
    }

    /// Issue a one-time enrolment code for `owner`, valid for `ttl`, storing only
    /// its hash; codes expired for a day are deleted in the same transaction.
    /// The code uses the setup-token format (52 characters in groups of four;
    /// dashes and case ignored). Returns the code and its expiry in Unix ms.
    /// Cost: one protected transaction (recovery publication when configured).
    pub fn issue_code(&self, owner: Option<AccountId>, ttl: Duration) -> Result<(String, i64)> {
        let code = new_setup_token()?;
        let canon = normalize_setup_token(&code).ok_or_else(|| sql("bad new code"))?;
        let hash = hash32(&canon);
        let now = crate::db::now_ms();
        let expires = now.saturating_add(ttl.as_millis() as i64);
        self.db.run_blocking(move |c| {
            let tx = c
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(sql)?;
            tx.execute(
                "DELETE FROM machine_codes WHERE expires_ms < ?1",
                [now - 86_400_000],
            )
            .map_err(sql)?;
            tx.execute(
                "INSERT INTO machine_codes(hash,owner_account,created_ms,expires_ms) VALUES(?1,?2,?3,?4)",
                params![&hash[..], owner_text(owner), now, expires],
            )
            .map_err(sql)?;
            commit(tx)
        })?;
        Ok((code, expires))
    }

    /// Use a code and enrol the machine in one protected transaction. `None` when
    /// the code is unknown, used or expired; a code works exactly once even under
    /// races. Cost: one protected transaction.
    pub fn redeem_code(
        &self,
        hash: [u8; 32],
        public_key: Vec<u8>,
        name: String,
        os: String,
        arch: String,
    ) -> Result<Option<MachineRecord>> {
        self.db.run_blocking(move |c| {
            let tx = c
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(sql)?;
            let now = crate::db::now_ms();
            let owner: Option<Option<String>> = tx
                .query_row(
                    "UPDATE machine_codes SET used_ms=?2 WHERE hash=?1 AND used_ms IS NULL AND expires_ms>?2 RETURNING owner_account",
                    params![&hash[..], now],
                    |r| r.get(0),
                )
                .optional()
                .map_err(sql)?;
            let Some(owner) = owner else {
                return Ok(None);
            };
            let row = MachineRecord {
                id: Uuid::new_v4(),
                owner: owner
                    .map(|o| AccountId::parse(&o).map_err(sql))
                    .transpose()?,
                public_key,
                name,
                os,
                arch,
                revoked: false,
            };
            tx.execute(
                "INSERT INTO machines(id,owner_account,public_key,name,os,arch,enrolled_ms) VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![row.id.to_string(), owner_text(row.owner), row.public_key, row.name, row.os, row.arch, now],
            )
            .map_err(sql)?;
            commit(tx)?;
            Ok(Some(row))
        })
    }

    /// Mark a machine revoked; false when no such machine exists. Cost: one
    /// protected transaction.
    pub fn revoke(&self, id: Uuid) -> Result<bool> {
        self.db.run_blocking(move |c| {
            let tx = c
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(sql)?;
            let changed = tx
                .execute(
                    "UPDATE machines SET revoked_ms=coalesce(revoked_ms,?2) WHERE id=?1",
                    params![id.to_string(), crate::db::now_ms()],
                )
                .map_err(sql)?;
            commit(tx)?;
            Ok(changed == 1)
        })
    }

    /// Grant or withdraw an existing account's use of pool machines. Cost: one
    /// protected transaction.
    pub fn set_pool_grant(&self, account: AccountId, granted: bool) -> Result<()> {
        self.db.run_blocking(move |c| {
            let tx = c
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(sql)?;
            if granted {
                tx.execute(
                    "INSERT OR IGNORE INTO machine_pool_grants(account,granted_ms) VALUES(?1,?2)",
                    params![account.to_string(), crate::db::now_ms()],
                )
            } else {
                tx.execute(
                    "DELETE FROM machine_pool_grants WHERE account=?1",
                    [account.to_string()],
                )
            }
            .map_err(sql)?;
            commit(tx)
        })
    }

    /// Every enrolled machine, revoked ones included. Cost: one read job.
    pub fn list(&self) -> Result<Vec<MachineRecord>> {
        self.db.run_blocking(|c| {
            let mut st = c
                .prepare("SELECT id,owner_account,public_key,name,os,arch,revoked_ms IS NOT NULL FROM machines ORDER BY enrolled_ms,id")
                .map_err(sql)?;
            let rows = st.query_map([], record).map_err(sql)?;
            rows.map(|r| decode(r.map_err(sql)?)).collect()
        })
    }

    /// Accounts allowed to use pool machines. Cost: one read job.
    pub fn pool_grants(&self) -> Result<Vec<AccountId>> {
        self.db.run_blocking(|c| {
            let mut st = c
                .prepare("SELECT account FROM machine_pool_grants")
                .map_err(sql)?;
            let rows = st.query_map([], |r| r.get::<_, String>(0)).map_err(sql)?;
            rows.map(|r| AccountId::parse(&r.map_err(sql)?).map_err(sql))
                .collect()
        })
    }
}
