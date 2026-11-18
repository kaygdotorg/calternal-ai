//! Single-space development packages. Never compiled into release images.
use super::{AccountId, Arc, Catalog, Package, Path, Shared, SpaceId, manager, reason};
use crate::plugins::Plugin;

/// Publish a development revision after readiness. Bundles live in the scratch
/// cache only. A fresh process invalidates all outstanding draft revisions.
pub(super) async fn publish(
    root: &Path,
    state: &Shared,
    base: &Catalog,
    package: Package,
    space: SpaceId,
    account: AccountId,
) -> Result<(), String> {
    manager(state, space, account).await?;
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
    let result = activate(state, base, &plugin, space, account, previous.as_deref()).await;
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
) -> Result<(), String> {
    let batch = base
        .plugins()
        .iter()
        .filter(|p| p.manifest.id != plugin.manifest.id)
        .cloned()
        .chain([plugin.clone()])
        .collect::<Vec<_>>();
    let retained = base.retained.clone();
    let next = crate::blocking::spawn(move || {
        let count = batch.len();
        let mut next = Catalog::from_plugins(batch);
        if next.plugins().len() != count {
            return Err("draft conflicts with a currently registered plugin".into());
        }
        next.retained = retained;
        Ok::<_, String>(next)
    })
    .await
    .map_err(reason)??;
    state
        .workers
        .prepare(plugin, space, account)
        .await
        .map_err(|error| error.message)?;
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
    state.registry.publish(next);
    Ok(())
}
