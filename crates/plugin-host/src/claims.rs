//! One ownership index for installed package declarations.
use crate::{MAX_MANIFEST_BYTES, ManifestV2, registry_v2::strict_json};
use calternal_core::{ManifestPlacement, ManifestV1};
use calternal_fsio::names::fold;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Category {
    Operation,
    Namespace,
    Kind,
    Folder,
    Provider,
    Collection,
    Palette,
}

impl Category {
    fn label(self) -> &'static str {
        match self {
            Self::Namespace => "Plugin or app namespace",
            Self::Kind => "Kind",
            Self::Operation => "Operation",
            Self::Folder => "Storage folder",
            Self::Provider => "Provider",
            Self::Collection => "Collection",
            Self::Palette => "Palette entry",
        }
    }
}

/// Declaration names reserved by one package, without permission to use them.
#[derive(Clone, Debug)]
pub struct PackageClaims {
    owned: BTreeSet<(Category, String)>,
    reads: BTreeSet<String>,
}

impl PackageClaims {
    fn package(id: &str) -> Self {
        Self {
            owned: BTreeSet::from([(Category::Namespace, id.into())]),
            reads: BTreeSet::new(),
        }
    }

    fn operation(&mut self, id: &str) {
        self.owned.insert((Category::Operation, id.into()));
        if let Some((prefix, _)) = id.split_once('.') {
            self.owned.insert((Category::Namespace, prefix.into()));
        }
    }

    /// Extract bounded ownership declarations from an already decoded v1 manifest.
    pub fn v1(manifest: &ManifestV1) -> Self {
        use Category::*;
        let mut claims = Self::package(&manifest.id);
        if manifest.provider.is_some() {
            claims.owned.insert((Provider, manifest.id.clone()));
        }
        for kind in &manifest.kinds {
            claims.owned.insert((Kind, kind.id.clone()));
            match &kind.storage {
                ManifestPlacement::Visible { folder }
                | ManifestPlacement::VisibleTree { folder, .. } => {
                    claims.owned.insert((Folder, fold(folder)));
                }
                ManifestPlacement::Hidden { .. } => {}
            }
        }
        for operation in &manifest.operations {
            claims.operation(&operation.id);
        }
        for collection in &manifest.collections {
            claims.owned.insert((Collection, collection.id.clone()));
        }
        if let Some(menu) = &manifest.palette {
            claims.owned.extend(
                menu.commands
                    .iter()
                    .map(|c| &c.id)
                    .chain(menu.settings_sections.iter().map(|section| &section.id))
                    .map(|id| (Palette, id.clone())),
            );
        }
        claims
    }

    /// Extract bounded ownership declarations from an already checked v2 manifest.
    pub fn v2(manifest: &ManifestV2) -> Self {
        Self::from_json(manifest.id(), manifest.declaration())
            .expect("checked v2 ownership declarations")
    }

    /// Recover claims from a checksum-verified stored manifest even when it
    /// cannot register against today's rules. None requires operator removal
    /// before admitting new declarations. This path never grants authority.
    pub fn recover(package: &str, bytes: &[u8]) -> Option<Self> {
        if bytes.len() > MAX_MANIFEST_BYTES {
            return None;
        }
        Self::from_json(package, &strict_json(bytes).ok()?)
    }

    fn from_json(package: &str, manifest: &Value) -> Option<Self> {
        use Category::*;
        let version = manifest["api_version"].as_u64()?;
        if manifest["id"].as_str()? != package || !matches!(version, 1 | 2) {
            return None;
        }
        let required: &[&str] = if version == 2 {
            &["apps", "kinds", "operations"]
        } else {
            &["operations", "collections"]
        };
        if required.iter().any(|key| manifest.get(key).is_none()) {
            return None;
        }
        let mut claims = Self::package(package);
        for (key, category) in [
            ("kinds", Kind),
            ("operations", Operation),
            ("providers", Provider),
            ("collections", Collection),
        ] {
            for entry in entries(manifest, key)? {
                let id = entry["id"].as_str()?;
                if category == Operation {
                    claims.operation(id);
                } else {
                    claims.owned.insert((category, id.into()));
                }
                if category == Kind {
                    let storage = entry["storage"].as_object()?;
                    if let Some(folder) = storage.get("folder") {
                        claims.owned.insert((Folder, fold(folder.as_str()?)));
                    } else if storage.get("hidden") != Some(&Value::Bool(true)) {
                        return None;
                    }
                }
            }
        }
        if version == 1 {
            if manifest
                .get("provider")
                .is_some_and(|provider| !provider.is_null())
            {
                claims.owned.insert((Provider, package.into()));
            }
            if let Some(palette) = manifest.get("palette").filter(|value| !value.is_null()) {
                palette.as_object()?;
                for key in ["commands", "settings_sections"] {
                    for entry in entries(palette, key)? {
                        claims.owned.insert((Palette, entry["id"].as_str()?.into()));
                    }
                }
            }
        } else {
            for kind in entries(manifest, "reads")? {
                claims.reads.insert(kind.as_str()?.into());
            }
        }
        Some(claims)
    }
}

