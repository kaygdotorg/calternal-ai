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

#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

use calternal_auth::InstallationId;
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
    /// An account session on one installation (browser or app sign-in). It is live
    /// while that installation is neither revoked nor expired.
    Installation {
        /// Account the installation belongs to.
        account: String,
        /// Installation ID; not a secret.
        installation: String,
    },
}

impl Session {
    /// Build an installation session from the account service's identify result.
    /// Cost: two UUID formats.
    pub fn installation(account: impl ToString, installation: InstallationId) -> Self {
        Self::Installation {
            account: account.to_string(),
            installation: installation.0.to_string(),
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
            Self::Installation {
                account,
                installation,
            } => {
                let (Some(service), Some(account)) = (
                    directory.http_auth(),
                    calternal_auth::AccountId::parse(account),
                ) else {
                    return false;
                };
                service
                    .auth
                    .installations(account)
                    .await
                    .is_ok_and(|live| live.iter().any(|i| i.id.0.to_string() == *installation))
            }
        }
    }

    /// [`Self::is_live`] from a thread outside the async runtime, such as a plugin
    /// worker's session thread. Must not be called from a runtime worker thread.
    /// Cost: as [`Self::is_live`], plus one blocking wait on `runtime`.
    pub fn is_live_blocking(&self, directory: &impl Directory, runtime: &Handle) -> bool {
        match self {
            Self::Static => true,
            Self::Installation { .. } => runtime.block_on(self.is_live(directory)),
        }
    }
}
