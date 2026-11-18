//! Protected authority cutover and a replayable list of owner trees to erase.
use crate::{Auth, Error, ErrorCode, Result, db::sql, spaces::Spaces};
use calternal_types::{AccountId, Capability, InstallationId, Owner, SpaceId};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

/// A shared space requiring an ownership transfer before account removal.
#[derive(Debug, serde::Serialize)]
pub struct TransferRequired {
    /// The space UUID to use in the transfer command.
    pub space_id: SpaceId,
    /// The space's current display name.
    pub name: String,
}

/// Result of the protected account cutover. Physical cleanup follows for a
/// committed erasure and can be replayed from the same protected rows.
pub enum AccountErasure {
    /// The account and these private spaces have lost their authority.
    Committed(Vec<Owner>),
    /// A shared space still needs an Owner. No authority changed.
    Transfer(Vec<TransferRequired>),
}

impl Spaces {
    /// Read retained erasure UUIDs and pending flags during installation startup.
    /// One indexed table scan, with no owner store opens.
    pub fn erasure_records(&self) -> Result<Vec<(Owner, bool)>> {
        self.db.run_blocking(|connection| {
            let mut query = connection
                .prepare("SELECT kind,id,pending FROM owner_erasures")
                .map_err(sql)?;
            let rows = query
                .query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, bool>(2)?,
                    ))
                })
                .map_err(sql)?;
            rows.map(|row| {
                let (kind, id, pending) = row.map_err(sql)?;
                let owner = if kind == "account" {
                    Owner::Account(id.parse().map_err(Error::internal)?)
                } else {
                    Owner::Space(id.parse().map_err(Error::internal)?)
                };
                Ok((owner, pending))
            })
            .collect()
        })
    }

    /// Mark a set of already drained and removed owners complete. One protected
    /// commit for the group. UUIDs remain reserved against restoration or reuse.
    pub fn finish_erasure(&self, owners: Vec<Owner>) -> Result<()> {
        self.db.run_blocking(move |connection| {
            // Superseded authority frames can contain the removed account's
            // profile and credentials. Remove the old generation before the
            // progress marker, so a crash repeats compaction if necessary.
            if let Some(sink) = crate::db::active_sink() {
                let _serial = sink.serial().lock().map_err(Error::internal)?;
                sink.compact_erasure(connection)?;
            }
            let checkpoint_busy = connection
                .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |r| {
                    r.get::<_, i64>(0)
                })
                .map_err(sql)?;
            if checkpoint_busy != 0 {
                return Err(Error::internal(
                    "auth WAL checkpoint blocked during erasure",
                ));
            }
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(sql)?;
            for owner in owners {
                tx.execute(
                    "UPDATE owner_erasures SET pending=0 WHERE id=?1 AND pending=1",
                    [owner.uuid().to_string()],
                )
                .map_err(sql)?;
            }
            crate::spaces::commit(tx)
        })
    }
}

fn refused(message: &str) -> Error {
    Error::new(ErrorCode::Forbidden, message)
}

