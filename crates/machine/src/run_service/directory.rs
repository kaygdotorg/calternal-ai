use std::{
    fs::File,
    io,
    os::fd::AsRawFd,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

/// Private owner for per-run socket files with nonblocking drop cleanup.
pub(super) struct RunDirectory {
    path: PathBuf,
    sockets: File,
    cleaned: AtomicBool,
}

impl RunDirectory {
    /// Provision private, isolated engine and socket paths under the sweep root.
    pub(super) fn create(root: &Path) -> io::Result<Self> {
        calternal_fsio::private::directory(root)?;
        let directory = tempfile::Builder::new().prefix("run.").tempdir_in(root)?;
        let path = directory.path().join("sockets");
        std::fs::create_dir(&path)?;
        let sockets = File::open(path)?;
        Ok(Self {
            path: directory.keep(),
            sockets,
            cleaned: AtomicBool::new(false),
        })
    }

    /// The actual directory mounted into the run for Unix sockets.
    pub(super) fn sockets(&self) -> PathBuf {
        self.path.join("sockets")
    }

    /// A short path to bind or unlink sockets regardless of the data root's length.
    pub(super) fn socket_path(&self, name: &str) -> PathBuf {
        PathBuf::from(format!("/proc/self/fd/{}/{name}", self.sockets.as_raw_fd()))
    }

    /// Remove the directory asynchronously after the proxy and container stop.
    pub(super) async fn cleanup(&self) {
        if self.cleaned.swap(true, Ordering::AcqRel) {
            return;
        }
        if let Err(error) = tokio::fs::remove_dir_all(&self.path).await {
            if error.kind() != std::io::ErrorKind::NotFound {
                self.cleaned.store(false, Ordering::Release);
                eprintln!("harness run artifact cleanup failed: {}", error.kind());
            }
        }
    }
}

impl Drop for RunDirectory {
    fn drop(&mut self) {
        if self.cleaned.swap(true, Ordering::AcqRel) {
            return;
        }
        let path = self.path.clone();
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                if let Err(error) = tokio::fs::remove_dir_all(path).await {
                    if error.kind() != std::io::ErrorKind::NotFound {
                        eprintln!("harness run artifact cleanup failed: {}", error.kind());
                    }
                }
            });
        }
    }
}
