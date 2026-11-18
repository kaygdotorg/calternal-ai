//! Serialize the daemon with a machine owner's explicit local discard.
use crate::{Error, Result};
use std::{fs::File, path::Path};

/// Administrative lock guard. Explicit nonblocking unlock prevents fork-inherited
/// descriptors from extending ownership beyond this guard. No data write or flush.
pub struct OwnerLock(File);
impl Drop for OwnerLock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

/// Hold the machine-directory lock for a daemon's lifetime or one local deletion.
/// This is an administrative lock, never a per-space request lock. Opening and
/// nonblocking acquisition run on a blocking worker; another owner command fails
/// with a next step instead of deleting a live workspace.
pub(crate) async fn lock(dir: &Path) -> Result<OwnerLock> {
    let dir = dir.to_owned();
    tokio::task::spawn_blocking(move || lock_for_upgrade(&dir))
        .await
        .map_err(|_| Error::Protocol("machine ownership check failed".into()))?
}

/// Lock an existing machine state directory while its role is stopped. The
/// lock shares the daemon's exclusion and lasts until the returned guard drops.
/// Opens one local file and tries the lock once. Call on a blocking worker.
pub fn lock_for_upgrade(dir: &Path) -> Result<OwnerLock> {
    use std::os::unix::fs::OpenOptionsExt;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(dir.join(".machine-role.lock"))?;
    file.try_lock().map_err(|_| Error::Protocol(
        "the machine role is already running; stop it before another role or local workspace deletion".into()
    ))?;
    Ok(OwnerLock(file))
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn owner_deletion_cannot_overlap_a_machine_role() {
        let dir = tempfile::tempdir().unwrap();
        let role = super::lock(dir.path()).await.unwrap();
        assert!(super::lock(dir.path()).await.is_err());
        drop(role);
        assert!(super::lock(dir.path()).await.is_ok());
    }
}
