//! Load the installed catalogue, isolating unavailable archive rows.
use super::super::{Catalog, Registry, TrustedFile};
use super::*;
use std::sync::Arc;

#[derive(Default)]
struct InstalledDeclarations {
    v2: Vec<(usize, Arc<host::ManifestV2>)>,
    reserved: host::DeclarationIndex,
}

/// Initialize fresh installations from the distribution catalog and construct a bounded,
/// immutable plugin snapshot. While the server holds its installation lock,
/// reuse private current and retained workers matching the archive and discard
/// stale cache files. Cost: one bounded read per worker and one cache scan.
/// Missing or invalid workers are atomically written and synced. Worker launches
/// verify the pinned hash again. Development may use the launcher's source catalog
/// to validate packages in an alternate root.
pub fn load(
    appdata: &Path,
    seed_root: Option<&Path>,
    test_root: Option<&Path>,
    development: bool,
) -> Result<Registry, String> {
    let mut conn = open(appdata)?;
    initialize_distribution(
        &mut conn,
        seed_root.unwrap_or(Path::new("plugins")),
        seed_root.is_none(),
        development,
    )?;
    let cache = appdata.join("installed-plugin-workers");
    calternal_fsio::private::directory(&cache).map_err(reason)?;
    let mut installed = InstalledDeclarations::default();
    let plugins = load_rows(
        &conn,
        &cache,
        "installed_plugins",
        MAX_PACKAGES,
        Some(&mut installed),
    )?;
    let retained = load_rows(&conn, &cache, "retained_plugins", MAX_RETAINED, None)?;
    worker_cache::remove_stale(
        &cache,
        plugins
            .iter()
            .chain(&retained)
            .filter_map(|(_, plugin)| plugin.worker.as_deref()),
    )
    .map_err(reason)?;

    let registry = Registry::default();
    let mut catalog = Catalog::default();
    let mut v1 = plugins.into_iter().peekable();
    for (position, manifest) in installed.v2 {
        while v1.peek().is_some_and(|(earlier, _)| *earlier < position) {
            let (_, plugin) = v1.next().expect("peeked v1 row");
            catalog.load_v1(Arc::new(plugin), &mut installed.reserved);
        }
        if let Err(error) = catalog.register_stored_v2(manifest.clone()) {
            let id = manifest.id().to_owned();
            tracing::warn!(plugin = %id, reason = %error, "installed v2 package unavailable");
            installed
                .reserved
                .reserve_unavailable(id, Some(host::PackageClaims::v2(&manifest)));
        }
    }
    for (_, plugin) in v1 {
        catalog.load_v1(Arc::new(plugin), &mut installed.reserved);
    }
    // Reservations affect subsequent admission, never the choice of live owners on load.
    catalog
        .declarations
        .carry_unavailable(&installed.reserved, None);
    if let Some(root) = test_root {
        for entry in super::super::test_entries() {
            match super::super::load_one(root, &entry).and_then(|plugin| {
                catalog.declarations.check_reserved(
                    &plugin.manifest.id,
                    &host::PackageClaims::v1(&plugin.manifest),
                )?;
                catalog.register_v1(Arc::new(plugin))
            }) {
                Ok(()) => {}
                Err(error) => {
                    tracing::warn!(plugin = %entry.id, reason = %error, "test package unavailable")
                }
            }
        }
    }
    catalog.finish_registration();
    catalog.retained = retained
        .into_iter()
        .map(|(_, plugin)| {
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
    mut installed: Option<&mut InstalledDeclarations>,
) -> Result<Vec<(usize, Plugin)>, String> {
    let order = if installed.is_some() {
        "COALESCE(install_order, revision), revision"
    } else {
        "revision"
    };
    let mut query = conn.prepare(&format!(
        "SELECT archive, checksum, default_enabled, revision, approval, id FROM {table} ORDER BY {order}"
    )).map_err(reason)?;
    let mut rows = query.query([]).map_err(reason)?;
    let mut plugins = Vec::new();
    let mut count = 0;
    while let Some(row) = rows.next().map_err(reason)? {
        count += 1;
        if count > limit {
            return Err("the installed plugin catalog exceeds its capacity".into());
        }
        let bytes: Vec<u8> = row.get(0).map_err(reason)?;
        let checksum: String = row.get(1).map_err(reason)?;
        let id: String = row.get(5).map_err(reason)?;
        let package = match verify_archive(&bytes, &checksum) {
            Ok(()) => read_stored_package(&bytes, &id, count, installed.as_deref_mut()),
            Err(error) => {
                if let Some(installed) = installed.as_deref_mut() {
                    installed.reserved.reserve_unavailable(id.clone(), None);
                }
                Err(error)
            }
        };
        let package = match package {
            Ok(Some(package)) => package,
            Ok(None) => continue,
            Err(error) => {
                tracing::warn!(plugin = %id, reason = %error, "installed package archive unavailable");
                continue;
            }
        };
        let entry = package.entry(row.get(2).map_err(reason)?);
        let revision: u32 = row.get(3).map_err(reason)?;
        let worker = if let Some(code) = package.worker {
            let path = cache.join(format!("{}.{revision}.js", entry.id));
            if !worker_cache::matches_private_file(&path, code.as_bytes()) {
                calternal_fsio::write_atomic_mode(&path, code.as_bytes(), 0o600).map_err(reason)?;
            }
            Some(path)
        } else {
            None
        };
        let mut plugin = Plugin::new(entry, package.manifest, worker)?;
        plugin.revision = revision;
        plugin.activation_approval = row.get(4).map_err(reason)?;
        plugins.push((count, plugin));
    }
    Ok(plugins)
}

fn read_stored_package(
    bytes: &[u8],
    id: &str,
    position: usize,
    installed: Option<&mut InstalledDeclarations>,
) -> Result<Option<Package>, String> {
    let legacy_error = match Package::read_archive(bytes) {
        Ok(package) if package.manifest.id == id => return Ok(Some(package)),
        Ok(_) => "installed v1 package ID does not match its archive".into(),
        Err(error) => error,
    };
    let Some(installed) = installed else {
        return Err(legacy_error);
    };
    let error = if v2::archive_version(bytes) == Ok(2) {
        match validate_v2_archive(bytes) {
            Ok(manifest) if manifest.id() == id => {
                installed.v2.push((position, manifest));
                return Ok(None);
            }
            Ok(_) => "installed v2 package ID does not match its archive".into(),
            Err(error) => error,
        }
    } else {
        legacy_error
    };
    // The checksum was checked above. Recover declarations for reservation
    // only, even when newer validation rejects the package or its assets.
    let claims = v2::recover_stored_claims(bytes, id);
    installed.reserved.reserve_unavailable(id.into(), claims);
    Err(error)
}

fn distribution_entries(root: &Path, development: bool) -> Result<Vec<TrustedEntry>, String> {
    let mut candidates = vec![
        root.join("trusted-plugins.json"),
        root.join("../crates/server/trusted-plugins.json"),
    ];
    // Development may overlay packages from a temporary root while still using
    // the catalog from the launcher's checkout.
    if development && root.is_dir() {
        candidates.extend([
            PathBuf::from("crates/server/trusted-plugins.json"),
            PathBuf::from("trusted-plugins.json"),
        ]);
    }
    let mut path = None;
    for candidate in candidates {
        if candidate.try_exists().map_err(reason)? {
            path = Some(candidate);
            break;
        }
    }
    let Some(path) = path else {
        return Ok(Vec::new());
    };
    let bytes = read_regular(&path, MAX_MANIFEST_BYTES)?;
    let file: TrustedFile =
        serde_json::from_slice(&bytes).map_err(|e| format!("distribution plugin catalog: {e}"))?;
    if file.plugins.len() > MAX_PACKAGES {
        return Err("distribution plugin catalog exceeds the installation limit".into());
    }
    Ok(file.plugins)
}

fn install_distribution_package(
    tx: &Connection,
    package: Package,
    default_enabled: bool,
) -> Result<(), String> {
    validate_registered(tx, &package)?;
    let archive = package.archive()?;
    let checksum = super::super::sha256_hex(&archive);
    tx.execute(
        "INSERT OR IGNORE INTO installed_plugins (id, archive, checksum, default_enabled)
         VALUES (?1, ?2, ?3, ?4)",
        params![package.manifest.id, archive, checksum, default_enabled],
    )
    .map_err(reason)?;
    Ok(())
}

fn initialize_distribution(
    conn: &mut Connection,
    root: &Path,
    defaults: bool,
    development: bool,
) -> Result<(), String> {
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
    let entries = distribution_entries(root, development)?;
    if defaults
        && !entries
            .iter()
            .any(|entry| entry.id == "calternal-orchestrator")
    {
        let manifest = host::parse_manifest(include_bytes!(
            "../../../../../plugins/calternal-orchestrator/manifest.json"
        ))
        .map_err(reason)?;
        let package = Package {
            manifest,
            worker: None,
        };
        install_distribution_package(&tx, package, true)?;
    }
    for entry in entries {
        let plugin = match super::super::load_one(root, &entry) {
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
                if entry.worker_sha256.as_deref() != Some(&super::super::sha256_hex(&bytes)) {
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
        install_distribution_package(&tx, package, entry.default_enabled)?;
    }
    tx.execute("INSERT INTO plugin_seed VALUES (1)", [])
        .map_err(reason)?;
    tx.commit().map_err(reason)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_legacy_manifest_keeps_its_original_error_and_claims() {
        let mut manifest: serde_json::Value = serde_json::from_slice(include_bytes!(
            "../../../../../plugins/calternal-notes/manifest.json"
        ))
        .unwrap();
        manifest["unknown_v1_field"] = serde_json::json!(true);
        let bytes =
            serde_json::to_vec(&serde_json::json!({"manifest": manifest, "worker": null})).unwrap();
        let mut installed = InstalledDeclarations::default();
        let error = read_stored_package(&bytes, "calternal-notes", 1, Some(&mut installed))
            .err()
            .expect("the legacy manifest is invalid");
        assert!(error.contains("unknown_v1_field"), "{error}");
        let mut candidate = host::parse_manifest(include_bytes!(
            "../../../../../plugins/calternal-notes/manifest.json"
        ))
        .unwrap();
        candidate.id = "other-package".into();
        let error = installed
            .reserved
            .check_reserved(&candidate.id, &host::PackageClaims::v1(&candidate))
            .unwrap_err();
        assert!(
            error.contains("reserved by unavailable package calternal-notes"),
            "{error}"
        );
    }
}
