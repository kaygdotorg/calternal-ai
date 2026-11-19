//! Live membership and sign-in at admission to a retained space grant revision.
use super::*;
use crate::{InstallationId, db::GrantRevision};

/// Current invocation authority sampled against the assignment's revision.
pub struct InvocationGrant {
    /// Clean revision shared with the frame path.
    pub revision: u64,
    /// Installation expiry in Unix milliseconds, when the sign-in expires.
    pub expires_at: Option<i64>,
}

impl Spaces {
    /// Capture live member and sign-in authority for an item-read invocation.
    /// The caller supplies a verified identity and installation. None is only
    /// for an already authenticated host or development caller.
    /// Cost: one auth-worker round trip, one capability lookup and, for an
    /// installation, one primary-key session read. No storage writes or flushes.
    pub fn admit_plugin_read(
        &self,
        grants: &GrantRevision,
        account: AccountId,
        installation: Option<InstallationId>,
    ) -> Result<Option<InvocationGrant>> {
        if !self.db.owns_grant_revision(grants) {
            return Ok(None);
        }
        let handle = grants.clone();
        let admitted =
            self.db.read_blocking(move |c| {
                let Some(revision) = handle.current() else {
                    return Ok(None);
                };
                let current = current_capabilities(c, account, handle.space())?;
                if current.is_none_or(|caps| {
                    !caps.contains(Capability::SpaceRead) || !caps.contains(Capability::PluginsUse)
                }) {
                    return Ok(None);
                }
                let expires_at =
                    match installation {
                        Some(id) => {
                            let live: Option<Option<i64>> = c.prepare_cached(
                        "SELECT expires_at FROM installations WHERE id=?1 AND user_id=?2
                         AND revoked_at IS NULL AND kind <> 'dav'
                         AND (expires_at IS NULL OR expires_at > ?3)",
                    ).map_err(sql)?.query_row(
                        params![id.to_string(), account.to_string(), crate::db::now_ms()],
                        |row| row.get(0),
                    ).optional().map_err(sql)?;
                            let Some(expires_at) = live else {
                                return Ok(None);
                            };
                            expires_at
                        }
                        None => None,
                    };
                Ok(Some(InvocationGrant {
                    revision,
                    expires_at,
                }))
            })?;
        Ok(admitted.filter(|snapshot| grants.current() == Some(snapshot.revision)))
    }
}
