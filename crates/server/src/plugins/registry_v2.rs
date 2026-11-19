//! Immutable v2 declaration indexes carried by the existing plugin catalog.
use std::{collections::BTreeMap, sync::Arc};

use calternal_core::ManifestPlacement;
use calternal_fsio::names::fold;
use calternal_plugin_host::ManifestV2;
use calternal_types::Capability;
use serde_json::Value;

use super::Catalog;

#[derive(Clone, Debug, Default)]
pub(super) struct Declarations {
    packages: BTreeMap<String, Arc<ManifestV2>>,
    kinds: BTreeMap<String, (Arc<ManifestV2>, usize)>,
    operations: BTreeMap<String, (Arc<ManifestV2>, usize)>,
    folders: BTreeMap<String, String>,
}

impl Catalog {
    /// Installed v2 declaration by package ID. Cost: O(log packages), no I/O.
    /// Callers still check space activation and current viewer authority.
    pub fn v2_manifest(&self, package: &str) -> Option<&ManifestV2> {
        self.v2.packages.get(package).map(Arc::as_ref)
    }

    /// Owning package and checked kind declaration, including storage, indexed
    /// fields, search and creation. Cost: O(log kinds), no I/O or wake.
    pub fn v2_kind(&self, kind: &str) -> Option<(&ManifestV2, &Value)> {
        let (package, position) = self.v2.kinds.get(kind)?;
        Some((package, &package.declaration()["kinds"][*position]))
    }

    /// Owning package and checked operation declaration, with no dispatch
    /// authority. Cost: O(log operations), no I/O or wake.
    pub fn v2_operation(&self, operation: &str) -> Option<(&ManifestV2, &Value)> {
        let (package, position) = self.v2.operations.get(operation)?;
        Some((package, &package.declaration()["operations"][*position]))
    }

    pub(super) fn check_v2_reads(&self, manifest: &ManifestV2) -> Result<(), String> {
        let data = manifest.declaration();
        for kind in data["reads"].as_array().into_iter().flatten() {
            let id = kind.as_str().expect("checked kind ID");
            let local = data["kinds"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|decl| decl["id"] == id);
            if !local
                && self.v2_kind(id).is_none()
                && !self.v1_owns_kind(id)
                && !ManifestV2::host_owns_kind(id)
            {
                return Err(format!("Declared read kind {id} is not installed"));
            }
        }
        Ok(())
    }

    pub(super) fn register_v2(&mut self, manifest: Arc<ManifestV2>) -> Result<(), String> {
        let id = manifest.id();
        if self.get(id).is_some() || self.v2_manifest(id).is_some() {
            return Err(format!("Plugin ID {id} is already installed"));
        }
        let data = manifest.declaration();
        for kind in fields(data, "kinds") {
            if self.v2_kind(kind).is_some() || self.v1_owns_kind(kind) {
                return Err(format!("Kind {kind} is already registered"));
            }
        }
        for folder in v2_folders(data) {
            if self.v2.folders.contains_key(&folder)
                || self
                    .plugins()
                    .iter()
                    .any(|plugin| v1_folders(plugin).any(|name| name == folder))
            {
                return Err(format!("Storage folder {folder} is already registered"));
            }
        }
        for operation in data["operations"].as_array().into_iter().flatten() {
            let id = operation["id"].as_str().expect("checked operation ID");
            if self.v2_operation(id).is_some() || self.policy(id).is_some() {
                return Err(format!("Operation {id} is already registered"));
            }
            let capability = operation["required_capability"]
                .as_str()
                .and_then(Capability::from_name);
            if matches!(
                capability,
                None | Some(Capability::SpacesList | Capability::SpacesCreate)
            ) {
                return Err(format!("Operation {id} must use a space capability"));
            }
        }
        for site in data["network"]["fixed_sites"]
            .as_array()
            .into_iter()
            .flatten()
        {
            let site = site.as_str().expect("checked hostname");
            if crate::netguard::checked_url(&format!("https://{site}"), &[site]).is_none() {
                return Err(format!(
                    "Network site {site} must be an exact HTTPS hostname"
                ));
            }
        }

        for (index, kind) in fields(data, "kinds").enumerate() {
            self.v2
                .kinds
                .insert(kind.to_owned(), (manifest.clone(), index));
        }
        for (index, operation) in fields(data, "operations").enumerate() {
            self.v2
                .operations
                .insert(operation.to_owned(), (manifest.clone(), index));
        }
        for folder in v2_folders(data) {
            self.v2.folders.insert(folder, id.to_owned());
        }
        self.v2.packages.insert(id.to_owned(), manifest);
        Ok(())
    }

    pub(super) fn remove_v2(&mut self, id: &str) {
        if let Some(manifest) = self.v2.packages.remove(id) {
            for kind in fields(manifest.declaration(), "kinds") {
                self.v2.kinds.remove(kind);
            }
            for folder in v2_folders(manifest.declaration()) {
                self.v2.folders.remove(&folder);
            }
            for operation in fields(manifest.declaration(), "operations") {
                self.v2.operations.remove(operation);
            }
        }
    }

    pub(super) fn carry_v2(
        &mut self,
        base: &Self,
        replacement: Option<&super::Plugin>,
    ) -> Result<(), String> {
        if let Some(plugin) = replacement {
            let manifest = &plugin.manifest;
            if v1_folders(plugin).any(|folder| base.v2.folders.contains_key(&folder))
                || base.v2_manifest(&manifest.id).is_some()
                || manifest
                    .kinds
                    .iter()
                    .any(|kind| base.v2_kind(&kind.id).is_some())
                || manifest
                    .operations
                    .iter()
                    .any(|operation| base.v2_operation(&operation.id).is_some())
            {
                return Err("Candidate conflicts with a registered v2 package".into());
            }
        }
        self.v2 = base.v2.clone();
        Ok(())
    }

    fn v1_owns_kind(&self, kind: &str) -> bool {
        self.plugins()
            .iter()
            .any(|plugin| plugin.manifest.kinds.iter().any(|decl| decl.id == kind))
    }
}

fn fields<'a>(data: &'a Value, key: &str) -> impl Iterator<Item = &'a str> {
    data[key]
        .as_array()
        .into_iter()
        .flatten()
        .map(|entry| entry["id"].as_str().expect("checked declaration ID"))
}

fn v2_folders(manifest: &Value) -> impl Iterator<Item = String> {
    manifest["kinds"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|kind| kind["storage"]["folder"].as_str())
        .map(fold)
}

fn v1_folders(plugin: &super::Plugin) -> impl Iterator<Item = String> {
    plugin
        .manifest
        .kinds
        .iter()
        .filter_map(|kind| match &kind.storage {
            ManifestPlacement::Visible { folder }
            | ManifestPlacement::VisibleTree { folder, .. } => Some(fold(folder)),
            ManifestPlacement::Hidden { .. } => None,
        })
}
