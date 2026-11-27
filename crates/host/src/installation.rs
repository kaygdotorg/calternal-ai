//! Bounded plugin bundle reads, validation and durable installation.
use crate::declarations;
use crate::internal;
use crate::space::{Space, bounded_read};
use crate::worker_calls::{InstalledCode, validate_registrations};
use calternal_fsio::beneath::Folder;
use calternal_plugin_host::operations::Registrations;
use calternal_types::{Error, Result};
use rusqlite::params;
use std::collections::BTreeMap;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

const PLUGIN_BYTES: usize = 16 * 1024 * 1024;
const PLUGIN_FILES: usize = 256;

impl Space {
    pub(crate) fn load_plugins(&mut self) -> Result<()> {
        let mut plugins = Vec::new();
        let mut code = BTreeMap::new();
        // The INTEGER PRIMARY KEY is a durable alias for rowid. Older
        // add-only ledgers can still be read in their recorded rowid order.
        let mut stmt = self
            .db
            .prepare_cached("SELECT i.plugin,i.digest,r.data FROM installations AS i LEFT JOIN worker_registrations AS r ON r.plugin=i.plugin ORDER BY i.rowid")
            .map_err(internal)?;
        let trusted = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })
            .map_err(internal)?;
        for row in trusted {
            let (name, digest, registration) = row.map_err(internal)?;
            let installed = self
                .open_folder(&format!(".calternal/plugins/{name}"), false)
                .map_err(internal)?;
            let files = bundle_from(&installed)?;
            if digest != self.bundle_digest(&files) {
                return Err(Error::conflict("An installed plugin was altered on disk."));
            }
            let plugin = declarations::parse_plugin(&files, true, Some(&name), &plugins)?;
            check_save_context(&plugin)?;
            if let Some(registration) = registration {
                let registration = serde_json::from_str(&registration).map_err(internal)?;
                let source = files
                    .get("operations.ts")
                    .ok_or_else(|| internal("registered source missing"))?;
                let mut installed = InstalledCode::new(source.clone(), registration);
                installed.endpoint = self
                    .code
                    .get(&name)
                    .and_then(|existing| existing.endpoint.clone());
                code.insert(name.clone(), installed);
            }
            plugins.push(plugin);
        }
        self.browser_catalog.take();
        self.plugins = plugins.into_iter()
            .map(|plugin| (plugin.manifest.id.clone(), plugin))
            .collect();
        self.code = code;
        Ok(())
    }

    fn bundle_digest(&self, files: &BTreeMap<String, Vec<u8>>) -> [u8; 32] {
        let mut digest = blake3::Hasher::new_keyed(&self.key);
        for (path, content) in files {
            digest.update(&(path.len() as u64).to_le_bytes());
            digest.update(path.as_bytes());
            digest.update(&(content.len() as u64).to_le_bytes());
            digest.update(content);
        }
        *digest.finalize().as_bytes()
    }

    pub(crate) fn install(
        &mut self,
        files: BTreeMap<String, Vec<u8>>,
        official: bool,
        registration: Option<Registrations>,
    ) -> Result<()> {
        let installed = self.plugins.values().cloned().collect::<Vec<_>>();
        let declaration = declarations::parse_plugin(&files, official, None, &installed)?;
        check_save_context(&declaration)?;
        if self.plugins.contains_key(&declaration.manifest.id) {
            return Err(Error::conflict(
                "A plugin with that ID is already installed.",
            ));
        }
        let base = format!(".calternal/plugins/{}", declaration.manifest.id);
        match self.open_folder(&base, false) {
            Ok(_) => {
                return Err(Error::conflict(
                    "The plugin destination is already occupied.",
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(internal(error)),
        }
        let code = registration
            .map(|registration| {
                validate_registrations(&declaration, &self.plugins, &registration)?;
                let source = files
                    .get("operations.ts")
                    .ok_or_else(|| internal("probed source missing"))?;
                Ok::<_, Error>(InstalledCode::new(source.clone(), registration))
            })
            .transpose()?;
        self.recovery_needed = true;
        let digest = self.bundle_digest(&files);
        let placements = files
            .into_iter()
            .map(|(path, data)| self.stage(format!("{base}/{path}"), None, Some(&data)))
            .collect::<Result<Vec<_>>>()?;
        let tx = self.db.transaction().map_err(internal)?;
        tx.execute(
            "INSERT INTO installations(plugin,digest) VALUES(?1,?2)",
            params![declaration.manifest.id, &digest[..]],
        )
        .map_err(internal)?;
        if let Some(code) = &code {
            tx.execute(
                "INSERT INTO worker_registrations VALUES(?1,?2)",
                params![
                    declaration.manifest.id,
                    serde_json::to_string(&code.registration).map_err(internal)?
                ],
            )
            .map_err(internal)?;
        }
        tx.execute(
            "INSERT INTO publication VALUES(1,?1)",
            [serde_json::to_string(&placements).map_err(internal)?],
        )
        .map_err(internal)?;
        tx.commit().map_err(internal)?;
        self.finish(&placements)?;
        if let Some(code) = code {
            self.code.insert(declaration.manifest.id.clone(), code);
        }
        self.browser_catalog.take();
        self.plugins
            .insert(declaration.manifest.id.clone(), declaration);
        Ok(())
    }
}

// The shared add/change renderer checks the final typed item for every
// caller. Reject dependencies its clock-only evaluation cannot resolve.
fn check_save_context(plugin: &declarations::Plugin) -> Result<()> {
    for kind in plugin.kinds.values().filter(|kind| !kind.checks.is_empty()) {
        let field = |name: &str| {
            !matches!(name, "id" | "kind" | "created" | "edited" | "at" | "author" | "anchor")
                && kind.fields.get(name).is_some_and(|decl| {
                    !decl.from_metadata && decl.sidecar.is_none() && !decl.heading
                })
        };
        let checks = kind.checks.iter().map(|(check, _)| check);
        let computed = kind.fields.values().filter_map(|field| field.expression.as_ref());
        if checks.chain(computed).any(|expr| !expr.uses_only_row_and_clock(field)) {
            return Err(Error::forbidden("The kind's save checks require unavailable context."));
        }
    }
    Ok(())
}

fn visit(
    dir: &Folder,
    parent: &str,
    remaining: &mut usize,
    out: &mut BTreeMap<String, Vec<u8>>,
) -> Result<()> {
    for name in dir.entry_names(PLUGIN_FILES).map_err(internal)? {
        let path = format!("{parent}{name}");
        if out.len() >= PLUGIN_FILES || name.starts_with('.') || name.chars().any(char::is_control)
        {
            return Err(Error::invalid_input(
                "Plugin has too many entries or contains an invalid name.",
            ));
        }
        if parent.is_empty()
            && matches!(
                name.as_str(),
                "kinds" | "collections" | "sidebars" | "templates"
            )
        {
            visit(
                &dir.child(&name, false).map_err(internal)?,
                &format!("{path}/"),
                remaining,
                out,
            )?;
        } else {
            let definition = if parent.is_empty() {
                matches!(name.as_str(), "plugin.md" | "operations.ts")
            } else {
                name.ends_with(".md")
            };
            if !definition {
                return Err(Error::forbidden(
                    "Plugin assets and frames are outside this host slice.",
                ));
            }
            let file = dir.open_file(&name).map_err(internal)?;
            if file.metadata().map_err(internal)?.nlink() != 1 {
                return Err(Error::invalid_input("Plugin contains a hard-linked file."));
            }
            let data = bounded_read(file, *remaining)?;
            *remaining -= data.len();
            out.insert(path, data);
        }
    }
    Ok(())
}
pub(crate) fn bundle(path: &Path) -> Result<BTreeMap<String, Vec<u8>>> {
    bundle_from(&Folder::open_existing(path).map_err(internal)?)
}

fn bundle_from(dir: &Folder) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut files = BTreeMap::new();
    let mut remaining = PLUGIN_BYTES;
    visit(dir, "", &mut remaining, &mut files)?;
    Ok(files)
}

#[cfg(test)]
mod tests {
    #[test]
    fn installation_sequence_is_persisted_in_insertion_order() {
        let root = tempfile::tempdir().unwrap();
        let id = calternal_types::SpaceId::new();
        let mut space = super::Space::open(root.path(), id, 86_400_000).unwrap();
        for plugin in ["z-first", "a-last"] {
            let files = [("plugin.md".into(), format!(
                "---\nplugin: {plugin}\nname: {plugin}\nversion: 1.0.0\nformat: 1\nsends out: nothing\n---\nInstalled.\n"
            ).into_bytes())].into_iter().collect();
            space.install(files, false, None).unwrap();
        }
        drop(space);
        let mut opened = super::Space::open(root.path(), id, 86_400_000).unwrap();
        opened.db.execute_batch("PRAGMA reverse_unordered_selects=ON; VACUUM;").unwrap();
        let sequence: (i64, i64) = opened.db.query_row(
            "SELECT (SELECT sequence FROM installations WHERE plugin='z-first'),
                    (SELECT sequence FROM installations WHERE plugin='a-last')", [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).unwrap();
        assert!(sequence.0 < sequence.1);
        opened.load_plugins().unwrap();
        assert_eq!(opened.plugins.len(), 2);
    }

    #[test]
    fn extensions_do_not_authorize_unclassified_assets() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("image.png"), b"<svg/>").unwrap();
        let error = super::bundle(root.path()).unwrap_err();
        assert_eq!(error.code, calternal_types::ErrorCode::Forbidden);
    }
}
