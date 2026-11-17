//! Synchronous test teardown for containers under one disposable sandbox root.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Removes Podman artifacts for one disposable test root, including on panic.
pub struct PodmanGuard(PathBuf);

impl PodmanGuard {
    /// Register ownership before starting any test container under this root.
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self(root.as_ref().to_owned())
    }
}

impl Drop for PodmanGuard {
    fn drop(&mut self) {
        let Ok(root) = self.0.canonicalize() else {
            return;
        };
        let filter = format!("label=org.calternal.sandbox-root={}", root.display());
        let Ok(output) = Command::new("podman")
            .args(["ps", "-aq", "--filter", &filter])
            .output()
        else {
            return;
        };
        let ids = String::from_utf8_lossy(&output.stdout);
        if !output.status.success() {
            eprintln!(
                "Podman could not inspect test sandbox root {}",
                root.display()
            );
            return;
        }
        if ids.trim().is_empty() {
            return;
        }
        let removed = Command::new("podman")
            .args(["rm", "-f", "-t", "0", "--ignore"])
            .args(ids.split_whitespace())
            .stdout(Stdio::null())
            .status()
            .is_ok_and(|status| status.success());
        if !removed {
            eprintln!(
                "Podman could not remove test sandbox root {}",
                root.display()
            );
        }
    }
}
