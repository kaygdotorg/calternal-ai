//! Live membership and sign-in at admission to a retained space grant revision.
use super::*;
use crate::{InstallationId, db::GrantRevision};
use std::sync::Mutex;

/// Current invocation authority sampled against the assignment's revision.
pub struct InvocationGrant {
    /// Clean revision shared with the frame path.
    pub revision: u64,
    /// Installation expiry in Unix milliseconds, when the sign-in expires.
    pub expires_at: Option<i64>,
    /// The person's space capabilities at `revision`. Any membership change
    /// moves the revision, so a ticket holding both observes revocation.
    pub capabilities: CapabilitySet,
}

/// One awake space's dedicated read-only admission connection and revision latch.
pub struct InvocationReader {
    grants: GrantRevision,
    connection: Mutex<Connection>,
}

impl Spaces {
    /// Pin a read-only WAL connection to the assignment's space. Rejects a
    /// revision from a different authority store. Opens on a blocking worker
    /// during wake and does not queue on the authority worker.
    pub fn invocation_reader(&self, grants: &GrantRevision) -> Result<InvocationReader> {
        if !self.db.owns_grant_revision(grants) {
            return Err(Error::internal(
                "invocation revision belongs to a different authority store",
            ));
        }
        Ok(InvocationReader {
            grants: grants.clone(),
            connection: Mutex::new(self.db.open_admission_reader()?),
        })
    }
}

impl InvocationReader {
    /// The assignment's locally fenced grant revision. Cost: one reference.
    pub fn grants(&self) -> &GrantRevision {
        &self.grants
    }

    /// Check current membership and, when supplied, the authenticated person's
    /// installation. The per-space lock keeps the queries on one SQLite
    /// snapshot. This read-only WAL transaction performs no fsync or cross-thread
    /// call and fails on contention. Call on a blocking worker. Cost: one
    /// capability query and optionally one indexed installation query.
    pub fn admit(
        &self,
        account: AccountId,
        installation: Option<InstallationId>,
    ) -> Result<Option<InvocationGrant>> {
        let Some(revision) = self.grants.current() else {
            return Ok(None);
        };
        let mut connection = self.connection.lock().map_err(Error::internal)?;
        let read = connection.transaction().map_err(sql)?;
        let Some(capabilities) =
            current_capabilities(&read, account, self.grants.space())?.filter(|caps| {
                caps.contains(Capability::SpaceRead) && caps.contains(Capability::PluginsUse)
            })
        else {
            return Ok(None);
        };
        let expires_at = match installation {
            Some(id) => {
                let live: Option<Option<i64>> = read
                    .prepare_cached(
                        "SELECT expires_at FROM installations WHERE id=?1 AND user_id=?2
                     AND revoked_at IS NULL AND kind <> 'dav'
                     AND (expires_at IS NULL OR expires_at > ?3)",
                    )
                    .map_err(sql)?
                    .query_row(
                        params![id.to_string(), account.to_string(), crate::db::now_ms()],
                        |row| row.get(0),
                    )
                    .optional()
                    .map_err(sql)?;
                let Some(expires_at) = live else {
                    return Ok(None);
                };
                expires_at
            }
            None => None,
        };
        read.commit().map_err(sql)?;
        Ok(
            (self.grants.current() == Some(revision)).then_some(InvocationGrant {
                revision,
                expires_at,
                capabilities,
            }),
        )
    }
}
