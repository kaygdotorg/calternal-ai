//! Trusted plugin registry over manifest v1, manifest-derived policy rows, and
//! the long-lived worker pool.
//!
//! Only plugins listed in the compiled-in `trusted-plugins.json` can load. The
//! browser can never supply code, paths or URLs. Each manifest is read once at
//! startup, parsed and validated by [`calternal_plugin_host`] without running
//! plugin code, and registered atomically (kind, operation, collection and folder
//! collisions are refused). Worker bundle SHA-256 digests are pinned in the trusted
//! file and checked again before every worker start.
//!
//! Policy: every plugin operation's authorization row is derived from its manifest
//! declaration ([`calternal_plugin_host::Registry::policy_rows`]) and checked through
//! the same [`Policy`] path as host operations. No plugin row is written by hand.
//!
//! Workers are NOT an OS security sandbox; see [`worker`] for what one can still reach.

#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

pub mod broker;
pub mod job_api;
pub mod job_sql;
pub mod jobs;
pub mod live;
pub mod runner;
pub mod session;
pub mod worker;

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use calternal_content::{FileFormat, Placement};
use calternal_core::{
    False, ManifestCapability, ManifestOperation, ManifestPlacement, ManifestTarget, ManifestV1,
    PluginManifest, PluginOperation,
};
use calternal_plugin_host as host;
use serde::Deserialize;

use crate::authz::{self, Constraint, Policy, Scope};
use calternal_types::{Capability, SpaceId};
use calternal_types::Error;
use crate::store::{PluginRow, Store, sha256_hex};

pub use broker::{InvocationHost, PREVIEW_FIXTURE, PREVIEW_ITEM_ID, PreviewHost};
pub use jobs::Jobs;
pub use session::Session;
pub use worker::{
    Call, CancelFlag, MAX_WORKERS, NodeRuntime, WORKER_IDLE, WORKER_TIMEOUT, WorkerPool,
    resolve_node,
};

const TRUSTED_JSON: &str = include_str!("../../trusted-plugins.json");

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TrustedFile {
    plugins: Vec<TrustedEntry>,
}

/// One compiled-in trusted plugin entry. Paths are relative to `--plugin-root`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrustedEntry {
    /// Plugin identifier; must equal the manifest's `id`.
    pub id: String,
    /// Exact version; must equal the manifest's `version`.
    pub version: String,
    /// Manifest path relative to the plugin root.
    pub manifest: String,
    /// Worker bundle path relative to the plugin root; `None` for plugins without code.
    pub worker: Option<String>,
    /// Pinned lowercase hex SHA-256 of the worker bundle.
    pub worker_sha256: Option<String>,
    /// Whether a space with no stored activation row treats the plugin as enabled.
    pub default_enabled: bool,
}

/// A trusted plugin whose manifest passed validation and registration.
#[derive(Debug)]
pub struct Plugin {
    /// The trusted entry it was loaded from.
    pub entry: TrustedEntry,
    /// Validated manifest v1; immutable for the server's lifetime.
    pub manifest: ManifestV1,
    /// Compatibility projection served by `plugins.list`/`enable`/`disable`, built once.
    pub wire: PluginManifest,
    /// Canonical absolute worker path, when the plugin has a built worker.
    pub worker: Option<PathBuf>,
    /// Why the plugin cannot execute or be enabled, if it cannot (digest mismatch etc).
    pub unavailable: Option<String>,
    /// Operation schemas compiled once, shared by every (plugin, space) worker.
    pub schemas: Arc<host::CompiledSchemas>,
}

impl Plugin {
    /// Build a plugin from an in-memory manifest, as trusted startup does after
    /// reading the file. Validates the manifest; does not check the worker digest
    /// (callers set [`Self::unavailable`] if it fails).
    /// Cost: manifest validation and schema compilation; no I/O.
    pub fn new(
        entry: TrustedEntry,
        manifest: ManifestV1,
        worker: Option<PathBuf>,
    ) -> Result<Self, String> {
        let schemas = Arc::new(
            host::CompiledSchemas::compile(&manifest).map_err(|e| format!("manifest invalid: {e}"))?,
        );
        if manifest.id != entry.id || manifest.version != entry.version {
            return Err(format!(
                "manifest id/version {}@{} does not match registry {}@{}",
                manifest.id, manifest.version, entry.id, entry.version
            ));
        }
        let wire = wire_manifest(&manifest);
        Ok(Self {
            entry,
            manifest,
            wire,
            worker,
            unavailable: None,
            schemas,
        })
    }

