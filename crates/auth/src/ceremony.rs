//! Open passkey prompts.
//!
//! Between "start" and "finish" the server keeps the WebAuthn challenge in
//! memory under a random id. Finishing takes it out, so each challenge works
//! exactly once: a replayed answer finds nothing. Prompts expire, and the
//! number open at once is capped so nobody can fill the server's memory.

use crate::token::random32;
use crate::{AccountId, Error, ErrorCode, Result};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use webauthn_rs::prelude::{DiscoverableAuthentication, PasskeyRegistration};

/// What a prompt is for.
pub(crate) enum Pending {
    /// Registering a passkey with a setup token.
    Setup {
        state: PasskeyRegistration,
        /// Checked enrollment target and grant binding.
        request: crate::setup::Redeemable,
    },
    /// A signed-in person adding another passkey in Settings.
    Add {
        state: PasskeyRegistration,
        user: AccountId,
    },
    /// Signing in with any passkey for this host.
    Login {
        state: DiscoverableAuthentication,
        /// The account being reverified; `None` for an anonymous sign-in.
        user: Option<AccountId>,
        purpose: crate::LoginPurpose,
        source: String,
    },
}

pub(crate) struct Ceremonies {
    ttl: Duration,
    max: usize,
    open: Mutex<HashMap<String, (Instant, Pending)>>,
}

impl Ceremonies {
    pub fn new(ttl: Duration, max: usize) -> Self {
        Self {
            ttl,
            max,
            open: Mutex::new(HashMap::new()),
        }
    }

    /// Stores a prompt and returns its id.
    pub fn insert(&self, pending: Pending) -> Result<String> {
        let id = hex(&random32()?[..16]);
        let mut open = self.open.lock().unwrap_or_else(|p| p.into_inner());
        open.retain(|_, (at, _)| at.elapsed() < self.ttl);
        let login_source = match &pending {
            Pending::Login { source, .. } => Some(source),
            _ => None,
        };
        // Anonymous prompts have their own bounded pool. Setup and Add
        // each retain a full pool that an anonymous flood cannot consume.
        let same_pool = |p: &Pending| {
            matches!(
                (&pending, p),
                (Pending::Login { .. }, Pending::Login { .. })
                    | (Pending::Setup { .. }, Pending::Setup { .. })
                    | (Pending::Add { .. }, Pending::Add { .. })
            )
        };
        if self.max == 0 {
            return Err(Error::new(
                ErrorCode::TooManyRequests,
                "passkey prompts disabled",
            ));
        }
        if open.values().filter(|(_, p)| same_pool(p)).count() >= self.max {
            if let Some(source) = login_source {
                let oldest = open
                    .iter()
                    .filter(|(_, (_, p))| matches!(p, Pending::Login { source: owner, .. } if owner == source))
                    .min_by_key(|(_, (at, _))| *at)
                    .map(|(id, _)| id.clone());
                if let Some(oldest) = oldest {
                    open.remove(&oldest);
                } else {
                    return Err(Error::new(
                        ErrorCode::TooManyRequests,
                        "too many open passkey prompts",
                    ));
                }
            } else {
                return Err(Error::new(
                    ErrorCode::TooManyRequests,
                    "too many open passkey prompts",
                ));
            }
        }
        open.insert(id.clone(), (Instant::now(), pending));
        Ok(id)
    }

    /// Returns an enrollment's bound account without consuming another purpose.
    pub(crate) fn setup_account(&self, id: &str) -> Result<AccountId> {
        let open = self.open.lock().unwrap_or_else(|p| p.into_inner());
        match open.get(id) {
            Some((at, Pending::Setup { request, .. })) if at.elapsed() < self.ttl => {
                Ok(request.user)
            }
            _ => Err(Error::new(ErrorCode::PasskeyFailed, "no such prompt")),
        }
    }

    /// Takes an "add a passkey" prompt, but only for the person who
    /// started it. Anyone else finds nothing and leaves it in place, so
    /// they can't cancel someone else's prompt (#409).
    pub fn take_add(&self, id: &str, user: AccountId) -> Result<Pending> {
        let mut open = self.open.lock().unwrap_or_else(|p| p.into_inner());
        let mine = matches!(open.get(id), Some((_, Pending::Add { user: u, .. })) if *u == user);
        if !mine {
            return Err(Error::new(ErrorCode::PasskeyFailed, "no such prompt"));
        }
        match open.remove(id) {
            Some((at, p)) if at.elapsed() < self.ttl => Ok(p),
            _ => Err(Error::new(ErrorCode::PasskeyFailed, "prompt expired")),
        }
    }

