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

mod colocated;
mod resources;
pub use calternal_types::ResourceOwner;

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
    /// Owner permitted unisolated execution; pool machines always stay sandboxed.
    pub direct: bool,
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
        r.get(7)?,
    ))
}

fn decode(
    (id, owner, public_key, name, os, arch, revoked, direct): (
        String,
        Option<String>,
        Vec<u8>,
        String,
        String,
        String,
        bool,
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
        direct,
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
        direct: bool,
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
            if direct && owner.is_none() { return Err(Error::new(crate::ErrorCode::Forbidden,"pool machines require a sandbox")); }
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
                direct,
            };
            tx.execute(
                "INSERT INTO machines(id,owner_account,public_key,name,os,arch,enrolled_ms,direct) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                params![row.id.to_string(), owner_text(row.owner), row.public_key, row.name, row.os, row.arch, now, direct],
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

    /// Change isolation policy only for the enrolled owner. Pool machines refuse
    /// direct execution. One protected transaction; call from a blocking task.
    pub fn set_direct(&self, account: AccountId, machine: Uuid, direct: bool) -> Result<()> {
        self.db.run_blocking(move |c| {
            let tx=c.transaction_with_behavior(TransactionBehavior::Immediate).map_err(sql)?;
            let n=tx.execute("UPDATE machines SET direct=?1 WHERE id=?2 AND owner_account=?3 AND revoked_ms IS NULL",params![direct,machine.to_string(),account.to_string()]).map_err(sql)?;
            if n!=1 { return Err(Error::new(crate::ErrorCode::Forbidden,"only the machine owner may change isolation")); }
            commit(tx)
        })
    }

    /// Authorize explicit removal of one retained thread workspace. This does not
    /// delete files: delivery is fenced by this exact lease. Bounded to 1024
    /// outstanding authorizations per account; repeated requests replace one row.
    /// One protected transaction on the existing auth worker, called blocking.
    pub fn authorize_discard(&self, account: AccountId, thread: Uuid, lease: Uuid) -> Result<()> {
        self.db.run_blocking(move |c| {
            let tx=c.transaction_with_behavior(TransactionBehavior::Immediate).map_err(sql)?;
            let count:i64=tx.query_row("SELECT count(*) FROM agent_discards WHERE account=?1 AND thread!=?2",params![account.to_string(),thread.to_string()],|r|r.get(0)).map_err(sql)?;
            if count>=1024{return Err(Error::invalid("too many retained discard authorizations"))}
            let changed=tx.execute("UPDATE agent_discards SET lease=?1,authorized_ms=?2 WHERE account=?3 AND thread=?4",params![lease.to_string(),calternal_types::now_ms(),account.to_string(),thread.to_string()]).map_err(sql)?;
            if changed==0 {tx.execute("INSERT INTO agent_discards VALUES(?1,?2,?3,?4)",params![account.to_string(),thread.to_string(),lease.to_string(),calternal_types::now_ms()]).map_err(sql)?;}

            commit(tx)
        })
    }
    /// Check a pending delivery against protected authority. One indexed read;
    /// absence or any changed lease refuses deletion. Call blocking.
    pub fn discard_authorized(
        &self,
        account: AccountId,
        thread: Uuid,
        lease: Uuid,
    ) -> Result<bool> {
        self.db.run_blocking(move |c|c.query_row("SELECT EXISTS(SELECT 1 FROM agent_discards WHERE account=?1 AND thread=?2 AND lease=?3)",params![account.to_string(),thread.to_string(),lease.to_string()],|r|r.get(0)).map_err(sql))
    }

    /// Every enrolled machine, revoked ones included. Cost: one read job.
    pub fn list(&self) -> Result<Vec<MachineRecord>> {
        self.db.run_blocking(|c| {
            let mut st = c
                .prepare("SELECT id,owner_account,public_key,name,os,arch,revoked_ms IS NOT NULL,direct FROM machines ORDER BY enrolled_ms,id")
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
