//! Installation administrator operations for sandboxed harness versions.
use super::{Session, package_store::provider_runtime as runtime};
use crate::{
    api::Shared,
    auth::Directory,
    machines::{LeaseState, MachineService},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use calternal_core::{HarnessOffer, HarnessSelect, HarnessStage, JsonObject};
use calternal_machine::protocol::{ChunkEvent, ProviderLoginScope, Stream};
use calternal_types::{AccountId, Capability, Error, SpaceId};
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};
use uuid::Uuid;

pub(crate) async fn dispatch<D: Directory>(
    state: Shared<D>,
    machines: Option<Arc<MachineService>>,
    account: AccountId,
    session: Session,
    op: &str,
    input: JsonObject,
) -> Result<Value, Error> {
    let auth = state.directory.http_auth().ok_or_else(denied)?.auth.clone();
    blocking(move || {
        if auth
            .has_server_capability(account, Capability::ServerSettings)
            .map_err(|_| denied())?
        {
            Ok(())
        } else {
            Err(denied())
        }
    })
    .await?;
    if op != "harnesses.list" {
        crate::turns::fresh(&state, &session).await?;
    }
    let appdata = state.store.appdata().to_owned();
    match op {
        "harnesses.list" => {
            blocking(move || runtime::list(&appdata).map_err(Error::unavailable)).await
        }
        "harnesses.activate" | "harnesses.rollback" => {
            let request: HarnessSelect = parse(input)?;
            let rollback = op == "harnesses.rollback";
            blocking(move || {
                let pin = runtime::select(&appdata, &request.id, &request.image, rollback)
                    .map_err(Error::conflict)?;
                if let Some(machines) = machines {
                    let _ = machines.invalidate_harness_checks(&request.id);
                }
                Ok(json!(pin))
            })
            .await
        }
        "harnesses.stage" => {
            let request: HarnessStage = parse(input)?;
            runtime::validate_offer(&request.offer).map_err(Error::invalid_input)?;
            let space = SpaceId::parse(&request.space_id)
                .map_err(|_| Error::not_found("space not found"))?;
            let machine = Uuid::parse_str(&request.machine_id)
                .map_err(|_| Error::not_found("machine not found"))?;
            let machines = machines.ok_or_else(|| Error::conflict("machines are not enabled"))?;
            let plugin = state
                .registry
                .snapshot()
                .get(&request.id)
                .cloned()
                .filter(|plugin| plugin.unavailable.is_none())
                .ok_or_else(|| Error::not_found("harness is not installed"))?;
            let executable = plugin
                .manifest
                .provider
                .as_ref()
                .ok_or_else(|| Error::not_found("harness is not installed"))?
                .executable
                .clone();
            let guest = request
                .guest_machine_id
                .as_deref()
                .map(|id| {
                    Uuid::parse_str(id).map_err(|_| Error::not_found("guest machine not found"))
                })
                .transpose()?;
            if guest.is_some() != request.offer.pin.guest_image_digest.is_some()
                || guest == Some(machine)
            {
                return Err(Error::invalid_input(
                    "select a distinct guest machine for the candidate guest image",
                ));
            }
            let id = request.id.clone();
            let revision = plugin.revision;
            let s = state.clone();
            blocking(move || {
                if !s
                    .directory
                    .capabilities(account, space)
                    .is_some_and(|caps| caps.contains(Capability::PluginsUse))
                {
                    return Err(Error::not_found("space not found"));
                }
                Ok(())
            })
            .await?;
            tokio::spawn(async move {
                let podman = probe(
                    machines.clone(),
                    account,
                    space,
                    machine,
                    false,
                    &id,
                    &executable,
                    &request.offer,
                );
                let guest_probe = async {
                    if let Some(machine) = guest {
                        probe(
                            machines,
                            account,
                            space,
                            machine,
                            true,
                            &id,
                            &executable,
                            &request.offer,
                        )
                        .await?;
                    }
                    Ok::<(), Error>(())
                };
                // Join both so each allocated lease is torn down on failure.
                let (podman, guest) = tokio::join!(podman, guest_probe);
                podman?;
                guest?;
                blocking(move || {
                    let auth = state.directory.http_auth().ok_or_else(denied)?.auth.clone();
                    if !auth
                        .has_server_capability(account, Capability::ServerSettings)
                        .map_err(|_| denied())?
                    {
                        return Err(denied());
                    }
                    runtime::stage(&appdata, &id, revision, request.offer.clone())
                        .map_err(Error::conflict)?;
                    Ok(json!(request.offer.pin))
                })
                .await
            })
            .await
            .map_err(|_| Error::internal("harness smoke task failed"))?
        }
        _ => Err(Error::not_found("harness operation not found")),
    }
}