    /// The declared operation `id`, if this plugin declares it.
    /// Cost: O(operations), no allocation.
    pub fn operation(&self, id: &str) -> Option<&ManifestOperation> {
        self.manifest.operations.iter().find(|op| op.id == id)
    }

    /// The item a declared operation targets, or `None` for a space-scoped one.
    /// Cost: O(operations), no allocation.
    pub fn target(&self, operation: &str) -> Option<&ManifestTarget> {
        self.operation(operation).and_then(|op| op.target.as_ref())
    }

    /// Content registrations for every kind this plugin declares, in manifest order.
    /// Visible kinds become top-level folders; hidden kinds live under
    /// `.calternal/<plugin-id>/<kind>/`. Claimed text is stored as Markdown; any other
    /// kind is stored unparsed under its declared extension (manifest validation
    /// guarantees one). Cost: O(kinds) small allocations.
    pub fn kind_registrations(&self) -> Vec<(String, Placement, FileFormat)> {
        self.manifest
            .kinds
            .iter()
            .map(|kind| {
                let placement = match &kind.storage {
                    ManifestPlacement::Visible { folder } => Placement::Visible {
                        folder: folder.clone(),
                    },
                    ManifestPlacement::Hidden { .. } => Placement::Hidden {
                        plugin_id: self.manifest.id.clone(),
                    },
                };
                let format = match (&kind.extension, kind.claimed_text) {
                    (Some(extension), false) => FileFormat::Opaque {
                        extension: extension.clone(),
                    },
                    _ => FileFormat::Markdown,
                };
                (kind.id.clone(), placement, format)
            })
            .collect()
    }

    /// Whether this plugin declares `kind` (and therefore may write items of it).
    /// Cost: O(kinds), no allocation.
    pub fn declares_kind(&self, kind: &str) -> bool {
        self.manifest.kinds.iter().any(|k| k.id == kind)
    }
}

/// Immutable registry of trusted plugins, with manifest-derived policy rows.
#[derive(Debug, Default)]
pub struct Registry {
    plugins: Vec<Arc<Plugin>>,
    /// Operation ID -> (owning plugin index, derived policy).
    operations: BTreeMap<String, (usize, Policy)>,
}

/// Parse the compiled-in trusted plugin list. Cost: one small JSON parse.
pub fn registry_entries() -> Vec<TrustedEntry> {
    let file: TrustedFile =
        serde_json::from_str(TRUSTED_JSON).expect("compiled-in trusted-plugins.json is valid");
    file.plugins
}

/// Test-only trusted plugins (`tests/plugins/trusted-test-plugins.json`), compiled in
/// only with the `test-plugins` feature; empty otherwise. Cost: one small JSON parse.
pub fn test_entries() -> Vec<TrustedEntry> {
    #[cfg(feature = "test-plugins")]
    {
        let json = include_str!("../../../../tests/plugins/trusted-test-plugins.json");
        let file: TrustedFile = serde_json::from_str(json).expect("test plugin list is valid");
        file.plugins
    }
    #[cfg(not(feature = "test-plugins"))]
    Vec::new()
}

/// Stable dotted name of a manifest capability (the same vocabulary as [`Capability`]).
/// Cost: O(1); serializes one unit enum variant.
pub fn capability_name(capability: ManifestCapability) -> String {
    match serde_json::to_value(capability) {
        Ok(serde_json::Value::String(name)) => name,
        _ => String::new(),
    }
}

/// Map a manifest capability onto the host's closed capability set.
/// Unknown names cannot confer authority. Cost: O(capabilities).
pub fn host_capability(capability: ManifestCapability) -> Option<Capability> {
    Capability::from_name(&capability_name(capability))
}

impl Registry {
    /// An empty registry: no plugin operations exist. Cost: none.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Load every trusted entry from `plugin_root`. Plugins whose manifest is
    /// missing, invalid or colliding are skipped with a warning; plugins whose worker
    /// is missing or whose digest differs load but are marked unavailable.
    /// Cost: one bounded manifest read and one bundle hash per entry, at startup only.
    pub fn load(plugin_root: &Path) -> Self {
        Self::load_with_tests(plugin_root, None)
    }

