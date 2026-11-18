//! Protected authority commit for a deployment's one-time local handoff.
use super::*;

impl Machines {
    /// Admit a locally provisioned pool identity after an administrator has
    /// saved a passkey. The same UUID and key can finish an interrupted handoff.
    /// The first administrator receives the existing pool grant in the same
    /// protected commit. A replay never changes a revoked identity or its grants.
    /// Cost: one authority job and, on first admission, one recovery publication.
    pub fn enroll_colocated(
        &self,
        machine: MachineRecord,
    ) -> Result<Option<(MachineRecord, Option<AccountId>)>> {
        if machine.owner.is_some() || machine.direct || machine.revoked {
            return Err(Error::new(
                crate::ErrorCode::Forbidden,
                "expected a pool machine",
            ));
        }
        self.db.run_blocking(move |c| {
            let tx = c
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(sql)?;
            if let Some(existing) = tx
                .query_row(
                    "SELECT id,owner_account,public_key,name,os,arch,revoked_ms IS NOT NULL,direct
                     FROM machines WHERE id=?1",
                    [machine.id.to_string()],
                    record,
                )
                .optional()
                .map_err(sql)?
            {
                let existing = decode(existing)?;
                if existing.owner.is_some() || existing.public_key != machine.public_key {
                    return Err(Error::new(crate::ErrorCode::Conflict, "machine identity already exists"));
                }
                return Ok(Some((existing, None)));
            }
            let account: Option<String> = tx
                .query_row(
                    "SELECT u.id FROM users u JOIN role_capabilities rc ON rc.role=u.role
                     WHERE rc.capability=?1 AND u.disabled_at IS NULL
                        AND EXISTS (SELECT 1 FROM passkeys p WHERE p.user_id=u.id)
                     ORDER BY u.created_at,u.id LIMIT 1",
                    [calternal_types::Capability::ServerSettings.name()],
                    |row| row.get(0),
                )
                .optional()
                .map_err(sql)?;
            let Some(account) = account else {
                return Ok(None);
            };
            let owner = AccountId::parse(&account).map_err(sql)?;
            let now = crate::db::now_ms();
            tx.execute(
                "INSERT INTO machines(id,public_key,name,os,arch,enrolled_ms) VALUES(?1,?2,?3,?4,?5,?6)",
                params![machine.id.to_string(), machine.public_key, machine.name, machine.os, machine.arch, now],
            )
            .map_err(sql)?;
            tx.execute(
                "INSERT OR IGNORE INTO machine_pool_grants(account,granted_ms) VALUES(?1,?2)",
                params![account, now],
            )
            .map_err(sql)?;
            commit(tx)?;
            Ok(Some((machine, Some(owner))))
        })
    }
}
