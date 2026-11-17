//! Stored space authority, adapted from the source auth roles and people actions.
//!
//! All APIs block on SQLite and must run on a blocking host thread. Mutations
//! acquire an IMMEDIATE transaction before checking current capability rows.
//! State commits optionally pass through a protected recovery barrier before acknowledgement.
use crate::{AccountId, Error, ErrorCode, Result, SpaceId};
use calternal_types::{Capability, CapabilitySet};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// A space and the caller's effective role; no content or account secrets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Space {
    /// Independent stable space UUID.
    pub id: SpaceId,
    /// Display name, unrelated to identity or authority.
    pub name: String,
    /// Sole owner account; ownership changes only through transfer.
    pub owner: AccountId,
    /// Creation timestamp in Unix milliseconds.
    pub created: i64,
    /// Effective role including the owner relation.
    pub role: String,
}
/// One membership visible to a current space reader.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Member {
    /// Stable account UUID.
    pub account_id: AccountId,
    /// Effective role; the sole owner is derived from the space record.
    pub role: String,
}
/// Protected stored directory. Clones share the bounded dedicated auth worker.
/// Independent opens are serialized by SQLite's IMMEDIATE transactions.
#[derive(Clone)]
pub struct Spaces {
    pub(crate) db: crate::db::Db,
    receipt_retention: std::time::Duration,
}
/// How long a directory receipt is kept by default: seven days, the same as space
/// receipts. A retry after that runs as a new request.
pub const DEFAULT_RECEIPT_RETENTION: std::time::Duration =
    std::time::Duration::from_secs(7 * 24 * 60 * 60);
