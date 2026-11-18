//! Trusted plugin registry over manifest v1, manifest-derived policy rows, and
//! the long-lived worker pool.
//!
//! Startup loads an immutable snapshot of operator-reviewed packages. Shipped
//! entries seed it on first start. Manifests are validated without running code,
//! and worker bundle hashes are checked again before each worker start.
//!
//! Policy: every plugin operation's authorization row is derived from its manifest
//! declaration ([`calternal_plugin_host::Registry::policy_rows`]) and checked through
//! the same [`Policy`] path as host operations. No plugin row is written by hand.
//!
//! Workers are NOT an OS security sandbox; see [`worker`] for what one can still reach.

#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

pub mod agent_access;
pub mod agent_settings;
pub(crate) use agent_settings::UPGRADE_SCHEMA as AGENT_SETTINGS_SCHEMA;
pub mod broker;
#[cfg(feature = "dev-plugin-drafts")]
mod draft_diagnostics;
pub(crate) mod harness_admin;
pub mod job_api;
pub mod job_sql;
mod job_streams;
pub mod jobs;
pub mod live;
pub mod live_catalog;
mod machine_worker;
pub mod package_store;
mod requests;
pub(crate) use requests::OnceAdmission;
pub mod runner;
pub mod session;
pub(crate) mod settings;
mod slots;
pub mod worker;

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use calternal_content::{FileFormat, Placement};
use calternal_core::{
    ManifestCapability, ManifestOperation, ManifestPlacement, ManifestTarget, ManifestV1,
};
use calternal_plugin_host as host;
use serde::Deserialize;

use crate::authz::{self, Constraint, Policy, Scope};
use crate::store::{PluginRow, Store, sha256_hex};
use calternal_types::Error;
use calternal_types::{Capability, SpaceId};

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

