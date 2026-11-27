use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

/// Private owner for per-run socket files with nonblocking drop cleanup.
pub(super) struct RunDirectory {
    path: PathBuf,
    cleaned: AtomicBool,
}

impl RunDirectory {
    /// Retain a uniquely created temporary directory without its blocking destructor.
    pub(super) fn from_tempdir(directory: tempfile::TempDir) -> Self {
        Self {
            path: directory.keep(),
            cleaned: AtomicBool::new(false),
        }
    }

    /// Return the run's private filesystem path.
    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    /// Remove the directory asynchronously after the proxy and container stop.
    pub(super) async fn cleanup(&self) {
        if self.cleaned.swap(true, Ordering::AcqRel) {
            return;
        }
        if let Err(error) = tokio::fs::remove_dir_all(&self.path).await {
            if error.kind() != std::io::ErrorKind::NotFound {
                self.cleaned.store(false, Ordering::Release);
                tracing::warn!(error_kind = ?error.kind(), "cannot remove harness run directory");
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
                        tracing::warn!(error_kind = ?error.kind(), "cannot remove harness run directory");
                    }
                }
            });
        }
    }
}
