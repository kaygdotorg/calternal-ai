//! V2 declarations share ownership checks with v1 and grant no runtime authority.
use std::sync::Arc;

use calternal_plugin_host::ManifestV2;
use calternal_types::Capability;
use serde_json::Value;

use super::{Catalog, check_operation};

impl Catalog {
    /// Installed v2 declaration by package ID. Cost: O(log packages), no I/O.
    /// Callers still check space activation and current viewer authority.
    pub fn v2_manifest(&self, package: &str) -> Option<&ManifestV2> {
        self.v2.get(package).map(Arc::as_ref)
    }

    /// Owning v2 package and checked kind declaration. Cost: indexed owner
    /// lookup and at most 64 declaration comparisons. No I/O or wake.
    pub fn v2_kind(&self, kind: &str) -> Option<(&ManifestV2, &Value)> {
        let package = self.v2_manifest(self.declarations.kind_owner(kind)?)?;
        Some((
            package,
            declared(package, "kinds").find(|entry| entry["id"] == kind)?,
        ))
    }

    /// Owning v2 package and checked operation declaration. Cost: indexed
    /// owner lookup and at most 128 comparisons. Grants no dispatch authority.
    pub fn v2_operation(&self, operation: &str) -> Option<(&ManifestV2, &Value)> {
        let package = self.v2_manifest(self.declarations.operation_owner(operation)?)?;
        Some((
            package,
            declared(package, "operations").find(|entry| entry["id"] == operation)?,
        ))
    }

    pub(super) fn check_v2_reads(&self, manifest: &ManifestV2) -> Result<(), String> {
        for kind in declared(manifest, "reads") {
            let id = kind.as_str().expect("checked kind ID");
            let local = declared(manifest, "kinds").any(|decl| decl["id"] == id);
            if !local
                && self.declarations.kind_owner(id).is_none()
                && !ManifestV2::host_owns_kind(id)
            {
                return Err(format!("Declared read kind {id} is not installed"));
            }
        }
        Ok(())
    }

    pub(super) fn register_v2(&mut self, manifest: Arc<ManifestV2>) -> Result<(), String> {
        self.check_v2_reads(&manifest)?;
        for operation in declared(&manifest, "operations") {
            let id = operation["id"].as_str().expect("checked operation ID");
            let capability = operation["required_capability"]
                .as_str()
                .and_then(Capability::from_name);
            check_operation(id, capability)?;
        }
        for site in manifest.declaration()["network"]["fixed_sites"]
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
        self.declarations.register_v2(&manifest)?;
        self.v2.insert(manifest.id().into(), manifest);
        Ok(())
    }

    pub(super) fn carry_v2(&mut self, base: &Self, removed: Option<&str>) -> Result<(), String> {
        for manifest in base.v2.values() {
            if Some(manifest.id()) != removed {
                self.declarations.register_v2(manifest)?;
                self.v2.insert(manifest.id().into(), manifest.clone());
            }
        }
        for manifest in self.v2.values() {
            self.check_v2_reads(manifest).map_err(|why| {
                format!(
                    "Package {} depends on the removed kind. {why}",
                    manifest.id()
                )
            })?;
        }
        Ok(())
    }
}

fn declared<'a>(manifest: &'a ManifestV2, key: &str) -> impl Iterator<Item = &'a Value> {
    manifest.declaration()[key].as_array().into_iter().flatten()
}
