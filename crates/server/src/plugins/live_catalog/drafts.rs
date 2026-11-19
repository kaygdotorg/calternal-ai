//! Single-space development packages. Never compiled into release images.
use super::{AccountId, Arc, Catalog, Package, Path, Shared, SpaceId, manager, reason};
use crate::plugins::Plugin;
use std::{sync::Mutex, time::Instant};

/// Publish a development revision after readiness. Bundles live in the scratch
/// cache only. A fresh process invalidates all outstanding draft revisions.
///
/// A failed candidate leaves the previous revision published and is remembered
/// (phase, message, elapsed) for `plugin diagnostics`: at most 16 packages, one
/// entry each, dropped by the next success or a restart.
pub(super) async fn publish(
    root: &Path,
    state: &Shared,
    base: &Catalog,
    package: Package,
    space: SpaceId,
    account: AccountId,
) -> Result<(), String> {
    let started = Instant::now();
    manager(state, space, account).await?;
    let id = package.manifest.id.clone();
    let phase = Mutex::new("validation");
    let result = publish_checked(root, state, base, package, space, account, started, &phase).await;
    let failed = state.registry.failed_drafts();
    match &result {
        Ok(()) => failed.clear(space, &id),
        Err(message) => failed.record(
            space,
            &id,
            *phase.lock().unwrap_or_else(|e| e.into_inner()),
            message,
            started.elapsed(),
        ),
    }
    result
}

#[allow(clippy::too_many_arguments)]
async fn publish_checked(
    root: &Path,
    state: &Shared,
    base: &Catalog,
    package: Package,
    space: SpaceId,
    account: AccountId,
    started: Instant,
    phase: &Mutex<&'static str>,
) -> Result<(), String> {
    let id = package.manifest.id.clone();
    let previous = base.get(&id).cloned();
    if previous
        .as_ref()
        .is_some_and(|p| p.draft_space != Some(space))
    {
        return Err("this package ID is already registered outside this development space".into());
    }
    if base.plugins().len() >= super::package_store::MAX_PACKAGES && previous.is_none() {
        return Err("the plugin catalog is full".into());
    }
    let recent = previous.clone();
    let root = root.to_owned();
    let plugin = crate::blocking::spawn(move || {
        package.validate()?;
        let entry = package.entry(true);
        let code = package
            .worker
            .ok_or("build dist/worker.js before loading this draft")?;
        let revision = super::package_store::next_draft_revision(&root)?;
        let file = root
            .join("installed-plugin-workers")
            .join(format!("{}.{revision}.js", entry.id));
        if let Some(prior) = recent.as_ref()
            && (prior.manifest.kinds != package.manifest.kinds
                || prior.manifest.collections != package.manifest.collections)
        {
            return Err(
                "a draft cannot change kind or collection declarations after activation".into(),
            );
        }
        let mut plugin = Plugin::new(entry, package.manifest, Some(file.clone()))?;
        crate::plugins::check_operations(&plugin)?;
        plugin.revision = revision;
        plugin.draft_space = Some(space);
        calternal_fsio::write_atomic_mode(&file, code.as_bytes(), 0o600).map_err(reason)?;
        Ok::<_, String>(Arc::new(plugin))
    })
    .await
    .map_err(reason)??;
    let result = activate(
        state,
        base,
        &plugin,
        space,
        account,
        previous.as_deref(),
        started,
        phase,
    )
    .await;
    if result.is_err() {
        state.workers.discard_candidate(&plugin, space);
        if let Some(path) = &plugin.worker {
            let _ = tokio::fs::remove_file(path).await;
        }
    } else if let Some(previous) = previous {
        if let Some(path) = &previous.worker {
            let _ = tokio::fs::remove_file(path).await;
        }
    }
    result
}

