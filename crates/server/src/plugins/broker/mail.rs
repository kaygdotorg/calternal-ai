//! Named IMAPS host capability. The worker supplies only endpoint and message identity.
use super::*;
use calternal_secrets::{Consumer, SecretName, SecretStore};
use serde::Deserialize;
use std::time::Duration;

mod imaps;
pub(in crate::plugins) use imaps::snapshot_html;

static CONNECTIONS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(8);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    connection: String,
    endpoint: Endpoint,
    action: Action,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Endpoint {
    host: String,
    port: u16,
    username: String,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum Action {
    RequestPassword,
    Folders,
    Headers {
        folder: String,
        before: Option<u32>,
        uid_validity: Option<u32>,
    },
    Read {
        folder: String,
        uid_validity: u32,
        uid: u32,
    },
    MarkRead {
        folder: String,
        uid_validity: u32,
        uid: u32,
    },
}

impl Endpoint {
    fn validate(&mut self) -> Result<(), Error> {
        self.host.make_ascii_lowercase();
        if self.port == 0
            || self.host.len() > 253
            || !self.host.contains('.')
            || self.host.parse::<std::net::IpAddr>().is_ok()
            || !self.host.split('.').all(|label| {
                !label.is_empty()
                    && label.len() <= 63
                    && !label.starts_with('-')
                    && !label.ends_with('-')
                    && label
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            })
            || !imaps::safe_string(&self.username)
            || self.host.len() + self.username.len() > 460
        {
            return Err(Error::Invalid);
        }
        Ok(())
    }
}

impl<D: Directory> SpaceHost<D> {
    /// Fence the named connection to an interactive owner in their personal space.
    /// Resolve public addresses once and release the secret only after verified TLS.
    pub(super) fn mail(&self, input: &Value) -> Result<Value, Error> {
        let Request {
            connection,
            mut endpoint,
            action,
        } = serde_json::from_value(input.clone()).map_err(|_| Error::Invalid)?;
        let account = self.identity.id;
        if !matches!(self.session, Session::Installation { .. } | Session::Static)
            || self
                .plugin
                .manifest
                .network
                .as_ref()
                .and_then(|n| n.mail_connection.as_ref())
                != Some(&connection)
            || !self
                .state
                .directory
                .protected_spaces()
                .and_then(|spaces| spaces.list(account).ok())
                .is_some_and(|spaces| {
                    spaces.iter().any(|space| {
                        space.personal && space.id == self.space && space.owner == account
                    })
                })
        {
            return Err(Error::Refused);
        }
        if matches!(action, Action::RequestPassword | Action::MarkRead { .. })
            && !self.authorize(ManifestCapability::ContentWrite)
        {
            return Err(Error::Refused);
        }
        endpoint.validate()?;
        action.validate()?;
        let owner = Owner::Account(account);
        let actor = Actor {
            account,
            via: Some(Via::Plugin {
                plugin: self.plugin.manifest.id.clone(),
            }),
        };
        let key = json!([
            self.plugin.manifest.id,
            connection,
            endpoint.host,
            endpoint.port,
            endpoint.username
        ]);
        let endpoint_key = sha256_hex(key.to_string().as_bytes());
        let name = SecretName::new(&format!("mail:{endpoint_key}")).map_err(|_| Error::Invalid)?;
        let secrets = self.state.store.secrets();
        let handle = secrets.handle(&owner, &name);
        if matches!(action, Action::RequestPassword) {
            if handle.is_ok() {
                return Ok(json!({ "ready": true, "endpoint_key": endpoint_key }));
            }
            // The protected form identifies the exact destination for consent.
            let label = format!(
                "Mail password for {}:{} ({})",
                endpoint.host, endpoint.port, endpoint.username
            );
            let pending = secrets.pending(&owner).map_err(|_| Error::Refused)?;
            let id = if let Some(row) = pending.iter().find(|row| row.name == name.as_str()) {
                row.id
            } else {
                let id = secrets
                    .request(&owner, &name, "Mail password", &label, &actor)
                    .map_err(|_| Error::Refused)?;
                self.state
                    .inputs
                    .invalidate(account)
                    .map_err(|_| Error::Refused)?;
                id.0
            };
            return Ok(
                json!({ "ready": false, "secret_request_id": id, "endpoint_key": endpoint_key }),
            );
        }
        let handle = handle.map_err(|_| Error::Refused)?;
        let _permit = CONNECTIONS.try_acquire().map_err(|_| Error::Refused)?;
        let _budget = crate::plugins::network::reserve(self.space, imaps::MAX_RESPONSE as u64)
            .map_err(|_| Error::Refused)?;
        let addresses = self
            .runtime
            .block_on(async {
                tokio::time::timeout(
                    Duration::from_secs(3),
                    crate::netguard::public_addresses(&endpoint.host, endpoint.port),
                )
                .await
            })
            .map_err(|_| Error::Refused)?
            .map_err(|_| Error::Refused)?;
        let stream = imaps::connect(&endpoint.host, &addresses)?;
        let authority = if matches!(action, Action::MarkRead { .. }) {
            ManifestCapability::ContentWrite
        } else {
            ManifestCapability::SpaceRead
        };
        if !self.authorize(authority) {
            return Err(Error::Refused);
        }
        let password = secrets
            .use_value(
                &handle,
                &actor,
                &Consumer::MailLogin {
                    host: endpoint.host,
                    port: endpoint.port,
                },
            )
            .map_err(|_| Error::Refused)?;
        let snapshot_key = match &action {
            Action::Read {
                folder,
                uid_validity,
                uid,
            } => Some(sha256_hex(
                json!([endpoint_key, folder, uid_validity, uid])
                    .to_string()
                    .as_bytes(),
            )),
            _ => None,
        };
        let mut result = imaps::execute(stream, &endpoint.username, password.expose(), action)?;
        if let Some(key) = snapshot_key {
            result["snapshot_key"] = json!(key);
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        api::{AppState, allowed_hosts},
        auth::StoredDirectory,
        plugins::{Jobs, Registry, TrustedEntry, WorkerPool, default_row},
        store::Store,
    };
    use calternal_secrets::{RequestId, SecretValue};

    #[tokio::test]
    async fn mail_prompt_is_account_owned_and_connection_fenced() {
        let root = tempfile::tempdir().unwrap();
        let roots = calternal_roots::Roots {
            appdata: root.path().join("appdata"),
            userdata_ssd: root.path().join("ssd"),
            userdata_hdd: root.path().join("hdd"),
        };
        let store = Store::open(
            &roots,
            Uuid::new_v4(),
            true,
            calternal_content::DEFAULT_RECEIPT_RETENTION,
        )
        .unwrap();
        let directory = StoredDirectory::load_or_create(&roots.appdata).unwrap();
        let identity = |role: &str| {
            let token =
                std::fs::read(roots.appdata.join(format!("dev-tokens/{role}.token"))).unwrap();
            directory.authenticate(&token).unwrap()
        };
        let owner = identity("owner");
        let guest = identity("guest");
        let spaces = directory.spaces.list(owner.id).unwrap();
        let personal = spaces.iter().find(|space| space.personal).unwrap().id;
        let shared = spaces.iter().find(|space| !space.personal).unwrap().id;
        let manifest = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../plugins/calternal-mail/manifest.json"
        )))
        .unwrap();
        let plugin = Plugin::new(
            TrustedEntry {
                id: "calternal-mail".into(),
                version: "0.1.0".into(),
                manifest: "manifest.json".into(),
                worker: None,
                worker_sha256: None,
                default_enabled: true,
            },
            manifest,
            None,
        )
        .unwrap();
        assert!(!default_row(&plugin).enabled);
        let actor = Actor::account(owner.id);
        store
            .provision_plugin(personal, actor.clone(), &plugin.kind_registrations(), &[])
            .unwrap();
        store
            .set_plugin_enabled(
                actor.clone(),
                "enable-mail",
                "plugins.enable",
                "enable-mail",
                personal,
                &plugin.manifest.id,
                default_row(&plugin),
                true,
                |_| json!({}),
            )
            .unwrap();
        let generation = activation(&store, personal, &plugin).unwrap().generation;
        let state = Arc::new(AppState {
            inputs: store.account_inputs(),
            store,
            directory,
            registry: Registry::from_plugins(vec![plugin]),
            workers: WorkerPool::new(None),
            jobs: Jobs::disabled(),
            allowed_hosts: allowed_hosts(0),
            blocking: crate::blocking::RequestWorkers::new().unwrap(),
        });
        let plugin = state
            .registry
            .snapshot()
            .get("calternal-mail")
            .unwrap()
            .clone();
        let broker = |identity, space| {
            SpaceHost::new(
                state.clone(),
                identity,
                Session::Static,
                space,
                plugin.clone(),
                generation,
                "mail".into(),
            )
        };
        let guest_id = guest.id;
        let owner_broker = broker(owner, personal);
        let shared_broker = broker(owner, shared);
        let foreign_broker = broker(guest, personal);
        tokio::task::spawn_blocking(move || {
            let mut request = json!({
                "connection": "inbox",
                "endpoint": {"host": "MAIL.example.test", "port": 993, "username": "reader"},
                "action": {"type": "request_password"},
            });
            assert!(matches!(foreign_broker.mail(&request), Err(Error::Refused)));
            assert!(matches!(shared_broker.mail(&request), Err(Error::Refused)));
            request["connection"] = json!("wrong");
            assert!(matches!(owner_broker.mail(&request), Err(Error::Refused)));
            request["connection"] = json!("inbox");
            request["endpoint"]["password"] = json!("unexpected field");
            assert!(matches!(owner_broker.mail(&request), Err(Error::Invalid)));
            request["endpoint"]
                .as_object_mut()
                .unwrap()
                .remove("password");

            let first = owner_broker.mail(&request).unwrap();
            assert_eq!(first["ready"], false);
            assert_eq!(first, owner_broker.mail(&request).unwrap());
            let id = Uuid::parse_str(first["secret_request_id"].as_str().unwrap()).unwrap();
            let secrets = state.store.secrets();
            let owner_scope = Owner::Account(owner.id);
            assert!(
                secrets
                    .pending(&Owner::Account(guest_id))
                    .unwrap()
                    .is_empty()
            );
            assert!(secrets.pending(&Owner::Space(personal)).unwrap().is_empty());
            let pending = secrets.pending(&owner_scope).unwrap();
            assert_eq!(pending.len(), 1);
            assert_eq!(pending[0].id, id);
            assert!(pending[0].reason.contains("mail.example.test:993 (reader)"));
            secrets
                .provide(
                    &owner_scope,
                    &RequestId(id),
                    SecretValue::new(Uuid::new_v4().to_string().into_bytes()).unwrap(),
                    &actor,
                )
                .unwrap();

            let ready = owner_broker.mail(&request).unwrap();
            assert_eq!(
                ready,
                json!({ "ready": true, "endpoint_key": first["endpoint_key"] })
            );
            request["endpoint"]["port"] = json!(1993);
            assert_eq!(owner_broker.mail(&request).unwrap()["ready"], false);
        })
        .await
        .unwrap();
    }
}
