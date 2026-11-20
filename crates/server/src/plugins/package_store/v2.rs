//! Version dispatch and pinned v2 archive validation for the operator installer.
use std::{collections::BTreeMap, path::Path, sync::Arc};

use base64::{Engine, engine::general_purpose::STANDARD};
use calternal_fsio::beneath::Folder;
use calternal_plugin_host::{ManifestV2, PackageClaims};
use rusqlite::{TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, value::RawValue};

use super::{
    MAX_ARCHIVE, MAX_MANIFEST_BYTES, MAX_PACKAGES, MAX_WORKER, Package, open, read_bounded,
    read_regular, reason,
};

const MAX_TEMPLATE: usize = 256 * 1024;
const MAX_ICON: usize = 256 * 1024;
const MAX_BINARY_ASSETS: usize = MAX_ARCHIVE * 2 / 3;

/// Package selected by its manifest's explicit API version.
/// Version 1 retains the original archive and activation path.
pub enum InstallPackage {
    /// Original UTF-8 worker archive.
    V1(Package),
    /// Manifest v2 and its pinned assets.
    V2(PackageV2, ManifestV2),
}

#[derive(Deserialize)]
struct Version {
    api_version: u32,
}

#[derive(Deserialize)]
struct ArchiveHeader {
    manifest: Version,
}

pub(super) fn archive_version(bytes: &[u8]) -> Result<u32, String> {
    serde_json::from_slice::<ArchiveHeader>(bytes)
        .map(|header| header.manifest.api_version)
        .map_err(|_| "package archive must declare manifest.api_version".into())
}

pub(super) fn recover_stored_claims(bytes: &[u8], id: &str) -> Option<PackageClaims> {
    #[derive(Deserialize)]
    struct StoredManifest<'a> {
        #[serde(borrow)]
        manifest: &'a RawValue,
    }

    let archive = serde_json::from_slice::<StoredManifest<'_>>(bytes).ok()?;
    PackageClaims::recover(id, archive.manifest.get().as_bytes())
}

impl InstallPackage {
    /// Read the versioned source directory or archive once. Cost: bounded
    /// file reads, offline validation and hashing. No package code runs.
    pub fn read(path: &Path) -> Result<Self, String> {
        if path.is_dir() {
            let base = path.canonicalize().map_err(reason)?;
            let source = Folder::open_existing(&base).map_err(reason)?;
            let manifest = source.open_file("manifest.json").map_err(reason)?;
            let bytes = read_bounded(manifest, MAX_MANIFEST_BYTES)?;
            let version: Version = serde_json::from_slice(&bytes)
                .map_err(|error| format!("manifest.json: {error}"))?;
            return match version.api_version {
                1 => Package::read_directory(&source, &bytes).map(Self::V1),
                2 => {
                    let (archive, manifest) = PackageV2::read_directory(&source, bytes)?;
                    Ok(Self::V2(archive, manifest))
                }
                _ => Err("Unsupported plugin manifest API version. Use version 1 or 2.".into()),
            };
        }
        let bytes = read_regular(path, MAX_ARCHIVE)?;
        match archive_version(&bytes)? {
            1 => Package::read_archive(&bytes).map(Self::V1),
            2 => {
                let archive: PackageV2 = serde_json::from_slice(&bytes).map_err(reason)?;
                let manifest = archive.validate()?;
                Ok(Self::V2(archive, manifest))
            }
            _ => Err("Unsupported plugin manifest API version. Use version 1 or 2.".into()),
        }
    }

    /// Return bytes accepted by the same operator install path. Cost: one
    /// serialization of the bounded package.
    pub fn archive(&self) -> Result<Vec<u8>, String> {
        match self {
            Self::V1(package) => package.archive(),
            Self::V2(package, _) => package.archive(),
        }
    }
}

/// Closed v2 archive shape. Asset values hold base64 of exact package bytes.
/// The raw manifest preserves duplicate keys until the strict reader sees it.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PackageV2 {
    manifest: Box<RawValue>,
    #[serde(deserialize_with = "calternal_types::unique_json::deserialize_unique_members")]
    assets: BTreeMap<String, String>,
}

