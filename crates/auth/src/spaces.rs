//! Stored space authority, adapted from the source auth roles and people actions.
//!
//! Synchronous APIs block on SQLite and belong on blocking host threads. Async
//! grant reads await the same bounded authority worker directly. Mutations
//! acquire an IMMEDIATE transaction before checking current capability rows.
//! State commits optionally pass through a protected recovery barrier before acknowledgement.
use crate::{AccountId, Error, ErrorCode, Result, SpaceId};
use calternal_core::PeopleOperation;
use calternal_types::{Capability, CapabilitySet, GroupId};
mod development;
mod erasure;
mod membership;
mod receipt;
mod recovery;
mod search;
pub use development::DevelopmentIds;
use membership::*;
pub use membership::{GroupMember, MemberPage, MemberTarget};
pub(crate) use membership::{assignable_role, privileged};
use receipt::{Receipt, apply_receipted};
pub use recovery::RecoverySink;
pub(crate) use recovery::{commit, commit_with};
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
    /// Whether this is the personal space provisioned for the caller.
    pub personal: bool,
}
/// One membership visible to a current space reader.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Member {
    /// Stable account UUID.
    pub account_id: AccountId,
    /// Current account display name.
    pub name: String,
    /// Effective role for this direct grant; the sole owner is derived from the space record.
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
pub(crate) fn is_owner(c: &Connection, actor: AccountId, space: SpaceId) -> Result<bool> {
    c.query_row(
        "SELECT EXISTS(SELECT 1 FROM spaces WHERE id=?1 AND owner_account=?2)",
        params![space.to_string(), actor.to_string()],
        |r| r.get(0),
    )
    .map_err(sql)
}
/// Tests an active account's effective direct or group capability in this space.
pub(crate) fn has(c: &Connection, actor: AccountId, space: SpaceId, cap: &str) -> Result<bool> {
    c.query_row(
        "SELECT EXISTS(SELECT 1 FROM space_account_roles m
        JOIN space_role_capabilities rc ON rc.role=m.role
        WHERE m.account_id=?1 AND m.space_id=?2 AND rc.capability=?3)",
        params![actor.to_string(), space.to_string(), cap],
        |r| r.get(0),
    )
    .map_err(sql)
}
pub(crate) fn require(c: &Connection, actor: AccountId, space: SpaceId, cap: &str) -> Result<()> {
    if has(c, actor, space, cap)? {
        Ok(())
    } else {
        Err(denied())
    }
}
fn exists_account(c: &Connection, actor: AccountId) -> Result<()> {
    let exists: bool = c
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM users WHERE id=?1 AND disabled_at IS NULL)",
            [actor.to_string()],
            |r| r.get(0),
        )
        .map_err(sql)?;
    if exists { Ok(()) } else { Err(denied()) }
}
fn list(c: &Connection, actor: AccountId) -> Result<Vec<Space>> {
    exists_account(c, actor)?;
    let mut st = c
        .prepare_cached(
            "WITH effective AS (
           SELECT m.space_id,m.role, row_number() OVER (
             PARTITION BY m.space_id ORDER BY m.role='owner' DESC,
               (SELECT count(*) FROM space_role_capabilities rc WHERE rc.role=m.role) DESC,
               m.role) AS preference
           FROM space_account_roles m WHERE m.account_id=?1
         )
         SELECT s.id,s.name,s.owner_account,s.created,m.role,COALESCE(s.personal_for=?1,0)
         FROM effective m JOIN spaces s ON s.id=m.space_id
         WHERE m.preference=1 ORDER BY s.created,s.id",
        )
        .map_err(sql)?;
    let rows = st
        .query_map([actor.to_string()], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get(1)?,
                r.get::<_, String>(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
            ))
        })
        .map_err(sql)?;
    rows.map(|r| {
        let (id, name, owner, created, role, personal) = r.map_err(sql)?;
        Ok(Space {
            id: space_id(id)?,
            name,
            owner: account(owner)?,
            created,
            role,
            personal,
        })
    })
    .collect()
}
fn create(c: &Connection, actor: AccountId, name: &str) -> Result<Space> {
    exists_account(c, actor)?;
    if name.trim().is_empty() || name.chars().count() > 256 || name.chars().any(char::is_control) {
        return Err(Error::invalid("invalid space name"));
    }
    insert_owned(c, actor, name, crate::db::now_ms(), false)
}

