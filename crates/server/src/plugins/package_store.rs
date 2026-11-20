//! Installation-local package catalog and checksum-backed worker materialization.
use super::{Plugin, TrustedEntry, check_operations};
use calternal_core::ManifestV1;
use calternal_plugin_host::{self as host, MAX_MANIFEST_BYTES};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::Duration,
};

mod bundled_notes;
pub(crate) mod provider_runtime;
mod startup;
mod v2;
mod worker_cache;

pub use startup::load;
pub use v2::{InstallPackage, PackageV2};
pub(super) use v2::{commit_v2, installed_worker, validate_v2_archive};

const MAX_WORKER: usize = calternal_machine::sandbox::worker::MAX_SOURCE_BYTES;
pub(super) const MAX_PACKAGES: usize = 128;
pub(super) const MAX_RETAINED: usize = 128;
pub(super) const MAX_ARCHIVE: usize = (MAX_MANIFEST_BYTES + MAX_WORKER) * 6 + 1024;

/// One self-contained UTF-8 package. Contains data only until activation.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Package {
    /// Closed manifest-v1 declaration.
    pub manifest: ManifestV1,
    /// Bundled worker source, with all imports resolved or runtime built-ins.
    pub worker: Option<String>,
}

impl Package {
    /// Read a bounded manifest plus optional dist/worker.js, or one .calplugin
    /// JSON archive. Reject invalid declarations without executing code.
    pub fn read(path: &Path) -> Result<Self, String> {
        if path.is_dir() {
            let source = calternal_fsio::beneath::Folder::open_existing(path).map_err(reason)?;
            let manifest = read_bounded(
                source.open_file("manifest.json").map_err(reason)?,
                MAX_MANIFEST_BYTES,
            )?;
            Self::read_directory(&source, &manifest)
        } else {
            Self::read_archive(&read_regular(path, MAX_ARCHIVE)?)
        }
    }

