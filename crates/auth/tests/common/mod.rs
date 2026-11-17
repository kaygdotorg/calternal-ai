//! Disposable account-library fixtures and the source software passkey helper.
#![allow(dead_code)]
use calternal_auth::{AccountId, Auth, AuthConfig, InstallationId, LoginPurpose, Result};
use serde_json::{Value, json};
use url::Url;
use webauthn_authenticator_rs::{WebauthnAuthenticator, softpasskey::SoftPasskey};
use webauthn_rs::prelude::{CreationChallengeResponse, RequestChallengeResponse};
pub const ORIGIN: &str = "http://localhost:8080";
pub struct App {
    pub dir: tempfile::TempDir,
    pub auth: Auth,
}
pub fn app() -> App {
    app_with(|_| {})
}
pub fn app_with(tweak: impl FnOnce(&mut AuthConfig)) -> App {
    let dir = tempfile::tempdir().unwrap();
    app_in(dir, tweak)
}
pub fn app_in(dir: tempfile::TempDir, tweak: impl FnOnce(&mut AuthConfig)) -> App {
    let mut config = AuthConfig::new(ORIGIN).unwrap();
    tweak(&mut config);
    let auth = Auth::open(dir.path(), config).unwrap();
    App { dir, auth }
}
pub fn db(app: &App) -> rusqlite::Connection {
    rusqlite::Connection::open(app.dir.path().join("system/auth.db")).unwrap()
}
pub struct Device {
    pub key: WebauthnAuthenticator<SoftPasskey>,
    pub cred_id: Option<String>,
    pub user: Option<String>,
}
impl Device {
    pub fn new() -> Self {
        Self {
            key: WebauthnAuthenticator::new(SoftPasskey::new(true)),
            cred_id: None,
            user: None,
        }
    }
    pub fn create(&mut self, options: &Value) -> Value {
        let mut opts = options.clone();
        let sel = &mut opts["publicKey"]["authenticatorSelection"];
        assert_eq!(
            sel["residentKey"], "required",
            "server must request discoverable credentials"
        );
        // Source helper adaptation: SoftPasskey cannot store discoverable keys.
        sel["residentKey"] = json!("discouraged");
        sel["requireResidentKey"] = json!(false);
        let ccr: CreationChallengeResponse = serde_json::from_value(opts).unwrap();
        self.user = Some(
            options["publicKey"]["user"]["id"]
                .as_str()
                .unwrap()
                .to_owned(),
        );
        let cred = self
            .key
            .do_registration(Url::parse(ORIGIN).unwrap(), ccr)
            .unwrap();
        let v = serde_json::to_value(&cred).unwrap();
        self.cred_id = Some(v["rawId"].as_str().unwrap().to_owned());
        v
    }
    pub fn sign(&mut self, options: &Value) -> Value {
        let mut opts = options.clone();
        opts["publicKey"]["allowCredentials"] =
            json!([{ "type": "public-key", "id": self.cred_id.clone().unwrap() }]);
        let rcr: RequestChallengeResponse = serde_json::from_value(opts).unwrap();
        let cred = self
            .key
            .do_authentication(Url::parse(ORIGIN).unwrap(), rcr)
            .unwrap();
        let mut v = serde_json::to_value(&cred).unwrap();
        v["response"]["userHandle"] = json!(self.user.clone().unwrap());
        v
    }
}
pub type SignedIn = (AccountId, InstallationId, String);
pub async fn enroll(app: &App, token: &str, name: &str) -> (SignedIn, Device) {
    let (account, ceremony, options) = app.auth.start_setup(token, name, "local").await.unwrap();
    let mut device = Device::new();
    let cred = device.create(&options);
    let session = app
        .auth
        .finish_setup(account, &ceremony, cred, "Test browser")
        .await
        .unwrap();
    (session, device)
}
pub async fn first_owner(app: &App, name: &str) -> (SignedIn, Device) {
    let token = app.auth.first_owner_token().await.unwrap().unwrap();
    enroll(app, &token, name).await
}
pub async fn login(app: &App, account: AccountId, device: &mut Device) -> Result<SignedIn> {
    let (id, options) = app
        .auth
        .start_login(account, LoginPurpose::SignIn, "local")?;
    app.auth
        .finish_login(
            account,
            LoginPurpose::SignIn,
            &id,
            device.sign(&options),
            "Phone",
        )
        .await
}
pub async fn add(
    app: &App,
    session: &SignedIn,
    name: &str,
) -> (Device, calternal_auth::PasskeyInfo) {
    let (id, options) = app
        .auth
        .start_add_passkey(session.0, session.1)
        .await
        .unwrap();
    let mut device = Device::new();
    let cred = device.create(&options);
    let row = app
        .auth
        .finish_add_passkey(session.0, session.1, &id, cred, name)
        .await
        .unwrap();
    (device, row)
}

pub fn remove_v11_sharing_schema(db: &rusqlite::Connection) -> rusqlite::Result<()> {
    db.execute_batch(
        "PRAGMA foreign_keys=OFF;
         DROP TRIGGER installation_notes_scopes_insert;
         DROP TABLE recipient_control_acks;
         DROP TABLE recipient_control;
         DROP TABLE sharing_installation_resets;
         DROP TABLE share_fanout_acks;
         DROP TABLE sharing_reset_cursors;
         DROP TABLE share_fanout_deliveries;
         DROP TABLE recipient_share_fences;
         DROP TABLE recipient_share_state;
         DROP TABLE share_fanout;
         DROP TABLE note_journal;
         DROP TABLE share_grants;
         DROP TABLE note_shares;
         DROP TABLE installation_notes_scopes;",
    )
}
