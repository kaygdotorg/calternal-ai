//! Installation-local package catalog and checksum-backed worker materialization.
use super::{Plugin, Registry, TrustedEntry, check_operations, registry_entries};
use calternal_core::ManifestV1;
use calternal_plugin_host::{self as host, MAX_MANIFEST_BYTES};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use std::{fs, io::Read, path::Path, time::Duration};

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
        let package = if path.is_dir() {
            let base = path.canonicalize().map_err(reason)?;
            let manifest = safe_member(&base, "manifest.json", MAX_MANIFEST_BYTES)?;
            let manifest =
                serde_json::from_slice(&manifest).map_err(|e| format!("manifest.json: {e}"))?;
            let worker = if base.join("dist/worker.js").exists() {
                let bytes = safe_member(&base, "dist/worker.js", MAX_WORKER)?;
                Some(String::from_utf8(bytes).map_err(reason)?)
            } else {
                None
            };
            Self { manifest, worker }
        } else {
            serde_json::from_slice(&read_regular(path, MAX_ARCHIVE)?).map_err(reason)?
        };
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

    pub(super) fn validate(&self) -> Result<(), String> {
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
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(reason)?
        .take((max + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(reason)?;
    if bytes.len() > max {
        return Err("package member exceeds its byte budget".into());
    }
    Ok(bytes)
}

fn safe_member(root: &Path, relative: &str, max: usize) -> Result<Vec<u8>, String> {
    let path = root.join(relative);
    let resolved = path.canonicalize().map_err(reason)?;
    if resolved != path {
        return Err("package members must be regular files without symbolic links".into());
    }
    read_regular(&resolved, max)
}

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
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS installed_plugins (
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
        CREATE TABLE IF NOT EXISTS plugin_seed (singleton INTEGER PRIMARY KEY CHECK (singleton = 1));",
    ).map_err(reason)?;
    Ok(db)
}

// Reserve an ID in a disjoint namespace. Only the counter is durable so an
// interrupted job cannot claim a different draft after the server restarts.
#[cfg(feature = "dev-plugin-drafts")]
pub(super) fn next_draft_revision(root: &Path) -> Result<u32, String> {
    let db = open(root)?;
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS plugin_draft_counter (
            singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
            revision INTEGER NOT NULL CHECK (revision BETWEEN 2147483648 AND 4294967295)
        );",
    )
    .map_err(reason)?;
    db.query_row(
        "INSERT INTO plugin_draft_counter VALUES (1, 2147483648)
         ON CONFLICT DO UPDATE SET revision = revision + 1 RETURNING revision",
        [],
        |row| row.get(0),
    )
    .map_err(reason)
}

fn validate_registered(conn: &Connection, package: &Package) -> Result<(), String> {
    let plugin = Plugin::new(package.entry(false), package.manifest.clone(), None)?;
    check_operations(&plugin)?;
    let mut index = host::Registry::default();
    let mut query = conn
        .prepare("SELECT archive, checksum FROM installed_plugins WHERE id <> ?1")
        .map_err(reason)?;
    let mut rows = query.query([&package.manifest.id]).map_err(reason)?;
    while let Some(row) = rows.next().map_err(reason)? {
        let existing = decode(row.get(0).map_err(reason)?, row.get(1).map_err(reason)?)?;
        index.register(existing.manifest).map_err(reason)?;
    }
    index.register(package.manifest.clone()).map_err(reason)
}

fn decode(bytes: Vec<u8>, checksum: String) -> Result<Package, String> {
    let package: Package = serde_json::from_slice(&bytes).map_err(reason)?;
    if checksum != super::sha256_hex(&bytes) {
        return Err("installed archive checksum does not match".into());
    }
    package.validate()?;
    Ok(package)
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
        .map(|(archive, checksum)| decode(archive, checksum).map(|package| package.manifest))
        .transpose()
}

