//! Signed, bounded transport for one waiting job. The worker never listens.
use super::*;
use axum::{
    body::{Bytes, to_bytes},
    extract::{Path, Request, State},
    response::{IntoResponse, Response},
};
use calternal_secrets::{Consumer, Owner, SecretName, SecretStore};
use calternal_types::Actor;
use ring::hmac;

/// Ingress prefix where the webhook verifies its own signature.
pub(crate) const PREFIX: &str = "/api/v1/plugin-webhooks/";
/// Ingress path bound to a single wait correlation.
pub(crate) const PATH: &str = "/api/v1/plugin-webhooks/{space}/{package}/{route}/{job}/{correlation}";
type Coordinates = (String, String, String, String, String);

/// Authenticate and resolve the named job wait, returning 202 for its accepted
/// decision or for an identical replay. Bounds the body before deserialization.
pub(crate) async fn webhook<D: Directory>(
    State(state): State<Shared<D>>, Path(keys): Path<Coordinates>, req: Request,
) -> Response {
    match receive(state, keys, req).await {
        Ok(()) => axum::http::StatusCode::ACCEPTED.into_response(),
        Err(error) => crate::api::error_response(error),
    }
}

async fn receive<D: Directory>(state: Shared<D>, keys: Coordinates, req: Request) -> Result<(), Error> {
    let hidden = || Error::not_found("webhook wait not found");
    let (space, package, route, job, correlation) = keys;
    let space_id = SpaceId::parse(&space).map_err(|_| hidden())?;
    let parsed_correlation = Uuid::parse_str(&correlation).map_err(|_| hidden())?;
    let uri = format!("{PREFIX}{space}/{package}/{route}/{job}/{correlation}");
    if req.uri().path() != uri || req.uri().query().is_some() {
        return Err(hidden());
    }
    let timestamp = req.headers().get("x-calternal-timestamp").and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<i64>().ok())
        .filter(|value| value.abs_diff(super::super::job_sql::now_ms() / 1000) <= 300)
        .ok_or_else(hidden)?;
    let signature: [u8; 32] = req.headers().get("x-calternal-signature").and_then(|v| v.to_str().ok())
        .filter(|v| v.len() == 64)
        .and_then(|v| hex::decode(v).ok())
        .and_then(|v| v.try_into().ok())
        .ok_or_else(hidden)?;
    let route_lookup = route.clone();
    let st = state.clone();
    let (wait, job, plugin) = crate::blocking::spawn(move || {
        let Some((wait, job)) = st.jobs.webhook_wait(&space, &job, &route_lookup, parsed_correlation)? else {
            return Err(hidden());
        };
        if job.plugin != package {
            return Err(hidden());
        }
        let plugin = enabled_plugin(&st, &job, space_id)?;
        Ok((wait, job, plugin))
    }).await.map_err(|_| Error::internal("webhook routing task failed"))??;
    let body = tokio::time::timeout(std::time::Duration::from_secs(5), to_bytes(req.into_body(), 16 * 1024))
        .await.map_err(|_| Error::invalid_input("webhook body timed out"))?
        .map_err(|_| Error::invalid_input("webhook body exceeds 16 KiB"))?;
    if !job.session.is_live_in(&state).await {
        return Err(hidden());
    }
    let runtime = tokio::runtime::Handle::current();
    crate::blocking::spawn(move || {
        let webhook = plugin.manifest.webhooks.iter().find(|entry| entry.id == route)
            .filter(|_| plugin.manifest.secrets.contains(&route)).ok_or_else(hidden)?;
        let _plugin = enabled_plugin(&state, &job, space_id)?;
        validate(&state, &job, space_id, &route, &uri, timestamp, &signature, &body)?;
        let payload = serde_json::from_slice(&body)
            .map_err(|_| Error::invalid_input("webhook body must be JSON"))?;
        plugin.schemas.validate_input(&webhook.operation, &payload)
            .map_err(|_| Error::invalid_input("webhook payload does not match the declared schema"))?;
        let fingerprint = crate::store::sha256_hex(&body);
        let resolution = state.jobs.resolve_wait(&job.space, &wait, &job.id, Some(Decision {
            outcome: json!({"kind": "webhook", "value": payload}),
            receipt: Some((&correlation, &fingerprint)),
        }))?;
        if matches!(resolution, Resolution::Woken) {
            let _runtime = runtime.enter();
            super::super::runner::schedule(&state, job.space, job.id);
        }
        Ok(())
    }).await.map_err(|_| Error::internal("webhook decision task failed"))?
}

#[allow(clippy::too_many_arguments)]
fn validate<D: Directory>(
    state: &Shared<D>, job: &Job, space: SpaceId, route: &str, uri: &str,
    timestamp: i64, signature: &[u8; 32], body: &Bytes,
) -> Result<(), Error> {
    let hidden = || Error::not_found("webhook wait not found");
    let secrets = state.store.secrets();
    let name = SecretName::for_plugin(&job.plugin, route).map_err(|_| hidden())?;
    let handle = secrets.handle(&Owner::Space(space), &name).map_err(|_| hidden())?;
    let secret = secrets.use_value(
        &handle, &Actor::plugin(job.account, job.plugin.clone()),
        &Consumer::WebhookSignature { package: job.plugin.clone(), route: route.into() },
    ).map_err(|_| hidden())?;
    if secret.expose().len() < 32 {
        return Err(hidden());
    }
    let mut message = format!("{timestamp}\nPOST\n{uri}\n").into_bytes();
    message.extend_from_slice(body);
    hmac::verify(&hmac::Key::new(hmac::HMAC_SHA256, secret.expose()), &message, signature)
        .map_err(|_| hidden())
}
