//! Secret operations, the trusted provide endpoint and the plugin-host adapter.
//!
//! - `secrets.request`, `secrets.list`, `secrets.revoke` go through the operation
//!   registry. Without `space_id` the owner is the caller's account; with it, the
//!   owner is that space and the policy row's capability is checked against the
//!   current directory (no membership: the same not-found as an unknown space).
//! - Values enter only through `POST /api/v1/auth/secrets/provide`, which needs a
//!   browser or app session whose passkey assertion is within the freshness window
//!   (`Auth::require_fresh`). Development bearer tokens, plugins and agents have no
//!   such session, and the operation registry has no `secrets.provide` row.
//! - Plugins get handles through [`plugin_handle`]; values are released only by
//!   `SecretStore::use_value` at a named consumer.
//!
//! Responses and errors never contain a value; [`calternal_secrets::Error`] text is
//! fixed. Each call clones the secret store's `Arc` and runs the owner's I/O on a
//! blocking task; the secret store has no installation-wide lock or connection.
//! Spaces are named by UUID only.

use std::borrow::Cow;

use axum::Router;
use axum::extract::{Request, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use calternal_core::{
    JsonObject, SecretMetadata, SecretPendingRequest, SecretsListInput, SecretsListResult,
    SecretsProvideInput, SecretsProvideResult, SecretsRequestInput, SecretsRequestResult,
    SecretsRevokeInput, SecretsRevokeResult,
};
use calternal_secrets::{LocalStore, Owner, RequestId, SecretName, SecretStore, SecretValue};
use calternal_types::SpaceId;
use calternal_types::{Actor, Via};
use serde_json::{Value, json};

use crate::api::Shared;
use crate::auth::{Directory, Identity};
use crate::authz::{Constraint, Policy, Scope};
use calternal_types::Capability;
use calternal_types::Error;

/// Path of the trusted endpoint that supplies a value for a pending request.
pub const PROVIDE_PATH: &str = "/api/v1/auth/secrets/provide";
/// Capability a member needs to supply a space secret's value.
pub const PROVIDE_CAPABILITY: Capability = Capability::SecretsManage;
/// Largest provide body: a 64 KiB value plus JSON escaping and the other fields.
const PROVIDE_BODY_MAX: usize = 400 * 1024;

/// Translate a store refusal into a caller-safe host error.
fn host_error(e: calternal_secrets::Error) -> Error {
    use calternal_secrets::Error as E;
    match e {
        E::NotFound => Error::not_found("secret not found"),
        E::InvalidInput => Error::invalid_input("input does not match operation schema"),
        E::Conflict => Error::conflict("secret already exists"),
        E::Limit => Error::conflict("too many secrets or pending requests"),
        E::KeyMissing | E::KeyUnusable => Error::unavailable(e.to_string()),
        E::Internal => Error::internal("secret storage failed"),
    }
}

/// The optional `space_id` field: empty means the caller's own secrets; a malformed
/// ID names no space the caller can see.
fn space_arg(text: &str) -> Result<Option<SpaceId>, Error> {
    if text.is_empty() {
        return Ok(None);
    }
    SpaceId::parse(text)
        .map(Some)
        .map_err(|_| Error::not_found("space not found"))
}

/// Resolve the owner: the caller's account, or a space the caller holds
/// `required` in. Cost: one directory capability read for a space.
fn owner<D: Directory>(
    state: &Shared<D>,
    p: &Identity,
    space: Option<SpaceId>,
    operation: &str,
    required: Capability,
) -> Result<Owner, Error> {
    let Some(space) = space else {
        return Ok(Owner::Account(p.id));
    };
    Policy {
        operation: Cow::Owned(operation.to_owned()),
        required,
        scope: Scope::Space,
        constraint: Constraint::None,
    }
    .authorize_space(&state.directory, p, space)?;
    Ok(Owner::Space(space))
}

fn store<D: Directory>(state: &Shared<D>) -> Result<std::sync::Arc<LocalStore>, Error> {
    Ok(state.store.secrets())
}

fn decode<T: serde::de::DeserializeOwned>(input: JsonObject) -> Result<T, Error> {
    serde_json::from_value(Value::Object(input.into_iter().collect()))
        .map_err(|_| Error::invalid_input("input does not match operation schema"))
}

fn to_value<T: serde::Serialize>(v: T) -> Result<Value, Error> {
    serde_json::to_value(v).map_err(|_| Error::internal("encode"))
}

/// Run an admitted `secrets.*` operation. `policy` is its registry row; its
/// `required` capability applies when the input names a space. `via` is trusted
/// session provenance, never a value from operation input.
/// Cost: one directory read for a space owner, then one owner-database open and
/// at most one fsync'd transaction on a blocking task.
pub async fn dispatch<D: Directory>(
    state: Shared<D>,
    p: Identity,
    policy: Policy,
    input: JsonObject,
    via: Option<Via>,
) -> Result<Value, Error> {
    crate::blocking::spawn(move || {
        let op = policy.operation.as_ref();
        let secrets = store(&state)?;
        let actor = Actor { account: p.id, via };
        match op {
            "secrets.request" => {
                let i: SecretsRequestInput = decode(input)?;
                let owner = owner(&state, &p, space_arg(&i.space_id)?, op, policy.required)?;
                let name = SecretName::new(&i.name).map_err(|_| Error::invalid_input(
                    "Use 1 to 96 lowercase ASCII letters, digits, or . _ - : for the secret name. Start with a letter or digit, then retry with the corrected name.",
                ))?;
                let id = secrets
                    .request(&owner, &name, &i.label, &i.reason, &actor)
                    .map_err(host_error)?;
                state.inputs.invalidate(p.id)?;
                to_value(SecretsRequestResult {
                    request_id: id.0.to_string(),
                })
            }
            "secrets.list" => {
                let i: SecretsListInput = decode(input)?;
                let owner = owner(&state, &p, space_arg(&i.space_id)?, op, policy.required)?;
                to_value(list(&*secrets, &owner)?)
            }
            "secrets.revoke" => {
                let i: SecretsRevokeInput = decode(input)?;
                let owner = owner(&state, &p, space_arg(&i.space_id)?, op, policy.required)?;
                if i.name.is_empty() {
                    let id = uuid::Uuid::parse_str(&i.request_id)
                        .map_err(|_| Error::not_found("secret not found"))?;
                    let recipient = secret_recipient(&*secrets, &owner, id)?;
                    secrets
                        .cancel(&owner, &RequestId(id), &actor)
                        .map_err(host_error)?;
                    if let Some(account) = recipient {
                        state.inputs.invalidate(account)?;
                    }
                    Ok(())
                } else {
                    let name = SecretName::new(&i.name).map_err(host_error)?;
                    secrets.delete(&owner, &name, &actor)
                }
                .map_err(host_error)?;
                to_value(SecretsRevokeResult {})
            }
            _ => Err(Error::not_found(
                "operation has no host binding in this preview",
            )),
        }
    })
    .await
    .map_err(|_| Error::internal("secret task failed"))?
}

/// Metadata and pending requests of one owner. Cost: two scans of that owner's
/// database; no values are read.
fn list(secrets: &dyn SecretStore, owner: &Owner) -> Result<SecretsListResult, Error> {
    Ok(SecretsListResult {
        secrets: secrets
            .list(owner)
            .map_err(host_error)?
            .into_iter()
            .map(|m| SecretMetadata {
                name: m.name,
                label: m.label,
                version: m.version,
                created_at: m.created_at,
                updated_at: m.updated_at,
            })
            .collect(),
        requests: secrets
            .pending(owner)
            .map_err(host_error)?
            .into_iter()
            .map(|r| SecretPendingRequest {
                id: r.id.to_string(),
                name: r.name,
                label: r.label,
                reason: r.reason,
                requested_by: r.requested_by,
                created_at: r.created_at,
            })
            .collect(),
    })
}

/// The trusted provide route; merge it only when account sessions exist.
pub fn routes<D: Directory>() -> Router<Shared<D>> {
    Router::new().route(PROVIDE_PATH, post(provide::<D>))
}

fn reply(e: Error) -> Response {
    let status =
        StatusCode::from_u16(e.code.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (
        status,
        axum::Json(json!({"error": {"code": e.code, "message": e.message}})),
    )
        .into_response()
}

/// Supply a value for a pending request. Order: live session, fresh passkey
/// assertion, then bounded body, then space capability, then the store write.
/// The request body buffer is not zeroed (axum owns it); the parsed value moves
/// into a zeroing [`SecretValue`] without a further copy.
async fn provide<D: Directory>(State(state): State<Shared<D>>, req: Request) -> Response {
    let Some(service) = state.directory.http_auth().cloned() else {
        return reply(Error::not_found("not found"));
    };
    let Some((account, installation)) =
        crate::account_http::identify(&service, req.headers()).await
    else {
        return reply(Error::unauthorized("missing or invalid session"));
    };
    if let Err(error) = service
        .auth
        .require_fresh(account, installation, false)
        .await
    {
        return reply(crate::account_http::fresh_error(error));
    }
    let json = req
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| {
            v.split(';')
                .next()
                .unwrap_or("")
                .trim()
                .eq_ignore_ascii_case("application/json")
        });
    if !json {
        return reply(Error::invalid_input(
            "Content-Type must be application/json",
        ));
    }
    let Ok(bytes) = axum::body::to_bytes(req.into_body(), PROVIDE_BODY_MAX).await else {
        return reply(Error::invalid_input("request too large"));
    };
    let Ok(input) = serde_json::from_slice::<SecretsProvideInput>(&bytes) else {
        return reply(Error::invalid_input("input does not match schema"));
    };
    let result = crate::blocking::spawn(move || {
        let p = Identity { id: account };
        let owner = owner(
            &state,
            &p,
            space_arg(&input.space_id)?,
            "secrets.provide",
            PROVIDE_CAPABILITY,
        )?;
        let request = uuid::Uuid::parse_str(&input.request_id)
            .map_err(|_| Error::not_found("secret not found"))?;
        let recipient = secret_recipient(&*store(&state)?, &owner, request)?;
        let value = SecretValue::new(input.value.into_bytes()).map_err(host_error)?;
        let actor = Actor { account, via: None };
        let handle = store(&state)?
            .provide(&owner, &RequestId(request), value, &actor)
            .map_err(host_error)?;
        if let Some(account) = recipient {
            state.inputs.invalidate(account)?;
        }
        Ok::<_, Error>(SecretsProvideResult {
            name: handle.name.as_str().to_owned(),
            version: handle.version,
        })
    })
    .await
    .unwrap_or_else(|_| Err(Error::internal("secret task failed")));
    match result {
        Ok(r) => (StatusCode::OK, axum::Json(json!({"secret": r}))).into_response(),
        Err(e) => reply(e),
    }
}

/// Resolve the stable account attribution of a stored secret prompt.
pub(crate) fn request_actor(
    text: &str,
) -> Option<(calternal_types::AccountId, Option<uuid::Uuid>)> {
    if let Some((account, thread)) = text
        .strip_prefix("an agent for ")
        .and_then(|v| v.split_once(", in thread "))
    {
        return Some((
            calternal_types::AccountId::parse(account).ok()?,
            Some(uuid::Uuid::parse_str(thread).ok()?),
        ));
    }
    let account = text
        .strip_prefix("plugin ")
        .and_then(|v| v.rsplit_once(" for "))
        .map_or(text, |(_, account)| account);
    Some((calternal_types::AccountId::parse(account).ok()?, None))
}

fn secret_recipient(
    store: &dyn SecretStore,
    owner: &Owner,
    id: uuid::Uuid,
) -> Result<Option<calternal_types::AccountId>, Error> {
    Ok(store
        .pending(owner)
        .map_err(host_error)?
        .into_iter()
        .find(|r| r.id == id)
        .and_then(|request| request_actor(&request.requested_by))
        .map(|(account, _)| account))
}

/// Adapter for the plugin host's `request_secret_handle` call. Its caller is the
/// broker in `plugins::broker` (plugin-host lane, issue #9 records the hand-off),
/// which must pass the space's UUID owner, not the content-space key.
///
/// `name` is the manifest-declared secret name (the plugin host has already
/// checked the declaration); it is stored as `plugin:<plugin>:<name>` under
/// `owner`. Returns `{"handle": "sh-…"}` when the secret exists. Otherwise it
/// records one pending request (unless one is already pending) for the trusted
/// prompt and refuses, so the plugin retries after the person supplies it.
/// Cost: one handle lookup, a pending-owner scan on a miss, and a protected
/// secret-store insert plus account invalidation when creating a prompt.
pub fn plugin_handle(
    secrets: &dyn SecretStore,
    owner: &Owner,
    actor: &Actor,
    plugin: &str,
    name: &str,
    invalidate: impl FnOnce() -> Result<(), calternal_plugin_host::Error>,
) -> Result<Value, calternal_plugin_host::Error> {
    use calternal_plugin_host::Error as E;
    let full = SecretName::for_plugin(plugin, name).map_err(|_| E::Invalid)?;
    match secrets.handle(owner, &full) {
        Ok(handle) => {
            Ok(json!({"handle": secrets.token(&handle).map_err(|_| E::Refused)?.as_str()}))
        }
        Err(calternal_secrets::Error::NotFound) => {
            let pending = secrets.pending(owner).map_err(|_| E::Refused)?;
            if !pending.iter().any(|r| {
                r.name == full.as_str()
                    && request_actor(&r.requested_by)
                        .is_some_and(|(account, _)| account == actor.account)
            }) {
                let reason = format!("The {plugin} plugin asks for its \"{name}\" secret.");
                secrets
                    .request(owner, &full, name, &reason, actor)
                    .map_err(|_| E::Refused)?;
                invalidate()?;
            }
            Err(E::Refused)
        }
        Err(_) => Err(E::Refused),
    }
}

/// Attach protected owner-key recovery and reconcile deletions before admission.
/// Missing live keys and fingerprint mismatches refuse startup; erasing records
/// finish unlinking after an interrupted deletion. Runs on a blocking thread.
/// Cost: one metadata scan and one key-file read per live owner, directory fsync
/// per interrupted erasure; ordinary requests do not scan this registry.
pub fn configure_keys(
    store: &LocalStore,
    spaces: calternal_auth::spaces::Spaces,
    appdata: &std::path::Path,
) -> Result<(), Error> {
    struct Lifecycle(calternal_auth::spaces::Spaces);
    impl calternal_secrets::KeyLifecycle for Lifecycle {
        fn record(
            &self,
            owner: &Owner,
            fingerprint: &[u8; 32],
            erasing: bool,
        ) -> calternal_secrets::Result<()> {
            self.0
                .record_secret_key(owner.kind(), owner.uuid(), *fingerprint, erasing)
                .map_err(|error| {
                    tracing::error!(?error, "owner key recovery publication failed");
                    calternal_secrets::Error::KeyUnusable
                })
        }
    }
    store
        .configure_lifecycle(std::sync::Arc::new(Lifecycle(spaces.clone())))
        .map_err(host_error)?;
    for (kind, id, fingerprint, erasing) in spaces
        .secret_key_records()
        .map_err(|_| Error::unavailable("secret key registry unavailable"))?
    {
        let dir = appdata
            .join(if kind == "account" {
                "accounts"
            } else {
                "spaces"
            })
            .join(id.to_string());
        let path = dir.join("secret.key");
        if erasing {
            match std::fs::remove_file(&path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => return Err(Error::unavailable("secret key erase failed")),
            }
            if dir.exists() {
                calternal_fsio::sync_dir(&dir)
                    .map_err(|_| Error::unavailable("secret key erase failed"))?;
            }
            continue;
        }
        let actual = calternal_secrets::key_fingerprint(&path).map_err(host_error)?;
        if actual.as_slice() != fingerprint.as_slice() {
            return Err(host_error(calternal_secrets::Error::KeyUnusable));
        }
    }
    Ok(())
}
