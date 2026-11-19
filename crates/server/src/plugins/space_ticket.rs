//! Host-minted authority for a single invocation on an enrolled space stream.
use calternal_auth::{db::GrantRevision, spaces::Spaces};
use calternal_plugin_host::space_channel::{PackagePin, SpaceFrame};
use calternal_types::{AccountId, Error, InstallationId};
use std::time::{Duration, Instant};
use uuid::Uuid;

const TTL: Duration = Duration::from_secs(60);

#[derive(Clone)]
pub(super) struct Scope {
    pub actor: AccountId,
    pub installation: Option<InstallationId>,
    pub package: PackagePin,
    pub epoch: Uuid,
    pub operation: String,
    pub kind: String,
    pub item: Uuid,
}

pub(super) struct Ticket {
    id: Uuid,
    pub(super) scope: Scope,
    revision: u64,
    grants: GrantRevision,
    expires: Instant,
}

impl Ticket {
    /// Issue a short-lived ticket from the existing auth writer and retain its
    /// assigned-space revision. Call only on a blocking admission thread.
    pub(super) fn mint(
        spaces: &Spaces,
        grants: &GrantRevision,
        scope: Scope,
    ) -> Result<Self, Error> {
        let snapshot = spaces
            .admit_plugin_read(grants, scope.actor, scope.installation)
            .map_err(|_| denied())?
            .ok_or_else(denied)?;
        let now = Instant::now();
        let remaining = snapshot.expires_at.map_or(TTL, |expires_at| {
            let millis = expires_at.saturating_sub(calternal_types::now_ms());
            Duration::from_millis(u64::try_from(millis).unwrap_or(0)).min(TTL)
        });
        if remaining.is_zero() || grants.current() != Some(snapshot.revision) {
            return Err(denied());
        }
        Ok(Self {
            id: Uuid::new_v4(),
            scope,
            revision: snapshot.revision,
            grants: grants.clone(),
            expires: now + remaining,
        })
    }

    pub(super) fn id(&self) -> Uuid {
        self.id
    }

    pub(super) fn deadline(&self) -> Instant {
        self.expires
    }

    pub(super) fn authorize_at(&self, frame: &SpaceFrame, now: Instant) -> Result<(), Error> {
        if frame.ticket != Some(self.id)
            || frame.epoch != self.scope.epoch
            || frame.package.as_deref() != Some(&self.scope.package.package)
            || frame.digest.as_deref() != Some(&self.scope.package.digest)
        {
            return Err(denied());
        }
        self.require_live(now)
    }

    pub(super) fn require_live(&self, now: Instant) -> Result<(), Error> {
        if now >= self.expires || self.grants.current() != Some(self.revision) {
            Err(denied())
        } else {
            Ok(())
        }
    }
}

fn denied() -> Error {
    Error::forbidden("plugin invocation authority unavailable")
}

#[cfg(test)]
mod tests;
