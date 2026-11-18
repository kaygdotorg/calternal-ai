//! SQLite authority for account preferences, uploads, and shared cache manifests.
use super::{
    assets, db_error,
    face::{Face, Format},
    missing, prefs,
};
use calternal_core::*;
use calternal_types::Error;
use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior, params};
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

const MIB: usize = 1024 * 1024;
pub(super) const FAMILY_BYTES: usize = 32 * MIB;
const SHARED_BYTES: usize = 500 * MIB;
const SHARED_FAMILIES: usize = 4096;
pub(super) const UPLOAD_BYTES: usize = 50 * MIB;
pub(super) const FILE_BYTES: usize = 8 * MIB;
pub(super) const UPLOAD_FILES: usize = 12;

/// Initial version of the new appearance authority. Shared with the release inventory.
pub(crate) const SCHEMA: calternal_store::migrations::Schema<'_> =
    calternal_store::migrations::Schema::new(
        "appearance.db",
        1,
        &["
CREATE TABLE appearance(account TEXT PRIMARY KEY, prefs TEXT NOT NULL) STRICT;
CREATE TABLE shared_families(
    id TEXT PRIMARY KEY, family TEXT NOT NULL, source TEXT NOT NULL,
    bytes INTEGER NOT NULL, faces TEXT NOT NULL, attribution TEXT NOT NULL,
    installed INTEGER NOT NULL
) STRICT;
CREATE TABLE shared_blobs(
    id TEXT NOT NULL REFERENCES shared_families(id) ON DELETE CASCADE,
    digest TEXT NOT NULL, bytes INTEGER NOT NULL, PRIMARY KEY(id,digest)
) STRICT;
CREATE TABLE font_pins(
    account TEXT NOT NULL REFERENCES appearance(account) ON DELETE CASCADE,
    id TEXT NOT NULL REFERENCES shared_families(id),
    PRIMARY KEY(account,id)
) STRICT;
CREATE INDEX font_pins_family ON font_pins(id);
CREATE TABLE uploads(
    id TEXT PRIMARY KEY, account TEXT NOT NULL,
    family TEXT NOT NULL, name_key TEXT NOT NULL, UNIQUE(account,name_key)
) STRICT;
CREATE TABLE upload_faces(
    id TEXT NOT NULL REFERENCES uploads(id) ON DELETE CASCADE, digest TEXT NOT NULL,
    descriptor TEXT NOT NULL, data BLOB NOT NULL, PRIMARY KEY(id,digest)
) STRICT;
"],
        &[],
    );

pub(super) struct Storage {
    path: PathBuf,
    cache: PathBuf,
    pub(super) used: std::sync::Mutex<HashMap<String, Instant>>,
}

pub(super) struct RemoteInstall {
    pub family: FontCatalogueFamily,
    pub faces: Vec<Face>,
    pub files: BTreeMap<String, Vec<u8>>,
    pub attribution: String,
}

pub(super) struct Upload {
    pub face: Face,
    pub bytes: Vec<u8>,
}

pub(super) fn encode(v: &impl serde::Serialize) -> Result<String, Error> {
    serde_json::to_string(v).map_err(db_error)
}

pub(super) fn decode<T: serde::de::DeserializeOwned>(raw: &str) -> Result<T, Error> {
    serde_json::from_str(raw).map_err(db_error)
}

impl Storage {
    pub(super) fn open(appdata: &Path) -> Result<Self, Error> {
        let system = appdata.join("system");
        calternal_fsio::private::directory(&system).map_err(db_error)?;
        let path = system.join("appearance.db");
        calternal_fsio::private::file(&path).map_err(db_error)?;
        let cache = appdata.join("fonts-cache-v1");
        calternal_fsio::private::directory(&cache).map_err(db_error)?;
        let storage = Self {
            path,
            cache,
            used: Default::default(),
        };
        let c = storage.connect(true)?;
        calternal_store::migrations::migrate(&c, SCHEMA).map_err(db_error)?;
        storage.cleanup(&c)?;
        Ok(storage)
    }

    pub(super) fn connect(&self, write: bool) -> Result<Connection, Error> {
        let flags = if write {
            OpenFlags::SQLITE_OPEN_READ_WRITE
        } else {
            OpenFlags::SQLITE_OPEN_READ_ONLY
        };
        let c = Connection::open_with_flags(&self.path, flags).map_err(db_error)?;
        c.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(db_error)?;
        if write {
            // DELETE journaling plus secure-delete retires account bytes with
            // their durable deletion, including during owner-erasure replay.
            c.execute_batch("PRAGMA journal_mode=DELETE; PRAGMA synchronous=EXTRA; PRAGMA foreign_keys=ON; PRAGMA secure_delete=ON;").map_err(db_error)?;
        }
        Ok(c)
    }

    pub(super) fn write<T>(
        &self,
        f: impl FnOnce(&Connection) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let mut c = self.connect(true)?;
        let tx = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let result = f(&tx)?;
        tx.commit().map_err(db_error)?;
        Ok(result)
    }

    pub(super) fn touch(&self, id: &str) {
        let mut used = self.used.lock().unwrap_or_else(|p| p.into_inner());
        if used.len() >= SHARED_FAMILIES
            && !used.contains_key(id)
            && let Some(old) = used
                .iter()
                .min_by_key(|(_, time)| *time)
                .map(|(id, _)| id.clone())
        {
            used.remove(&old);
        }
        used.insert(id.to_owned(), Instant::now());
    }

    fn cleanup(&self, c: &Connection) -> Result<(), Error> {
        // A manifest is published only after all bytes have been synced. Verify
        // its size inventory at startup before removing anything derived.
        let mut q = c
            .prepare("SELECT id,digest,bytes FROM shared_blobs")
            .map_err(db_error)?;
        let blobs = q
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, u32>(2)?,
                ))
            })
            .map_err(db_error)?;
        for blob in blobs {
            let (id, digest, bytes) = blob.map_err(db_error)?;
            let path = self.blob_path(&id, &digest)?;
            let meta = path.symlink_metadata().map_err(db_error)?;
            if !meta.is_file() || meta.len() != u64::from(bytes) {
                return Err(Error::unavailable("a published font cache file is damaged"));
            }
        }
        self.prune_unpublished(c)
    }

    fn prune_unpublished(&self, c: &Connection) -> Result<(), Error> {
        for entry in fs::read_dir(&self.cache).map_err(db_error)? {
            let entry = entry.map_err(db_error)?;
            let name = entry.file_name();
            let retained: bool = c
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM shared_families WHERE id=?1)",
                    [name.to_string_lossy()],
                    |r| r.get(0),
                )
                .map_err(db_error)?;
            if !retained {
                remove(&entry.path()).map_err(db_error)?;
            }
        }
        Ok(())
    }

    fn blob_path(&self, id: &str, digest: &str) -> Result<PathBuf, Error> {
        super::validate_id(id).map_err(db_error)?;
        super::validate_digest(digest).map_err(db_error)?;
        Ok(self.cache.join(id).join(digest))
    }

    pub(super) fn deliver(
        &self,
        account: &str,
        id: &str,
        digest: &str,
    ) -> Result<(Format, Vec<u8>, bool), Error> {
        let c = self.connect(false)?;
        if id.starts_with("upload-") {
            let hit: Option<(String, Vec<u8>)> = c
                .query_row(
                    "SELECT descriptor,data FROM upload_faces JOIN uploads USING(id)
                 WHERE uploads.account=?1 AND id=?2 AND digest=?3",
                    params![account, id, digest],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()
                .map_err(db_error)?;
            let (raw, bytes) = hit.ok_or_else(missing)?;
            let face: Face = decode(&raw)?;
            return Ok((face.format, bytes, true));
        }
        let cap: u32 = c
            .query_row(
                "SELECT bytes FROM shared_blobs WHERE id=?1 AND digest=?2",
                params![id, digest],
                |r| r.get(0),
            )
            .optional()
            .map_err(db_error)?
            .ok_or_else(missing)?;
        let cap = cap as usize;
        let path = self.blob_path(id, digest)?;
        let bytes = assets::read(&path, cap.min(FAMILY_BYTES))
            .map_err(db_error)?
            .ok_or_else(missing)?;
        if bytes.len() != cap
            || Format::identify(&bytes) != Some(Format::Woff2)
            || crate::store::sha256_hex(&bytes) != digest
        {
            return Err(Error::unavailable("a published font cache file is damaged"));
        }
        self.touch(id);
        Ok((Format::Woff2, bytes, false))
    }

    // Only the install publisher calls this. Its lock covers file and manifest
    // publication, never an account operation or outbound request.
    pub(super) fn publish(&self, install: RemoteInstall) -> Result<(), Error> {
        let id = &install.family.id;
        let bytes: usize = install.files.values().map(Vec::len).sum();
        let incoming = self.cache.join(format!(".{id}.staging"));
        let result = (|| {
            self.prune_unpublished(&self.connect(false)?)?;
            let evicted = self.write(|c| self.evict(c, bytes, SHARED_BYTES))?;
            for victim in evicted {
                super::validate_id(&victim).map_err(db_error)?;
                remove(&self.cache.join(&victim)).map_err(db_error)?;
                self.used
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .remove(&victim);
            }
            fs::create_dir(&incoming).map_err(db_error)?;
            for (key, data) in &install.files {
                assets::write(&incoming.join(key), data).map_err(db_error)?;
            }
            let placed = self.cache.join(id);
            remove(&placed).map_err(db_error)?;
            fs::rename(&incoming, &placed).map_err(db_error)?;
            calternal_fsio::sync_dir(&self.cache).map_err(db_error)?;
            let written = self.write(|c| {
                c.execute(
                    "INSERT INTO shared_families VALUES(?1,?2,?3,?4,?5,?6,unixepoch())",
                    params![
                        id,
                        install.family.family,
                        encode(&install.family.source)?,
                        bytes as u32,
                        encode(&install.faces)?,
                        install.attribution
                    ],
                )
                .map_err(db_error)?;
                for (digest, data) in &install.files {
                    c.execute(
                        "INSERT INTO shared_blobs VALUES(?1,?2,?3)",
                        params![id, digest, data.len() as u32],
                    )
                    .map_err(db_error)?;
                }
                Ok(())
            });
            // On a commit error, the next publisher or startup reconciles
            // the directory with SQLite before unlinking any prepared bytes.
            if written.is_ok() {
                self.touch(id);
            }
            written
        })();
        if result.is_err() {
            remove(&incoming).map_err(db_error)?;
        }
        result
    }

    fn evict(&self, c: &Connection, bytes: usize, max: usize) -> Result<Vec<String>, Error> {
        let (total, count): (u32, u32) = c
            .query_row(
                "SELECT coalesce(sum(bytes),0),count(*) FROM shared_families",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(db_error)?;
        let (mut total, mut count) = (total as usize, count as usize);
        let fits = |total, count| total + bytes <= max && count < SHARED_FAMILIES;
        if fits(total, count) {
            return Ok(Vec::new());
        }
        let mut q = c
            .prepare(
                "SELECT id,bytes,installed FROM shared_families s
            WHERE NOT EXISTS(SELECT 1 FROM font_pins p WHERE p.id=s.id)",
            )
            .map_err(db_error)?;
        let mut candidates = q
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, u32>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            })
            .map_err(db_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(db_error)?;
        {
            let used = self.used.lock().unwrap_or_else(|p| p.into_inner());
            candidates.sort_unstable_by(|a, b| {
                used.get(&a.0)
                    .cmp(&used.get(&b.0))
                    .then(a.2.cmp(&b.2))
                    .then(a.0.cmp(&b.0))
            });
        }
        let mut removed = Vec::new();
        for (id, size, _) in candidates {
            c.execute("DELETE FROM shared_families WHERE id=?1", [&id])
                .map_err(db_error)?;
            removed.push(id);
            total -= size as usize;
            count -= 1;
            if fits(total, count) {
                return Ok(removed);
            }
        }
        Err(Error::unavailable(
            "selected families leave no room in the shared font cache",
        ))
    }
}

