//! Operator-selected OS sandbox and sealed source snapshots.
use calternal_plugin_host::operations::{self, EventKind, Operations, Registrations};
use calternal_types::{Error, Result, SpaceId};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::process::Command;

const DEADLINE: Duration = Duration::from_secs(6);
const IDLE: Duration = Duration::from_secs(30);

/// Opt-in launcher for a disposable plugin process. A new user, network and
/// PID namespace expose only a read-only Nix runtime, trusted SDK and bundle
/// source snapshot. The host never falls back to a direct Node launch.
#[derive(Clone)]
pub struct WorkerRuntime {
    sandbox: PathBuf,
    node: PathBuf,
    sdk: PathBuf,
}

impl WorkerRuntime {
    /// Accept absolute operator-managed Bubblewrap, Nix Node and SDK package
    /// paths. Validates the paths without starting a process.
    pub fn new(sandbox: PathBuf, node: PathBuf, sdk: PathBuf) -> Result<Self> {
        if !sandbox.is_absolute()
            || !sandbox.is_file()
            || !node.starts_with("/nix/store")
            || !node.is_file()
            || !sdk.is_absolute()
            || !sdk.join("package.json").is_file()
            || !sdk.join("dist/operations-worker.js").is_file()
        {
            return Err(Error::invalid_input(
                "Configure absolute Bubblewrap, Nix Node and built plugin SDK paths.",
            ));
        }
        Ok(Self { sandbox, node, sdk })
    }

    pub(crate) async fn probe(&self, source: &Path) -> Result<Registrations> {
        let mut command = self.command(source);
        command.arg("--probe");
        operations::probe(command, DEADLINE).await
    }

    pub(crate) fn snapshot(bytes: &[u8]) -> Result<TempDir> {
        let snapshot = tempfile::tempdir().map_err(crate::internal)?;
        std::fs::write(snapshot.path().join("operations.ts"), bytes).map_err(crate::internal)?;
        Ok(snapshot)
    }

    pub(crate) fn endpoint(
        &self,
        space: SpaceId,
        bytes: &[u8],
        registration: &Registrations,
    ) -> Result<Arc<Operations>> {
        let source = Self::snapshot(bytes)?;
        let launcher = self.clone();
        let registration = registration.clone();
        Ok(Arc::new(Operations::new(
            space,
            move |kind, selector| match kind {
                EventKind::Call => registration.calls.iter().any(|value| value == selector),
                EventKind::Change => registration.changes.iter().any(|value| value == selector),
            },
            move || launcher.command(source.path()),
            IDLE,
            DEADLINE,
        )))
    }

    fn command(&self, source: &Path) -> Command {
        let mut command = Command::new(&self.sandbox);
        command
            .args([
                "--unshare-all",
                "--unshare-user",
                "--disable-userns",
                "--die-with-parent",
                "--new-session",
                "--ro-bind",
                "/nix/store",
                "/nix/store",
                "--proc",
                "/proc",
                "--tmpfs",
                "/tmp",
                "--dir",
                "/app",
                "--ro-bind",
            ])
            .arg(source.join("operations.ts"))
            .arg("/app/operations.ts")
            .arg("--ro-bind")
            .arg(self.sdk.join("package.json"))
            .arg("/app/node_modules/@calternal/plugin-sdk/package.json")
            .arg("--ro-bind")
            .arg(self.sdk.join("dist"))
            .arg("/app/node_modules/@calternal/plugin-sdk/dist")
            .args(["--chdir", "/app", "--"])
            .arg(&self.node)
            .args([
                "--max-old-space-size=128",
                "--permission",
                "--allow-fs-read=/app",
                "--conditions=calternal-operations",
                "/app/node_modules/@calternal/plugin-sdk/dist/operations-worker.js",
                "/app/operations.ts",
            ]);
        command
    }
}
