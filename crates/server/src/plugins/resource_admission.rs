//! An operator-selected read-only block hook on ordinary installed workers.

use std::{sync::Arc, time::Duration};

use calternal_core::{HostCallKind, ManifestCapability, ManifestHookMode};
use calternal_plugin_host::Error as WorkerError;
use calternal_types::{AccountId, Capability, Error, SpaceId};
use serde_json::Value;
use uuid::Uuid;

use super::{Call, InvocationHost, Plugin, activation, host_capability, worker::Outcome};
use crate::{
    api::Shared,
    auth::Directory,
    machines::{
        MachineService,
        resource_hooks::{Decision, Invocation},
    },
};

const HOOK_TARGET: &str = "resources.admit";
const HOOK_DEADLINE: Duration = Duration::from_secs(2);

struct AdmissionHost<D: Directory> {
    state: Shared<D>,
    account: AccountId,
    space: SpaceId,
    plugin: Arc<Plugin>,
    generation: u32,
    binding: crate::machines::ResourceHookBinding,
    owner: Option<calternal_types::ResourceOwner>,
}

impl<D: Directory> InvocationHost for AdmissionHost<D> {
    fn authorize(&self, required: ManifestCapability) -> bool {
        self.state
            .directory
            .capabilities(self.account, self.space)
            .is_some_and(|caps| {
                caps.contains(Capability::PluginsUse)
                    && host_capability(required).is_some_and(|capability| caps.contains(capability))
            })
            && self
                .state
                .registry
                .snapshot()
                .get(&self.plugin.manifest.id)
                .is_some_and(|active| Arc::ptr_eq(active, &self.plugin))
            && activation(&self.state.store, self.space, &self.plugin)
                .is_ok_and(|row| row.enabled && row.generation == self.generation)
    }

    fn call(&self, method: HostCallKind, input: &Value) -> Result<Value, WorkerError> {
        match method {
            HostCallKind::ReadRuntimeWindowV1 => {
                let (since, until) = super::broker::runtime_window_bounds(input)?;
                let svc = self.state.store.machines().ok_or(WorkerError::Refused)?;
                svc.resource_runtime_window(self.space, &self.binding, since, until)
                    .map_err(|_| WorkerError::Refused)
            }
            HostCallKind::ReadPluginStateV1 => {
                let key = input
                    .as_object()
                    .filter(|object| object.len() == 1)
                    .ok_or(WorkerError::Invalid)?
                    .get("key")
                    .and_then(Value::as_str)
                    .ok_or(WorkerError::Invalid)?;
                let expected = match self.owner.ok_or(WorkerError::Refused)? {
                    calternal_types::ResourceOwner::Account(id) => format!("account:{id}"),
                    calternal_types::ResourceOwner::Group(id) => format!("group:{id}"),
                };
                if key != expected {
                    return Err(WorkerError::Refused);
                }
                let state_space = self.binding.state_space.unwrap_or(self.space);
                let activation = activation(&self.state.store, state_space, &self.plugin)
                    .map_err(|_| WorkerError::Refused)?;
                if !activation.enabled {
                    return Err(WorkerError::Refused);
                }
                let svc = self.state.store.machines().ok_or(WorkerError::Refused)?;
                if svc
                    .resource_binding(self.space, &self.binding.package, &self.binding.checksum)
                    .ok()
                    .as_ref()
                    != Some(&self.binding)
                {
                    return Err(WorkerError::Refused);
                }
                let record = self
                    .state
                    .store
                    .content()
                    .plugin_state(state_space, &self.plugin.manifest.id, key)
                    .map_err(|_| WorkerError::Refused)?;
                Ok(serde_json::json!({"state":record}))
            }
            // No hook writes, starts another job or acquires credentials.
            _ => Err(WorkerError::Refused),
        }
    }
}

/// Reevaluate a waiting native lease using its currently bound ordinary plugin.
/// Execute outside the placement lock, then offer one fresh decision to the
/// same admission path used for administrator-set limits.
pub(crate) async fn refresh<D: Directory>(
    state: &Shared<D>,
    machines: &Arc<MachineService>,
    account: AccountId,
    lease: Uuid,
) -> Result<(), Error> {
    let svc = machines.clone();
    let invocation = crate::blocking::spawn(move || svc.resource_hook_invocation(account, lease))
        .await
        .map_err(|_| Error::internal("resource admission task failed"))??;
    let Some(invocation) = invocation else {
        return Ok(());
    };
    let binding = invocation.binding.clone();
    let result = invoke(state, account, invocation)
        .await
        .unwrap_or_else(|| Decision::Queue {
            reason: "Resource policy is unavailable".into(),
            retry_after_ms: 1_000,
        });
    let svc = machines.clone();
    crate::blocking::spawn(move || svc.settle_resource_hook(account, lease, binding, result))
        .await
        .map_err(|_| Error::internal("resource admission task failed"))?
}

async fn invoke<D: Directory>(
    state: &Shared<D>,
    account: AccountId,
    mut request: Invocation,
) -> Option<Decision> {
    let plugin = state
        .registry
        .snapshot()
        .get(&request.binding.package)?
        .clone();
    if plugin.review_checksum.as_deref() != Some(request.binding.checksum.as_str()) {
        return None;
    }
    let mut matches = plugin.manifest.hooks.iter().filter(|hook| {
        hook.mode == ManifestHookMode::Block
            && hook.patterns.iter().any(|pattern| pattern == HOOK_TARGET)
    });
    let hook = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    let operation = plugin.operation(&hook.operation)?;
    if operation.mutates || operation.target.is_some() {
        return None;
    }
    let state_copy = state.clone();
    let space = request.space;
    let selected = plugin.clone();
    let (generation, usage) = crate::blocking::spawn(move || {
        let row = activation(&state_copy.store, space, &selected).ok()?;
        if !row.enabled {
            return None;
        }
        Some((
            row.generation,
            state_copy.store.content().storage_usage(space).ok()?,
        ))
    })
    .await
    .ok()??;
    request.input["storage"] = serde_json::json!({
        "text_bytes": usage.text_bytes.to_string(), "binary_bytes": usage.binary_bytes.to_string(),
    });
    let host = Arc::new(AdmissionHost {
        state: state.clone(),
        account,
        space,
        plugin: plugin.clone(),
        generation,
        binding: request.binding,
        owner: request.owner,
    });
    let outcome = state
        .workers
        .invoke(
            &plugin,
            space,
            Call {
                operation: hook.operation.clone(),
                input: request.input,
                account,
                host: host.clone(),
                timeout: HOOK_DEADLINE,
                cancel: None,
                job: None,
            },
        )
        .await;
    let Outcome::Complete(output) = outcome else {
        return None;
    };
    let capability = operation.required_capability;
    let current = crate::blocking::spawn(move || host.authorize(capability))
        .await
        .ok()?;
    if !current {
        return None;
    }
    serde_json::from_value::<Decision>(output)
        .ok()
        .filter(Decision::valid)
}