/// A distribution plugin entry. Paths are relative to `--plugin-root`.
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
    /// Palette data projected once from this immutable manifest version.
    pub(crate) palette: Option<calternal_core::PaletteManifest>,
    /// Canonical absolute worker path, when the plugin has a built worker.
    pub worker: Option<PathBuf>,
    /// Why the plugin cannot execute or be enabled, if it cannot (digest mismatch etc).
    pub unavailable: Option<String>,
    /// Operation schemas compiled once, shared by every (plugin, space) worker.
    pub schemas: Arc<host::CompiledSchemas>,
    revision: u32,
    activation_approval: u32,
    #[cfg(feature = "dev-plugin-drafts")]
    draft_space: Option<SpaceId>,
    #[cfg(feature = "dev-plugin-drafts")]
    diagnostics: draft_diagnostics::DraftDiagnostics,
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
            host::CompiledSchemas::compile(&manifest)
                .map_err(|e| format!("manifest invalid: {e}"))?,
        );
        if manifest.id != entry.id || manifest.version != entry.version {
            return Err(format!(
                "manifest id/version {}@{} does not match registry {}@{}",
                manifest.id, manifest.version, entry.id, entry.version
            ));
        }
        let palette = manifest
            .palette
            .as_ref()
            .map(|menu| calternal_core::PaletteManifest {
                plugin: manifest.id.clone(),
                version: manifest.version.clone(),
                commands: menu
                    .commands
                    .iter()
                    .cloned()
                    .chain(menu.settings_sections.iter().map(|section| {
                        calternal_core::PaletteCommand {
                            id: section.id.clone(),
                            label: section.label.clone(),
                            group: "Settings".into(),
                            words: Vec::new(),
                            go: Some(section.go.clone()),
                            op: None,
                        }
                    }))
                    .collect(),
                settings_sections: menu.settings_sections.clone(),
            });
        Ok(Self {
            entry,
            manifest,
            palette,
            worker,
            unavailable: None,
            schemas,
            revision: 0,
            activation_approval: 0,
            #[cfg(feature = "dev-plugin-drafts")]
            draft_space: None,
            #[cfg(feature = "dev-plugin-drafts")]
            diagnostics: Default::default(),
        })
    }

    /// Whether this package is visible in the given space. Cost: one comparison.
    pub(crate) fn available_in(&self, _space: SpaceId) -> bool {
        #[cfg(feature = "dev-plugin-drafts")]
        if let Some(scope) = self.draft_space {
            return scope == _space;
        }
        true
    }

    /// Revision that durable jobs created by this worker must continue under.
    /// Cost: one field access, without storage I/O.
    fn job_revision(&self) -> u32 {
        if self.worker.is_some() {
            self.revision
        } else {
            0
        }
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
                    ManifestPlacement::VisibleTree {
                        folder,
                        subdirectories,
                    } => Placement::VisibleTree {
                        folder: folder.clone(),
                        subdirectories: subdirectories.clone(),
                    },
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

/// Atomically published plugin catalog. Reader snapshots never lock the catalog writer.
#[derive(Debug, Default)]
pub struct Registry(
    arc_swap::ArcSwap<Catalog>,
    #[cfg(feature = "dev-plugin-drafts")] draft_diagnostics::FailedCandidates,
);

/// Immutable installed plugins and their derived operation policy.
#[derive(Clone, Debug, Default)]
pub struct Catalog {
    plugins: Vec<Arc<Plugin>>,
    retained: BTreeMap<(String, u32), Arc<Plugin>>,
    tool_index: std::sync::OnceLock<String>,
    search: std::sync::OnceLock<Arc<calternal_content::SearchConfig>>,
    slots: slots::SlotIndex,
    /// Operation ID -> (owning plugin index, derived policy).
    operations: BTreeMap<String, (usize, Policy)>,
}

/// Parse the source tree's reference catalog for fixtures and digest reports.
/// Serving installations use the distribution file. Cost: one small JSON parse.
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
    /// Pin the catalog for one admission or lookup. Cost: one atomic reference load.
    pub fn snapshot(&self) -> Arc<Catalog> {
        self.0.load_full()
    }

    /// Publish a fully validated snapshot. Existing references stay valid for admitted work.
    fn publish(&self, catalog: Catalog) {
        self.0.store(Arc::new(catalog));
    }

    /// Build the initial catalog from known packages. Cost: validates the supplied declarations.
    pub fn from_plugins(plugins: Vec<Plugin>) -> Self {
        Self(
            arc_swap::ArcSwap::from_pointee(Catalog::from_plugins(
                plugins.into_iter().map(Arc::new).collect(),
            )),
            #[cfg(feature = "dev-plugin-drafts")]
            Default::default(),
        )
    }

    /// Failed development candidates, kept in memory for author inspection.
    #[cfg(feature = "dev-plugin-drafts")]
    pub(crate) fn failed_drafts(&self) -> &draft_diagnostics::FailedCandidates {
        &self.1
    }

    /// An empty registry: no plugin operations exist. Cost: none.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Load the shipped fixture catalog directly for in-memory server fixtures.
    /// The serving binary seeds and loads packages through [`package_store::load`].
    /// Cost: one manifest read and optional bundle verification per shipped package.
    pub fn load(plugin_root: &Path) -> Self {
        let plugins = registry_entries()
            .into_iter()
            .filter_map(|entry| match load_one(plugin_root, &entry) {
                Ok(plugin) => Some(plugin),
                Err(why) => {
                    tracing::warn!(plugin = %entry.id, reason = %why, "fixture plugin not loaded");
                    None
                }
            })
            .collect();
        Self::from_plugins(plugins)
    }
}

impl Catalog {
    /// Derive the search view once per immutable snapshot. Builds from trusted,
    /// available packages, with no storage access on subsequent requests.
    pub(crate) fn search_configuration(&self) -> &Arc<calternal_content::SearchConfig> {
        self.search.get_or_init(|| {
            let kinds = self
                .plugins()
                .iter()
                .filter(|plugin| plugin.unavailable.is_none())
                .flat_map(|plugin| {
                    let default = default_row(plugin);
                    plugin.manifest.kinds.iter().filter_map(move |kind| {
                        (kind.searchable == Some(true)).then(|| calternal_content::SearchKind {
                            kind: kind.id.clone(),
                            control: crate::store::plugin_key(&plugin.manifest.id),
                            default_enabled: default.enabled,
                            approval: default.approval,
                            space: {
                                #[cfg(feature = "dev-plugin-drafts")]
                                {
                                    plugin.draft_space
                                }
                                #[cfg(not(feature = "dev-plugin-drafts"))]
                                {
                                    None
                                }
                            },
                        })
                    })
                })
                .collect();
            Arc::new(calternal_content::SearchConfig::new(kinds))
        })
    }

    /// Generated one-line operation index, built once after registry construction.
    /// Includes manifest operations as well as host operations; never grants access.
    pub fn tool_index(&self) -> &str {
        self.tool_index.get_or_init(|| {
            let mut rows: Vec<_> = calternal_core::operation_catalog()
                .into_iter()
                .map(|d| format!("{}: {} ({:?})", d.id, d.description, d.risk))
                .collect();
            for p in &self.plugins {
                for op in &p.manifest.operations {
                    rows.push(format!("{}: {} ({:?})", op.id, p.manifest.name, op.risk));
                }
            }
            rows.join("\n")
        })
    }

