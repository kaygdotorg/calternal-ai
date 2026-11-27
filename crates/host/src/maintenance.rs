//! Bounded expiry and residency for the standalone host's per-space registry.

use crate::{Host, internal, space::Space};
use calternal_fsio::{DataLock, beneath::Folder};
use calternal_types::{Error, Result, SpaceId};
use rusqlite::{Connection, OpenFlags};
use std::{
    fs::{self, ReadDir},
    io::ErrorKind,
    os::unix::fs::MetadataExt,
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

const RESIDENT_LIMIT: usize = 128;
const DORMANT_BATCH: usize = 16;
const IDLE: Duration = Duration::from_secs(5 * 60);
const INTERVAL: Duration = Duration::from_secs(30);

pub(crate) type Slot = Arc<SpaceSlot>;

pub(crate) struct SpaceSlot {
    pub(crate) resident: Mutex<Resident>,
}

pub(crate) struct Resident {
    pub(crate) space: Option<Space>,
    pub(crate) visited: Instant,
}

impl Host {
    /// Start one maintenance loop for a serving host. Each tick uses one
    /// blocking job for indexed expiry and idle slot reclamation. The
    /// loop drops its host reference between ticks to allow shutdown.
    pub fn start_maintenance(self: &Arc<Self>) {
        let host = Arc::downgrade(self);
        tokio::spawn(async move {
            let mut cursor = None;
            loop {
                tokio::time::sleep(INTERVAL).await;
                let Some(host) = host.upgrade() else {
                    return;
                };
                let result = tokio::task::spawn_blocking(move || {
                    if let Err(error) = host.sweep(&mut cursor) {
                        internal(error);
                    }
                    cursor
                })
                .await;
                let Ok(next) = result else { return };
                cursor = next;
            }
        });
    }

    pub(crate) fn space_slot(&self, id: SpaceId) -> Result<Slot> {
        let mut registry = self.spaces.lock().map_err(internal)?;
        if let Some(slot) = registry.get(&id) {
            return Ok(slot.clone());
        }
        if registry.len() == RESIDENT_LIMIT {
            return Err(Error::unavailable("The host has too many open spaces."));
        }
        let slot = Arc::new(SpaceSlot {
            resident: Mutex::new(Resident {
                space: None,
                visited: Instant::now(),
            }),
        });
        registry.insert(id, slot.clone());
        Ok(slot)
    }

    fn sweep(&self, cursor: &mut Option<ReadDir>) -> Result<()> {
        let active: Vec<_> = self
            .spaces
            .lock()
            .map_err(internal)?
            .iter()
            .map(|(id, slot)| (*id, slot.clone()))
            .collect();
        for (id, slot) in active {
            self.sweep_open(id, slot)?;
        }
        if cursor.is_none() {
            let state = self.root.join("state");
            let state_meta = match fs::symlink_metadata(&state) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
                Err(error) => return Err(internal(error)),
            };
            let root = fs::symlink_metadata(&self.root).map_err(internal)?;
            checked_directory(&root, root.uid())?;
            checked_directory(&state_meta, root.uid())?;
            match fs::read_dir(state) {
                Ok(entries) => *cursor = Some(entries),
                Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
                Err(error) => return Err(internal(error)),
            }
        }
        let entries = cursor.as_mut().expect("opened directory");
        for _ in 0..DORMANT_BATCH {
            let Some(entry) = entries.next() else {
                *cursor = None;
                break;
            };
            let entry = entry.map_err(internal)?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            let Ok(id) = name.parse::<SpaceId>() else {
                continue;
            };
            if name == id.to_string() && entry.file_type().map_err(internal)?.is_dir() {
                if let Err(error) = self.sweep_dormant(id) {
                    internal(error);
                }
            }
        }
        Ok(())
    }

    fn sweep_open(&self, id: SpaceId, slot: Slot) -> Result<()> {
        let Ok(mut entry) = slot.resident.try_lock() else {
            return Ok(());
        };
        let idle = entry.visited.elapsed() >= IDLE;
        let busy = entry
            .space
            .as_ref()
            .is_some_and(|space| space.live.has_subscribers() || space.runs.is_active());
        if let Some(space) = &mut entry.space {
            if let Err(error) = space.expire_trash() {
                internal(error);
            }
        }
        if idle && !busy {
            let registry = self.spaces.lock().map_err(internal)?;
            if Arc::strong_count(&slot) == 2 {
                // The maintenance guard and the registry are the last two
                // holders. Close under the slot lock so a caller arriving
                // during SQLite close waits on this same slot.
                drop(registry);
                drop(entry.space.take());
                self.unregister_empty(id, &slot)?;
            }
        }
        Ok(())
    }

    fn unregister_empty(&self, id: SpaceId, slot: &Slot) -> Result<()> {
        let mut registry = self.spaces.lock().map_err(internal)?;
        if Arc::strong_count(slot) == 2 {
            registry.remove(&id);
        }
        Ok(())
    }

    fn sweep_dormant(&self, id: SpaceId) -> Result<()> {
        let slot = self.space_slot(id)?;
        let Ok(entry) = slot.resident.try_lock() else {
            return Ok(());
        };
        if entry.space.is_some() {
            return Ok(());
        }
        // Keep the same slot visible to concurrent demand until the dormant
        // ledger has closed. This path opens no item catalog or worker.
        let result = self.expire_dormant(id);
        self.unregister_empty(id, &slot)?;
        result
    }

    fn expire_dormant(&self, id: SpaceId) -> Result<()> {
        let name = id.to_string();
        let state = self.root.join("state");
        let private = state.join(&name);
        let root = fs::symlink_metadata(&self.root).map_err(internal)?;
        checked_directory(&root, root.uid())?;
        let base = Folder::open_existing(&self.root).map_err(internal)?;
        let folder = existing_child(&base, &state, "state", root.uid())?;
        let folder = existing_child(&folder, &private, &name, root.uid())?;
        let ledger = folder.open_file("ledger.sqlite3").map_err(internal)?;
        let metadata = ledger.metadata().map_err(internal)?;
        if metadata.uid() != root.uid() || metadata.mode() & 0o077 != 0 || metadata.nlink() != 1 {
            return Err(Error::forbidden(
                "The dormant ledger must be private to the host.",
            ));
        }
        let private_handle = fs::File::open(&private).map_err(internal)?;
        let _guard = DataLock::acquire_at(&private_handle, &private).map_err(internal)?;
        let mut db = Connection::open_with_flags(
            private.join("ledger.sqlite3"),
            OpenFlags::SQLITE_OPEN_READ_WRITE,
        )
        .map_err(internal)?;
        db.execute_batch("PRAGMA secure_delete=ON")
            .map_err(internal)?;
        let tx = db.transaction().map_err(internal)?;
        let now = calternal_types::now_ms();
        let cutoff = now - self.retention_ms;
        tx.execute(
            "DELETE FROM receipts WHERE rowid IN
                (SELECT rowid FROM receipts WHERE at<?1 ORDER BY at LIMIT 64)",
            [cutoff],
        )
        .map_err(internal)?;
        tx.execute(
            "DELETE FROM recovered_appends WHERE rowid IN
                (SELECT rowid FROM recovered_appends WHERE at<?1 ORDER BY at LIMIT 64)",
            [cutoff],
        )
        .map_err(internal)?;
        let expired = {
            let mut query = tx
                .prepare_cached(
                    "SELECT item FROM trash WHERE until<=?1
                AND NOT EXISTS (SELECT 1 FROM publication) ORDER BY until LIMIT 64",
                )
                .map_err(internal)?;
            query
                .query_map([now], |row| row.get::<_, String>(0))
                .map_err(internal)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(internal)?
        };
        if !expired.is_empty() {
            let spaces_path = self.root.join("spaces");
            let spaces = existing_child(&base, &spaces_path, "spaces", root.uid())?;
            let space = existing_child(&spaces, &spaces_path.join(&name), &name, root.uid())?;
            let trash = space
                .child(".calternal", false)
                .and_then(|metadata| metadata.child("trash", false))
                .map_err(internal)?;
            for item in expired {
                let id = uuid::Uuid::parse_str(&item).map_err(internal)?;
                match trash.remove(&format!("{id}.md")) {
                    Ok(()) => (),
                    Err(error) if error.kind() == ErrorKind::NotFound => (),
                    Err(error) => return Err(internal(error)),
                }
                tx.execute("DELETE FROM trash WHERE item=?1", [item])
                    .map_err(internal)?;
            }
            trash.sync().map_err(internal)?;
        }
        tx.commit().map_err(internal)
    }
}

