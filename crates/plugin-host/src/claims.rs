//! One ownership index for installed package declarations.
use crate::ManifestV2;
use calternal_core::{ManifestPlacement, ManifestV1};
use calternal_fsio::names::fold;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Category {
    Namespace,
    Kind,
    Operation,
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

/// Ownership of unique installed declarations across manifest versions.
#[derive(Clone, Debug, Default)]
pub struct DeclarationIndex(BTreeMap<Category, BTreeMap<String, String>>);

impl DeclarationIndex {
    fn register(
        &mut self,
        package: &str,
        declarations: impl IntoIterator<Item = (Category, String)>,
    ) -> Result<(), String> {
        let declarations: BTreeSet<_> = declarations.into_iter().collect();
        for (category, name) in &declarations {
            if let Some(owner) = self.owner(*category, name) {
                return Err(format!(
                    "{} {name} is already registered by {owner}",
                    category.label()
                ));
            }
        }
        for (category, name) in declarations {
            self.0
                .entry(category)
                .or_default()
                .insert(name, package.into());
        }
        Ok(())
    }

    /// Register an already validated v1 declaration atomically. Cost: one
    /// bounded index update per claimed identifier.
    pub fn register_v1(&mut self, manifest: &ManifestV1) -> Result<(), String> {
        use Category::*;
        let folders = manifest
            .kinds
            .iter()
            .filter_map(|kind| match &kind.storage {
                ManifestPlacement::Visible { folder }
                | ManifestPlacement::VisibleTree { folder, .. } => Some((Folder, fold(folder))),
                ManifestPlacement::Hidden { .. } => None,
            });
        let commands = manifest.palette.iter().flat_map(|menu| {
            menu.commands
                .iter()
                .map(|c| &c.id)
                .chain(menu.settings_sections.iter().map(|section| &section.id))
                .map(|id| (Palette, id.clone()))
        });
        let id = &manifest.id;
        self.register(
            id,
            [(Namespace, id.clone())]
                .into_iter()
                .chain(manifest.provider.as_ref().map(|_| (Provider, id.clone())))
                .chain(manifest.kinds.iter().map(|entry| (Kind, entry.id.clone())))
                .chain(
                    manifest
                        .operations
                        .iter()
                        .map(|entry| (Operation, entry.id.clone())),
                )
                .chain(
                    manifest
                        .collections
                        .iter()
                        .map(|entry| (Collection, entry.id.clone())),
                )
                .chain(folders)
                .chain(commands),
        )
    }

    /// Register a validated v2 declaration atomically. Package and app IDs
    /// share one operation namespace. Cost: one bounded index update per claim.
    pub fn register_v2(&mut self, manifest: &ManifestV2) -> Result<(), String> {
        use Category::*;
        let declared = manifest.declaration();
        let names = |key: &str, category| {
            declared[key]
                .as_array()
                .into_iter()
                .flatten()
                .map(move |entry| {
                    (
                        category,
                        entry["id"].as_str().expect("checked identifier").to_owned(),
                    )
                })
        };
        let folders = declared["kinds"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|kind| kind["storage"]["folder"].as_str())
            .map(|name| (Folder, fold(name)));
        self.register(
            manifest.id(),
            [(Namespace, manifest.id().into())]
                .into_iter()
                .chain(names("apps", Namespace))
                .chain(names("kinds", Kind))
                .chain(names("operations", Operation))
                .chain(names("providers", Provider))
                .chain(folders),
        )
    }

    /// Package that owns a declared kind. Cost: O(log kinds), no allocation.
    pub fn kind_owner(&self, name: &str) -> Option<&str> {
        self.owner(Category::Kind, name)
    }

    /// Package that owns a declared operation. Cost: O(log operations), no allocation.
    pub fn operation_owner(&self, name: &str) -> Option<&str> {
        self.owner(Category::Operation, name)
    }

    fn owner(&self, category: Category, name: &str) -> Option<&str> {
        self.0.get(&category)?.get(name).map(String::as_str)
    }
}
