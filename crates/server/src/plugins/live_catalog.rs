//! Private operator transport for publishing validated package revisions.
use std::{
    io::{Read, Write},
    os::unix::{fs::PermissionsExt, net::UnixStream},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use calternal_types::{AccountId, Capability, SpaceId};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::{
    Catalog,
    package_store::{self, Package, PackageV2},
};
use crate::{api::Shared, auth::Directory};

#[cfg(feature = "dev-plugin-drafts")]
mod drafts;

const TIMEOUT: Duration = Duration::from_secs(20);
// The v2 UTF-8 archive is carried as one string. Its manifest can need
// up to one extra byte per byte for JSON escaping in the operator frame.
const MAX_REQUEST: usize =
    package_store::MAX_ARCHIVE + calternal_plugin_host::MAX_MANIFEST_BYTES + 4096;
const MAX_REPLY: usize = 4096;

/// Local installation action, authorized by ownership of the private operator socket.
/// The optional placement fields are required when the package has a worker.
#[derive(Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Change {
    /// Publish a checksum-reviewed package after its worker is ready.
    Install {
        /// Bounded package archive and manifest.
        package: Package,
        /// Reviewed archive checksum.
        approval: String,
        /// True for replacing a registered package.
        update: bool,
        /// Space used to start the candidate sandbox.
        space: Option<SpaceId>,
        /// Enrolled account whose machine allocation starts the sandbox.
        account: Option<AccountId>,
    },
    /// Register a checksum-reviewed v2 package without enabling execution.
    InstallV2 {
        /// Exact reviewed v2 archive as UTF-8 JSON.
        archive: String,
        /// Reviewed archive checksum.
        approval: String,
    },
    /// Activate an unpinned package in a single development space.
    #[cfg(feature = "dev-plugin-drafts")]
    Draft {
        /// Bounded package archive and manifest.
        package: Package,
        /// Target development space.
        space: SpaceId,
        /// Enrolled development account with plugin management access.
        account: AccountId,
    },
    /// Inspect the current draft revision and the requesting author's failures.
    #[cfg(feature = "dev-plugin-drafts")]
    Diagnostics {
        /// Draft package ID.
        id: String,
        /// Declared operation whose recent errors are requested.
        operation: String,
        /// Draft space.
        space: SpaceId,
        /// Development Manager whose own diagnostics are requested.
        account: AccountId,
    },
    /// Unpublish a package and fence its existing workers.
    Remove {
        /// Registered package identifier.
        id: String,
    },
}

impl Change {
    /// Frame the exact archive approved by review, keeping its raw manifest
    /// intact across the versioned socket envelope. Cost: one serialization.
    pub fn install_v2(package: &PackageV2, approval: String) -> Result<Self, String> {
        let archive = String::from_utf8(package.archive()?).map_err(reason)?;
        Ok(Self::InstallV2 { archive, approval })
    }
}

fn socket(appdata: &Path) -> PathBuf {
    crate::machines::admin::socket_path(appdata).with_extension("plg")
}

fn reason(error: impl std::fmt::Display) -> String {
    error.to_string()
}

/// Apply a package change through the running server's private socket.
/// Cost: one bounded request and reply. No direct access to the catalog database.
pub fn request(appdata: &Path, change: Change) -> Result<(), String> {
    exchange(appdata, change).map(|_| ())
}

/// Inspect a draft through the private development socket. Refuses foreign
/// spaces and callers without current management authority. Cost: one
/// bounded local request and reply.
#[cfg(feature = "dev-plugin-drafts")]
pub fn diagnostics(
    appdata: &Path,
    id: String,
    operation: String,
    space: SpaceId,
    account: AccountId,
) -> Result<serde_json::Value, String> {
    exchange(
        appdata,
        Change::Diagnostics {
            id,
            operation,
            space,
            account,
        },
    )?
    .ok_or_else(|| "development server returned no diagnostics".into())
}