fn checked_directory(metadata: &fs::Metadata, owner: u32) -> Result<()> {
    if !metadata.is_dir() || metadata.uid() != owner || metadata.mode() & 0o022 != 0 {
        return Err(Error::forbidden(
            "The storage tree must be writable only by its owner.",
        ));
    }
    Ok(())
}

fn existing_child(parent: &Folder, path: &Path, name: &str, owner: u32) -> Result<Folder> {
    let child = parent.child(name, false).map_err(internal)?;
    checked_directory(&fs::symlink_metadata(path).map_err(internal)?, owner)?;
    Ok(child)
}

#[cfg(test)]
mod tests {
    use super::*;
    use calternal_auth::{Auth, AuthConfig};
    use calternal_types::Actor;
    use serde_json::Value;

    #[tokio::test]
    async fn dormant_receipts_expire_and_live_or_held_slots_stay_unique() {
        let root = tempfile::tempdir().unwrap();
        let auth_dir = root.path().join("auth");
        let auth = tokio::task::spawn_blocking(move || {
            Auth::open(&auth_dir, AuthConfig::new("http://localhost").unwrap()).unwrap()
        })
        .await
        .unwrap();
        let owner = auth.create("Owner").await.unwrap().id;
        let host = Host::new(root.path().into(), auth, 1).unwrap();
        tokio::task::spawn_blocking(move || {
            let space = host.auth.spaces().list(owner).unwrap().remove(0).id;
            let observer = host
                .with_space(&Actor::account(owner), space, |storage| {
                    let receipt = storage
                        .receipt("fixture", Some("old"), b"payload")?
                        .unwrap();
                    receipt.store(
                        &storage.db,
                        &Value::Null,
                        calternal_types::now_ms() - 86_400_100,
                    )?;
                    Ok(storage.live.subscribe(owner))
                })
                .unwrap();
            let slot = host.space_slot(space).unwrap();
            slot.resident.lock().unwrap().visited = Instant::now() - IDLE;
            host.sweep_open(space, slot.clone()).unwrap();
            assert!(Arc::ptr_eq(&slot, &host.space_slot(space).unwrap()));
            drop(slot);
            host.sweep(&mut None).unwrap();
            assert!(host.spaces.lock().unwrap().contains_key(&space));
            drop(observer);
            host.sweep(&mut None).unwrap();
            assert!(!host.spaces.lock().unwrap().contains_key(&space));
            let db = Connection::open(
                host.root
                    .join("state")
                    .join(space.to_string())
                    .join("ledger.sqlite3"),
            )
            .unwrap();
            assert_eq!(
                db.query_row("SELECT count(*) FROM receipts", [], |row| row
                    .get::<_, i64>(0))
                    .unwrap(),
                0
            );
        })
        .await
        .unwrap();
    }
}
