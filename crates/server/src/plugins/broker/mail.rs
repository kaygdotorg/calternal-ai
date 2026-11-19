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
        let _budget = self
            .state
            .store
            .plugin_fetch
            .reserve(self.space, imaps::MAX_RESPONSE as u64)
            .map_err(|_| Error::Refused)?;
        let addresses = self
            .runtime
            .block_on(async {
                tokio::time::timeout(
                    Duration::from_secs(3),
                    self.state
                        .store
                        .plugin_fetch
                        .mail_addresses(&endpoint.host, endpoint.port),
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
mod tests;
