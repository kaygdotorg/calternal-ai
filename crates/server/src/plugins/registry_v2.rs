//! V2 declarations share ownership checks with v1 and grant no runtime authority.
use std::{
    collections::{BTreeMap, VecDeque},
    sync::Arc,
};

use calternal_plugin_host::{DeclarationIndex, ManifestV2, PackageClaims};
use serde_json::Value;

use super::{Catalog, check_operation, check_operation_namespace};

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

    fn read_owners<'a>(
        &'a self,
        manifest: &'a ManifestV2,
    ) -> impl Iterator<Item = Result<Option<&'a str>, String>> + 'a {
        declared(manifest, "reads").map(|kind| {
            let id = kind.as_str().expect("checked kind ID");
            if ManifestV2::host_owns_kind(id)
                || declared(manifest, "kinds").any(|decl| decl["id"] == id)
            {
                return Ok(None);
            }
            self.declarations
                .kind_owner(id)
                .map(Some)
                .ok_or_else(|| format!("Declared read kind {id} is not installed"))
        })
    }

    pub(super) fn check_v2_reads(&self, manifest: &ManifestV2) -> Result<(), String> {
        for owner in self.read_owners(manifest) {
            owner?;
        }
        Ok(())
    }

    pub(super) fn reserve_invalid_v2_reads(&mut self, unavailable: &mut DeclarationIndex) {
        // Check declarations against the complete set of ownership winners.
        // Walk each dependency edge once, then withdraw the failed closure.
        let mut dependents: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        let mut refused = BTreeMap::new();
        let mut queue = VecDeque::new();
        for (id, manifest) in &self.v2 {
            for result in self.read_owners(manifest) {
                match result {
                    Ok(Some(owner)) if self.v2.contains_key(owner) => {
                        dependents.entry(owner).or_default().push(id);
                    }
                    Err(error) if !refused.contains_key(id) => {
                        refused.insert(id.clone(), error);
                        queue.push_back(id.clone());
                    }
                    _ => {}
                }
            }
        }
        while let Some(owner) = queue.pop_front() {
            for dependent in dependents.get(owner.as_str()).into_iter().flatten() {
                if !refused.contains_key(*dependent) {
                    refused.insert(
                        (*dependent).to_owned(),
                        format!("Declared read owner {owner} is unavailable"),
                    );
                    queue.push_back((*dependent).to_owned());
                }
            }
        }
        for (id, error) in refused {
            tracing::warn!(plugin = %id, reason = %error, "installed v2 package unavailable");
            let manifest = self
                .v2
                .remove(&id)
                .expect("only registered owners can be withdrawn");
            self.declarations.withdraw_live(&id);
            unavailable.reserve_unavailable(id, Some(PackageClaims::v2(&manifest)));
        }
    }

    pub(super) fn register_v2(&mut self, manifest: Arc<ManifestV2>) -> Result<(), String> {
        self.check_v2_reads(&manifest)?;
        self.declarations
            .check_reserved(manifest.id(), &PackageClaims::v2(&manifest))?;
        self.register_stored_v2(manifest)
    }

    pub(super) fn register_stored_v2(&mut self, manifest: Arc<ManifestV2>) -> Result<(), String> {
        for operation in declared(&manifest, "operations") {
            let id = operation["id"].as_str().expect("checked operation ID");
            let capability = operation["required_capability"]
                .as_str()
                .expect("checked capability");
            check_operation(manifest.id(), id, capability)?;
        }
        check_operation_namespace(manifest.id(), manifest.id())?;
        for app in declared(&manifest, "apps") {
            check_operation_namespace(manifest.id(), app["id"].as_str().expect("checked app ID"))?;
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
        self.declarations
            .carry_unavailable(&base.declarations, removed);
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