fn exchange(appdata: &Path, change: Change) -> Result<Option<serde_json::Value>, String> {
    let bytes = serde_json::to_vec(&change).map_err(reason)?;
    if bytes.len() > MAX_REQUEST {
        return Err("plugin request exceeds its byte budget".into());
    }
    let mut stream = UnixStream::connect(socket(appdata))
        .map_err(|error| format!("cannot reach the running plugin catalog: {error}"))?;
    stream.set_read_timeout(Some(TIMEOUT)).map_err(reason)?;
    stream.set_write_timeout(Some(TIMEOUT)).map_err(reason)?;
    stream
        .write_all(&(bytes.len() as u32).to_be_bytes())
        .map_err(reason)?;
    stream.write_all(&bytes).map_err(reason)?;
    let mut reply = Vec::new();
    stream
        .take((MAX_REPLY + 1) as u64)
        .read_to_end(&mut reply)
        .map_err(reason)?;
    if reply.len() > MAX_REPLY {
        return Err("plugin catalog reply exceeds its byte budget".into());
    }
    serde_json::from_slice(&reply).map_err(reason)?
}

/// Start the installation-local operator listener. One writer at a time validates
/// and publishes a snapshot, while request admissions retain their existing snapshot.
/// Cost: one local listener and one asynchronous task for the installation.
pub fn serve(
    appdata: PathBuf,
    state: Shared,
    _development: bool,
) -> std::io::Result<tokio::task::JoinHandle<()>> {
    let path = socket(&appdata);
    match std::fs::remove_file(&path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let listener = tokio::net::UnixListener::bind(&path)?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    Ok(tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            if !stream
                .peer_cred()
                .is_ok_and(|peer| peer.uid() == rustix::process::getuid().as_raw())
            {
                continue;
            }
            let change = tokio::time::timeout(TIMEOUT, read(&mut stream)).await;
            let result: Result<Option<serde_json::Value>, String> = match change {
                #[cfg(feature = "dev-plugin-drafts")]
                Ok(Ok(Change::Diagnostics {
                    id,
                    operation,
                    space,
                    account,
                })) => {
                    if _development {
                        drafts::diagnostics(&state, &id, &operation, space, account)
                            .await
                            .map(Some)
                    } else {
                        Err("diagnostics require a development server".into())
                    }
                }
                Ok(Ok(change)) => apply(&appdata, &state, change, _development)
                    .await
                    .map(|()| None),
                Ok(Err(error)) => Err(error),
                Err(_) => Err("plugin request timed out".into()),
            };
            if let Ok(bytes) = serde_json::to_vec(&result) {
                let _ = tokio::time::timeout(TIMEOUT, stream.write_all(&bytes)).await;
            }
        }
    }))
}

async fn read(stream: &mut tokio::net::UnixStream) -> Result<Change, String> {
    let size = stream.read_u32().await.map_err(reason)? as usize;
    if size > MAX_REQUEST {
        return Err("plugin request exceeds its byte budget".into());
    }
    let mut bytes = vec![0; size];
    stream.read_exact(&mut bytes).await.map_err(reason)?;
    crate::blocking::spawn(move || serde_json::from_slice(&bytes).map_err(reason))
        .await
        .map_err(reason)?
}

async fn apply(
    appdata: &Path,
    state: &Shared,
    change: Change,
    _development: bool,
) -> Result<(), String> {
    let base = state.registry.snapshot();
    let root = appdata.to_owned();
    match change {
        #[cfg(feature = "dev-plugin-drafts")]
        Change::Diagnostics { .. } => {
            return Err("diagnostic request sent to package writer".into());
        }
        #[cfg(feature = "dev-plugin-drafts")]
        Change::Draft {
            package,
            space,
            account,
        } => {
            if !_development {
                return Err("drafts require a development server".into());
            }
            return drafts::publish(appdata, state, &base, package, space, account).await;
        }
        Change::Install {
            package,
            approval,
            update,
            space,
            account,
        } => {
            #[cfg(feature = "dev-plugin-drafts")]
            if base
                .get(&package.manifest.id)
                .is_some_and(|plugin| plugin.draft_space.is_some())
            {
                return Err("a draft already uses this package ID".into());
            }
            let candidate = crate::blocking::spawn(move || {
                package_store::prepare(&root, package, approval, update)
            })
            .await
            .map_err(reason)??;
            let Some(candidate) = candidate else {
                return Ok(());
            };
            let plugin = candidate.plugin.clone();
            let result =
                activate_install(appdata, state, &base, candidate, space.zip(account)).await;
            if result.is_err() {
                if let Some(space) = space {
                    state.workers.discard_candidate(&plugin, space);
                }
                if let Some(path) = &plugin.worker {
                    let _ = tokio::fs::remove_file(path).await;
                }
            }
            return result;
        }
        Change::InstallV2 { archive, approval } => {
            let next = crate::blocking::spawn(move || {
                install_v2(&root, &base, archive.as_bytes(), &approval)
            })
            .await
            .map_err(reason)??;
            state.registry.publish(next);
        }
        Change::Remove { id } => {
            let prior = base.clone();
            let next = crate::blocking::spawn(move || {
                let next = catalog(&base, &id, None)?;
                package_store::remove(&root, &id)?;
                Ok::<_, String>(next)
            })
            .await
            .map_err(reason)??;
            publish(state, &prior, next).await;
        }
    }
    Ok(())
}

