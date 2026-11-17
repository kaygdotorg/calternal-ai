//! Missing host adapters for anonymous discoverable sign-in and private profile reads.

use crate::{AccountId, Auth, InstallationId, LoginPurpose, Result};
use serde_json::{Value, json};

impl Auth {
    /// Whether no passkey has been enrolled; reveals no account directory.
    pub async fn setup_needed(&self) -> Result<bool> {
        self.inner
            .db
            .run(|c| {
                c.query_row("SELECT COUNT(*) = 0 FROM passkeys", [], |r| r.get(0))
                    .map_err(crate::db::sql)
            })
            .await
    }

    /// Completes the account-bound setup identified by its opaque, take-once prompt.
    /// The account comes from protected ceremony state, never a request field.
    pub async fn finish_http_setup(
        &self,
        ceremony: &str,
        credential: Value,
        device: &str,
    ) -> Result<(AccountId, InstallationId, String)> {
        let account = self.inner.ceremonies.setup_account(ceremony)?;
        self.finish_setup(account, ceremony, credential, device)
            .await
    }

    /// Admits an HTTP login start before reading its body, using a transport-derived source.
    /// Call once before `start_http_login`; malformed requests also spend this budget.
    pub fn admit_http_login(&self, source: &str) -> Result<()> {
        if self.inner.login_starts.attempt(source) {
            Ok(())
        } else {
            Err(crate::Error::new(
                crate::ErrorCode::TooManyRequests,
                "too many login starts",
            ))
        }
    }

    /// Starts discoverable sign-in, or account-bound reverification, after admission.
    /// The HTTP adapter must call `admit_http_login` before reading the request body.
    /// Anonymous prompts bind to no account until the signed
    /// WebAuthn answer identifies the account.
    pub fn start_http_login(
        &self,
        expected: Option<AccountId>,
        source: &str,
    ) -> Result<(String, Value)> {
        crate::passkey::start_login(
            &self.inner,
            expected,
            if expected.is_some() {
                LoginPurpose::Reverify
            } else {
                LoginPurpose::SignIn
            },
            source,
        )
    }

    /// Finishes discoverable login or reverification of a trusted session account.
    /// Purpose/account checks precede prompt consumption; signature verification
    /// selects the account only for anonymous sign-in, preserving source behavior.
    pub async fn finish_http_login(
        &self,
        expected: Option<AccountId>,
        ceremony: &str,
        credential: Value,
        device: &str,
    ) -> Result<(AccountId, InstallationId, String)> {
        crate::passkey::finish_login_inner(
            &self.inner,
            ceremony,
            credential,
            device.to_owned(),
            expected,
            if expected.is_some() {
                LoginPurpose::Reverify
            } else {
                LoginPurpose::SignIn
            },
        )
        .await
    }

    /// Reads this authenticated account's profile and separate server capabilities.
    /// The host must derive `account` from a current session; no space access is granted.
    pub async fn account_profile(&self, account: AccountId) -> Result<Value> {
        self.inner.db.run(move |c| {
            let id = account.to_string();
            let (name, username, role): (String,String,String) = c.query_row("SELECT name, username, role FROM users WHERE id=?1", [&id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(crate::db::sql)?;
            let mut stmt = c.prepare("SELECT capability FROM role_capabilities WHERE role=?1 ORDER BY capability").map_err(crate::db::sql)?;
            let capabilities = stmt.query_map([&role], |r| r.get::<_,String>(0)).map_err(crate::db::sql)?.collect::<rusqlite::Result<Vec<_>>>().map_err(crate::db::sql)?;
            Ok(json!({"id":account,"name":name,"username":username,"role":role,"capabilities":capabilities}))
        }).await
    }
}