#[allow(clippy::too_many_arguments)]
async fn probe(
    machines: Arc<MachineService>,
    account: AccountId,
    space: SpaceId,
    machine: Uuid,
    guest: bool,
    id: &str,
    executable: &str,
    offer: &HarnessOffer,
) -> Result<(), Error> {
    let mut sandbox = runtime::sandbox(
        &offer.pin,
        id.to_owned(),
        ProviderLoginScope::Account(account),
        false,
    );
    sandbox.disposable_profile = true;
    let image = offer.pin.image.clone();
    let svc = machines.clone();
    let lease = blocking(move || {
        let pool = svc.verify_harness_target(account, machine, guest)?;
        svc.request_harness(
            space,
            account,
            Uuid::new_v4(),
            Some(machine),
            image,
            sandbox,
            true,
            pool,
        )
    })
    .await?
    .lease_id;
    let result = tokio::time::timeout(
        Duration::from_secs(12),
        smoke(&machines, account, lease, executable.to_owned(), offer),
    )
    .await;
    blocking(move || machines.release_probe(account, lease)).await?;
    result.map_err(|_| Error::unavailable("harness smoke test timed out"))?
}

async fn smoke(
    machines: &Arc<MachineService>,
    account: AccountId,
    lease: Uuid,
    executable: String,
    offer: &HarnessOffer,
) -> Result<(), Error> {
    let svc = machines.clone();
    let exec = loop {
        let m = svc.clone();
        match blocking(move || m.run(account, lease)).await?.state {
            LeaseState::Running => {
                break {
                    let version = offer.pin.version.clone();
                    let m = svc.clone();
                    blocking(move || {
                        m.sign_in_exec(
                            account,
                            lease,
                            vec![executable, "--harness-smoke-test".into(), version],
                        )
                    })
                    .await?
                };
            }
            LeaseState::Waiting | LeaseState::Assigned => {
                tokio::time::sleep(Duration::from_millis(100)).await
            }
            _ => {
                return Err(Error::unavailable(
                    "harness scratch sandbox could not start",
                ));
            }
        }
    };
    let mut cursor = 0;
    let mut output = Vec::new();
    loop {
        let batch = svc
            .read_output(account, exec, cursor, Duration::from_secs(1))
            .await?;
        if batch.lost_before.is_some() {
            return Err(smoke_failed());
        }
        cursor = batch.next;
        for chunk in batch.chunks {
            if let ChunkEvent::Data {
                stream: Stream::Stdout,
                data,
            } = chunk.event
            {
                output.extend(STANDARD.decode(data).map_err(|_| smoke_failed())?);
                if output.len() > 4096 {
                    return Err(smoke_failed());
                }
            }
        }
        if let Some(exit) = batch.exit {
            let report: Value = serde_json::from_slice(&output).map_err(|_| smoke_failed())?;
            if exit.code != Some(0)
                || report["interface"] != 1
                || report["version"] != offer.pin.version
                || report["package_sha256"] != offer.pin.package_sha256
                || report["passed"] != true
            {
                return Err(smoke_failed());
            }
            return Ok(());
        }
    }
}

fn parse<T: serde::de::DeserializeOwned>(input: JsonObject) -> Result<T, Error> {
    serde_json::from_value(json!(input)).map_err(|_| Error::invalid_input("invalid harness input"))
}

fn smoke_failed() -> Error {
    Error::unavailable("The isolated harness smoke test failed.")
}

fn denied() -> Error {
    Error::forbidden("server settings authority is required")
}

async fn blocking<T: Send + 'static>(
    task: impl FnOnce() -> Result<T, Error> + Send + 'static,
) -> Result<T, Error> {
    crate::blocking::spawn(task)
        .await
        .map_err(|_| Error::internal("harness task failed"))?
}