fn install_v2(
    appdata: &Path,
    base: &Catalog,
    archive: &[u8],
    approval: &str,
) -> Result<Catalog, String> {
    let manifest = package_store::validate_v2_archive(archive)?;
    let mut next = base.clone();
    next.register_v2(manifest.clone())?;
    package_store::commit_v2(appdata, manifest.id(), archive, approval)?;
    Ok(next)
}

async fn activate_install(
    appdata: &Path,
    state: &Shared,
    base: &Arc<Catalog>,
    candidate: package_store::Candidate,
    target: Option<(SpaceId, AccountId)>,
) -> Result<(), String> {
    let plugin = candidate.plugin.clone();
    let previous = base.clone();
    let next = crate::blocking::spawn(move || {
        catalog(&previous, &plugin.manifest.id, Some(plugin.clone()))
    })
    .await
    .map_err(reason)??;
    if candidate.plugin.worker.is_some() {
        let (space, account) = target.ok_or("a worker install requires --space and --account")?;
        manager(state, space, account).await?;
        state
            .workers
            .prepare(&candidate.plugin, space, account)
            .await
            .map_err(|e| e.message)?;
        manager(state, space, account)
            .await
            .map_err(|_| "plugin management access changed during readiness")?;
    }
    let root = appdata.to_owned();
    crate::blocking::spawn(move || package_store::commit(&root, candidate))
        .await
        .map_err(reason)??;
    publish(state, base, next).await;
    Ok(())
}

async fn publish(state: &Shared, base: &Catalog, next: Catalog) {
    let obsolete: Vec<_> = base
        .plugins()
        .iter()
        .chain(base.retained.values())
        .filter(|plugin| !next.authorizes(plugin))
        .filter(|plugin| plugin.revision != 0)
        .filter_map(|plugin| plugin.worker.clone())
        .collect();
    state.registry.publish(next);
    for path in obsolete {
        if let Err(error) = tokio::fs::remove_file(&path).await {
            tracing::warn!(path = %path.display(), %error, "could not remove retired plugin bundle");
        }
    }
}

async fn manager(state: &Shared, space: SpaceId, account: AccountId) -> Result<(), String> {
    let allowed = state
        .directory
        .capabilities_async(account, space)
        .ok_or("the space directory is unavailable")?
        .await
        .is_some_and(|caps| caps.contains(Capability::PluginsManage));
    if !allowed {
        return Err("the account cannot manage plugins in the readiness space".into());
    }
    Ok(())
}

fn catalog(
    base: &Catalog,
    id: &str,
    replacement: Option<Arc<super::Plugin>>,
) -> Result<Catalog, String> {
    let mut entries: Vec<_> = base
        .plugins()
        .iter()
        .filter(|p| p.manifest.id != id)
        .cloned()
        .collect();
    entries.extend(replacement.iter().cloned());
    let expected = entries.len();
    let mut next = Catalog::from_plugins(entries);
    if next.plugins().len() != expected {
        return Err("candidate conflicts with a currently registered plugin".into());
    }
    next.retained = base.retained.clone();
    next.carry_v2(base, replacement.is_none().then_some(id))?;
    match (base.get(id), replacement) {
        (Some(old), Some(new))
            if !package_store::authority_changed(&new.manifest, &old.manifest) =>
        {
            if next.retained.len() >= package_store::MAX_RETAINED {
                return Err("too many live package revisions are retained".into());
            }
            next.retained
                .insert((id.to_owned(), old.revision), old.clone());
        }
        _ => next.retained.retain(|(name, _), _| name != id),
    }
    Ok(next)
}

#[cfg(test)]
mod tests;
