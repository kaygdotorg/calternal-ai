//! Small-VM checkpoint retention and physical erasure.
use super::space::SmallRuntime;
use super::*;
use calternal_types::SpaceId;

/// Provisional preview lifetime of a dormant snapshot.
pub(super) const RETENTION: Duration = Duration::from_secs(24 * 60 * 60);
/// How often a running role collects expired dormant snapshots.
pub(super) const RETENTION_SWEEP: Duration = Duration::from_secs(15 * 60);

/// Sweep individual checkpoint directories independently.
pub(super) async fn sweep(root: PathBuf, runtime: Option<&SmallRuntime>, owner: Option<Arc<File>>) {
    let mut entries = match tokio::fs::read_dir(root).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return,
        Err(error) => {
            tracing::warn!(%error, "cannot list small VM checkpoints");
            return;
        }
    };
    loop {
        let entry = match entries.next_entry().await {
            Ok(Some(entry)) => entry,
            Ok(None) => return,
            Err(error) => {
                tracing::warn!(%error, "cannot enumerate small VM checkpoint directory");
                return;
            }
        };
        let name = entry.file_name();
        let Some(space) = name.to_str().and_then(|name| name.parse::<SpaceId>().ok()) else {
            continue;
        };
        let gate = match runtime {
            Some(runtime) => match runtime.gate(space).try_lock_owned() {
                Ok(gate) => Some(Arc::new(gate)),
                Err(_) => continue,
            },
            None => None,
        };
        let result = async {
            let path = entry.path();
            if super::snapshot::expired(&path, RETENTION).await? {
                erase_saved(path, (gate, owner.clone())).await?;
            }
            Ok::<_, SandboxError>(())
        }
        .await;
        if let Err(error) = result {
            tracing::warn!(%space, %error, "cannot sweep this small VM checkpoint");
        }
    }
}

/// Sweep saved data in a role without a small-VM profile. The VMM root lock
/// proves an old role's guests are gone before this task touches their data.
pub(crate) async fn sweep_unconfigured(sandbox_root: &Path) {
    let root = sandbox_root.join("space-runtime/microvm");
    loop {
        sweep_inactive(&root).await;
        tokio::time::sleep(RETENTION_SWEEP).await;
    }
}

async fn sweep_inactive(root: &Path) {
    match lock_inactive(root).await {
        Ok(Some(owner)) => sweep(root.join("spaces"), None, Some(Arc::new(owner))).await,
        Ok(None) => {}
        Err(error) => tracing::warn!(%error, "cannot sweep inactive small VM storage"),
    }
}

/// Erase a checkpoint with no configured small runtime. Do not unlink files
/// while any Firecracker process from that root still holds the VMM lock.
pub(crate) async fn erase_unconfigured(
    sandbox_root: &Path,
    space: SpaceId,
    through: Option<u64>,
) -> SandboxResult<()> {
    let root = sandbox_root.join("space-runtime/microvm");
    if let Some(owner) = lock_inactive(&root).await? {
        erase_through(root.join("spaces").join(space.to_string()), through, owner).await?;
    }
    Ok(())
}

async fn lock_inactive(root: &Path) -> SandboxResult<Option<File>> {
    let root = root.to_owned();
    tokio::task::spawn_blocking(move || -> io::Result<Option<File>> {
        if !root.try_exists()? {
            return Ok(None);
        }
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(root.join("vmm.lock"))?;
        file.try_lock()
            .map_err(|_| io::Error::other("small plugin VM storage is in use"))?;
        Ok(Some(file))
    })
    .await
    .map_err(SandboxError::new)?
    .map_err(SandboxError::new)
}

/// Remove the checkpoint and flush its parent under the caller’s exclusion.
pub(super) async fn erase_saved(path: PathBuf, gate: impl Send + 'static) -> SandboxResult<()> {
    erase_through(path, None, gate).await
}

/// Erase stale copies under the caller's lock, retaining newer checkpoints.
pub(super) async fn erase_through(
    path: PathBuf,
    through: Option<u64>,
    gate: impl Send + 'static,
) -> SandboxResult<()> {
    tokio::task::spawn_blocking(move || {
        let _gate = gate;
        if let Some(through) = through
            && match super::snapshot::saved_counter(&path) {
                Ok(counter) => counter.is_some_and(|counter| counter > through),
                Err(error) if error.kind() == io::ErrorKind::InvalidData => false,
                Err(error) => return Err(error),
            }
        {
            return Ok(());
        }
        match std::fs::remove_dir_all(&path) {
            Ok(()) => calternal_fsio::sync_dir(path.parent().expect("runtime space parent")),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    })
    .await
    .map_err(SandboxError::new)?
    .map_err(SandboxError::new)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt as _;

    #[tokio::test]
    async fn unconfigured_sweep_skips_a_bad_entry_and_runner_erases_other_checkpoints() {
        use crate::{link::Link, protocol::Down, runner::Runner, sandbox::direct::DirectBackend};
        let temp = tempfile::tempdir().unwrap();
        let spaces = temp.path().join("space-runtime/microvm/spaces");
        let unreadable = spaces.join(SpaceId::new().to_string());
        std::fs::create_dir_all(&unreadable).unwrap();
        std::fs::set_permissions(&unreadable, std::fs::Permissions::from_mode(0)).unwrap();
        let expired = spaces.join(SpaceId::new().to_string());
        std::fs::create_dir(&expired).unwrap();
        let marker = File::create(expired.join("current")).unwrap();
        marker
            .set_modified(std::time::SystemTime::now() - RETENTION - Duration::from_secs(1))
            .unwrap();
        let retained = SpaceId::new();
        std::fs::create_dir(spaces.join(retained.to_string())).unwrap();
        sweep_inactive(spaces.parent().unwrap()).await;
        std::fs::set_permissions(unreadable, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert!(!expired.exists());
        let runner = Arc::new(
            Runner::new(
                Arc::new(DirectBackend::new(temp.path().to_owned())),
                Arc::new(Link::new("http://127.0.0.1:1").unwrap()),
                1,
            )
            .with_space_runtime_root(temp.path().to_owned()),
        );
        let request_id = Uuid::new_v4();
        runner
            .handle(Down::EraseSpaceRuntime {
                request_id,
                space_id: retained,
                through: None,
                background: false,
            })
            .await;
        assert!(!spaces.join(retained.to_string()).exists());
        let reports = runner.discard_reports();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].request_id, request_id);
        assert!(reports[0].success);
    }
}
