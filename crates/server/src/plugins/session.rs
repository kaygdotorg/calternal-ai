//! The caller's sign-in, carried into plugin work so it can be rechecked.
//!
//! The HTTP guard authenticates a request once. Plugin work outlives that check: a
//! worker's host calls arrive later, and a job keeps running after its request has
//! been answered. Each such boundary asks [`Session::is_live`] (or its blocking form)
//! whether the sign-in still holds, so a write cannot land after sign-out, after the
//! installation is revoked from another device, or after the session expires.
//!
//! A session is identified by its installation ID, never by its token, so it can be
//! stored in a job record without storing a secret.

use calternal_types::{AccountId, InstallationId};
use serde::{Deserialize, Serialize};
use tokio::runtime::Handle;

use crate::auth::Directory;

/// How an operation's caller is signed in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Session {
    /// A credential that cannot be revoked while the server runs: an explicitly
    /// enabled development bearer token, or an in-process caller (tests, host tasks).
    /// Membership and enablement are still rechecked at every boundary.
    Static,
    /// A thread credential's server-owned identity; only its hash is retained.
    /// Rotation, expiry and the turn ending are checked at every worker boundary.
    Agent {
        /// UUID space holding the per-thread access record.
        space: String,
        /// Owning turn job UUID.
        job: String,
        /// BLAKE3 credential hash, never the raw token.
        token_hash: Vec<u8>,
    },
    /// An account session on one installation (browser or app sign-in). It is live
    /// while that installation is neither revoked nor expired.
    Installation {
        /// Account the installation belongs to.
        account: AccountId,
        /// Installation ID; not a secret.
        installation: InstallationId,
    },
}

impl Session {
    /// Build an installation session from the account service's identify result.
    /// Cost: two 16-byte copies.
    pub fn installation(account: AccountId, installation: InstallationId) -> Self {
        Self::Installation {
            account,
            installation,
        }
    }

    /// Whether the sign-in still holds right now.
    ///
    /// Cost: `Static` is free. `Installation` lists the account's live installations:
    /// one indexed read on the account database worker, no cache. An unknown account,
    /// a missing account service or a lookup failure answer `false` (fail closed).
    pub async fn is_live(&self, directory: &impl Directory) -> bool {
        match self {
            Self::Static => true,
            Self::Agent { .. } => false,
            Self::Installation {
                account,
                installation,
            } => {
                let Some(service) = directory.http_auth() else {
                    return false;
                };
                service
                    .auth
                    .installations(*account)
                    .await
                    .is_ok_and(|live| live.iter().any(|i| i.id == *installation))
            }
        }
    }

    /// [`Self::is_live`] from a thread outside the async runtime, such as a plugin
    /// worker's session thread. Must not be called from a runtime worker thread.
    /// Cost: as [`Self::is_live`], plus one blocking wait on `runtime`.
    pub fn is_live_blocking(&self, directory: &impl Directory, runtime: &Handle) -> bool {
        match self {
            Self::Static => true,
            Self::Agent { .. } => false,
            Self::Installation { .. } => runtime.block_on(self.is_live(directory)),
        }
    }
}

impl Session {
    /// Revalidate a person or thread session against the current protected state.
    /// Thread sessions read their per-space access row and active turn once on a
    /// blocking task. Person sessions use the existing installation proof.
    pub async fn is_live_in<D: Directory>(&self, state: &crate::api::Shared<D>) -> bool {
        if let Self::Agent {
            space,
            job,
            token_hash,
        } = self
        {
            let (st, space, job, hash) = (
                state.clone(),
                space.clone(),
                job.clone(),
                token_hash.clone(),
            );
            return tokio::task::spawn_blocking(move || {
                st.jobs
                    .agent_current(&space, &job, &hash)
                    .ok()
                    .flatten()
                    .is_some()
            })
            .await
            .unwrap_or(false);
        }
        self.is_live(&state.directory).await
    }

    /// Blocking worker-boundary equivalent of [`Self::is_live_in`]. Call only
    /// outside async workers. One per-space read for a thread, otherwise the
    /// existing installation-session check.
    pub fn is_live_in_blocking<D: Directory>(
        &self,
        state: &crate::api::Shared<D>,
        runtime: &Handle,
    ) -> bool {
        match self {
            Self::Agent {
                space,
                job,
                token_hash,
            } => state
                .jobs
                .agent_current(space, job, token_hash)
                .ok()
                .flatten()
                .is_some(),
            _ => self.is_live_blocking(&state.directory, runtime),
        }
    }
}