    fn read_directory(
        base: &calternal_fsio::beneath::Folder,
        bytes: &[u8],
    ) -> Result<Self, String> {
        let manifest = serde_json::from_slice(bytes).map_err(|e| format!("manifest.json: {e}"))?;
        let worker = match base.open_sub("dist", false) {
            Ok(dist) => match dist.open_file("worker.js") {
                Ok(file) => {
                    Some(String::from_utf8(read_bounded(file, MAX_WORKER)?).map_err(reason)?)
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    dist.check_replacement("worker.js").map_err(reason)?;
                    None
                }
                Err(error) => return Err(reason(error)),
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(reason(error)),
        };
        let package = Self { manifest, worker };
        package.validate()?;
        Ok(package)
    }

    fn read_archive(bytes: &[u8]) -> Result<Self, String> {
        let package: Self = serde_json::from_slice(bytes).map_err(reason)?;
        package.validate()?;
        Ok(package)
    }

    /// Compute the review checksum over the normalized manifest and exact worker
    /// text. Cost: serialize and hash the bounded package once.
    pub fn checksum(&self) -> Result<String, String> {
        Ok(super::sha256_hex(
            &serde_json::to_vec(self).map_err(reason)?,
        ))
    }

    /// Return a compact archive accepted by the same install path. Cost: O(package bytes).
    pub fn archive(&self) -> Result<Vec<u8>, String> {
        serde_json::to_vec(self).map_err(reason)
    }

    /// Validate the manifest and the bounded worker payload without executing the package.
    pub fn validate(&self) -> Result<(), String> {
        host::validate_manifest(&self.manifest).map_err(reason)?;
        if self
            .worker
            .as_ref()
            .is_some_and(|w| w.is_empty() || w.len() > MAX_WORKER)
        {
            return Err("worker bundle must contain 1 to 1048576 bytes".into());
        }
        Ok(())
    }

    pub(super) fn entry(&self, default_enabled: bool) -> TrustedEntry {
        TrustedEntry {
            id: self.manifest.id.clone(),
            version: self.manifest.version.clone(),
            manifest: "manifest.json".into(),
            worker: self.worker.as_ref().map(|_| "dist/worker.js".into()),
            worker_sha256: self
                .worker
                .as_ref()
                .map(|w| super::sha256_hex(w.as_bytes())),
            default_enabled,
        }
    }
}

fn reason(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn read_regular(path: &Path, max: usize) -> Result<Vec<u8>, String> {
    if !fs::symlink_metadata(path).map_err(reason)?.is_file() {
        return Err("package member must be a regular file".into());
    }
    read_bounded(fs::File::open(path).map_err(reason)?, max)
}

fn read_bounded(file: fs::File, max: usize) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    file.take((max + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(reason)?;
    if bytes.len() > max {
        return Err("package member exceeds its byte budget".into());
    }
    Ok(bytes)
}

const PACKAGE_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS installed_plugins (
            revision INTEGER PRIMARY KEY AUTOINCREMENT CHECK (revision <= 2147483647),
            id TEXT NOT NULL UNIQUE, archive BLOB NOT NULL, checksum TEXT NOT NULL,
            default_enabled INTEGER NOT NULL CHECK (default_enabled IN (0, 1)),
            approval INTEGER NOT NULL DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS retained_plugins (
            revision INTEGER PRIMARY KEY, id TEXT NOT NULL, archive BLOB NOT NULL,
            checksum TEXT NOT NULL, default_enabled INTEGER NOT NULL, approval INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS retained_plugins_id ON retained_plugins (id);
        CREATE TABLE IF NOT EXISTS plugin_seed (singleton INTEGER PRIMARY KEY CHECK (singleton = 1));
        CREATE TABLE IF NOT EXISTS plugin_draft_counter (
            singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
            revision INTEGER NOT NULL CHECK (revision BETWEEN 2147483648 AND 4294967295)
        );";

fn open(appdata: &Path) -> Result<Connection, String> {
    use std::os::unix::fs::OpenOptionsExt;
    let path = appdata.join("installed-plugins.sqlite3");
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
    {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(reason(error)),
    }
    let db = Connection::open(path).map_err(reason)?;
    db.busy_timeout(Duration::from_secs(5)).map_err(reason)?;
    calternal_store::migrations::migrate(&db, UPGRADE_SCHEMA).map_err(reason)?;
    Ok(db)
}

// Reserve an ID in a disjoint namespace. Only the counter is durable so an
// interrupted job cannot claim a different draft after the server restarts.
#[cfg(feature = "dev-plugin-drafts")]
pub(super) fn next_draft_revision(root: &Path) -> Result<u32, String> {
    let db = open(root)?;
    db.query_row(
        "INSERT INTO plugin_draft_counter VALUES (1, 2147483648)
         ON CONFLICT DO UPDATE SET revision = revision + 1 RETURNING revision",
        [],
        |row| row.get(0),
    )
    .map_err(reason)
}

fn validate_registered(conn: &Connection, package: &Package) -> Result<(), String> {
    check_operations(&package.manifest)?;
    let mut index = host::Registry::default();
    let mut query = conn
        .prepare("SELECT archive, checksum, id FROM installed_plugins WHERE id <> ?1")
        .map_err(reason)?;
    let mut rows = query.query([&package.manifest.id]).map_err(reason)?;
    while let Some(row) = rows.next().map_err(reason)? {
        let bytes: Vec<u8> = row.get(0).map_err(reason)?;
        let checksum: String = row.get(1).map_err(reason)?;
        let id: String = row.get(2).map_err(reason)?;
        verify_archive(&bytes, &checksum).map_err(|error| unavailable(&id, error))?;
        if let Some(existing) = read_v1_archive(&bytes).map_err(|error| unavailable(&id, error))? {
            index
                .register(existing.manifest)
                .map_err(|error| unavailable(&id, error))?;
        }
    }
    index.register(package.manifest.clone()).map_err(reason)
}

fn unavailable(id: &str, error: impl std::fmt::Display) -> String {
    format!(
        "Installed package {id} is unavailable: {error}. Remove it with calternal plugin remove {id}."
    )
}

fn decode(bytes: Vec<u8>, checksum: String) -> Result<Package, String> {
    verify_archive(&bytes, &checksum)?;
    read_v1_archive(&bytes)?
        .ok_or_else(|| "This package uses manifest v2. Review it with the v2 installer.".into())
}

// Try the legacy reader once. The fallback discriminator is only needed for
// a non-v1 archive; a successful v1 read never scans the worker twice.
fn read_v1_archive(bytes: &[u8]) -> Result<Option<Package>, String> {
    match Package::read_archive(bytes) {
        Ok(package) => Ok(Some(package)),
        Err(_) if v2::archive_version(bytes) == Ok(2) => Ok(None),
        Err(error) => Err(error),
    }
}

fn verify_archive(bytes: &[u8], checksum: &str) -> Result<(), String> {
    if bytes.len() > MAX_ARCHIVE || checksum != super::sha256_hex(bytes) {
        return Err(
            "installed archive checksum does not match or archive exceeds its byte budget".into(),
        );
    }
    Ok(())
}

/// Look up the current manifest for an operator's capability comparison.
/// Cost: one bounded archive read, verification and validation. Creates no database.
pub fn installed_manifest(appdata: &Path, id: &str) -> Result<Option<ManifestV1>, String> {
    let db = Connection::open_with_flags(
        appdata.join("installed-plugins.sqlite3"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .map_err(reason)?;
    let entry = db
        .query_row(
            "SELECT archive, checksum FROM installed_plugins WHERE id = ?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(reason)?;
    entry
        .map(|(archive, checksum)| {
            decode(archive, checksum)
                .map(|package| package.manifest)
                .map_err(|error| unavailable(id, error))
        })
        .transpose()
}

/// Check whether an update requires the space manager to approve it again.
/// Cost: bounded declaration comparisons, with no code execution or I/O.
pub fn authority_changed(old: &ManifestV1, new: &ManifestV1) -> bool {
    let old_content = old
        .network
        .as_ref()
        .and_then(|network| network.content_fetch.as_ref());
    let new_content = new
        .network
        .as_ref()
        .and_then(|network| network.content_fetch.as_ref());
    new_content.is_some_and(|new| {
        old_content.is_none_or(|old| {
            new.profile != old.profile
                || new
                    .sources
                    .iter()
                    .any(|source| !old.sources.contains(source))
        })
    }) || new
        .host_calls
        .iter()
        .any(|call| !old.host_calls.contains(call))
        || new
            .secrets
            .iter()
            .any(|secret| !old.secrets.contains(secret))
        || new.operations.iter().any(|op| {
            !old.operations.iter().any(|before| {
                before.id == op.id
                    && before.required_capability == op.required_capability
                    && before.risk == op.risk
                    && before.mutates == op.mutates
                    && before.target == op.target
            })
        })
        || new.hooks.iter().any(|hook| !old.hooks.contains(hook))
        || new
            .webhooks
            .iter()
            .any(|webhook| !old.webhooks.contains(webhook))
        || new
            .schedules
            .iter()
            .any(|schedule| !old.schedules.contains(schedule))
        || new.provider != old.provider
}

pub(super) struct Candidate {
    pub(super) plugin: std::sync::Arc<Plugin>,
    archive: Vec<u8>,
    checksum: String,
    retain_previous: bool,
}

pub(super) fn prepare(
    appdata: &Path,
    package: Package,
    approved: String,
    update: bool,
) -> Result<Option<Candidate>, String> {
    package.validate()?;
    let archive = package.archive()?;
    if super::sha256_hex(&archive) != approved {
        return Err("approval checksum differs from the package being installed".into());
    }
    let mut conn = open(appdata)?;
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(reason)?;
    let id = &package.manifest.id;
    let seeded = tx
        .query_row("SELECT 1 FROM plugin_seed", [], |_| Ok(()))
        .optional()
        .map_err(reason)?;
    if seeded.is_none() {
        return Err("start the installation once before changing its plugin catalog".into());
    }
    let previous: Option<(bool, Vec<u8>, String, u32)> = tx
        .query_row(
            "SELECT default_enabled, archive, checksum, approval FROM installed_plugins WHERE id = ?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(reason)?;
    let count: i64 = tx
        .query_row("SELECT COUNT(*) FROM installed_plugins", [], |row| {
            row.get(0)
        })
        .map_err(reason)?;
    if update != previous.is_some() {
        return Err(if update {
            "plugin is not installed"
        } else {
            "plugin is already installed"
        }
        .into());
    }
    if previous.is_none() && count >= MAX_PACKAGES as i64 {
        return Err("the installation has reached its 128-plugin limit".into());
    }
    let (default_enabled, approval, retain_previous) =
        if let Some((enabled, previous_bytes, previous_checksum, approval)) = previous {
            if approved == previous_checksum {
                return Ok(None);
            }
            let old = decode(previous_bytes, previous_checksum)
                .map_err(|error| unavailable(id, error))?;
            // Search coverage is a disposable projection. Only changes to the
            // kinds' stored declarations require a migration.
            let mut prior_kinds = old.manifest.kinds.clone();
            for (prior, next) in prior_kinds.iter_mut().zip(&package.manifest.kinds) {
                prior.searchable = next.searchable;
            }
            if prior_kinds != package.manifest.kinds
                || old.manifest.collections != package.manifest.collections
            {
                return Err(
                    "plugin update changes stored kinds or collections. This requires a migration."
                        .into(),
                );
            }
            if old.worker.is_some() && package.worker.is_none() {
                return Err(
                    "include the built worker at dist/worker.js when replacing a worker plugin"
                        .into(),
                );
            }
            let retain_previous = !authority_changed(&package.manifest, &old.manifest);
            if authority_changed(&old.manifest, &package.manifest) {
                (false, None, retain_previous)
            } else {
                (enabled, Some(approval), retain_previous)
            }
        } else {
            if package.worker.is_none() && !package.manifest.operations.is_empty() {
                return Err(
                    "build dist/worker.js before installing a plugin that declares operations"
                        .into(),
                );
            }
            (false, None, false)
        };
    check_operations(&package.manifest)?;
    let revision: u32 = tx.query_row(
        "SELECT COALESCE((SELECT seq FROM sqlite_sequence WHERE name = 'installed_plugins'), 0) + 1",
        [], |row| row.get(0),
    ).map_err(reason)?;
    drop(tx);
    let entry = package.entry(default_enabled);
    let worker = package
        .worker
        .as_ref()
        .map(|code| {
            let path = appdata
                .join("installed-plugin-workers")
                .join(format!("{id}.{revision}.js"));
            calternal_fsio::write_atomic_mode(&path, code.as_bytes(), 0o600).map_err(reason)?;
            Ok::<_, String>(path)
        })
        .transpose()?;
    let mut plugin = Plugin::new(entry, package.manifest, worker)?;
    plugin.revision = revision;
    plugin.activation_approval = approval.unwrap_or(revision);
    Ok(Some(Candidate {
        plugin: std::sync::Arc::new(plugin),
        archive,
        checksum: approved,
        retain_previous,
    }))
}

pub(super) fn commit(appdata: &Path, candidate: Candidate) -> Result<(), String> {
    let mut conn = open(appdata)?;
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(reason)?;
    let plugin = &candidate.plugin;
    let next: u32 = tx.query_row(
        "SELECT COALESCE((SELECT seq FROM sqlite_sequence WHERE name = 'installed_plugins'), 0) + 1",
        [], |row| row.get(0),
    ).map_err(reason)?;
    if plugin.revision != next {
        return Err("the package catalog changed while the worker was starting".into());
    }
    let id = &plugin.manifest.id;
    let install_order: Option<u32> = tx
        .query_row(
            "SELECT COALESCE(install_order, revision) FROM installed_plugins WHERE id = ?1",
            [id],
            |row| row.get(0),
        )
        .optional()
        .map_err(reason)?;
    if candidate.retain_previous {
        tx.execute(
            "INSERT INTO retained_plugins SELECT revision, id, archive, checksum, default_enabled, approval FROM installed_plugins WHERE id = ?1",
            [id],
        )
        .map_err(reason)?;
    } else {
        tx.execute("DELETE FROM retained_plugins WHERE id = ?1", [id])
            .map_err(reason)?;
    }
    tx.execute("DELETE FROM installed_plugins WHERE id = ?1", [id])
        .map_err(reason)?;
    tx.execute(
        "INSERT INTO installed_plugins (revision, id, archive, checksum, default_enabled, approval, install_order)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            plugin.revision,
            id,
            candidate.archive,
            candidate.checksum,
            plugin.entry.default_enabled,
            plugin.activation_approval,
            install_order,
        ],
    )
    .map_err(reason)?;
    tx.commit().map_err(reason)
}

// Space content and activation records keep their existing retention policy.
pub(super) fn remove(appdata: &Path, id: &str) -> Result<(), String> {
    let mut db = open(appdata)?;
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(reason)?;
    let count = tx
        .execute("DELETE FROM installed_plugins WHERE id = ?1", [id])
        .map_err(reason)?;
    if count != 1 {
        return Err("plugin is not installed".into());
    }
    tx.execute("DELETE FROM retained_plugins WHERE id = ?1", [id])
        .map_err(reason)?;
    tx.execute(
        "DELETE FROM provider_runtime WHERE id = ?1 AND legacy = 0",
        [id],
    )
    .map_err(reason)?;
    tx.commit().map_err(reason)
}

/// Package database schema shared by startup and release admission. Worker
/// cache files are verified against their archive bytes and need no version.
pub(crate) const UPGRADE_SCHEMA: calternal_store::migrations::Schema<'_> =
    calternal_store::migrations::Schema::new(
        "installed-plugins.sqlite3",
        1,
        &[PACKAGE_SCHEMA],
        &[
            (2, bundled_notes::LIVE_EDITING),
            (3, provider_runtime::V3),
            // NULL keeps existing packages ordered by revision without touching their rows.
            (
                4,
                "ALTER TABLE installed_plugins ADD COLUMN install_order INTEGER;",
            ),
        ],
    );

#[cfg(test)]
mod tests;