fn sql(e: impl std::fmt::Display) -> Error {
    Error::internal(e)
}
fn denied() -> Error {
    Error::new(ErrorCode::Forbidden, "no access to this space")
}
fn account(s: String) -> Result<AccountId> {
    AccountId::parse(&s).map_err(|_| sql("invalid account UUID"))
}
fn space_id(s: String) -> Result<SpaceId> {
    SpaceId::parse(&s).map_err(|_| sql("invalid space UUID"))
}
fn role(c: &Connection, actor: AccountId, space: SpaceId) -> Result<Option<String>> {
    c.query_row(
        "SELECT CASE WHEN s.owner_account=m.account_id THEN 'owner' ELSE m.role END
        FROM space_memberships m JOIN spaces s ON s.id=m.space_id
        WHERE m.account_id=?1 AND s.id=?2",
        params![actor.to_string(), space.to_string()],
        |r| r.get(0),
    )
    .optional()
    .map_err(sql)
}
fn has(c: &Connection, actor: AccountId, space: SpaceId, cap: &str) -> Result<bool> {
    c.query_row("SELECT EXISTS(SELECT 1 FROM space_memberships m JOIN spaces s ON s.id=m.space_id
        JOIN space_role_capabilities rc ON rc.role=CASE WHEN s.owner_account=m.account_id THEN 'owner' ELSE m.role END
        WHERE m.account_id=?1 AND s.id=?2 AND rc.capability=?3)", params![actor.to_string(),space.to_string(),cap], |r|r.get(0)).map_err(sql)
}
fn require(c: &Connection, actor: AccountId, space: SpaceId, cap: &str) -> Result<()> {
    if has(c, actor, space, cap)? {
        Ok(())
    } else {
        Err(denied())
    }
}
fn exists_account(c: &Connection, actor: AccountId) -> Result<()> {
    let exists: bool = c
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM users WHERE id=?1)",
            [actor.to_string()],
            |r| r.get(0),
        )
        .map_err(sql)?;
    if exists { Ok(()) } else { Err(denied()) }
}
fn list(c: &Connection, actor: AccountId) -> Result<Vec<Space>> {
    exists_account(c, actor)?;
    let mut st=c.prepare("SELECT s.id,s.name,s.owner_account,s.created,
        CASE WHEN s.owner_account=m.account_id THEN 'owner' ELSE m.role END
        FROM spaces s JOIN space_memberships m ON m.space_id=s.id WHERE m.account_id=?1 ORDER BY s.created,s.id").map_err(sql)?;
    let rows = st
        .query_map([actor.to_string()], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get(1)?,
                r.get::<_, String>(2)?,
                r.get(3)?,
                r.get(4)?,
            ))
        })
        .map_err(sql)?;
    rows.map(|r| {
        let (id, name, owner, created, role) = r.map_err(sql)?;
        Ok(Space {
            id: space_id(id)?,
            name,
            owner: account(owner)?,
            created,
            role,
        })
    })
    .collect()
}
fn create(c: &Connection, actor: AccountId, name: &str) -> Result<Space> {
    exists_account(c, actor)?;
    if name.trim().is_empty() || name.chars().count() > 256 || name.chars().any(char::is_control) {
        return Err(Error::invalid("invalid space name"));
    }
    let id = SpaceId::new();
    let created = crate::db::now_ms();
    c.execute(
        "INSERT INTO spaces(id,name,owner_account,created) VALUES(?1,?2,?3,?4)",
        params![id.to_string(), name, actor.to_string(), created],
    )
    .map_err(sql)?;
    c.execute(
        "INSERT INTO space_memberships VALUES(?1,?2,'manager')",
        params![id.to_string(), actor.to_string()],
    )
    .map_err(sql)?;
    Ok(Space {
        id,
        name: name.into(),
        owner: actor,
        created,
        role: "owner".into(),
    })
}
/// A capability-checked mutation. Target restrictions are additional to its grant.
#[derive(Debug, Clone)]
pub enum SpaceChange {
    /// Create a space; any existing authenticated account may create one.
    Create {
        /// Display name.
        name: String,
    },
    /// Change an existing member's role; this never admits a new member.
    SetRole {
        /// Member account.
        target: AccountId,
        /// Stored role name; owner is assigned only by transfer.
        role: String,
    },
    /// Remove a member; Managers may remove only Readers and Editors.
    Remove {
        /// Member account.
        target: AccountId,
    },
    /// Leave one's own membership; an owner must transfer first.
    Leave,
    /// Explicitly transfer to an existing member; the former owner becomes Manager.
    Transfer {
        /// New owner, already a member.
        target: AccountId,
    },
    /// Delete directory records only. Content cleanup is a later step.
    Delete,
}
impl SpaceChange {
    fn capability(&self) -> &'static str {
        match self {
            Self::Create { .. } => "spaces.create",
            Self::SetRole { .. } | Self::Remove { .. } => "members.manage",
            Self::Leave => "space.read",
            Self::Transfer { .. } => "space.transfer",
            Self::Delete => "space.delete",
        }
    }
}
fn check(
    c: &Connection,
    actor: AccountId,
    space: Option<SpaceId>,
    change: &SpaceChange,
) -> Result<()> {
    if matches!(change, SpaceChange::Create { .. }) {
        return exists_account(c, actor);
    }
    let space = space.ok_or_else(Error::invalid_space)?;
    require(c, actor, space, change.capability())?;
    // Even a custom role with a transfer/delete grant cannot bypass sole ownership.
    if matches!(change, SpaceChange::Transfer { .. } | SpaceChange::Delete)
        && role(c, actor, space)?.as_deref() != Some("owner")
    {
        return Err(denied());
    }
    Ok(())
}
fn apply(
    c: &Connection,
    actor: AccountId,
    space: Option<SpaceId>,
    change: &SpaceChange,
) -> Result<serde_json::Value> {
    if let SpaceChange::Create { name } = change {
        return serde_json::to_value(serde_json::json!({"space":create(c,actor,name)?}))
            .map_err(sql);
    }
    let space = space.ok_or_else(Error::invalid_space)?;
    let sid = space.to_string();
    match change {
        SpaceChange::SetRole {
            target,
            role: new_role,
        } => {
            protect_target(c, actor, space, *target)?;
            let known: bool = c
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM space_roles WHERE name=?1 AND name<>'owner')",
                    [new_role],
                    |r| r.get(0),
                )
                .map_err(sql)?;
            if !known {
                return Err(Error::invalid("unknown or nonassignable space role"));
            }
            let privileged:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM space_role_capabilities WHERE role=?1 AND capability NOT IN ('space.read','content.write','plugins.use'))",[new_role],|r|r.get(0)).map_err(sql)?;
            if privileged {
                require(c, actor, space, "managers.manage")?;
                if role(c, actor, space)?.as_deref() != Some("owner") {
                    return Err(denied());
                }
            }
            c.execute(
                "UPDATE space_memberships SET role=?3 WHERE space_id=?1 AND account_id=?2",
                params![sid, target.to_string(), new_role],
            )
            .map_err(sql)?;
        }
        SpaceChange::Remove { target } => {
            protect_target(c, actor, space, *target)?;
            c.execute(
                "DELETE FROM space_memberships WHERE space_id=?1 AND account_id=?2",
                params![sid, target.to_string()],
            )
            .map_err(sql)?;
        }
        SpaceChange::Leave => {
            if role(c, actor, space)?.as_deref() == Some("owner") {
                return Err(denied());
            }
            c.execute(
                "DELETE FROM space_memberships WHERE space_id=?1 AND account_id=?2",
                params![sid, actor.to_string()],
            )
            .map_err(sql)?;
        }
        SpaceChange::Transfer { target } => {
            if *target == actor || role(c, *target, space)?.is_none() {
                return Err(denied());
            }
            c.execute("UPDATE space_memberships SET role='manager' WHERE space_id=?1 AND account_id IN (?2,?3)",params![sid,actor.to_string(),target.to_string()]).map_err(sql)?;
            c.execute(
                "UPDATE spaces SET owner_account=?2 WHERE id=?1",
                params![sid, target.to_string()],
            )
            .map_err(sql)?;
        }
        SpaceChange::Delete => {
            c.execute("DELETE FROM spaces WHERE id=?1", [sid])
                .map_err(sql)?;
        }
        SpaceChange::Create { .. } => unreachable!(),
    }
    Ok(serde_json::json!({}))
}
fn protect_target(
    c: &Connection,
    actor: AccountId,
    space: SpaceId,
    target: AccountId,
) -> Result<()> {
    let target_role = role(c, target, space)?.ok_or_else(denied)?;
    if target_role == "owner" {
        return Err(denied());
    }
    // Protect authority-bearing custom roles as well as the built-in Manager.
    let privileged:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM space_role_capabilities WHERE role=?1 AND capability NOT IN ('space.read','content.write','plugins.use'))",[&target_role],|r|r.get(0)).map_err(sql)?;
    if privileged {
        require(c, actor, space, "managers.manage")?;
        if role(c, actor, space)?.as_deref() != Some("owner") {
            return Err(denied());
        }
    }
    Ok(())
}
impl Error {
    fn invalid_space() -> Self {
        Self::invalid("space required")
    }
}
impl Spaces {
    /// Watches this directory's worker mutations; subscribers must recheck current
    /// capability rows after a hint. Clones bounded watch state without I/O.
    pub fn changes(&self) -> tokio::sync::watch::Receiver<u64> {
        self.db.changes()
    }

