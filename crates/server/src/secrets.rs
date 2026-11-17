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
//! fixed. Each call takes the host's store lock only to clone the secret store's
//! `Arc` (no I/O under it), then runs the owner's I/O on a blocking task; the
//! secret store itself has no installation-wide lock or connection.
//! Spaces are named by UUID only; development aliases are not accepted here.

#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

use std::borrow::Cow;

use axum::Router;
use axum::extract::{Request, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use calternal_auth::{AccountId, SpaceId};
use calternal_core::{
    JsonObject, SecretMetadata, SecretPendingRequest, SecretsListInput, SecretsListResult,
    SecretsProvideInput, SecretsProvideResult, SecretsRequestInput, SecretsRequestResult,
    SecretsRevokeInput, SecretsRevokeResult,
};
use calternal_secrets::{
    Actor, Error, LocalStore, Owner, RequestId, SecretName, SecretStore, SecretValue, Via,
};
use serde_json::{Value, json};

use crate::api::Shared;
use crate::auth::{Directory, Identity};
use crate::authz::{Capability, Constraint, Policy, Scope};
use crate::error::HostError;

/// Path of the trusted endpoint that supplies a value for a pending request.
pub const PROVIDE_PATH: &str = "/api/v1/auth/secrets/provide";
/// Capability a member needs to supply a space secret's value.
pub const PROVIDE_CAPABILITY: Capability = Capability::SpaceSettings;
/// Largest provide body: a 64 KiB value plus JSON escaping and the other fields.
const PROVIDE_BODY_MAX: usize = 400 * 1024;

/// Translate a store refusal into a caller-safe host error.
fn host_error(e: Error) -> HostError {
    match e {
        Error::NotFound => HostError::not_found("secret not found"),
        Error::InvalidInput => HostError::invalid_input("input does not match operation schema"),
        Error::Conflict => HostError::conflict("secret already exists"),
        Error::Limit => HostError::conflict("too many secrets or pending requests"),
        Error::KeyMissing | Error::KeyUnusable => HostError::unavailable(e.to_string()),
        Error::Internal => HostError::internal("secret storage failed"),
    }
}

fn account(p: &Identity) -> Result<AccountId, HostError> {
    AccountId::parse(&p.id).ok_or_else(|| HostError::forbidden("secrets need a stored account"))
}

/// Resolve the owner: the caller's account, or a space the caller holds
/// `required` in. Cost: one directory capability read for a space.
fn owner<D: Directory>(
    state: &Shared<D>,
    p: &Identity,
    space: &str,
    operation: &str,
    required: Capability,
) -> Result<Owner, HostError> {
    if space.is_empty() {
        return Ok(Owner::Account(account(p)?));
    }
    Policy {
        operation: Cow::Owned(operation.to_owned()),
        required,
        scope: Scope::Space,
        constraint: Constraint::None,
    }
    .authorize_space(&state.tokens, p, space)?;
    SpaceId::parse(space)
        .map(Owner::Space)
        .ok_or_else(|| HostError::not_found("space not found"))
}

fn store<D: Directory>(state: &Shared<D>) -> Result<std::sync::Arc<LocalStore>, HostError> {
    Ok(state
        .store
        .lock()
        .map_err(|_| HostError::internal("store poisoned"))?
        .secrets())
}

fn decode<T: serde::de::DeserializeOwned>(input: JsonObject) -> Result<T, HostError> {
    serde_json::from_value(Value::Object(input.into_iter().collect()))
        .map_err(|_| HostError::invalid_input("input does not match operation schema"))
}

fn to_value<T: serde::Serialize>(v: T) -> Result<Value, HostError> {
    serde_json::to_value(v).map_err(|_| HostError::internal("encode"))
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
    via: Via,
) -> Result<Value, HostError> {
    tokio::task::spawn_blocking(move || {
        let op = policy.operation.as_ref();
        let secrets = store(&state)?;
        let actor = Actor {
            account: account(&p)?,
            via,
        };
        match op {
            "secrets.request" => {
                let i: SecretsRequestInput = decode(input)?;
                let owner = owner(&state, &p, &i.space_id, op, policy.required)?;
                let name = SecretName::new(&i.name).map_err(host_error)?;
                let id = secrets
                    .request(&owner, &name, &i.label, &i.reason, &actor)
                    .map_err(host_error)?;
                to_value(SecretsRequestResult {
                    request_id: id.0.to_string(),
                })
            }
            "secrets.list" => {
                let i: SecretsListInput = decode(input)?;
                let owner = owner(&state, &p, &i.space_id, op, policy.required)?;
                to_value(list(&*secrets, &owner)?)
            }
            "secrets.revoke" => {
                let i: SecretsRevokeInput = decode(input)?;
                let owner = owner(&state, &p, &i.space_id, op, policy.required)?;
                if i.name.is_empty() {
                    let id = uuid::Uuid::parse_str(&i.request_id)
                        .map_err(|_| HostError::not_found("secret not found"))?;
                    secrets.cancel(&owner, &RequestId(id), &actor)
                } else {
                    let name = SecretName::new(&i.name).map_err(host_error)?;
                    secrets.delete(&owner, &name, &actor)
                }
                .map_err(host_error)?;
                to_value(SecretsRevokeResult {})
            }
            _ => Err(HostError::unavailable(
                "operation has no host binding in this preview",
            )),
        }
    })
    .await
    .map_err(|_| HostError::internal("secret task failed"))?
}

/// Metadata and pending requests of one owner. Cost: two scans of that owner's
/// database; no values are read.
fn list(secrets: &dyn SecretStore, owner: &Owner) -> Result<SecretsListResult, HostError> {
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

fn reply(e: HostError) -> Response {
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
    let Some(service) = state.tokens.http_auth().cloned() else {
        return reply(HostError::not_found("not found"));
    };
    let Some((account, installation)) =
        crate::account_http::identify(&service, req.headers()).await
    else {
        return reply(HostError::unauthorized("missing or invalid session"));
    };
    if service
        .auth
        .require_fresh(account, installation, false)
        .await
        .is_err()
    {
        return reply(HostError::forbidden("verify with your passkey again first"));
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
        return reply(HostError::invalid_input(
            "Content-Type must be application/json",
        ));
    }
    let Ok(bytes) = axum::body::to_bytes(req.into_body(), PROVIDE_BODY_MAX).await else {
        return reply(HostError::invalid_input("request too large"));
    };
    let Ok(input) = serde_json::from_slice::<SecretsProvideInput>(&bytes) else {
        return reply(HostError::invalid_input("input does not match schema"));
    };
    let result = tokio::task::spawn_blocking(move || {
        let p = Identity {
            id: account.to_string().into(),
        };
        let owner = owner(
            &state,
            &p,
            &input.space_id,
            "secrets.provide",
            PROVIDE_CAPABILITY,
        )?;
        let request = uuid::Uuid::parse_str(&input.request_id)
            .map_err(|_| HostError::not_found("secret not found"))?;
        let value = SecretValue::new(input.value.into_bytes()).map_err(host_error)?;
        let actor = Actor {
            account,
            via: Via::Session,
        };
        let handle = store(&state)?
            .provide(&owner, &RequestId(request), value, &actor)
            .map_err(host_error)?;
        Ok::<_, HostError>(SecretsProvideResult {
            name: handle.name.as_str().to_owned(),
            version: handle.version,
        })
    })
    .await
    .unwrap_or_else(|_| Err(HostError::internal("secret task failed")));
    match result {
        Ok(r) => (StatusCode::OK, axum::Json(json!({"secret": r}))).into_response(),
        Err(e) => reply(e),
    }
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
/// Cost: one owner-database read; one fsync'd insert when a request is created.
pub fn plugin_handle(
    secrets: &dyn SecretStore,
    owner: &Owner,
    actor: &Actor,
    plugin: &str,
    name: &str,
) -> Result<Value, calternal_plugin_host::Error> {
    use calternal_plugin_host::Error as E;
    let full = SecretName::for_plugin(plugin, name).map_err(|_| E::Invalid)?;
    match secrets.handle(owner, &full) {
        Ok(handle) => Ok(json!({"handle": secrets.token(&handle).as_str()})),
        Err(Error::NotFound) => {
            let pending = secrets.pending(owner).map_err(|_| E::Refused)?;
            if !pending.iter().any(|r| r.name == full.as_str()) {
                let reason = format!("The {plugin} plugin asks for its \"{name}\" secret.");
                secrets
                    .request(owner, &full, name, &reason, actor)
                    .map_err(|_| E::Refused)?;
            }
            Err(E::Refused)
        }
        Err(_) => Err(E::Refused),
    }
}