fn entries<'a>(record: &'a Value, key: &str) -> Option<&'a [Value]> {
    match record.get(key) {
        None => Some(&[]),
        Some(values) => Some(values.as_array()?.as_slice()),
    }
}

/// Ownership of unique installed declarations across manifest versions.
/// Unavailable claims participate in admission and never answer lookups.
#[derive(Clone, Debug, Default)]
pub struct DeclarationIndex {
    active: BTreeMap<Category, BTreeMap<String, String>>,
    unavailable: BTreeMap<String, Option<PackageClaims>>,
}

impl DeclarationIndex {
    fn register(&mut self, package: &str, claims: PackageClaims) -> Result<(), String> {
        for (category, name) in &claims.owned {
            if let Some(owner) = self.owner(*category, name) {
                return Err(format!(
                    "{} {name} is already registered by {owner}",
                    category.label()
                ));
            }
        }
        for (category, name) in claims.owned {
            self.active
                .entry(category)
                .or_default()
                .insert(name, package.into());
        }
        Ok(())
    }

    /// Reserve an installed row that cannot supply live declarations.
    pub fn reserve_unavailable(&mut self, package: String, claims: Option<PackageClaims>) {
        self.unavailable.insert(package, claims);
    }

    /// Carry unavailable rows into a rebuilt snapshot, excluding the row being removed.
    pub fn carry_unavailable(&mut self, prior: &Self, removed: Option<&str>) {
        self.unavailable.extend(
            prior
                .unavailable
                .iter()
                .filter(|(id, _)| Some(id.as_str()) != removed)
                .map(|(id, claims)| (id.clone(), claims.clone())),
        );
    }

    /// Check changes against skipped rows. Claims already held by this live
    /// owner survive overlap, but cannot be transferred to a different owner.
    /// Cost: bounded by the installed package and declaration caps.
    pub fn check_reserved(&self, package: &str, claims: &PackageClaims) -> Result<(), String> {
        let mut new_claims = claims
            .owned
            .iter()
            .filter(|(category, name)| self.owner(*category, name) != Some(package))
            .peekable();
        if self.unavailable.contains_key(package)
            && self.owner(Category::Namespace, package) != Some(package)
        {
            return Err(format!(
                "Package {package} is unavailable. Remove it with calternal plugin remove {package}."
            ));
        }
        if new_claims.peek().is_none() {
            return Ok(());
        }
        for (unavailable, reserved) in &self.unavailable {
            let Some(reserved) = reserved else {
                return Err(format!(
                    "Cannot verify ownership for unavailable package {unavailable}. Remove it with calternal plugin remove {unavailable}."
                ));
            };
            if let Some((category, name)) = claims
                .owned
                .intersection(&reserved.owned)
                .find(|(category, name)| self.owner(*category, name) != Some(package))
            {
                return Err(format!(
                    "{} {name} is reserved by unavailable package {unavailable}. Remove it with calternal plugin remove {unavailable}.",
                    category.label()
                ));
            }
            for (category, name) in new_claims.clone() {
                if *category == Category::Kind
                    && self.kind_owner(name).is_none()
                    && reserved.reads.contains(name)
                {
                    return Err(format!(
                        "Kind {name} has an unresolved read by unavailable package {unavailable}. Remove it with calternal plugin remove {unavailable} before assigning the kind to another owner."
                    ));
                }
            }
        }
        Ok(())
    }

    /// Register a validated v1 declaration atomically, including operation prefixes.
    pub fn register_v1(&mut self, manifest: &ManifestV1) -> Result<(), String> {
        self.register(&manifest.id, PackageClaims::v1(manifest))
    }

    /// Register a validated v2 declaration atomically, including its package
    /// namespace and any operation namespace prefixes it uses.
    pub fn register_v2(&mut self, manifest: &ManifestV2) -> Result<(), String> {
        self.register(manifest.id(), PackageClaims::v2(manifest))
    }

    /// Withdraw a package's live keys after startup detects an unavailable
    /// dependency. Cost: one scan of the bounded installed declaration index.
    pub fn withdraw_live(&mut self, package: &str) {
        for entries in self.active.values_mut() {
            entries.retain(|_, owner| owner != package);
        }
    }

    /// Package that owns a live kind. Cost: O(log kinds), no allocation.
    pub fn kind_owner(&self, name: &str) -> Option<&str> {
        self.owner(Category::Kind, name)
    }

    /// Package that owns a live operation. Cost: O(log operations), no allocation.
    pub fn operation_owner(&self, name: &str) -> Option<&str> {
        self.owner(Category::Operation, name)
    }

    fn owner(&self, category: Category, name: &str) -> Option<&str> {
        self.active.get(&category)?.get(name).map(String::as_str)
    }
}