    /// Takes a prompt only when its account and purpose match; foreign calls
    /// leave the challenge available to its rightful caller.
    pub fn take_login(
        &self,
        id: &str,
        user: Option<AccountId>,
        purpose: crate::LoginPurpose,
    ) -> Result<Pending> {
        let mut open = self.open.lock().unwrap_or_else(|p| p.into_inner());
        if !matches!(open.get(id), Some((_, Pending::Login { user: owner, purpose: p, .. })) if *owner == user && *p == purpose)
        {
            return Err(Error::new(ErrorCode::PasskeyFailed, "no such prompt"));
        }
        match open.remove(id) {
            Some((at, pending)) if at.elapsed() < self.ttl => Ok(pending),
            _ => Err(Error::new(ErrorCode::PasskeyFailed, "prompt expired")),
        }
    }

    /// Takes enrollment only for its bound account, preserving foreign prompts.
    pub fn take_setup_for(&self, id: &str, user: AccountId, replacement: bool) -> Result<Pending> {
        let mut open = self.open.lock().unwrap_or_else(|p| p.into_inner());
        if !matches!(open.get(id), Some((_, Pending::Setup { request, .. })) if request.user == user && request.replacement == replacement)
        {
            return Err(Error::new(ErrorCode::PasskeyFailed, "no such prompt"));
        }
        match open.remove(id) {
            Some((at, pending)) if at.elapsed() < self.ttl => Ok(pending),
            _ => Err(Error::new(ErrorCode::PasskeyFailed, "prompt expired")),
        }
    }

    /// Takes a prompt out. Works once; expired or unknown ids fail.
    #[cfg(test)]
    pub fn take(&self, id: &str) -> Result<Pending> {
        let mut open = self.open.lock().unwrap_or_else(|p| p.into_inner());
        match open.remove(id) {
            Some((at, p)) if at.elapsed() < self.ttl => Ok(p),
            Some(_) => Err(Error::new(ErrorCode::PasskeyFailed, "prompt expired")),
            None => Err(Error::new(ErrorCode::PasskeyFailed, "no such prompt")),
        }
    }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use crate::AccountId;
    #[test]
    fn full_pool_refuses_foreign_source_without_discarding_live_prompt() {
        let dir = crate::test_support::TempDir::new();
        let mut config = crate::AuthConfig::new("http://localhost:8080").unwrap();
        config.max_ceremonies = 1;
        let auth = crate::Auth::open(dir.path(), config).unwrap();
        let (victim, _) = crate::passkey::start_login(
            &auth.inner,
            Some(AccountId::new()),
            crate::LoginPurpose::SignIn,
            "victim",
        )
        .unwrap();
        let error = crate::passkey::start_login(
            &auth.inner,
            Some(AccountId::new()),
            crate::LoginPurpose::SignIn,
            "other",
        )
        .unwrap_err();
        assert_eq!(error.code(), crate::ErrorCode::TooManyRequests);
        assert!(auth.inner.ceremonies.take(&victim).is_ok());
    }

    #[test]
    fn anonymous_pool_stays_bounded_and_evicts_oldest() {
        let dir = crate::test_support::TempDir::new();
        let mut config = crate::AuthConfig::new("http://localhost:8080").unwrap();
        config.max_ceremonies = 2;
        let auth = crate::Auth::open(dir.path(), config).unwrap();
        let (oldest, _) = crate::passkey::start_login(
            &auth.inner,
            Some(AccountId::new()),
            crate::LoginPurpose::SignIn,
            "same-source",
        )
        .unwrap();
        let (second, _) = crate::passkey::start_login(
            &auth.inner,
            Some(AccountId::new()),
            crate::LoginPurpose::SignIn,
            "same-source",
        )
        .unwrap();
        let (newest, _) = crate::passkey::start_login(
            &auth.inner,
            Some(AccountId::new()),
            crate::LoginPurpose::SignIn,
            "same-source",
        )
        .unwrap();
        assert_eq!(auth.inner.ceremonies.open.lock().unwrap().len(), 2);
        assert!(auth.inner.ceremonies.take(&oldest).is_err());
        assert!(auth.inner.ceremonies.take(&second).is_ok());
        assert!(auth.inner.ceremonies.take(&newest).is_ok());
    }
}
