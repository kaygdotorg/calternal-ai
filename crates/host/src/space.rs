//! Per-space ledger, declaration cache and publication recovery.
use crate::declarations::{self, Collection, Fields, Kind, Plugin};
use crate::{Call, decode, identity, internal};
use calternal_fsio::beneath::Folder;
use calternal_fsio::{DataLock, names};
use calternal_types::{Actor, Error, ErrorCode, Result, SpaceId};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use uuid::Uuid;

pub(crate) const TRASH_MS: i64 = 30 * 86_400_000;
pub(crate) const ITEM_BYTES: usize = 32 * 1024 * 1024;
const PLUGIN_BYTES: usize = 16 * 1024 * 1024;
const PLUGIN_FILES: usize = 256;

const SCHEMA: &str = "
PRAGMA journal_mode=WAL;
PRAGMA synchronous=FULL;
PRAGMA secure_delete=ON;
CREATE TABLE IF NOT EXISTS key_material (id INTEGER PRIMARY KEY CHECK(id=1), key BLOB NOT NULL);
CREATE TABLE IF NOT EXISTS head (id TEXT PRIMARY KEY, data TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS installations (plugin TEXT PRIMARY KEY, digest BLOB NOT NULL);
CREATE TABLE IF NOT EXISTS trash (item TEXT PRIMARY KEY, until INTEGER NOT NULL);
CREATE INDEX IF NOT EXISTS trash_expiry ON trash(until);
CREATE TABLE IF NOT EXISTS history (
  seq INTEGER PRIMARY KEY, item TEXT NOT NULL, version INTEGER NOT NULL,
  actor TEXT NOT NULL, action TEXT NOT NULL, anchor TEXT, at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS history_age ON history(at);
CREATE TABLE IF NOT EXISTS records (
  item TEXT NOT NULL, anchor TEXT NOT NULL, offset INTEGER NOT NULL, length INTEGER NOT NULL,
  author TEXT NOT NULL, at INTEGER NOT NULL, hash BLOB NOT NULL, ordinal INTEGER NOT NULL,
  PRIMARY KEY (item,anchor)
);
CREATE INDEX IF NOT EXISTS records_order ON records(item, offset);
CREATE INDEX IF NOT EXISTS records_ordinal ON records(item, ordinal);
CREATE TABLE IF NOT EXISTS receipts (
  actor TEXT NOT NULL, gesture TEXT NOT NULL, fingerprint BLOB NOT NULL,
  result TEXT NOT NULL, at INTEGER NOT NULL, PRIMARY KEY (actor, gesture)
);
CREATE INDEX IF NOT EXISTS receipts_age ON receipts(at);
CREATE TABLE IF NOT EXISTS viewer_state (
  plugin TEXT NOT NULL, item TEXT NOT NULL, name TEXT NOT NULL, value TEXT NOT NULL, ordinal INTEGER,
  PRIMARY KEY(plugin,item,name)
);
CREATE TABLE IF NOT EXISTS publication (
  id INTEGER PRIMARY KEY CHECK(id=1), data TEXT NOT NULL
);
";

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Head {
    pub(crate) id: Uuid,
    pub(crate) path: String,
    pub(crate) kind: String,
    pub(crate) author: String,
    pub(crate) version: u64,
    pub(crate) edited: i64,
    pub(crate) prefix: u64,
    pub(crate) length: u64,
    pub(crate) hash: [u8; 32],
}

#[derive(Clone)]
pub(crate) struct Row {
    pub(crate) head: Head,
    pub(crate) fields: Fields,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct Placement {
    pub(crate) destination: String,
    pub(crate) before: Option<[u8; 32]>,
    pub(crate) after: Option<[u8; 32]>,
    pub(crate) stage: Option<String>,
}

pub(crate) struct Space {
    pub(crate) db: Connection,
    pub(crate) files: Folder,
    pub(crate) pending_dir: Folder,
    pub(crate) rows: BTreeMap<Uuid, Row>,
    pub(crate) plugins: BTreeMap<String, Plugin>,
    pub(crate) retention_ms: i64,
    pub(crate) key: [u8; 32],
    pub(crate) seen: BTreeMap<Uuid, crate::records::Signature>,
    pub(crate) id: SpaceId,
    pub(crate) recovery_needed: bool,
    pub(crate) next_trash_check: i64,
    _lock: DataLock,
}

impl Space {
    pub(crate) fn open(root: &Path, id: SpaceId, retention_ms: i64) -> Result<Self> {
        let private = root.join("state").join(id.to_string());
        let root = root.join("spaces").join(id.to_string());
        calternal_fsio::create_dir_all_durable(&root).map_err(internal)?;
        let pending = private.join("pending");
        calternal_fsio::private::directory(&pending).map_err(internal)?;
        let lock = DataLock::acquire_existing(&private).map_err(internal)?;
        let ledger = private.join("ledger.sqlite3");
        calternal_fsio::private::file(&ledger).map_err(internal)?;
        let db = Connection::open(ledger).map_err(internal)?;
        db.execute_batch(SCHEMA).map_err(internal)?;
        let key: Option<Vec<u8>> = db
            .query_row("SELECT key FROM key_material WHERE id=1", [], |row| {
                row.get(0)
            })
            .optional()
            .map_err(internal)?;
        let key = match key {
            Some(bytes) => bytes
                .try_into()
                .map_err(|_| internal("invalid ledger key"))?,
            None => {
                let mut bytes = [0; 32];
                getrandom::fill(&mut bytes).map_err(internal)?;
                db.execute("INSERT INTO key_material VALUES(1,?1)", [&bytes[..]])
                    .map_err(internal)?;
                bytes
            }
        };
        let mut space = Self {
            db,
            files: Folder::open_existing(&root).map_err(internal)?,
            pending_dir: Folder::open_existing(&pending).map_err(internal)?,
            rows: BTreeMap::new(),
            plugins: BTreeMap::new(),
            retention_ms,
            key,
            seen: BTreeMap::new(),
            id,
            recovery_needed: true,
            next_trash_check: 0,
            _lock: lock,
        };
        space.recover()?;
        Ok(space)
    }

    pub(crate) fn recover(&mut self) -> Result<()> {
        if !self.recovery_needed {
            return Ok(());
        }
        let pending: Option<String> = self
            .db
            .query_row("SELECT data FROM publication WHERE id=1", [], |row| {
                row.get(0)
            })
            .optional()
            .map_err(internal)?;
        if let Some(pending) = pending {
            self.publish(&serde_json::from_str::<Vec<Placement>>(&pending).map_err(internal)?)?;
        }
        for name in self.pending_dir.entry_names(3_100).map_err(internal)? {
            self.pending_dir.remove(&name).map_err(internal)?;
        }
        self.load_plugins()?;
        self.rebuild()?;
        self.recovery_needed = false;
        Ok(())
    }

    pub(crate) fn expire_trash(&mut self) -> Result<()> {
        let now = calternal_types::now_ms();
        if now < self.next_trash_check {
            return Ok(());
        }
        let tx = self.db.transaction().map_err(internal)?;
        let entries = {
            let mut stmt = tx
                .prepare_cached("SELECT item FROM trash WHERE until<=?1 ORDER BY until LIMIT 64")
                .map_err(internal)?;
            stmt.query_map([now], |row| row.get::<_, String>(0))
                .map_err(internal)?
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(internal)?
        };
        if !entries.is_empty() {
            let folder = self
                .files
                .child(".calternal", false)
                .and_then(|folder| folder.child("trash", false))
                .map_err(internal)?;
            for item in &entries {
                let path = format!("{item}.md");
                match folder.remove(&path) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => return Err(internal(error)),
                }
                tx.execute("DELETE FROM trash WHERE item=?1", [item])
                    .map_err(internal)?;
            }
            folder.sync().map_err(internal)?;
        }
        tx.commit().map_err(internal)?;
        self.next_trash_check = if entries.len() == 64 {
            now
        } else {
            now + 60_000
        };
        Ok(())
    }

    fn load_plugins(&mut self) -> Result<()> {
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

    pub(crate) fn hash(&self, bytes: &[u8]) -> [u8; 32] {
        *blake3::keyed_hash(&self.key, bytes).as_bytes()
    }

    pub(crate) fn read_optional(&self, relative: &str) -> Result<Option<Vec<u8>>> {
        match self.open_regular(relative) {
            Ok(file) => bounded_read(file, ITEM_BYTES).map(Some),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(internal(err)),
        }
    }

    pub(crate) fn open_folder(&self, relative: &str, create: bool) -> std::io::Result<Folder> {
        let mut parts = calternal_fsio::beneath::plain_parts(relative)?.into_iter();
        let mut current = self
            .files
            .child(parts.next().expect("nonempty path"), create)?;
        for component in parts {
            current = current.child(component, create)?;
        }
        Ok(current)
    }

    pub(crate) fn open_regular(&self, relative: &str) -> std::io::Result<std::fs::File> {
        let (parent, name) = relative
            .rsplit_once('/')
            .ok_or(std::io::ErrorKind::InvalidInput)?;
        let file = self.open_folder(parent, false)?.open_file(name)?;
        if file.metadata()?.nlink() != 1 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "linked file",
            ));
        }
        Ok(file)
    }

    pub(crate) fn read_item(&self, head: &Head) -> Result<Vec<u8>> {
        self.read_optional(&head.path)?
            .ok_or_else(|| Error::conflict("The item file is missing from disk."))
    }

    pub(crate) fn kind(&self, qualified: &str) -> Result<&Kind> {
        qualified
            .split_once('.')
            .and_then(|(plugin, id)| self.plugins.get(plugin)?.kinds.get(id))
            .ok_or_else(|| Error::not_found("Kind was not found."))
    }

    pub(crate) fn collection(&self, qualified: &str) -> Result<&Collection> {
        qualified
            .split_once('.')
            .and_then(|(plugin, id)| self.plugins.get(plugin)?.collections.get(id))
            .ok_or_else(|| Error::not_found("Collection was not found."))
    }

    pub(crate) fn row(&self, id: Uuid) -> Result<&Row> {
        self.rows
            .get(&id)
            .ok_or_else(|| Error::not_found("Item was not found."))
    }

    pub(crate) fn permits(&self, actor: &Actor, kind: &str, write: bool) -> Result<()> {
        let (plugin, _) = identity(actor);
        let owning_plugin = kind.split_once('.').map(|(owner, _)| owner);
        if actor.via.is_none()
            || owning_plugin == Some(plugin)
            || (!write
                && self
                    .plugins
                    .get(plugin)
                    .is_some_and(|decl| decl.reads.iter().any(|id| id == kind)))
        {
            Ok(())
        } else {
            Err(Error::not_found("Item or kind was not found."))
        }
    }

    pub(crate) fn call(&mut self, actor: &Actor, call: Call) -> Result<Value> {
        if let Some(calternal_types::Via::Plugin { plugin }) = &actor.via
            && !self.plugins.contains_key(plugin)
        {
            return Err(Error::forbidden("The calling plugin is unavailable."));
        }
        match call.call.as_str() {
            "s.me" => Ok(json!({"id":actor.account,"role":"owner"})),
            "s.space" => Ok(json!({"id":self.id})),
            "s.query" => crate::query::query(self, actor, decode(call.input)?),
            "s.read" => crate::query::read(self, actor, decode(call.input)?),
            "s.write" => {
                crate::write::write(self, actor, decode(call.input)?, call.idempotency_key)
            }
            "s.state" => {
                crate::query::state(self, actor, decode(call.input)?, call.idempotency_key)
            }
            "index.rebuild" if actor.via.is_none() => {
                self.rebuild()?;
                Ok(json!({"items":self.rows.len()}))
            }
            _ => Err(Error::forbidden("Call is unavailable.")),
        }
    }

    pub(crate) fn rebuild(&mut self) -> Result<()> {
        let mut indexed = BTreeMap::new();
        let mut stmt = self
            .db
            .prepare_cached("SELECT data FROM head")
            .map_err(internal)?;
        for head in stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(internal)?
        {
            let head: Head = serde_json::from_str(&head.map_err(internal)?).map_err(internal)?;
            let bytes = self.read_item(&head)?;
            let prefix = bytes
                .get(..head.prefix as usize)
                .ok_or_else(|| Error::conflict("Item file was shortened on disk."))?;
            if self.hash(prefix) != head.hash {
                return Err(Error::conflict(
                    "Item frontmatter or body was altered on disk.",
                ));
            }
            let (fields, _) =
                declarations::parse_item(std::str::from_utf8(prefix).map_err(internal)?)?;
            indexed.insert(head.id, Row { head, fields });
        }
        self.rows = indexed;
        self.seen.clear();
        Ok(())
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

    pub(crate) fn stage(
        &self,
        path: String,
        before: Option<[u8; 32]>,
        after: Option<&[u8]>,
    ) -> Result<Placement> {
        let stage = if let Some(bytes) = after {
            let mut file = self.pending_dir.create_temp("write").map_err(internal)?;
            file.file().write_all(bytes).map_err(internal)?;
            let name = Uuid::now_v7().to_string();
            Some(
                file.persist(|i| (i == 1).then(|| name.clone()))
                    .map_err(internal)?,
            )
        } else {
            None
        };
        Ok(Placement {
            destination: path,
            before,
            after: after.map(|value| self.hash(value)),
            stage,
        })
    }

    pub(crate) fn finish(&mut self, placements: &[Placement]) -> Result<()> {
        self.publish(placements)?;
        for placement in placements {
            if let Some(stage) = &placement.stage {
                self.pending_dir.remove(stage).map_err(internal)?;
            }
        }
        self.recovery_needed = false;
        Ok(())
    }

    fn publish(&self, placements: &[Placement]) -> Result<()> {
        for p in placements {
            let existing = self
                .read_optional(&p.destination)?
                .map(|bytes| self.hash(&bytes));
            if existing == p.after {
                continue;
            }
            if existing != p.before {
                return Err(Error::conflict("A pending destination changed on disk."));
            }
            let (parent, name) = p
                .destination
                .rsplit_once('/')
                .ok_or_else(|| internal("placement has no directory"))?;
            let folder = self
                .open_folder(parent, p.after.is_some())
                .map_err(internal)?;
            if let Some(stage) = &p.stage {
                let bytes = bounded_read(
                    self.pending_dir.open_file(stage).map_err(internal)?,
                    ITEM_BYTES,
                )?;
                if Some(self.hash(&bytes)) != p.after {
                    return Err(internal("staged publication changed"));
                }
                let temporary_name = format!("{}{}", calternal_fsio::TEMP_PREFIX, stage);
                let mut temp = match folder.create_temp(&temporary_name) {
                    Ok(temp) => temp,
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                        folder.remove(&temporary_name).map_err(internal)?;
                        folder.create_temp(&temporary_name).map_err(internal)?
                    }
                    Err(error) => return Err(internal(error)),
                };
                temp.file().write_all(&bytes).map_err(internal)?;
                if p.before.is_some() {
                    temp.replace(name).map_err(internal)?;
                } else {
                    temp.persist(|i| (i == 1).then(|| name.to_owned()))
                        .map_err(internal)?;
                }
            } else {
                folder.remove(name).map_err(internal)?;
                folder.sync().map_err(internal)?;
            }
        }
        self.db
            .execute("DELETE FROM publication WHERE id=1", [])
            .map_err(internal)?;
        Ok(())
    }

    pub(crate) fn receipt(
        &self,
        actor: &str,
        key: Option<&str>,
        input: &[u8],
    ) -> Result<Option<Receipt>> {
        let Some(key) = key else {
            return Ok(None);
        };
        if key.is_empty() || key.len() > 128 || key.chars().any(char::is_control) {
            return Err(Error::invalid_input("Invalid idempotency key."));
        }
        let fingerprint = self.hash(input);
        let cutoff = calternal_types::now_ms() - self.retention_ms;
        let existing: Option<(Vec<u8>, String)> = self
            .db
            .query_row(
                "SELECT fingerprint,result FROM receipts WHERE actor=?1 AND gesture=?2 AND at>=?3",
                params![actor, key, cutoff],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(internal)?;
        let replay = match existing {
            Some((existing, _)) if existing != fingerprint => return Err(Error::request_reused()),
            Some((_, result)) => Some(serde_json::from_str(&result).map_err(internal)?),
            None => None,
        };
        Ok(Some(Receipt {
            actor: actor.to_owned(),
            key: key.to_owned(),
            fingerprint,
            cutoff,
            replay,
        }))
    }
}

pub(crate) struct Receipt {
    actor: String,
    key: String,
    fingerprint: [u8; 32],
    cutoff: i64,
    pub(crate) replay: Option<Value>,
}

impl Receipt {
    pub(crate) fn store(&self, tx: &Connection, value: &Value, now: i64) -> Result<()> {
        tx.execute("DELETE FROM receipts WHERE at<?1", [self.cutoff])
            .map_err(internal)?;
        tx.execute(
            "INSERT INTO receipts VALUES(?1,?2,?3,?4,?5)",
            params![
                self.actor,
                self.key,
                &self.fingerprint[..],
                value.to_string(),
                now
            ],
        )
        .map_err(internal)?;
        Ok(())
    }
}

pub(crate) fn bounded_read(file: std::fs::File, limit: usize) -> Result<Vec<u8>> {
    let mut content = Vec::new();
    file.take((limit + 1) as u64)
        .read_to_end(&mut content)
        .map_err(internal)?;
    if content.len() > limit {
        return Err(Error::new(
            ErrorCode::TooLarge,
            "File exceeds its byte limit.",
        ));
    }
    Ok(content)
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
