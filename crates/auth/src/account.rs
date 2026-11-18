//! Library entry points for protected account authentication.
//!
//! These methods are host APIs. The caller must authenticate administrative
//! create/find/recover operations and derive account/installation arguments from
//! its trusted session, never from an untrusted space request. They provide no
//! content paths, memberships or transport cookies. Recovery publication is optional.

use crate::{AccountId, Error, ErrorCode, InstallationId, Result, config::AuthConfig};
use serde_json::Value;
use std::{path::Path, sync::Arc};

/// A protected account authentication service sharing one worker and revoke fence.
#[derive(Clone)]
pub struct Auth {
    pub(crate) inner: Arc<crate::Inner>,
}

/// The purpose to which a login challenge is bound.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoginPurpose {
    /// Establish a new signed-in installation.
    SignIn,
    /// Establish a freshly asserted installation for sensitive account operations.
    Reverify,
}

impl Auth {
    /// Opens `state/system/auth.db` and builds the WebAuthn verifier.
    /// The host must validate and lock its state root first; this creates missing
    /// database parents and does not implement recovery of lost security state.
    /// Initializes the private invite key if there are no stored invites.
    /// A missing key with stored invites is an error. This startup does not
    /// provision any accounts or spaces.
    pub fn open(state: &Path, config: AuthConfig) -> Result<Self> {
        let webauthn = crate::passkey::build(&config)?;
        let database = state.join("system/auth.db");
        let db = crate::db::Db::open(&database)?;
        let invite_key = crate::invite_key::Key::open(&state.join("system/invite.key"), || {
            crate::db::open_conn(&database)?
                .query_row("SELECT EXISTS(SELECT 1 FROM invites)", [], |r| r.get(0))
                .map_err(crate::db::sql)
        })?;
        Ok(Self {
            inner: Arc::new(crate::Inner {
                db,
                invite_key,
                ceremonies: crate::ceremony::Ceremonies::new(
                    config.ceremony_ttl,
                    config.max_ceremonies,
                ),
                account_failures: crate::limit::Failures::default(),
                sessions: Default::default(),
                setup_failures: crate::limit::Limiter::new(config.setup_failures_per_minute),
                setup_targets: crate::limit::TargetLimiter::new(config.setup_failures_per_minute),
                login_starts: crate::limit::Limiter::new(config.login_starts_per_minute),
                webauthn,
                config,
            }),
        })
    }

    /// The space directory over this same `auth.db` handle: one connection, one
    /// worker thread and one recovery sink for accounts and spaces. Cost: two Arc
    /// clones.
    pub fn spaces(&self) -> crate::spaces::Spaces {
        crate::spaces::Spaces::from_db(self.inner.db.clone(), self.inner.config.receipt_retention)
    }

    /// Attaches the protected recovery barrier before serving requests. It covers
    /// this handle and every [`Self::spaces`] handle. The host must prohibit
    /// concurrent processes via root locks.
    pub async fn configure_recovery(
        &self,
        sink: Arc<dyn crate::spaces::RecoverySink>,
    ) -> Result<()> {
        self.inner.db.configure_recovery_async(sink).await
    }

    /// Issues a one-time first-account token only while no account exists.
    /// This is a privileged local administration operation; protect the result.
    pub async fn first_owner_token(&self) -> Result<Option<String>> {
        Ok(self.first_owner_code(None).await?.map(|grant| grant.code))
    }

    /// Reuse an unexpired, unused startup candidate or rotate the first-owner code.
    /// Returns its stored expiry, or None after the first account exists. A
    /// rotation costs one protected commit. This is a privileged local operation.
    pub async fn first_owner_code(
        &self,
        candidate: Option<String>,
    ) -> Result<Option<crate::FirstOwnerCode>> {
        crate::setup::first_owner_code(&self.inner, candidate).await
    }

    /// Creates identity records, an owned personal space, and a first-passkey
    /// enrollment token in one protected commit. Requires privileged host
    /// authorization. No content store opens during enrollment.
    pub async fn create(&self, name: &str) -> Result<crate::Created> {
        let name = name.to_owned();
        let ttl = self.inner.config.setup_ttl.as_millis() as i64;
        self.inner
            .db
            .run(move |c| crate::setup::create_in(c, &name, ttl))
            .await
    }

    /// Finds an exact username or canonical UUID, never a display name.
    /// Requires privileged host authorization; returns identity and display name.
    pub async fn find(&self, who: &str) -> Result<(AccountId, String)> {
        let who = who.to_owned();
        let (id, name) = self
            .inner
            .db
            .run(move |c| crate::setup::find_in(c, &who))
            .await?;
        Ok((
            AccountId::parse(&id).map_err(|_| Error::internal("bad account id"))?,
            name,
        ))
    }

    /// Replaces an account's unused recovery token; returns display name and token.
    /// Requires administrator identity verification outside this library. It
    /// preserves existing credentials and sessions, as the source implementation.
    pub async fn recover(&self, who: &str) -> Result<(String, String)> {
        let who = who.to_owned();
        let ttl = self.inner.config.setup_ttl.as_millis() as i64;
        self.inner
            .db
            .run(move |c| crate::setup::recover_in(c, &who, ttl))
            .await
    }

