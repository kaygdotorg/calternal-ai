//! Bounded plugin bundle reads, validation and durable installation.
use crate::declarations;
use crate::internal;
use crate::space::{Space, bounded_read};
use calternal_fsio::{beneath::Folder, names};
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
        let mut stmt = self
            .db
            .prepare_cached("SELECT plugin,digest FROM installations")
            .map_err(internal)?;
        let trusted = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
            })
            .map_err(internal)?;
        for row in trusted {
            let (name, digest) = row.map_err(internal)?;
            let installed = self
                .open_folder(&format!(".calternal/plugins/{name}"), false)
                .map_err(internal)?;
            let files = bundle_from(&installed)?;
            if digest != self.bundle_digest(&files) {
                return Err(Error::conflict("An installed plugin was altered on disk."));
            }
            let plugin = declarations::parse_plugin(&files, true)?;
            plugins.insert(name, plugin);
        }
        self.plugins = plugins;
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

    pub(crate) fn install(&mut self, source: &Path, official: bool) -> Result<()> {
        let files = bundle(source)?;
        let declaration = declarations::parse_plugin(&files, official)?;
        if self.plugins.contains_key(&declaration.id) {
            return Err(Error::conflict(
                "A plugin with that ID is already installed.",
            ));
        }
        let mut folders = self
            .plugins
            .values()
            .flat_map(|p| p.kinds.values())
            .map(|k| names::fold(&k.folder))
            .collect::<Vec<_>>();
        for kind in declaration.kinds.values() {
            let path = names::fold(&kind.folder);
            if folders.iter().any(|other| {
                path == *other
                    || path
                        .strip_prefix(other)
                        .is_some_and(|tail| tail.starts_with('/'))
                    || other
                        .strip_prefix(&path)
                        .is_some_and(|tail| tail.starts_with('/'))
            }) {
                return Err(Error::conflict("Installed kind folders must be disjoint."));
            }
            folders.push(path);
        }
        let base = format!(".calternal/plugins/{}", declaration.id);
        match self.open_folder(&base, false) {
            Ok(_) => {
                return Err(Error::conflict(
                    "The plugin destination is already occupied.",
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(internal(error)),
        }
        self.recovery_needed = true;
        let digest = self.bundle_digest(&files);
        let placements = files
            .into_iter()
            .map(|(path, data)| self.stage(format!("{base}/{path}"), None, Some(&data)))
            .collect::<Result<Vec<_>>>()?;
        let tx = self.db.transaction().map_err(internal)?;
        tx.execute(
            "INSERT INTO installations VALUES(?1,?2)",
            params![declaration.id, &digest[..]],
        )
        .map_err(internal)?;
        tx.execute(
            "INSERT INTO publication VALUES(1,?1)",
            [serde_json::to_string(&placements).map_err(internal)?],
        )
        .map_err(internal)?;
        tx.commit().map_err(internal)?;
        self.finish(&placements)?;
        self.plugins.insert(declaration.id.clone(), declaration);
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
fn bundle(path: &Path) -> Result<BTreeMap<String, Vec<u8>>> {
    bundle_from(&Folder::open_existing(path).map_err(internal)?)
}

fn bundle_from(dir: &Folder) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut files = BTreeMap::new();
    let mut remaining = PLUGIN_BYTES;
    visit(dir, "", &mut remaining, &mut files)?;
    Ok(files)
}