#[derive(Clone, Copy)]
enum Asset {
    Worker,
    Template,
    Icon,
}

impl Asset {
    fn limit(self) -> usize {
        match self {
            Self::Worker => MAX_WORKER,
            Self::Template => MAX_TEMPLATE,
            Self::Icon => MAX_ICON,
        }
    }
}

fn pins(manifest: &Value) -> impl Iterator<Item = (Asset, &Value)> {
    fn icon(declaration: &Value) -> Option<(Asset, &Value)> {
        declaration["icon"]
            .get("image")
            .map(|pin| (Asset::Icon, pin))
    }

    manifest
        .get("worker")
        .map(|p| (Asset::Worker, p))
        .into_iter()
        .chain(icon(manifest))
        .chain(
            manifest["apps"]
                .as_array()
                .into_iter()
                .flatten()
                .flat_map(|app| {
                    Some((Asset::Template, &app["description"]["template"]))
                        .into_iter()
                        .chain(icon(app))
                }),
        )
        .chain(
            manifest["kinds"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(icon),
        )
        .chain(
            manifest["providers"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|provider| (Asset::Worker, &provider["adapter"])),
        )
}

fn pin_path(pin: &Value) -> &str {
    pin["path"].as_str().expect("checked asset path")
}

impl PackageV2 {
    fn read_directory(base: &Folder, bytes: Vec<u8>) -> Result<(Self, ManifestV2), String> {
        let checked = ManifestV2::parse(&bytes)?;
        let mut assets = BTreeMap::new();
        let mut total = 0;
        for (role, pin) in pins(checked.declaration()) {
            let path = pin_path(pin);
            if assets.contains_key(path) {
                continue;
            }
            let file = base
                .open_file(path)
                .map_err(|error| format!("Pinned asset {path}: {error}"))?;
            let bytes = read_bounded(file, role.limit())
                .map_err(|error| format!("Pinned asset {path}: {error}"))?;
            total += bytes.len();
            if total > MAX_BINARY_ASSETS {
                return Err("v2 package assets exceed the byte budget".into());
            }
            assets.insert(path.to_owned(), STANDARD.encode(bytes));
        }
        let manifest =
            RawValue::from_string(String::from_utf8(bytes).map_err(reason)?).map_err(reason)?;
        let archive = Self { manifest, assets };
        archive.check_assets(&checked)?;
        Ok((archive, checked))
    }

    /// Check the raw manifest and every referenced asset pin again at the
    /// operator request boundary. Cost: bounded parsing, hashing and validation.
    pub(in crate::plugins) fn validate(&self) -> Result<ManifestV2, String> {
        let manifest = ManifestV2::parse(self.manifest.get().as_bytes())?;
        self.check_assets(&manifest)?;
        Ok(manifest)
    }

    fn check_assets(&self, manifest: &ManifestV2) -> Result<(), String> {
        let mut checked = BTreeMap::new();
        let mut total = 0;
        for (role, pin) in pins(manifest.declaration()) {
            let path = pin_path(pin);
            let (bytes, digest) = match checked.entry(path) {
                std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
                std::collections::btree_map::Entry::Vacant(entry) => {
                    let encoded = self
                        .assets
                        .get(path)
                        .ok_or_else(|| format!("Pinned asset {path} is missing"))?;
                    if encoded.len() > role.limit().div_ceil(3) * 4 {
                        return Err(format!("Pinned asset {path} exceeds its byte budget"));
                    }
                    let bytes = STANDARD
                        .decode(encoded)
                        .map_err(|_| format!("Pinned asset {path} has invalid base64"))?;
                    total += bytes.len();
                    if total > MAX_BINARY_ASSETS {
                        return Err("v2 package assets exceed the byte budget".into());
                    }
                    let digest = super::super::sha256_hex(&bytes);
                    entry.insert((bytes, digest))
                }
            };
            if bytes.is_empty() || bytes.len() > role.limit() {
                return Err(format!(
                    "Pinned asset {path} is empty or exceeds its byte budget"
                ));
            }
            if pin["sha256"].as_str() != Some(digest.as_str()) {
                return Err(format!("Pinned asset {path} has a SHA-256 digest mismatch"));
            }
            match role {
                Asset::Template => manifest.check_description(bytes)?,
                Asset::Icon
                    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n")
                        && !bytes.starts_with(b"\xff\xd8\xff") =>
                {
                    return Err(format!("Pinned icon {path} must be PNG or JPEG"));
                }
                _ => {}
            }
        }
        if checked.len() != self.assets.len() {
            return Err("v2 package archive contains an unpinned asset".into());
        }
        Ok(())
    }

    pub(in crate::plugins) fn archive(&self) -> Result<Vec<u8>, String> {
        let bytes = serde_json::to_vec(self).map_err(reason)?;
        if bytes.len() > MAX_ARCHIVE {
            return Err("v2 package archive exceeds its byte budget".into());
        }
        Ok(bytes)
    }

    /// Review checksum of the archive submitted to the operator socket.
    /// Cost: one bounded serialization and hash.
    pub fn checksum(&self) -> Result<String, String> {
        Ok(super::super::sha256_hex(&self.archive()?))
    }
}

/// Read the pinned worker of one installed v2 package for a cold placement
/// wake. The asset must still match the running declaration's pin, so a
/// replaced row cannot change the code behind a live declaration. Call on a
/// blocking worker. Cost: one indexed SQLite read, one bounded base64 decode
/// and one SHA-256 of the worker. Warm invocations never call this.
pub(in crate::plugins) fn installed_worker(
    appdata: &Path,
    id: &str,
    pin: &Value,
) -> Result<String, String> {
    use rusqlite::OptionalExtension;
    let archive: Vec<u8> = open(appdata)?
        .query_row(
            "SELECT archive FROM installed_plugins WHERE id=?1",
            [id],
            |row| row.get(0),
        )
        .optional()
        .map_err(reason)?
        .ok_or("the package is not installed")?;
    let archive: PackageV2 = serde_json::from_slice(&archive).map_err(reason)?;
    let encoded = pin["path"]
        .as_str()
        .and_then(|path| archive.assets.get(path))
        .filter(|encoded| encoded.len() <= MAX_WORKER.div_ceil(3) * 4)
        .ok_or("the pinned worker is missing")?;
    let bytes = STANDARD.decode(encoded).map_err(reason)?;
    if pin["sha256"].as_str() != Some(super::super::sha256_hex(&bytes).as_str()) {
        return Err("the installed worker does not match its pin".into());
    }
    String::from_utf8(bytes).map_err(reason)
}

pub(in crate::plugins) fn validate_v2_archive(bytes: &[u8]) -> Result<Arc<ManifestV2>, String> {
    if bytes.len() > MAX_ARCHIVE {
        return Err("v2 package archive exceeds its byte budget".into());
    }
    let archive: PackageV2 = serde_json::from_slice(bytes).map_err(reason)?;
    Ok(Arc::new(archive.validate()?))
}

pub(in crate::plugins) fn commit_v2(
    appdata: &Path,
    id: &str,
    archive: &[u8],
    approved: &str,
) -> Result<(), String> {
    if super::super::sha256_hex(archive) != approved {
        return Err("The approval checksum does not match. Review the exact package again.".into());
    }
    let mut conn = open(appdata)?;
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(reason)?;
    let count: i64 = tx
        .query_row("SELECT COUNT(*) FROM installed_plugins", [], |row| {
            row.get(0)
        })
        .map_err(reason)?;
    if count >= MAX_PACKAGES as i64 {
        return Err("the installation has reached its 128-plugin limit".into());
    }
    let inserted = tx
        .execute(
            "INSERT INTO installed_plugins (id, archive, checksum, default_enabled, approval)
         VALUES (?1, ?2, ?3, 0, 0) ON CONFLICT (id) DO NOTHING",
            params![id, archive, approved],
        )
        .map_err(reason)?;
    if inserted != 1 {
        return Err("plugin is already installed".into());
    }
    tx.commit().map_err(reason)
}

#[cfg(test)]
mod tests;