    /// Register already-built plugins in order, skipping (with a warning) any whose
    /// declarations collide with an earlier plugin or a host operation, or whose
    /// operations need an authority that is not space-scoped.
    /// Cost: O(declarations * log registered) plus schema compilation per plugin.
    fn from_plugins(plugins: Vec<Arc<Plugin>>) -> Self {
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
            out.plugins.push(plugin);
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
                scope: if item { Scope::Item } else { Scope::Space },
                constraint: Constraint::PluginInvocation,
            };
            out.operations.insert(row.operation.id, (slot, policy));
        }
        out.slots = slots::SlotIndex::compile(&out);
        out
    }

    /// Whether there is any registered mount or compatible contribution.
    /// Cost: two checks on the immutable index.
    pub(crate) fn has_slots(&self) -> bool {
        !self.slots.is_empty()
    }

    /// Resolve a listing from activation rows in catalog order. Invisible packages
    /// occupy empty positions. Cost: one bounded scan, no I/O or package execution.
    pub(crate) fn resolve_slots(
        &self,
        plugins: &[Option<calternal_core::PluginState>],
        authority: calternal_types::CapabilitySet,
    ) -> (
        Vec<calternal_core::ResolvedPluginSlot>,
        Vec<calternal_core::ResolvedOwnedPluginSlot>,
    ) {
        self.slots.resolve(plugins, authority)
    }

    /// Registered plugins in trusted-file order.
    pub fn plugins(&self) -> &[Arc<Plugin>] {
        &self.plugins
    }

    /// Look up a plugin by identifier. Cost: O(plugins), no allocation.
    pub fn get(&self, id: &str) -> Option<&Arc<Plugin>> {
        self.plugins.iter().find(|p| p.manifest.id == id)
    }

    /// Choose the distribution's default provider if installed, otherwise the
    /// first available provider. An installation with no provider keeps its
    /// normal thread metadata default until a provider is installed.
    pub(crate) fn default_provider(&self) -> &str {
        self.plugins
            .iter()
            .filter(|plugin| plugin.manifest.provider.is_some() && plugin.unavailable.is_none())
            .max_by_key(|plugin| plugin.entry.default_enabled)
            .map_or("calternal-claude-code", |plugin| {
                plugin.manifest.id.as_str()
            })
    }

    /// Find code already pinned to an invocation or durable job. Removed and narrowed
    /// revisions disappear at publication. Cost: one bounded lookup.
    pub(crate) fn revision(&self, id: &str, revision: u32) -> Option<&Arc<Plugin>> {
        self.get(id)
            .filter(|plugin| plugin.revision == revision)
            .or_else(|| self.retained.get(&(id.to_owned(), revision)))
    }

    /// Check that the current catalog still authorizes this immutable revision.
    /// Cost: indexed in-memory lookups; per-space activation is checked separately.
    pub(crate) fn authorizes(&self, plugin: &Plugin) -> bool {
        self.revision(&plugin.manifest.id, plugin.revision)
            .is_some()
    }

    /// Default persisted control used when a manager enables or disables a package.
    /// Preserve the generation of implicitly enabled older revisions during approval.
    /// Cost: bounded scan of retained declarations, only on an activation write.
    pub(crate) fn control_default(&self, plugin: &Plugin) -> PluginRow {
        let mut row = default_row(plugin);
        row.enabled |= self.retained.values().any(|previous| {
            previous.manifest.id == plugin.manifest.id && default_row(previous).enabled
        });
        row
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

/// Effective activation of `plugin` in one content space: the stored row, or the
/// trusted default, and never enabled while the plugin is unavailable.
/// Cost: one indexed plugin-state read; no content I/O.
pub fn activation(store: &Store, space: SpaceId, plugin: &Plugin) -> Result<PluginRow, Error> {
    let mut row = store
        .plugin_row(space, &plugin.manifest.id)?
        .unwrap_or_else(|| default_row(plugin));
    row.enabled &= plugin.available_in(space)
        && plugin.unavailable.is_none()
        && row.approval >= plugin.activation_approval;
    Ok(row)
}

/// Row used before a space stores any activation for `plugin`.
pub fn default_row(plugin: &Plugin) -> PluginRow {
    PluginRow {
        enabled: plugin.entry.default_enabled && plugin.unavailable.is_none(),
        generation: 1,
        approval: plugin.activation_approval,
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

#[cfg(test)]
mod job_claim_tests;
