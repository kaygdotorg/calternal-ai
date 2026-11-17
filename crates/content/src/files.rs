//! Selected-SSD-tree placement using inherited names, exclusive moves and fsio syncs.
//! Committed path intents survive restart; outside changes are preserved and block writes.
use crate::types::{blocked, internal, missing};
use crate::{Error, FaultPoint, FileFormat, Item, Placement, Result, db::Space, file_hash};
use calternal_fsio::{names, rename_safely, write_atomic_mode};
use rusqlite::{OptionalExtension, params};
use std::{
    fs::{self, File},
    path::{Path, PathBuf},
};
use uuid::Uuid;

/// The reserved standing-instructions file is never an ordinary kind item.
fn ensure_item_path(path: &str) -> Result<()> {
    if calternal_types::is_reserved_space_path(path) {
        Err(Error::forbidden(
            "space instructions require a Manager-only operation",
        ))
    } else {
        Ok(())
    }
}

fn hash(path: &Path) -> Result<Option<String>> {
    match fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(internal(e)),
        Ok(meta) if !meta.is_file() => Err(blocked()),
        Ok(_) => Ok(Some(file_hash(&fs::read(path).map_err(internal)?))),
    }
}
pub(crate) fn validate_kind(kind: &str) -> Result<()> {
    if !kind.is_empty()
        && kind.len() <= 64
        && kind
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        Ok(())
    } else {
        Err(Error::invalid_input("invalid item kind"))
    }
}
fn single(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 120
        && !value.starts_with('.')
        && !value.contains(['/', '\\'])
        && !value.chars().any(char::is_control)
        && names::nfc(value) == value
}
pub(crate) fn validate_registration(placement: &Placement, format: &FileFormat) -> Result<()> {
    let valid = match placement {
        Placement::Visible { folder } => single(folder),
        Placement::VisibleTree {
            folder,
            subdirectories,
        } => {
            single(folder) && subdirectories.len() <= 8 && subdirectories.iter().all(|s| single(s))
        }
        Placement::Hidden { plugin_id } => validate_kind(plugin_id).is_ok(),
    };
    let encoding = match format {
        FileFormat::Markdown => true,
        FileFormat::Opaque { extension } => {
            !extension.is_empty()
                && extension.len() <= 16
                && extension
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        }
    };
    if valid && encoding && !calternal_types::is_reserved_space_path(&folder(placement, "")) {
        Ok(())
    } else {
        Err(Error::invalid_input("invalid kind file registration"))
    }
}
fn folder(placement: &Placement, kind: &str) -> String {
    match placement {
        Placement::Visible { folder } => folder.clone(),
        Placement::VisibleTree {
            folder,
            subdirectories,
        } => std::iter::once(folder.as_str())
            .chain(subdirectories.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join("/"),
        Placement::Hidden { plugin_id } => format!(".calternal/{plugin_id}/{kind}"),
    }
}
pub(crate) fn encoded(item: &Item, format: &FileFormat) -> Result<Vec<u8>> {
    if *format == FileFormat::Markdown {
        // The host owns identity; untrusted frontmatter cannot claim a second ID.
        // Preserve the submitted text as the version body, including its properties.
        let text = std::str::from_utf8(&item.bytes).map_err(internal)?;
        let mut out = format!("---\ncalternal-id: {}\n", item.id);
        if let Some(rest) = text.strip_prefix("---\n")
            && let Some((front, body)) = rest.split_once("\n---\n")
        {
            for line in front.lines() {
                if !line.trim_start().starts_with("calternal-id:") {
                    out.push_str(line);
                    out.push('\n');
                }
            }
            out.push_str("---\n");
            out.push_str(body);
            return Ok(out.into_bytes());
        }
        out.push_str("---\n");
        out.push_str(text);
        Ok(out.into_bytes())
    } else {
        Ok(item.bytes.clone())
    }
}
impl Space {
    fn registration(&self, kind: &str) -> Result<(Placement, FileFormat)> {
        if let Some(definition) = self.registrations.borrow().get(kind) {
            return Ok(definition.clone());
        }
        let json = self
            .conn
            .query_row("SELECT definition FROM kinds WHERE kind=?1", [kind], |r| {
                r.get::<_, String>(0)
            })
            .optional()
            .map_err(internal)?
            .ok_or_else(|| Error::invalid_input("kind is not registered"))?;
        let definition: (Placement, FileFormat) = serde_json::from_str(&json).map_err(internal)?;
        let mut cache = self.registrations.borrow_mut();
        if cache.len() >= crate::db::KIND_CACHE {
            cache.clear();
        }
        cache.insert(kind.into(), definition.clone());
        Ok(definition)
    }
    pub(crate) fn validate_bytes(&self, kind: &str, bytes: &[u8]) -> Result<()> {
        let (_, format) = self.registration(kind)?;
        if format == FileFormat::Markdown && std::str::from_utf8(bytes).is_err() {
            return Err(Error::invalid_input("Markdown must be UTF-8"));
        }
        Ok(())
    }
    pub(crate) fn file_path(&self, id: Uuid) -> Result<PathBuf> {
        let path: String = self
            .conn
            .query_row(
                "SELECT path FROM items WHERE id=?1",
                [id.to_string()],
                |r| r.get(0),
            )
            .optional()
            .map_err(internal)?
            .ok_or_else(missing)?;
        ensure_item_path(&path)?;
        Ok(self.files.join(path))
    }
    /// UUID names avoid Trash collisions; placement remains the existing guarded move.
    pub(crate) fn trash_path(&self, kind: &str, id: Uuid) -> Result<String> {
        let (_, format) = self.registration(kind)?;
        let extension = match format {
            FileFormat::Markdown => "md".to_owned(),
            FileFormat::Opaque { extension } => extension,
        };
        Ok(format!(".trash/{id}.{extension}"))
    }
    pub(crate) fn choose_path(
        &mut self,
        kind: &str,
        title: &str,
        current: Option<&str>,
    ) -> Result<String> {
        let (placement, format) = self.registration(kind)?;
        let folder = folder(&placement, kind);
        let dir = self.files.join(&folder);
        // Prepare the folder once per cached name set, not on every write.
        if !self.names.contains_key(&folder) {
            calternal_fsio::private::directory(&dir).map_err(internal)?;
            if self.names.len() >= crate::db::KIND_CACHE {
                self.names.clear();
            }
        }
        let names = self.names.entry(folder.clone()).or_default();
        names.refresh(&dir).map_err(internal)?;
        let mut stem = names::stem_from_title(title);
        // The inherited helper bounds characters; reserve bytes too for common
        // filesystems' 255-byte component limit, including collision suffixes.
        while stem.len() > 200 {
            stem.pop();
        }
        let extension = match format {
            FileFormat::Markdown => "md".to_owned(),
            FileFormat::Opaque { extension } => extension,
        };
        let taken = |folded: &str| -> Result<bool> {
            let rel = format!("{folder}/{folded}");
            let reserved=self.conn.query_row("SELECT EXISTS(SELECT 1 FROM items WHERE folded_path=?1 AND path!=COALESCE(?2,''))",params![names::fold(&rel),current],|r|r.get::<_,bool>(0)).map_err(internal)?;
            let own = current.is_some_and(|p| names::fold(p) == names::fold(&rel));
            Ok(reserved || (names.taken(folded) && !own))
        };
        let mut number = 1u32;
        let candidate = loop {
            let candidate = if number == 1 {
                format!("{stem}.{extension}")
            } else {
                format!("{stem} {number}.{extension}")
            };
            ensure_item_path(&format!("{folder}/{candidate}"))?;
            if !taken(&names::fold(&candidate))? {
                break candidate;
            }
            number = number.checked_add(1).ok_or_else(|| {
                Error::new(
                    calternal_types::ErrorCode::Unavailable,
                    "no filename is available",
                )
            })?;
        };
        Ok(format!("{folder}/{candidate}"))
    }
    pub(crate) fn check_file(&mut self, id: Uuid) -> Result<()> {
        let (state, expected): (String, Option<String>) = self
            .conn
            .query_row(
                "SELECT mat_state,file_hash FROM items WHERE id=?1",
                [id.to_string()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(internal)?;
        let found = match hash(&self.file_path(id)?) {
            Ok(found) => found,
            Err(e) if e.code == calternal_types::ErrorCode::Conflict => {
                self.block(id)?;
                return Err(e);
            }
            Err(e) => return Err(e),
        };
        if state == "blocked" || found != expected {
            self.block(id)?;
            return Err(blocked());
        }
        Ok(())
    }
    fn block(&self, id: Uuid) -> Result<()> {
        self.conn
            .execute(
                "UPDATE items SET mat_state='blocked' WHERE id=?1",
                [id.to_string()],
            )
            .map_err(internal)?;
        Ok(())
    }
    pub(crate) fn place(&mut self, id: Uuid, fault: fn(FaultPoint)) -> Result<()> {
        let (state, expected, old): (String, Option<String>, Option<String>) = self
            .conn
            .query_row(
                "SELECT mat_state,file_hash,old_path FROM items WHERE id=?1",
                [id.to_string()],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .map_err(internal)?;
        if state == "materialized" {
            return Ok(());
        }
        if state != "pending" {
            return Err(blocked());
        }
        let item = self.get(id)?;
        let (_, format) = self.registration(&item.kind)?;
        let bytes = encoded(&item, &format)?;
        let new_hash = file_hash(&bytes);
        let path = self.file_path(id)?;
        let dir = path.parent().ok_or_else(|| internal("parent"))?;
        // Normally prepared by `choose_path`; recreate only if it vanished since.
        if !dir.is_dir() {
            calternal_fsio::private::directory(dir).map_err(internal)?;
        }
        let result = (|| {
            if let Some(old) = old {
                ensure_item_path(&old)?;
                let old = self.files.join(old);
                let moving = dir.join(format!(".calternal-move-{id}"));
                // A fixed private staging name makes both halves of case-only
                // rename replayable. Every move refuses an occupied destination.
                if old != path && hash(&old)? == expected && expected.is_some() {
                    if names::fold(old.to_str().unwrap_or(""))
                        == names::fold(path.to_str().unwrap_or(""))
                    {
                        if !moving.exists() {
                            rename_safely(&old, &moving).map_err(internal)?;
                        }
                        rename_safely(&moving, &path).map_err(internal)?;
                    } else {
                        rename_safely(&old, &path).map_err(|e| {
                            if e.kind() == std::io::ErrorKind::AlreadyExists {
                                blocked()
                            } else {
                                internal(e)
                            }
                        })?;
                    }
                } else if hash(&moving)? == expected && expected.is_some() {
                    rename_safely(&moving, &path).map_err(internal)?;
                }
            }
            let found = hash(&path)?;
            if found.as_deref() == Some(&new_hash) {
                File::open(&path)
                    .and_then(|f| f.sync_all())
                    .map_err(internal)?;
                calternal_fsio::sync_dir(dir).map_err(internal)?;
            } else if found == expected {
                fault(FaultPoint::BeforePlacement);
                if expected.is_none() {
                    // Private staged bytes are published through inherited exclusive
                    // rename; a destination created after planning is preserved.
                    let stage = dir.join(format!(".calternal-content-{id}"));
                    if let Some(found) = hash(&stage)? {
                        if found != new_hash {
                            return Err(blocked());
                        }
                    } else {
                        write_atomic_mode(&stage, &bytes, 0o600).map_err(internal)?;
                    }
                    rename_safely(&stage, &path).map_err(|e| {
                        if e.kind() == std::io::ErrorKind::AlreadyExists {
                            blocked()
                        } else {
                            internal(e)
                        }
                    })?;
                } else {
                    // Same checked-copy contract as the preview: external writers
                    // racing the final hash/replace interval remain out of scope.
                    write_atomic_mode(&path, &bytes, 0o600).map_err(internal)?;
                }
                fault(FaultPoint::AfterPlacement);
            } else {
                return Err(blocked());
            }
            Ok(())
        })();
        if let Err(e) = result {
            if e.code == calternal_types::ErrorCode::Conflict {
                self.block(id)?;
            }
            return Err(e);
        }
        crate::db::bookkeeping(&self.conn, || {
            self.conn
            .execute(
                "UPDATE items SET mat_state='materialized',file_hash=?1,old_path=NULL WHERE id=?2",
                params![new_hash, id.to_string()],
            )
            .map_err(internal)?;
            Ok(())
        })?;
        Ok(())
    }
    pub(crate) fn cleanup(&self) -> Result<()> {
        // Startup-only cleanup, bounded to indexed directories and proven owned
        // temporary names. User/editor temporary files are always preserved.
        let mut stmt = self
            .conn
            .prepare("SELECT DISTINCT kind FROM items")
            .map_err(internal)?;
        let kinds = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(internal)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(internal)?;
        for kind in kinds {
            let dir = self.files.join(folder(&self.registration(&kind)?.0, &kind));
            if !fs::symlink_metadata(&dir).is_ok_and(|m| m.is_dir()) {
                continue;
            }
            for entry in fs::read_dir(dir).map_err(internal)?.flatten() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                // fsio atomic-write temps include nonce + the known item's final name.
                if let Some(rest) = name.strip_prefix(calternal_fsio::TEMP_PREFIX)
                    && let Some((nonce, final_name)) = rest.split_once('-')
                {
                    let known: bool = self
                        .conn
                        .query_row(
                            "SELECT EXISTS(SELECT 1 FROM items WHERE path=?1)",
                            [format!(
                                "{}/{final_name}",
                                folder(&self.registration(&kind)?.0, &kind)
                            )],
                            |r| r.get(0),
                        )
                        .map_err(internal)?;
                    if known
                        && !nonce.is_empty()
                        && nonce.bytes().all(|c| c.is_ascii_hexdigit())
                        && entry.file_type().is_ok_and(|t| t.is_file())
                    {
                        fs::remove_file(entry.path()).map_err(internal)?;
                    }
                }
            }
        }
        Ok(())
    }
}