    /// [`Self::load`], plus the test-only plugins from `test_root` when the server was
    /// built with the `test-plugins` feature (otherwise there are none to load).
    /// Cost: as [`Self::load`], per test entry as well.
    pub fn load_with_tests(plugin_root: &Path, test_root: Option<&Path>) -> Self {
        let mut plugins = Vec::new();
        let tests = test_root.map(|root| (root, test_entries()));
        let all = std::iter::once((plugin_root, registry_entries())).chain(tests);
        for (root, entry) in
            all.flat_map(|(root, entries)| entries.into_iter().map(move |e| (root, e)))
        {
            match load_one(root, &entry) {
                Ok(p) => plugins.push(p),
                Err(why) => {
                    tracing::warn!(plugin = %entry.id, reason = %why, "trusted plugin not loaded")
                }
            }
        }
        Self::from_plugins(plugins)
    }

    /// Register already-built plugins in order, skipping (with a warning) any whose
    /// declarations collide with an earlier plugin or a host operation, or whose
    /// operations need an authority that is not space-scoped.
    /// Cost: O(declarations * log registered) plus schema compilation per plugin.
    pub fn from_plugins(plugins: Vec<Plugin>) -> Self {
        let mut index = host::Registry::default();
        let mut out = Self::default();
        for plugin in plugins {
            let id = plugin.manifest.id.clone();
            if let Err(why) = check_operations(&plugin) {
                tracing::warn!(plugin = %id, reason = %why, "trusted plugin not loaded");
                continue;
            }
            if index.register(plugin.manifest.clone()).is_err() {
                tracing::warn!(plugin = %id, "trusted plugin collides with a registered plugin");
                continue;
            }
            match &plugin.unavailable {
                Some(why) => {
                    tracing::warn!(plugin = %id, reason = %why, "trusted plugin unavailable")
                }
                None => {
                    tracing::info!(plugin = %id, version = %plugin.manifest.version, "trusted plugin registered")
                }
            }
            out.plugins.push(Arc::new(plugin));
        }
        // Policy rows come from the manifests, never from a hand-written table.
        for row in index.policy_rows() {
            let Some(slot) = out.plugins.iter().position(|p| p.manifest.id == row.plugin) else {
                continue;
            };
            let item = row.operation.target.is_some();
            let Some(required) = host_capability(row.operation.required_capability) else {
                continue;
            };
            let policy = Policy {
                operation: Cow::Owned(row.operation.id.clone()),
                required,
                scope: if item { Scope::Note } else { Scope::Space },
                constraint: Constraint::PluginInvocation,
            };
            out.operations.insert(row.operation.id, (slot, policy));
        }
        out
    }

    /// Registered plugins in trusted-file order.
    pub fn plugins(&self) -> &[Arc<Plugin>] {
        &self.plugins
    }

    /// Look up a plugin by identifier. Cost: O(plugins), no allocation.
    pub fn get(&self, id: &str) -> Option<&Arc<Plugin>> {
        self.plugins.iter().find(|p| p.manifest.id == id)
    }

    /// The plugin that declares `operation`. Cost: O(log operations).
    pub fn owner_of_operation(&self, operation: &str) -> Option<&Arc<Plugin>> {
        self.operations
            .get(operation)
            .map(|(slot, _)| &self.plugins[*slot])
    }

    /// The single policy for `operation`: the host row if one exists, otherwise the
    /// row derived from the declaring plugin's manifest. `None` fails closed.
    /// Cost: bounded host-table scan plus O(log operations); no allocation.
    pub fn policy(&self, operation: &str) -> Option<&Policy> {
        authz::policy(operation).or_else(|| self.operations.get(operation).map(|(_, p)| p))
    }

    /// Every manifest-derived plugin policy row, ordered by operation ID.
    pub fn plugin_policies(&self) -> impl Iterator<Item = &Policy> {
        self.operations.values().map(|(_, p)| p)
    }
}

/// Reject plugins whose operations would shadow a host binding or require an
/// account-level capability that has no space scope.
fn check_operations(plugin: &Plugin) -> Result<(), String> {
    for op in &plugin.manifest.operations {
        if authz::policy(&op.id).is_some() {
            return Err(format!("operation {} shadows a host operation", op.id));
        }
        match host_capability(op.required_capability) {
            Some(Capability::SpacesList | Capability::SpacesCreate) | None => {
                return Err(format!("operation {} needs a non-space capability", op.id));
            }
            Some(_) => {}
        }
    }
    Ok(())
}

