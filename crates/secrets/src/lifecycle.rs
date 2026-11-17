//! Host recovery attachment and irreversible owner erasure.
use crate::local::{SCHEMA, begin, crash_point, open_db};
use crate::{Error, LocalStore, Owner, Result};
use rusqlite::OptionalExtension;
impl LocalStore {
    /// Attach the host's protected recovery barrier before admitting requests.
    /// Refuses a second attachment. Cost: one atomic cell write, no I/O.
    pub fn configure_lifecycle(
        &self,
        lifecycle: std::sync::Arc<dyn crate::KeyLifecycle>,
    ) -> Result<()> {
        self.lifecycle.set(lifecycle).map_err(|_| Error::Internal)
    }

    /// Irreversibly erase an owner's key, even if its database has been lost.
    /// The recovery tombstone precedes unlink, directory fsync and local commit.
    /// Retrying an interrupted erase is safe. The owner transaction spans fsync
    /// and recovery publication to serialize against competing value release.
    /// No old key is cached. Cost: one protected commit and one directory fsync,
    /// plus one local WAL commit when the owner's database still exists.
    pub fn erase_owner(&self, owner: &Owner) -> Result<()> {
        let path = self.path(owner);
        let dir = path.parent().ok_or(Error::Internal)?;
        if !path.try_exists().map_err(|_| Error::Internal)? {
            let fingerprint = match crate::key_fingerprint(&dir.join("secret.key")) {
                Ok(f) => f,
                Err(Error::KeyMissing) => return Ok(()),
                Err(e) => return Err(e),
            };
            return self.erase_key(owner, &fingerprint, dir);
        }
        let mut c = open_db(&path, SCHEMA)?;
        let tx = begin(&mut c)?;
        let fingerprint: Option<Vec<u8>> = tx
            .query_row(
                "SELECT fingerprint FROM key_state WHERE singleton=1",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(|_| Error::Internal)?;
        let fingerprint: [u8; 32] = match fingerprint {
            Some(f) => f.try_into().map_err(|_| Error::KeyUnusable)?,
            None => match crate::key_fingerprint(&dir.join("secret.key")) {
                Ok(f) => f,
                Err(Error::KeyMissing) => return Ok(()),
                Err(e) => return Err(e),
            },
        };
        self.erase_key(owner, &fingerprint, dir)?;
        tx.execute(
            "INSERT INTO key_state VALUES(1,?1,1) ON CONFLICT(singleton) DO UPDATE SET erasing=1",
            [&fingerprint[..]],
        )
        .map_err(|_| Error::Internal)?;
        tx.commit().map_err(|_| Error::Internal)
    }

    fn erase_key(
        &self,
        owner: &Owner,
        fingerprint: &[u8; 32],
        dir: &std::path::Path,
    ) -> Result<()> {
        if let Some(log) = self.lifecycle.get() {
            log.record(owner, fingerprint, true)?;
        }
        crash_point("erase-recorded");
        match std::fs::remove_file(dir.join("secret.key")) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(Error::Internal),
        }
        calternal_fsio::sync_dir(dir).map_err(|_| Error::Internal)?;
        crash_point("erase-unlinked");
        Ok(())
    }
}