/// Insert the space and its owner membership on the caller's transaction.
/// Cost: two writes, with durability provided by the enclosing authority commit.
pub(crate) fn insert_owned(
    c: &Connection,
    actor: AccountId,
    name: &str,
    created: i64,
    personal: bool,
) -> Result<Space> {
    let id = SpaceId::new();
    let account = actor.to_string();
    c.execute(
        "INSERT INTO spaces(id,name,owner_account,created,personal_for) VALUES(?1,?2,?3,?4,?5)",
        params![
            id.to_string(),
            name,
            account,
            created,
            personal.then_some(&account)
        ],
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
        personal,
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
    /// Admit one visible account or group with an assignable space role.
    Add {
        /// Account or group receiving the direct grant.
        target: MemberTarget,
        /// Requested role; ownership requires transfer to an account.
        role: String,
    },
    /// Change the role of a group already granted access.
    SetGroupRole {
        /// Group whose grant changes.
        target: GroupId,
        /// Requested role; a group cannot own a space.
        role: String,
    },
    /// Revoke a group's grant to this space.
    RemoveGroup {
        /// Group whose grant is removed.
        target: GroupId,
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
            Self::Add { .. } => PeopleOperation::AddSpaceMember.capability().name(),
            Self::SetRole { .. } | Self::SetGroupRole { .. } => {
                PeopleOperation::SetSpaceRole.capability().name()
            }
            Self::Remove { .. } | Self::RemoveGroup { .. } => {
                PeopleOperation::RemoveSpaceMember.capability().name()
            }
            Self::Leave => "space.read",
            Self::Transfer { .. } => PeopleOperation::TransferOwnership.capability().name(),
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
        && !is_owner(c, actor, space)?
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
        SpaceChange::Add { target, role } => {
            change_member(c, actor, space, *target, MemberEdit::Add(role))?
        }
        SpaceChange::SetRole { target, role } => change_member(
            c,
            actor,
            space,
            MemberTarget::Account(*target),
            MemberEdit::SetRole(role),
        )?,
        SpaceChange::SetGroupRole { target, role } => change_member(
            c,
            actor,
            space,
            MemberTarget::Group(*target),
            MemberEdit::SetRole(role),
        )?,
        SpaceChange::Remove { target } => change_member(
            c,
            actor,
            space,
            MemberTarget::Account(*target),
            MemberEdit::Remove,
        )?,
        SpaceChange::RemoveGroup { target } => change_member(
            c,
            actor,
            space,
            MemberTarget::Group(*target),
            MemberEdit::Remove,
        )?,
        SpaceChange::Leave => {
            if is_owner(c, actor, space)? {
                return Err(denied());
            }
            let removed = c
                .execute(
                    "DELETE FROM space_memberships WHERE space_id=?1 AND account_id=?2",
                    params![sid, actor.to_string()],
                )
                .map_err(sql)?;
            if removed == 0 {
                return Err(denied());
            }
        }
        SpaceChange::Transfer { target } => {
            if *target == actor || !has(c, *target, space, Capability::SpaceRead.name())? {
                return Err(denied());
            }
            // Use separate insert and update statements. The recovery dirty-row
            // triggers rely on their own conflict handling during UPDATE.
            c.execute(
                "INSERT OR IGNORE INTO space_memberships VALUES(?1,?2,'manager')",
                params![sid, target.to_string()],
            )
            .map_err(sql)?;
            c.execute(
                "UPDATE space_memberships SET role='manager'
                 WHERE space_id=?1 AND account_id=?2 AND role<>'manager'",
                params![sid, target.to_string()],
            )
            .map_err(sql)?;
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
    /// Expires directory receipts on the authority worker and allows the
    /// recovery sink to retire old reply bytes. Costs a sweep of eligible rows
    /// and the usual protected commit when deletions are present.
    pub async fn maintain_receipts(&self) -> Result<()> {
        let retention = i64::try_from(self.receipt_retention.as_millis()).unwrap_or(i64::MAX);
        self.db
            .run(move |c| {
                let tx = c
                    .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                    .map_err(sql)?;
                tx.execute(
                    "DELETE FROM space_receipts WHERE at_ms < ?1",
                    [crate::db::now_ms().saturating_sub(retention)],
                )
                .map_err(sql)?;
                recovery::commit(tx)?;
                if let Some(sink) = crate::db::active_sink() {
                    let _guard = sink.serial().lock().map_err(Error::internal)?;
                    sink.compact_retired_receipts(c)?;
                }
                Ok(())
            })
            .await
    }
    pub(crate) fn run<T: Send + 'static>(
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
    /// Read the current owner of a shared space. Personal and absent spaces
    /// return None. The caller must separately check the person's capability.
    /// Cost: one primary-key lookup on the authority worker.
    pub fn shared_execution_owner(&self, space: SpaceId) -> Result<Option<AccountId>> {
        self.db.read_blocking(move |c| {
            let owner = c
                .query_row(
                    "SELECT s.owner_account FROM spaces s JOIN users u ON u.id=s.owner_account
                        WHERE s.id=?1 AND s.personal_for IS NULL AND u.disabled_at IS NULL",
                    [space.to_string()],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(sql)?;
            owner.map(account).transpose()
        })
    }

    /// Lists only the account's current memberships; server admin adds no access.
    pub fn list(&self, actor: AccountId) -> Result<Vec<Space>> {
        self.run(move |c| list(c, actor))
    }
    /// The actor's current capabilities, derived from all live grants. A missing
    /// or disabled account and a missing membership all return `None`.
    /// Cost: one worker round trip and one indexed query on a cached statement.
    pub fn capabilities(&self, actor: AccountId, space: SpaceId) -> Result<Option<CapabilitySet>> {
        self.db
            .read_blocking(move |c| current_capabilities(c, actor, space))
    }
    /// Read fresh grants on the same bounded authority worker without taking a
    /// general blocking-pool slot. Missing/disabled accounts and absent membership
    /// return None; worker/SQL failures propagate. Cost: one boxed job, one bounded
    /// mailbox/oneshot round trip and the same cached indexed query as the sync API.
    /// No grant is cached across calls; protected publication ordering is unchanged.
    pub async fn capabilities_async(
        &self,
        actor: AccountId,
        space: SpaceId,
    ) -> Result<Option<CapabilitySet>> {
        self.db
            .read(move |c| current_capabilities(c, actor, space))
            .await
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
    /// Queued role writes share a commit while keeping separate savepoints.
    pub fn change_with_receipt(
        &self,
        actor: AccountId,
        space: Option<SpaceId>,
        change: SpaceChange,
        receipt: Option<(&str, &str, &str)>,
    ) -> Result<serde_json::Value> {
        if matches!(
            &change,
            SpaceChange::SetRole { .. } | SpaceChange::SetGroupRole { .. }
        ) {
            let receipt = receipt.map(|r| Receipt::new(actor, space, self.receipt_retention, r));
            return self.db.run_role_write_blocking(move |connection| {
                check(connection, actor, space, &change)?;
                let (value, _) = apply_receipted(connection, receipt.as_ref(), || {
                    Ok((apply(connection, actor, space, &change)?, ()))
                })?;
                Ok(value)
            });
        }
        self.mutate(actor, space, receipt, move |tx| {
            check(tx, actor, space, &change)?;
            Ok(move |tx: &rusqlite::Transaction<'_>| apply(tx, actor, space, &change))
        })
    }

    /// Authorize, replay or apply a protected directory mutation in one transaction.
    /// The admission closure prepares the mutation before any receipt is read.
    pub(crate) fn mutate<F, A>(
        &self,
        actor: AccountId,
        space: Option<SpaceId>,
        receipt: Option<(&str, &str, &str)>,
        admit: F,
    ) -> Result<serde_json::Value>
    where
        F: FnOnce(&rusqlite::Transaction<'_>) -> Result<A> + Send + 'static,
        A: FnOnce(&rusqlite::Transaction<'_>) -> Result<serde_json::Value>,
    {
        self.mutate_with(
            actor,
            space,
            receipt,
            move |tx| {
                let apply = admit(tx)?;
                Ok(move |tx: &rusqlite::Transaction<'_>| Ok((apply(tx)?, ())))
            },
            |_| {},
        )
    }

    /// Pass transient effects to a callback immediately after database commit.
    /// Callback state stays off disk and is absent on an ordinary receipt replay.
    pub(crate) fn mutate_with<F, A, C, E>(
        &self,
        actor: AccountId,
        space: Option<SpaceId>,
        receipt: Option<(&str, &str, &str)>,
        admit: F,
        committed: C,
    ) -> Result<serde_json::Value>
    where
        F: FnOnce(&rusqlite::Transaction<'_>) -> Result<A> + Send + 'static,
        A: FnOnce(&rusqlite::Transaction<'_>) -> Result<(serde_json::Value, E)>,
        C: FnOnce(E) + Send + 'static,
    {
        let receipt = receipt.map(|r| Receipt::new(actor, space, self.receipt_retention, r));
        self.run(move |c| {
            let tx = c
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(sql)?;
            let apply = admit(&tx)?;
            let (value, effect) = apply_receipted(&tx, receipt.as_ref(), || apply(&tx))?;
            commit_with(tx, || {
                if let Some(effect) = effect {
                    committed(effect);
                }
            })?;
            Ok(value)
        })
    }
}
