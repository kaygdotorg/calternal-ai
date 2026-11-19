//! Load the installed catalogue, isolating unavailable archive rows.
use super::super::{Registry, TrustedFile};
use super::*;

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
    let mut v2 = Vec::new();
    let mut plugins = load_rows(
        &conn,
        &cache,
        "installed_plugins",
        MAX_PACKAGES,
        Some(&mut v2),
    )?;
    let retained = load_rows(&conn, &cache, "retained_plugins", MAX_RETAINED, None)?;
    worker_cache::remove_stale(
        &cache,
        plugins
            .iter()
            .chain(&retained)
            .filter_map(|plugin| plugin.worker.as_deref()),
    )
    .map_err(reason)?;
    if let Some(root) = test_root {
        for entry in super::super::test_entries() {
            match super::super::load_one(root, &entry) {
                Ok(plugin) => plugins.push(plugin),
                Err(error) => {
                    tracing::warn!(plugin = %entry.id, reason = %error, "test package unavailable")
                }
            }
        }
    }
    let registry = Registry::from_plugins(plugins);
    let mut catalog = (*registry.snapshot()).clone();
    for manifest in v2 {
        let id = manifest.id().to_owned();
        if let Err(error) = catalog.register_v2(manifest) {
            tracing::warn!(plugin = %id, reason = %error, "installed v2 package unavailable");
        }
    }
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
    mut v2: Option<&mut Vec<std::sync::Arc<host::ManifestV2>>>,
) -> Result<Vec<Plugin>, String> {
    let mut query = conn.prepare(&format!(
        "SELECT archive, checksum, default_enabled, revision, approval, id FROM {table} ORDER BY revision"
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
        let package = (|| {
            verify_archive(&bytes, &checksum)?;
            match Package::read_archive(&bytes) {
                Ok(package) => {
                    if package.manifest.id != id {
                        return Err("installed package ID does not match its archive".into());
                    }
                    return Ok(Some(package));
                }
                Err(error) if v2.is_none() => return Err(error),
                Err(_) => {}
            }
            let entries = v2.as_deref_mut().expect("checked active table");
            let manifest = validate_v2_archive(&bytes)?;
            if manifest.id() != id {
                return Err("installed v2 package ID does not match its archive".into());
            }
            entries.push(manifest);
            Ok(None)
        })();
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
        plugins.push(plugin);
    }
    Ok(plugins)
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