impl Auth {
    /// Revoke an account, memberships and private spaces and publish pending
    /// erasures under the existing recovery barrier. Self-service and administrator
    /// deletion require a fresh passkey assertion from a person’s installation.
    /// No filesystem work waits on the auth worker. The callback may close
    /// admissions only, and runs even if post-commit recovery publication fails.
    pub async fn erase_account(
        &self,
        actor: AccountId,
        installation: InstallationId,
        target: AccountId,
        close: impl FnOnce(&[Owner]) + Send + 'static,
    ) -> Result<AccountErasure> {
        let inner = self.inner.clone();
        let db = inner.db.clone();
        db.run(move |connection| {
            let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(sql)?;
            crate::people::require_active(&tx, actor)?;
            if actor != target && !crate::people::has(&tx, actor, Capability::PeopleManage)? {
                return Err(refused("Only a server administrator can delete another account."));
            }
            crate::session::check_fresh(&tx, &inner.config, actor, installation, false)?;
            let id = target.to_string();
            tx.query_row("SELECT id FROM users WHERE id=?1", [&id], |r| r.get::<_,String>(0))
                .optional().map_err(sql)?.ok_or_else(|| Error::new(ErrorCode::NotFound,"account not found"))?;
            if crate::people::last_people_manager(&tx, target)? {
                return Err(refused("Transfer server administration before deleting its last administrator."));
            }
            let (private, shared) = owned_spaces(&tx, target)?;
            if !shared.is_empty() {
                return Ok(AccountErasure::Transfer(shared));
            }
            let mut installations = Vec::new();
            {
                let mut delete = tx.prepare_cached("DELETE FROM installations WHERE user_id=?1 RETURNING id").map_err(sql)?;
                for row in delete.query_map([&id], |r| r.get::<_,String>(0)).map_err(sql)? {
                    installations.push(row.map_err(sql)?.parse().map_err(Error::internal)?);
                }
            }
            for table in ["setup_tokens", "passkeys", "group_members"] {
                tx.execute(&format!("DELETE FROM {table} WHERE user_id=?1"), [&id]).map_err(sql)?;
            }
            tx.execute("DELETE FROM invites WHERE created_by=?1 AND space_id IS NULL", [&id]).map_err(sql)?;
            tx.execute("UPDATE invites SET revoked_at=coalesce(revoked_at,?2),created_by=NULL
                        WHERE created_by=?1 AND space_id IS NOT NULL",
                       params![id, crate::db::now_ms()]).map_err(sql)?;
            tx.execute("DELETE FROM space_receipts WHERE actor=?1 OR scope IN (SELECT id FROM spaces WHERE owner_account=?1)", [&id]).map_err(sql)?;
            tx.execute("DELETE FROM development_directory WHERE id=?1 OR id IN (SELECT id FROM spaces WHERE owner_account=?1)", [&id]).map_err(sql)?;
            tx.execute("DELETE FROM spaces WHERE owner_account=?1", [&id]).map_err(sql)?;
            tx.execute("DELETE FROM space_memberships WHERE account_id=?1", [&id]).map_err(sql)?;
            tx.execute("DELETE FROM users WHERE id=?1", [&id]).map_err(sql)?;
            let owners: Vec<_> = std::iter::once(Owner::Account(target))
                .chain(private.into_iter().map(Owner::Space)).collect();
            crate::spaces::commit_with(tx, || {
                close(&owners);
                inner.sessions.forget_installations(installations);
            })?;
            Ok(AccountErasure::Committed(owners))
        }).await
    }
}

fn owned_spaces(
    c: &Connection,
    account: AccountId,
) -> Result<(Vec<SpaceId>, Vec<TransferRequired>)> {
    let mut q = c.prepare_cached(
        "SELECT s.id,s.name,s.personal_for IS NULL AND (
           EXISTS(SELECT 1 FROM space_memberships other WHERE other.space_id=s.id AND other.account_id<>?1)
           OR EXISTS(SELECT 1 FROM space_group_memberships g JOIN group_members m USING(group_id)
                      WHERE g.space_id=s.id AND m.user_id<>?1))
         FROM space_memberships own JOIN spaces s ON s.id=own.space_id
         WHERE own.account_id=?1 AND s.owner_account=?1 ORDER BY s.id"
    ).map_err(sql)?;
    let rows = q
        .query_map([account.to_string()], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, bool>(2)?,
            ))
        })
        .map_err(sql)?;
    let mut private = Vec::new();
    let mut shared = Vec::new();
    for row in rows {
        let (id, name, has_members) = row.map_err(sql)?;
        let space_id = id.parse().map_err(Error::internal)?;
        if has_members {
            shared.push(TransferRequired { space_id, name });
        } else {
            private.push(space_id);
        }
    }
    Ok((private, shared))
}