    /// Opens a configured migrated database with the default receipt retention.
    /// The host must validate roots first.
    pub fn open(path: &Path) -> Result<Self> {
        Ok(Self::from_db(
            crate::db::Db::open(path)?,
            DEFAULT_RECEIPT_RETENTION,
        ))
    }
    pub(crate) fn from_db(db: crate::db::Db, receipt_retention: std::time::Duration) -> Self {
        Self {
            db,
            receipt_retention,
        }
    }
    /// Agent authority sharing this handle and recovery barrier. Cost: an Arc clone.
    pub fn agents(&self) -> crate::agents::Agents {
        crate::agents::Agents::new(self.db.clone())
    }
    /// Machine authority over this same `auth.db` handle, sharing its worker and
    /// recovery barrier. Cost: one Arc clone.
    pub fn machines(&self) -> crate::machines::Machines {
        crate::machines::Machines::new(self.db.clone())
    }
    /// Attaches recovery before any requests are admitted; reconciles pending
    /// proposals. Covers every handle sharing this database (see [`crate::Auth::spaces`]).
    pub fn configure_recovery(&self, sink: std::sync::Arc<dyn RecoverySink>) -> Result<()> {
        self.db.configure_recovery(sink)
    }
    fn run<T: Send + 'static>(
        &self,
        f: impl FnOnce(&mut Connection) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        self.db.run_blocking(f)
    }
    /// Creates a space with a fresh UUIDv7 and the creator as its sole owner.
    pub fn create(&self, actor: AccountId, name: &str) -> Result<Space> {
        let value = self.change(
            actor,
            None,
            SpaceChange::Create {
                name: name.to_owned(),
            },
        )?;
        serde_json::from_value(value["space"].clone()).map_err(sql)
    }
    /// Changes an existing membership after current capability and target checks.
    /// Appointing Managers requires the owner; Owner requires explicit transfer.
    pub fn set_role(
        &self,
        actor: AccountId,
        space: SpaceId,
        target: AccountId,
        role: &str,
    ) -> Result<()> {
        self.change(
            actor,
            Some(space),
            SpaceChange::SetRole {
                target,
                role: role.to_owned(),
            },
        )
        .map(|_| ())
    }
    /// Removes a member; the owner cannot be removed, and Managers cannot remove Managers.
    pub fn remove(&self, actor: AccountId, space: SpaceId, target: AccountId) -> Result<()> {
        self.change(actor, Some(space), SpaceChange::Remove { target })
            .map(|_| ())
    }
    /// Leaves a space; its owner must first transfer to another existing member.
    pub fn leave(&self, actor: AccountId, space: SpaceId) -> Result<()> {
        self.change(actor, Some(space), SpaceChange::Leave)
            .map(|_| ())
    }
    /// Transfers sole ownership to an existing member and makes the former owner Manager.
    pub fn transfer(&self, actor: AccountId, space: SpaceId, target: AccountId) -> Result<()> {
        self.change(actor, Some(space), SpaceChange::Transfer { target })
            .map(|_| ())
    }
    /// Removes directory and membership records only; content deletion is a later step.
    pub fn delete(&self, actor: AccountId, space: SpaceId) -> Result<()> {
        self.change(actor, Some(space), SpaceChange::Delete)
            .map(|_| ())
    }
    /// Lists only the account's current memberships; server admin adds no access.
    pub fn list(&self, actor: AccountId) -> Result<Vec<Space>> {
        self.run(move |c| list(c, actor))
    }
    /// The actor's current capabilities in `space`, read on every call; `None` for
    /// both a missing space and a missing membership. Stored names this build does
    /// not know grant nothing. Cost: one worker round trip and two indexed reads
    /// (role, then its capability rows) on cached statements; no allocation beyond
    /// the role name.
    pub fn capabilities(&self, actor: AccountId, space: SpaceId) -> Result<Option<CapabilitySet>> {
        self.run(move |c| {
            let Some(role) = role(c, actor, space)? else {
                return Ok(None);
            };
            let mut st = c
                .prepare_cached("SELECT capability FROM space_role_capabilities WHERE role=?1")
                .map_err(sql)?;
            let mut rows = st.query([role]).map_err(sql)?;
            let mut set = CapabilitySet::empty();
            while let Some(row) = rows.next().map_err(sql)? {
                if let Some(capability) =
                    Capability::from_name(row.get_ref(0).map_err(sql)?.as_str().map_err(sql)?)
                {
                    set = set.with(capability);
                }
            }
            Ok(Some(set))
        })
    }
    /// Lists members after checking `space.read` in one read snapshot.
    pub fn members(&self, actor: AccountId, space: SpaceId) -> Result<Vec<Member>> {
        self.run(move |c| {
        let tx=c.transaction().map_err(sql)?;
        require(&tx,actor,space,"space.read")?;
        let out={let mut st=tx.prepare("SELECT m.account_id,CASE WHEN s.owner_account=m.account_id THEN 'owner' ELSE m.role END FROM space_memberships m JOIN spaces s ON s.id=m.space_id WHERE s.id=?1 ORDER BY m.account_id").map_err(sql)?;
        st.query_map([space.to_string()],|r|Ok((r.get::<_,String>(0)?,r.get(1)?))).map_err(sql)?.map(|r|{let(id,role)=r.map_err(sql)?;Ok(Member{account_id:account(id)?,role})}).collect::<Result<Vec<_>>>()?};
        tx.commit().map_err(sql)?; Ok(out)
    })
    }

    /// Executes a mutation with fresh capability and target checks under a write lock.
    pub fn change(
        &self,
        actor: AccountId,
        space: Option<SpaceId>,
        change: SpaceChange,
    ) -> Result<serde_json::Value> {
        self.change_with_receipt(actor, space, change, None)
    }
    /// Executes or replays atomically. Authority is rechecked before reading receipts.
    /// A receipt is private to the actor and keyed by (actor, scope, request ID):
    /// the scope is `space`, or the actor's own account when there is none, so one
    /// request ID is independent between spaces. It binds the exact operation and
    /// fingerprint, and is kept for the receipt retention: the actor's expired
    /// receipts are pruned in the same transaction, so an expired ID is new again.
    /// Target checks execute on first application; retries cannot repeat a transfer.
    /// When configured, the recovery barrier completes before results or receipts
    /// are acknowledged; a barrier error can follow an unacknowledged DB commit.
    /// Cost: one IMMEDIATE transaction on the auth worker: authority checks, one
    /// bounded indexed prune, one receipt read and, when new, the change and insert.
    pub fn change_with_receipt(
        &self,
        actor: AccountId,
        space: Option<SpaceId>,
        change: SpaceChange,
        receipt: Option<(&str, &str, &str)>,
    ) -> Result<serde_json::Value> {
        let receipt = receipt.map(|(rid, op, fp)| (rid.to_owned(), op.to_owned(), fp.to_owned()));
        let scope = space.map_or_else(|| actor.to_string(), |s| s.to_string());
        let retention = self.receipt_retention.as_millis() as i64;
        self.run(move |c| {
            let tx = c
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(sql)?;
            check(&tx, actor, space, &change)?;
            let now = crate::db::now_ms();
            if let Some((rid, op, fp)) = &receipt {
                tx.execute(
                    "DELETE FROM space_receipts WHERE actor=?1 AND at_ms<?2",
                    params![actor.to_string(), now.saturating_sub(retention)],
                )
                .map_err(sql)?;
                let saved: Option<(String, String, String)> = tx
                    .query_row(
                        "SELECT operation,fingerprint,result FROM space_receipts WHERE actor=?1 AND scope=?2 AND request_id=?3",
                        params![actor.to_string(), scope, rid],
                        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                    )
                    .optional()
                    .map_err(sql)?;
                if let Some((saved_op, saved_fp, result)) = saved {
                    if saved_op != *op || saved_fp != *fp {
                        return Err(Error::new(ErrorCode::Conflict, "request_id reused"));
                    }
                    let value = serde_json::from_str(&result).map_err(sql)?;
                    commit(tx)?;
                    return Ok(value);
                }
            }
            let value = apply(&tx, actor, space, &change)?;
            if let Some((rid, op, fp)) = &receipt {
                tx.execute(
                    "INSERT INTO space_receipts VALUES(?1,?2,?3,?4,?5,?6,?7)",
                    params![actor.to_string(), scope, rid, op, fp, value.to_string(), now],
                )
                .map_err(sql)?;
            }
            commit(tx)?;
            Ok(value)
        })
    }
}

