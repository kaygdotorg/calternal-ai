//! Operator-selected OS sandbox and sealed source snapshots.
use calternal_plugin_host::operations::{self, EventKind, Operations, Registrations};
use calternal_types::{Error, Result, SpaceId};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::process::Command;

const DEADLINE: Duration = Duration::from_secs(6);
const IDLE: Duration = Duration::from_secs(30);

// Read the limits of this exact scope before starting Bubblewrap. A user
// manager without delegated controllers may accept properties it cannot apply.
const CHECK_SCOPE: &str = r#"
    IFS=: read -r hierarchy controllers path </proc/self/cgroup
    [ "$hierarchy" = 0 ] && [ -z "$controllers" ] || exit 125
    cd "/sys/fs/cgroup$path" || exit 125
    IFS= read -r memory <memory.max && [ "$memory" = "$1" ] || exit 125
    IFS= read -r swap <memory.swap.max && [ "$swap" = 0 ] || exit 125
    IFS= read -r cpu <cpu.max && [ "$cpu" = "$2 100000" ] || exit 125
    IFS= read -r pids <pids.max && [ "$pids" = "$3" ] || exit 125
    shift 3
    exec "$@"
"#;

#[derive(Clone)]
struct Limits {
    memory: u64,
    cpu: u64,
    tasks: u64,
    temporary: u64,
}

impl Limits {
    fn operator() -> Result<Self> {
        fn positive(key: &str, default: u64, maximum: u64) -> Result<u64> {
            match std::env::var(key) {
                Err(std::env::VarError::NotPresent) => Ok(default),
                Ok(raw) => raw
                    .parse::<u64>()
                    .ok()
                    .filter(|value| (1..=maximum).contains(value))
                    .ok_or_else(|| Error::invalid_input(format!("Invalid worker limit {key}."))),
                Err(_) => Err(Error::invalid_input(format!("Invalid worker limit {key}."))),
            }
        }

        // MemoryMax and tmpfs round to pages. MiB is aligned on the supported VMs.
        const MIB: u64 = 1024 * 1024;
        Ok(Self {
            memory: positive("CALTERNAL_WORKER_MEMORY_MIB", 384, 65_536)? * MIB,
            cpu: positive("CALTERNAL_WORKER_CPU_PERCENT", 100, 1_200)?,
            tasks: positive("CALTERNAL_WORKER_TASKS", 32, 1_024)?,
            temporary: positive("CALTERNAL_WORKER_TMP_MIB", 16, 4_096)? * MIB,
        })
    }
}

fn closure(node: &Path) -> Result<Vec<PathBuf>> {
    let root = node
        .ancestors()
        .find(|path| path.parent() == Some(Path::new("/nix/store")))
        .ok_or_else(|| Error::invalid_input("Node must be in the Nix store."))?;
    let output = process::Command::new("nix-store")
        .arg("--query")
        .arg("--requisites")
        .arg(root)
        .output()
        .map_err(crate::internal)?;
    let unavailable = || Error::unavailable("The Node runtime closure could not be resolved.");
    if !output.status.success() {
        return Err(unavailable());
    }
    let store = Path::new("/nix/store");
    let paths = std::str::from_utf8(&output.stdout)
        .map_err(|_| unavailable())?
        .lines()
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    if !paths.iter().any(|path| path == root)
        || paths
            .iter()
            .any(|path| path.parent() != Some(store) || !path.exists())
    {
        return Err(unavailable());
    }
    Ok(paths)
}

/// Opt-in launcher for a disposable plugin process. A new user, network and
/// PID namespace expose only a read-only Nix runtime, trusted SDK and bundle
/// source snapshot. The host never falls back to a direct Node launch.
#[derive(Clone)]
pub struct WorkerRuntime {
    sandbox: PathBuf,
    node: PathBuf,
    sdk: PathBuf,
    closure: Arc<[PathBuf]>,
    limits: Limits,
    runtime_dir: PathBuf,
}