/// Compatibility projection of a v1 manifest onto the v0 discovery DTO the web
/// client still reads. It carries no authority. Lossy by design until the discovery
/// contract switches: v1 has no description or dependencies, its views are component
/// references rather than inline portable views (so none are projected), and the v0
/// operation shape cannot express mutating operations (so those are omitted).
/// Capabilities list each distinct required capability, then each host call.
/// Cost: O(operations + host calls) small allocations, once per plugin at startup.
pub fn wire_manifest(manifest: &ManifestV1) -> PluginManifest {
    let mut capabilities: Vec<String> = Vec::new();
    for op in &manifest.operations {
        let name = capability_name(op.required_capability);
        if !capabilities.contains(&name) {
            capabilities.push(name);
        }
    }
    for call in &manifest.host_calls {
        if let Ok(serde_json::Value::String(name)) = serde_json::to_value(call) {
            capabilities.push(name);
        }
    }
    PluginManifest {
        id: manifest.id.clone(),
        version: manifest.version.clone(),
        api_version: manifest.api_version,
        name: manifest.name.clone(),
        description: String::new(),
        dependencies: vec![],
        capabilities,
        operations: manifest
            .operations
            .iter()
            .filter(|op| !op.mutates)
            .map(|op| PluginOperation {
                id: op.id.clone(),
                description: String::new(),
                mutates: False,
            })
            .collect(),
        views: vec![],
    }
}

/// Effective activation of `plugin` in one content space: the stored row, or the
/// trusted default, and never enabled while the plugin is unavailable.
/// Cost: one indexed plugin-state read; no content I/O.
pub fn activation(store: &Store, space: SpaceId, plugin: &Plugin) -> Result<PluginRow, Error> {
    let mut row = store
        .plugin_row(space, &plugin.manifest.id)?
        .unwrap_or_else(|| default_row(plugin));
    row.enabled &= plugin.unavailable.is_none();
    Ok(row)
}

/// Row used before a space stores any activation for `plugin`.
pub fn default_row(plugin: &Plugin) -> PluginRow {
    PluginRow {
        enabled: plugin.entry.default_enabled && plugin.unavailable.is_none(),
        generation: 1,
    }
}

fn rel_path_ok(p: &str) -> bool {
    !p.is_empty()
        && !p.starts_with('/')
        && Path::new(p)
            .components()
            .all(|c| matches!(c, std::path::Component::Normal(_)))
}

fn load_one(root: &Path, entry: &TrustedEntry) -> Result<Plugin, String> {
    if !rel_path_ok(&entry.manifest) {
        return Err("registry manifest path invalid".into());
    }
    let path = root.join(&entry.manifest);
    let meta = std::fs::symlink_metadata(&path).map_err(|e| format!("manifest: {e}"))?;
    if !meta.file_type().is_file() {
        return Err("manifest is not a regular file".into());
    }
    if meta.len() > host::MAX_MANIFEST_BYTES as u64 {
        return Err("manifest too large".into());
    }
    let bytes = std::fs::read(&path).map_err(|e| format!("manifest: {e}"))?;
    let manifest = host::parse_manifest(&bytes).map_err(|e| format!("manifest invalid: {e}"))?;
    let mut unavailable = None;
    let worker = match &entry.worker {
        None => None,
        Some(w) if !rel_path_ok(w) => return Err("registry worker path invalid".into()),
        Some(w) => match std::fs::canonicalize(root.join(w)) {
            Ok(abs) => {
                if let Err(why) = verify_digest(&abs, entry.worker_sha256.as_deref()) {
                    unavailable = Some(why);
                }
                Some(abs)
            }
            Err(_) => {
                unavailable = Some("worker bundle not built".into());
                None
            }
        },
    };
    let mut plugin = Plugin::new(entry.clone(), manifest, worker)?;
    plugin.unavailable = unavailable;
    Ok(plugin)
}

/// SHA-256 of a regular file as lowercase hex. Cost: reads the whole file once.
pub fn file_sha256(path: &Path) -> Result<String, String> {
    let meta = std::fs::symlink_metadata(path).map_err(|e| format!("worker: {e}"))?;
    if !meta.file_type().is_file() {
        return Err("worker bundle is not a regular file".into());
    }
    let bytes = std::fs::read(path).map_err(|e| format!("worker: {e}"))?;
    Ok(sha256_hex(&bytes))
}

/// Check a bundle against its pinned digest. Cost: one file hash.
pub fn verify_digest(path: &Path, expected: Option<&str>) -> Result<(), String> {
    let actual = file_sha256(path)?;
    match expected {
        Some(e) if e.eq_ignore_ascii_case(&actual) => Ok(()),
        Some(_) => Err("worker bundle digest does not match trusted registry".into()),
        None => Err("worker bundle digest not pinned in trusted registry".into()),
    }
}