/// Persistent development fixture identities; only explicit preview startup uses this.
#[derive(Debug, Clone)]
pub struct DevelopmentIds {
    /// Account associated with the development owner token.
    pub owner: AccountId,
    /// Account associated with the development guest token.
    pub guest: AccountId,
    /// The owner's space named `personal`; the guest is not a member.
    pub personal: SpaceId,
    /// The space named `shared`, where the guest is a reader.
    pub shared: SpaceId,
}
impl Spaces {
    /// Creates development fixtures exactly once, preserving later revocations,
    /// transfers and deletions across restart. Never call for real installations.
    /// `development_directory` records what it created so a restart returns the same
    /// IDs; nothing on the request path reads it, and requests never use the names.
    /// Accounts and spaces each receive a separate freshly allocated UUIDv7.
    /// A configured recovery barrier also covers this explicit fixture mutation.
    pub fn seed_development(&self) -> Result<DevelopmentIds> {
        self.run(move |c| {
            let tx = c
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(sql)?;
            let existing: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM development_directory)",
                    [],
                    |r| r.get(0),
                )
                .map_err(sql)?;
            if !existing {
                let owner = AccountId::new();
                let guest = AccountId::new();
                for (name, id) in [("owner", owner), ("guest", guest)] {
                    tx.execute(
                        "INSERT INTO users(id,name,username,created_at) VALUES(?1,?2,?3,?4)",
                        params![
                            id.to_string(),
                            name,
                            format!("dev-{name}"),
                            crate::db::now_ms()
                        ],
                    )
                    .map_err(sql)?;
                    tx.execute(
                        "INSERT INTO development_directory VALUES(?1,?2)",
                        params![name, id.to_string()],
                    )
                    .map_err(sql)?;
                }
                for name in ["personal", "shared"] {
                    let s = create(&tx, owner, name)?;
                    tx.execute(
                        "INSERT INTO development_directory VALUES(?1,?2)",
                        params![name, s.id.to_string()],
                    )
                    .map_err(sql)?;
                    if name == "shared" {
                        tx.execute(
                            "INSERT INTO space_memberships VALUES(?1,?2,'reader')",
                            params![s.id.to_string(), guest.to_string()],
                        )
                        .map_err(sql)?;
                    }
                }
            }
            let get = |name| {
                tx.query_row(
                    "SELECT id FROM development_directory WHERE name=?1",
                    [name],
                    |r| r.get::<_, String>(0),
                )
                .map_err(sql)
            };
            let ids = DevelopmentIds {
                owner: account(get("owner")?)?,
                guest: account(get("guest")?)?,
                personal: space_id(get("personal")?)?,
                shared: space_id(get("shared")?)?,
            };
            commit(tx)?;
            Ok(ids)
        })
    }
}

