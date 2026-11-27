//! Installation authority carried from authentication through dispatch and delivery.

use crate::{Host, block_on, internal};
use calternal_types::{AccountId, Error, InstallationId, Result};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, Weak},
};
use tokio::sync::{OwnedRwLockReadGuard, RwLock};

#[derive(Default)]
pub(crate) struct Registry(Mutex<HashMap<InstallationId, Weak<RwLock<()>>>>);

impl Registry {
    fn gate(&self, installation: InstallationId) -> Result<Arc<RwLock<()>>> {
        let mut gates = self.0.lock().map_err(internal)?;
        if let Some(gate) = gates.get(&installation).and_then(Weak::upgrade) {
            return Ok(gate);
        }
        gates.retain(|_, gate| gate.strong_count() != 0);
        let gate = Arc::new(RwLock::new(()));
        gates.insert(installation, Arc::downgrade(&gate));
        Ok(gate)
    }
}

#[derive(Clone)]
pub(crate) struct Session {
    pub(crate) account: AccountId,
    installation: InstallationId,
    gate: Arc<RwLock<()>>,
}

impl Host {
    pub(crate) async fn authenticate(&self, credential: &str) -> Result<Session> {
        let (account, installation) = self.auth.identify(credential).await.ok_or_else(revoked)?;
        Ok(Session {
            account,
            installation,
            gate: self.sessions.gate(installation)?,
        })
    }

    pub(crate) async fn logout(&self, session: Session) -> calternal_auth::Result<()> {
        // The browser's sign-out and the delivery or mutation it fences use
        // the same per-installation lock. Other devices have independent gates.
        let _disallow = session.gate.write().await;
        match self
            .auth
            .revoke(session.account, session.installation)
            .await
        {
            Ok(()) => Ok(()),
            Err(error) if error.code() == calternal_auth::ErrorCode::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }

    fn check_session(&self, session: &Session) -> Result<()> {
        self.auth
            .spaces()
            .agents()
            .installation_live(session.account, session.installation)?
            .then_some(())
            .ok_or_else(revoked)
    }

    pub(crate) fn admit_session_blocking(
        &self,
        session: Option<&Session>,
    ) -> Result<Option<OwnedRwLockReadGuard<()>>> {
        let Some(session) = session else {
            return Ok(None);
        };
        let guard = block_on(Arc::clone(&session.gate).read_owned())?;
        self.check_session(session)?;
        Ok(Some(guard))
    }

    pub(crate) async fn admit_session(
        &self,
        session: Option<&Session>,
    ) -> Result<Option<OwnedRwLockReadGuard<()>>> {
        let Some(session) = session else {
            return Ok(None);
        };
        let guard = Arc::clone(&session.gate).read_owned().await;
        let auth = self.auth.spaces().agents();
        let (account, installation) = (session.account, session.installation);
        let live =
            tokio::task::spawn_blocking(move || auth.installation_live(account, installation))
                .await
                .map_err(internal)??;
        if !live {
            return Err(revoked());
        }
        Ok(Some(guard))
    }
}

fn revoked() -> Error {
    Error::unauthorized("Sign in to use this space.")
}
