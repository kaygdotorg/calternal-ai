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
        let mut plugins = BTreeMap::new();
        let mut code = BTreeMap::new();
        let mut stmt = self
            .db
            .prepare_cached("SELECT i.plugin,i.digest,r.data FROM installations AS i LEFT JOIN worker_registrations AS r ON r.plugin=i.plugin")
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
            let installed = plugins.values().cloned().collect::<Vec<_>>();
            let plugin = declarations::parse_plugin(&files, true, Some(&name), &installed)?;
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
            plugins.insert(name, plugin);
        }
        self.browser_catalog.take();
        self.plugins = plugins;
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
            "INSERT INTO installations VALUES(?1,?2)",
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
    fn extensions_do_not_authorize_unclassified_assets() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("image.png"), b"<svg/>").unwrap();
        let error = super::bundle(root.path()).unwrap_err();
        assert_eq!(error.code, calternal_types::ErrorCode::Forbidden);
    }
}