/// Host-injected durable recovery barrier for every protected auth transaction.
/// Implementations reconcile earlier proposals through committed visibility, stage
/// only affected protected records, insert their witness in `transaction`, and
/// refuse ambiguous recovery. They must never replay a transaction's own witness.
/// The host serializes the prepare/commit/complete interval; direct trait callers
/// must do the same. Every durability failure prevents acknowledgement.
/// With no configured sink, transaction behavior is unchanged.
pub trait RecoverySink: Send + Sync {
    /// Reconciles pending work against the still-authoritative database.
    fn reconcile(&self, connection: &Connection) -> Result<()>;
    /// Durably appends a proposed protected delta, adding its chain witness
    /// to this transaction. An empty token denotes no protected-state change.
    fn prepare(&self, transaction: &Connection) -> Result<String>;
    /// Publishes only a witnessed committed delta before acknowledgement; an
    /// empty token from prepare requires no filesystem publication.
    fn complete(&self, connection: &Connection, intent: &str) -> Result<()>;
    /// Held from prepare through complete, so no other commit stages an intent while
    /// this one decides which witnesses are committed. It belongs to the sink, so
    /// database handles sharing a sink share it too.
    fn serial(&self) -> &std::sync::Mutex<()>;
}

/// Commits authority with its configured protected-publication barrier; no-op
/// deltas retain normal SQLite durability without writing recovery versions.
pub(crate) fn commit(transaction: rusqlite::Transaction<'_>) -> Result<()> {
    commit_with(transaction, || {})
}
/// Runs the in-memory revoke fence immediately after durable SQLite commit,
/// including when later recovery publication fails and no acknowledgement occurs.
/// A refused preparation rolls back authority before reconciling an aborted proposal.
pub(crate) fn commit_with(
    mut transaction: rusqlite::Transaction<'_>,
    committed: impl FnOnce(),
) -> Result<()> {
    let Some(sink) = crate::db::active_sink() else {
        transaction.commit().map_err(sql)?;
        committed();
        return Ok(());
    };
    // Retain the sink's barrier across commit: another worker must not stage an
    // intent while completion is deciding which witnesses are committed.
    let _guard = sink.serial().lock().map_err(Error::internal)?;
    let intent = match sink.prepare(&transaction) {
        Ok(intent) => intent,
        Err(error) => {
            // An aborted proposal requires a writable autocommit connection to
            // pin its abort marker. Reconcile only after rolling back authority.
            transaction.execute_batch("ROLLBACK").map_err(sql)?;
            transaction.set_drop_behavior(rusqlite::DropBehavior::Ignore);
            sink.reconcile(&transaction)?;
            return Err(error);
        }
    };
    // Keep the original pinned SQLite connection available after COMMIT. On a
    // failed COMMIT, the transaction still rolls back using its normal Drop.
    transaction.execute_batch("COMMIT").map_err(sql)?;
    transaction.set_drop_behavior(rusqlite::DropBehavior::Ignore);
    committed();
    sink.complete(&transaction, &intent)
}