async fn activate(
    state: &Shared,
    base: &Catalog,
    plugin: &Arc<Plugin>,
    space: SpaceId,
    account: AccountId,
    previous: Option<&Plugin>,
    started: Instant,
    phase: &Mutex<&'static str>,
) -> Result<(), String> {
    let batch = base
        .plugins()
        .iter()
        .filter(|p| p.manifest.id != plugin.manifest.id)
        .cloned()
        .chain([plugin.clone()])
        .collect::<Vec<_>>();
    let original = base.clone();
    let next = crate::blocking::spawn(move || {
        let count = batch.len();
        let mut next = Catalog::from_plugins(batch);
        if next.plugins().len() != count {
            return Err("draft conflicts with a currently registered plugin".into());
        }
        next.retained = original.retained.clone();
        next.carry_v2(&original, None)?;
        Ok::<_, String>(next)
    })
    .await
    .map_err(reason)??;
    set_phase(phase, "handshake");
    let handshake_start = Instant::now();
    state
        .workers
        .prepare(plugin, space, account)
        .await
        .map_err(|error| error.message)?;
    let handshake = handshake_start.elapsed();
    set_phase(phase, "publication");
    manager(state, space, account).await?;
    if previous.is_none() {
        let st = state.clone();
        let plugin = plugin.clone();
        crate::blocking::spawn(move || {
            let collections = plugin
                .manifest
                .collections
                .iter()
                .map(|c| (c.kind.clone(), c.id.clone(), c.name.clone()))
                .collect::<Vec<_>>();
            st.store
                .provision_plugin(
                    space,
                    calternal_types::Actor::account(account),
                    &plugin.kind_registrations(),
                    &collections,
                )
                .map_err(|error| error.message)
        })
        .await
        .map_err(reason)??;
    }
    if let Some(previous) = previous {
        state
            .workers
            .retire_after_first_result(plugin, previous, space);
    }
    plugin.diagnostics.ready(
        handshake_start.duration_since(started),
        handshake,
        started.elapsed(),
    );
    state.registry.publish(next);
    Ok(())
}

/// Report the live revision (if any), its caller's recent errors and the last
/// failed candidate. The revision must still be active in the caller's space when
/// the result is returned. Cost: one bounded snapshot of in-memory state.
pub(super) async fn diagnostics(
    state: &Shared,
    id: &str,
    operation: &str,
    space: SpaceId,
    account: AccountId,
) -> Result<serde_json::Value, String> {
    manager(state, space, account).await?;
    let catalog = state.registry.snapshot();
    let failed = state.registry.failed_drafts().get(space, id);
    let plugin = catalog
        .get(id)
        .filter(|plugin| plugin.draft_space == Some(space));
    let mut result = match plugin {
        Some(plugin) if plugin.operation(operation).is_some() => {
            plugin
                .diagnostics
                .snapshot(account, operation, plugin.revision)
        }
        Some(_) => return Err("operation not declared by the loaded draft".into()),
        None if failed.is_some() => {
            serde_json::json!({"phase":"failed","ready_revision":null,"timing":null,"errors":[]})
        }
        None => return Err("no draft with this ID was loaded in this development space".into()),
    };
    result["last_failed_candidate"] = failed.unwrap_or(serde_json::Value::Null);
    result["id"] = serde_json::json!(id);
    result["operation"] = serde_json::json!(operation);
    result["space_id"] = serde_json::json!(space);
    manager(state, space, account).await?;
    let live = state.registry.snapshot();
    let still = match plugin {
        Some(plugin) => live.get(id).is_some_and(|live| Arc::ptr_eq(plugin, live)),
        None => live.get(id).is_none(),
    };
    if !still {
        return Err("draft revision changed; inspect diagnostics again".into());
    }
    Ok(result)
}

/// Record how far a candidate got; a poisoned lock still holds a valid label.
fn set_phase(phase: &Mutex<&'static str>, value: &'static str) {
    *phase.lock().unwrap_or_else(|e| e.into_inner()) = value;
}