fn remove(path: &Path) -> std::io::Result<()> {
    match path.symlink_metadata() {
        Ok(meta) if meta.is_dir() => fs::remove_dir_all(path),
        Ok(_) => fs::remove_file(path),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

pub(super) fn family(
    c: &Connection,
    account: &str,
    id: &str,
) -> Result<Option<FontStoredFamily>, Error> {
    let mut q = c
        .prepare(
            "SELECT id,family,source,bytes FROM shared_families WHERE id=?2
        UNION ALL SELECT id,family,'\"upload\"',
            (SELECT coalesce(sum(length(data)),0) FROM upload_faces uf WHERE uf.id=u.id)
        FROM uploads u WHERE account=?1 AND id=?2",
        )
        .map_err(db_error)?;
    q.query_row(params![account, id], row)
        .optional()
        .map_err(db_error)?
        .map(|v| as_family(v, &[]))
        .transpose()
}

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<(String, String, String, u32)> {
    Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
}

fn as_family(
    (id, family, source, bytes): (String, String, String, u32),
    roles: &[(FontRole, &str)],
) -> Result<FontStoredFamily, Error> {
    let used_by = roles
        .iter()
        .filter_map(|(r, font)| (*font == id).then_some(*r))
        .collect();
    Ok(FontStoredFamily {
        id,
        family,
        source: decode(&source)?,
        bytes,
        used_by,
    })
}

pub(super) fn list(
    c: &Connection,
    account: &str,
    after: &str,
    limit: usize,
) -> Result<FontListResult, Error> {
    let prefs = prefs::get(c, account)?;
    let roles: Vec<_> = prefs::ROLES
        .into_iter()
        .filter_map(|role| {
            prefs::role(&prefs.fonts, role)
                .id
                .as_deref()
                .map(|id| (role, id))
        })
        .collect();
    let mut q = c
        .prepare(
            "SELECT id,family,source,bytes FROM shared_families WHERE id>?2
        UNION ALL SELECT id,family,'\"upload\"',
            (SELECT coalesce(sum(length(data)),0) FROM upload_faces uf WHERE uf.id=u.id)
        FROM uploads u WHERE account=?1 AND id>?2 ORDER BY id LIMIT ?3",
        )
        .map_err(db_error)?;
    let mut families = q
        .query_map(params![account, after, (limit + 1) as u32], row)
        .map_err(db_error)?
        .map(|result| as_family(result.map_err(db_error)?, &roles))
        .collect::<Result<Vec<_>, _>>()?;
    let more = families.len() > limit;
    families.truncate(limit);
    let next_cursor = more.then(|| families[limit - 1].id.clone());
    Ok(FontListResult {
        families,
        next_cursor,
    })
}

pub(super) fn faces(c: &Connection, account: &str, id: &str) -> Result<Option<Vec<Face>>, Error> {
    if id.starts_with("upload-") {
        let mut q = c
            .prepare(
                "SELECT descriptor FROM upload_faces JOIN uploads USING(id)
            WHERE account=?1 AND id=?2 ORDER BY digest",
            )
            .map_err(db_error)?;
        let mut faces = Vec::new();
        for raw in q
            .query_map(params![account, id], |r| r.get::<_, String>(0))
            .map_err(db_error)?
        {
            faces.push(decode(&raw.map_err(db_error)?)?);
        }
        return Ok((!faces.is_empty()).then_some(faces));
    }
    let raw: Option<String> = c
        .query_row("SELECT faces FROM shared_families WHERE id=?1", [id], |r| {
            r.get(0)
        })
        .optional()
        .map_err(db_error)?;
    raw.map(|v| decode(&v)).transpose()
}

pub(super) fn upload(
    c: &Connection,
    account: &str,
    name: &str,
    uploads: Vec<Upload>,
) -> Result<FontStoredFamily, Error> {
    let name_key = name.to_lowercase();
    let id = c
        .query_row(
            "SELECT id FROM uploads WHERE account=?1 AND name_key=?2",
            params![account, name_key],
            |r| r.get::<_, String>(0),
        )
        .optional()
        .map_err(db_error)?
        .unwrap_or_else(|| format!("upload-{}", uuid::Uuid::new_v4().simple()));
    c.execute(
        "INSERT OR IGNORE INTO uploads VALUES(?1,?2,?3,?4)",
        params![id, account, name, name_key],
    )
    .map_err(db_error)?;
    for upload in uploads {
        c.execute(
            "INSERT INTO upload_faces VALUES(?1,?2,?3,?4)
            ON CONFLICT(id,digest) DO NOTHING",
            params![id, upload.face.digest, encode(&upload.face)?, upload.bytes],
        )
        .map_err(db_error)?;
    }
    let files: u32 = c
        .query_row(
            "SELECT count(*) FROM upload_faces WHERE id=?1",
            [&id],
            |r| r.get(0),
        )
        .map_err(db_error)?;
    let total: u32 = c
        .query_row(
            "SELECT coalesce(sum(length(data)),0) FROM upload_faces
        JOIN uploads USING(id) WHERE account=?1",
            [account],
            |r| r.get(0),
        )
        .map_err(db_error)?;
    if files as usize > UPLOAD_FILES || total as usize > UPLOAD_BYTES {
        return Err(Error::invalid_input("personal font storage limit reached"));
    }
    family(c, account, &id)?.ok_or_else(missing)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_evicts_oldest_unused_and_rolls_back_if_pins_prevent_a_fill() {
        let root = tempfile::tempdir().unwrap();
        let s = Storage::open(root.path()).unwrap();
        s.write(|c| {
            c.execute("INSERT INTO appearance VALUES('a','{}')", [])
                .map_err(db_error)?;
            for (id, installed) in [("old", 1), ("pinned", 2), ("new", 3)] {
                c.execute(
                    "INSERT INTO shared_families VALUES(?1,?1,'google',10,'[]','',?2)",
                    params![id, installed],
                )
                .map_err(db_error)?;
            }
            c.execute("INSERT INTO font_pins VALUES('a','pinned')", [])
                .map_err(db_error)?;
            Ok(())
        })
        .unwrap();
        assert_eq!(s.write(|c| s.evict(c, 10, 30)).unwrap(), vec!["old"]);
        assert!(s.write(|c| s.evict(c, 25, 30)).is_err());
        let c = s.connect(false).unwrap();
        let count: i64 = c
            .query_row("SELECT count(*) FROM shared_families", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn failed_appearance_baseline_keeps_user_version_and_existing_bytes() {
        let root = tempfile::tempdir().unwrap();
        let system = root.path().join("system");
        fs::create_dir(&system).unwrap();
        let path = system.join("appearance.db");
        let c = Connection::open(&path).unwrap();
        c.execute_batch(
            "CREATE TABLE shared_blobs(marker TEXT); INSERT INTO shared_blobs VALUES('preserve');",
        )
        .unwrap();
        drop(c);
        assert!(Storage::open(root.path()).is_err());
        let c = Connection::open(path).unwrap();
        assert_eq!(
            c.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                .unwrap(),
            0
        );
        assert_eq!(
            c.query_row("SELECT marker FROM shared_blobs", [], |r| r
                .get::<_, String>(0))
                .unwrap(),
            "preserve"
        );
        assert!(c.prepare("SELECT * FROM appearance").is_err());
    }
}