/// Check whether an update requires the space manager to approve it again.
/// Cost: bounded declaration comparisons, with no code execution or I/O.
pub fn authority_changed(old: &ManifestV1, new: &ManifestV1) -> bool {
    new.host_calls
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
            let old = decode(previous_bytes, previous_checksum)?;
            if old.manifest.kinds != package.manifest.kinds
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
    validate_registered(&tx, &package)?;
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
    if candidate.retain_previous {
        tx.execute(
            "INSERT INTO retained_plugins SELECT * FROM installed_plugins WHERE id = ?1",
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
        "INSERT INTO installed_plugins (revision, id, archive, checksum, default_enabled, approval)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            plugin.revision,
            id,
            candidate.archive,
            candidate.checksum,
            plugin.entry.default_enabled,
            plugin.activation_approval
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
    tx.commit().map_err(reason)
}

fn seed(conn: &mut Connection, root: &Path, defaults: bool) -> Result<(), String> {
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(reason)?;
    if tx
        .query_row("SELECT 1 FROM plugin_seed", [], |_| Ok(()))
        .optional()
        .map_err(reason)?
        .is_some()
    {
        return Ok(());
    }
    for entry in registry_entries() {
        let fallback = (defaults && entry.id == "calternal-orchestrator").then_some(
            include_bytes!("../../../../plugins/calternal-orchestrator/manifest.json").as_slice(),
        );
        let plugin = match super::load_one(root, &entry, fallback) {
            Ok(plugin) if plugin.unavailable.is_none() => plugin,
            Ok(plugin) => {
                tracing::warn!(plugin = %entry.id, reason = ?plugin.unavailable, "seed package unavailable");
                continue;
            }
            Err(error) => {
                tracing::warn!(plugin = %entry.id, reason = %error, "seed package unavailable");
                continue;
            }
        };
        let worker = match plugin.worker {
            Some(path) => {
                let bytes = read_regular(&path, MAX_WORKER)?;
                if entry.worker_sha256.as_deref() != Some(&super::sha256_hex(&bytes)) {
                    return Err("seed worker changed during installation".into());
                }
                Some(String::from_utf8(bytes).map_err(reason)?)
            }
            None => None,
        };
        let package = Package {
            manifest: plugin.manifest,
            worker,
        };
        let archive = package.archive()?;
        let checksum = super::sha256_hex(&archive);
        tx.execute("INSERT OR IGNORE INTO installed_plugins (id, archive, checksum, default_enabled) VALUES (?1, ?2, ?3, ?4)",
            params![entry.id, archive, checksum, entry.default_enabled]).map_err(reason)?;
    }
    tx.execute("INSERT INTO plugin_seed VALUES (1)", [])
        .map_err(reason)?;
    tx.commit().map_err(reason)
}

/// Seed the installation once from the distribution and construct a bounded,
/// immutable plugin snapshot. Materialize current and retained worker bundles while the
/// server holds its installation lock. Starts verify the worker hash again.
pub fn load(
    appdata: &Path,
    seed_root: Option<&Path>,
    test_root: Option<&Path>,
) -> Result<Registry, String> {
    let mut conn = open(appdata)?;
    seed(
        &mut conn,
        seed_root.unwrap_or(Path::new("plugins")),
        seed_root.is_none(),
    )?;
    let cache = appdata.join("installed-plugin-workers");
    use std::os::unix::fs::DirBuilderExt;
    match fs::DirBuilder::new().mode(0o700).create(&cache) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(reason(error)),
    }
    for file in fs::read_dir(&cache).map_err(reason)? {
        let file = file.map_err(reason)?;
        if file.file_type().map_err(reason)?.is_file()
            && file.path().extension().is_some_and(|ext| ext == "js")
        {
            fs::remove_file(file.path()).map_err(reason)?;
        }
    }
    let mut plugins = load_rows(&conn, &cache, "installed_plugins", MAX_PACKAGES)?;
    let retained = load_rows(&conn, &cache, "retained_plugins", MAX_RETAINED)?;
    if let Some(root) = test_root {
        for entry in super::test_entries() {
            match super::load_one(root, &entry, None) {
                Ok(plugin) => plugins.push(plugin),
                Err(error) => {
                    tracing::warn!(plugin = %entry.id, reason = %error, "test package unavailable")
                }
            }
        }
    }
    let registry = Registry::from_plugins(plugins);
    let mut catalog = (*registry.snapshot()).clone();
    catalog.retained = retained
        .into_iter()
        .map(|plugin| {
            (
                (plugin.manifest.id.clone(), plugin.revision),
                std::sync::Arc::new(plugin),
            )
        })
        .collect();
    registry.publish(catalog);
    Ok(registry)
}

fn load_rows(
    conn: &Connection,
    cache: &Path,
    table: &str,
    limit: usize,
) -> Result<Vec<Plugin>, String> {
    let mut query = conn.prepare(&format!(
        "SELECT archive, checksum, default_enabled, revision, approval FROM {table} ORDER BY revision"
    )).map_err(reason)?;
    let mut rows = query.query([]).map_err(reason)?;
    let mut plugins = Vec::new();
    while let Some(row) = rows.next().map_err(reason)? {
        if plugins.len() >= limit {
            return Err("the installed plugin catalog exceeds its capacity".into());
        }
        let package = decode(row.get(0).map_err(reason)?, row.get(1).map_err(reason)?)?;
        let entry = package.entry(row.get(2).map_err(reason)?);
        let revision: u32 = row.get(3).map_err(reason)?;
        let worker = if let Some(code) = package.worker {
            let path = cache.join(format!("{}.{revision}.js", entry.id));
            calternal_fsio::write_atomic_mode(&path, code.as_bytes(), 0o600).map_err(reason)?;
            Some(path)
        } else {
            None
        };
        let mut plugin = Plugin::new(entry, package.manifest, worker)?;
        plugin.revision = revision;
        plugin.activation_approval = row.get(4).map_err(reason)?;
        plugins.push(plugin);
    }
    Ok(plugins)
}