impl WorkerRuntime {
    /// Accept absolute operator-managed Bubblewrap, Nix Node and SDK package
    /// paths. Resolves the Nix runtime closure once through the local store.
    /// Workers require a systemd user manager with cgroup v2 memory, CPU and
    /// task controllers. Operator variables set memory and tmpfs in MiB,
    /// CPU as a percentage of one core and the maximum number of tasks.
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
        let node = node.canonicalize().map_err(crate::internal)?;
        let closure = closure(&node)?.into();
        let runtime_dir = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .ok_or_else(|| {
                Error::unavailable("The worker needs a systemd user runtime directory.")
            })?;
        Ok(Self {
            sandbox,
            node,
            sdk,
            closure,
            limits: Limits::operator()?,
            runtime_dir,
        })
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
        // The pipe launcher clears inherited environment. Give systemd only
        // the user-manager address and clear it again inside the sandbox.
        let limits = &self.limits;
        let mut address = std::ffi::OsString::from("XDG_RUNTIME_DIR=");
        address.push(&self.runtime_dir);
        let mut command = Command::new("/usr/bin/env");
        command
            .arg(address)
            .args(["/usr/bin/systemd-run", "--user", "--scope", "--quiet"])
            .arg(format!("--property=MemoryMax={}", limits.memory))
            .arg("--property=MemorySwapMax=0")
            .arg(format!("--property=CPUQuota={}%", limits.cpu))
            .arg("--property=CPUQuotaPeriodSec=100ms")
            .arg(format!("--property=TasksMax={}", limits.tasks))
            .args(["--", "/bin/sh", "-c", CHECK_SCOPE, "worker-scope"])
            .arg(limits.memory.to_string())
            .arg((limits.cpu * 1_000).to_string())
            .arg(limits.tasks.to_string())
            .arg(&self.sandbox)
            .args([
                "--unshare-all",
                "--unshare-user",
                "--disable-userns",
                "--die-with-parent",
                "--new-session",
                "--clearenv",
                "--dir",
                "/nix/store",
                "--proc",
                "/proc",
                "--size",
            ])
            .arg(limits.temporary.to_string())
            .args(["--tmpfs", "/tmp", "--dir", "/app"]);
        for path in self.closure.iter() {
            command.arg("--ro-bind").arg(path).arg(path);
        }
        command
            .arg("--ro-bind")
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

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    fn executable(name: &str) -> PathBuf {
        std::env::split_paths(&std::env::var_os("PATH").unwrap())
            .map(|directory| directory.join(name))
            .find(|candidate| candidate.is_file())
            .unwrap()
            .canonicalize()
            .unwrap()
    }

    #[tokio::test]
    async fn worker_mounts_only_runtime_paths_in_a_limited_scope() {
        let runtime = tokio::task::spawn_blocking(|| {
            let sdk = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../packages/plugin-sdk")
                .canonicalize()
                .unwrap();
            WorkerRuntime::new(executable("bwrap"), executable("node"), sdk).unwrap()
        })
        .await
        .unwrap();
        let source = WorkerRuntime::snapshot(b"export const unused = true;").unwrap();
        let command = runtime.command(source.path());
        let args: Vec<_> = command
            .as_std()
            .get_args()
            .map(|arg| arg.to_string_lossy())
            .collect();
        assert!(
            !args
                .windows(3)
                .any(|args| args == ["--ro-bind", "/nix/store", "/nix/store"])
        );
        assert!(
            args.iter()
                .any(|arg| arg.starts_with("--property=MemoryMax="))
        );
        assert!(
            args.iter()
                .any(|arg| arg.starts_with("--property=CPUQuota="))
        );
        assert!(
            args.iter()
                .any(|arg| arg.starts_with("--property=TasksMax="))
        );
        assert!(
            args.windows(3)
                .any(|args| args[0] == "--size" && args[2] == "--tmpfs")
        );
        let registrations = runtime.probe(source.path()).await.unwrap();
        assert!(registrations.calls.is_empty());
    }
}