    /// Checks a setup/recovery token and begins account-bound enrollment.
    /// Returns the bound account, take-once challenge id, and browser options.
    /// `source` must be a host-derived transport peer, after trusted proxy checks.
    pub async fn start_setup(
        &self,
        token: &str,
        name: &str,
        source: &str,
    ) -> Result<(AccountId, String, Value)> {
        if !self
            .inner
            .setup_failures
            .attempt(&format!("source:{source}"))
        {
            return Err(Error::new(ErrorCode::TooManyRequests, "setup back-off"));
        }
        let redeemable = crate::setup::check(&self.inner, token, name).await?;
        // Only checked tokens get target buckets, preserving source admission.
        let target = blake3::hash(&redeemable.token_hash).to_hex();
        if !self.inner.setup_targets.attempt(target.as_str()) {
            return Err(Error::new(
                ErrorCode::TooManyRequests,
                "setup target back-off",
            ));
        }
        let account = redeemable.user;
        let (id, options) = crate::passkey::start_registration(&self.inner, redeemable).await?;
        Ok((account, id, options))
    }

    /// Verifies an enrollment answer and atomically consumes its setup token.
    /// Returns account, installation and plaintext token once; protect the token.
    pub async fn finish_setup(
        &self,
        account: AccountId,
        ceremony: &str,
        credential: Value,
        device: &str,
    ) -> Result<(AccountId, InstallationId, String)> {
        crate::passkey::finish_registration(
            &self.inner,
            account,
            ceremony,
            credential,
            device.to_owned(),
        )
        .await
    }

    /// Begins discoverable WebAuthn verification bound to account and purpose.
    /// The host supplies the expected account before this call. `source` is the
    /// trusted transport source, not an unchecked forwarded header.
    pub fn start_login(
        &self,
        account: AccountId,
        purpose: LoginPurpose,
        source: &str,
    ) -> Result<(String, Value)> {
        if !self.inner.login_starts.attempt(source) {
            return Err(Error::new(
                ErrorCode::TooManyRequests,
                "too many login starts",
            ));
        }
        crate::passkey::start_login(&self.inner, Some(account), purpose, source)
    }

    /// Consumes a matching challenge and verifies its signature/counter before
    /// creating a fresh installation. Other accounts/purposes cannot consume it.
    pub async fn finish_login(
        &self,
        account: AccountId,
        purpose: LoginPurpose,
        ceremony: &str,
        credential: Value,
        device: &str,
    ) -> Result<(AccountId, InstallationId, String)> {
        crate::passkey::finish_login(
            &self.inner,
            ceremony,
            credential,
            device.to_owned(),
            account,
            purpose,
        )
        .await
    }

    /// Identifies a live non-DAV session; returns only identity and installation.
    /// Never conveys space authority. Revoke fencing is shared by clones in this
    /// process; separately opened services/processes do not share that fence.
    pub async fn identify(&self, token: &str) -> Option<(AccountId, InstallationId)> {
        crate::session::identify(&self.inner, token).await
    }

    /// Lists this account's live installations, newest first.
    pub async fn installations(&self, account: AccountId) -> Result<Vec<crate::Installation>> {
        crate::session::list(&self.inner, account).await
    }

    /// Revokes an account-owned installation and fences subsequent identification.
    /// A foreign or already revoked id returns NotFound without fencing its owner.
    pub async fn revoke(&self, account: AccountId, installation: InstallationId) -> Result<()> {
        crate::session::revoke(&self.inner, account, installation).await
    }

    /// Checks recent browser/native proof for this account-owned live installation.
    /// `registration_ok` permits a recent setup/recovery registration; false
    /// requires an actual sign-in assertion. Uses the configured freshness window.
    /// Insufficient proof returns `PasskeyReverificationRequired`. A revoked
    /// installation still fails the session check.
    pub async fn require_fresh(
        &self,
        account: AccountId,
        installation: InstallationId,
        registration_ok: bool,
    ) -> Result<()> {
        crate::session::require_fresh(&self.inner, account, Some(installation), registration_ok)
            .await
    }

    /// Lists protected authenticators belonging to this account, oldest first.
    pub async fn passkeys(&self, account: AccountId) -> Result<Vec<crate::PasskeyInfo>> {
        crate::passkeys::list(&self.inner, account).await
    }

    /// Begins adding a passkey after recent account-owned interactive proof.
    pub async fn start_add_passkey(
        &self,
        account: AccountId,
        installation: InstallationId,
    ) -> Result<(String, Value)> {
        self.require_fresh(account, installation, true).await?;
        crate::passkeys::start_add(&self.inner, account).await
    }

    /// Finishes an account-bound add ceremony after rechecking recent proof.
    pub async fn finish_add_passkey(
        &self,
        account: AccountId,
        installation: InstallationId,
        ceremony: &str,
        credential: Value,
        name: &str,
    ) -> Result<crate::PasskeyInfo> {
        self.require_fresh(account, installation, true).await?;
        crate::passkeys::finish_add(&self.inner, account, ceremony, credential, name).await
    }

    /// Renames an account-owned passkey and returns its cleaned label.
    pub async fn rename_passkey(&self, account: AccountId, id: &str, name: &str) -> Result<String> {
        crate::passkeys::rename(&self.inner, account, id, name).await
    }

    /// Removes an account-owned key after fresh proof, refusing the last key
    /// inside the same IMMEDIATE transaction as the delete.
    pub async fn remove_passkey(
        &self,
        account: AccountId,
        installation: InstallationId,
        id: &str,
    ) -> Result<()> {
        self.require_fresh(account, installation, true).await?;
        crate::passkeys::remove(&self.inner, account, id).await
    }
}
