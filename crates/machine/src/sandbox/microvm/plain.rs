//! Package processes in the offline, single-space guest.

use crate::protocol::{SandboxSpec, Stream};
use crate::sandbox::{
    BoxFuture, Exec, ExecRequest, OutputEvent, Sandbox, SandboxError, SandboxResult,
    spawn_streaming, worker,
};
use std::{io, path::PathBuf, sync::Arc, time::Duration};
use tokio::{fs, sync::Mutex};
use uuid::Uuid;

const LAUNCHER: &str = "/opt/calternal-worker-launch";
const PACKAGE_ROOT: &str = "/run/agent/plain";
const CGROUP_ROOT: &str = "/sys/fs/cgroup/agent/ctr";
const MEMORY_BYTES: u64 = (crate::protocol::SMALL_VM_MEMORY_MIB as u64 * 3 / 4) << 20;

/// An isolated worker environment with a one-use package exec.
pub(super) struct PlainWorker {
    id: Uuid,
    source: PathBuf,
    cgroup: PathBuf,
    launch: Mutex<Launch>,
}

enum Launch {
    Fresh,
    Standby(Exec),
    Closed,
}

/// Adopt a parked launcher or prepare a fresh cgroup, then stage verified
/// code. The caller holds the small guest's admission lock and ready marker.
pub(super) async fn start(
    spec: &SandboxSpec,
    parked: &mut Option<Arc<PlainWorker>>,
    prestart: bool,
) -> SandboxResult<Arc<dyn Sandbox>> {
    if spec.image.as_deref() != Some(worker::SPACE_VM_IMAGE)
        || spec.provider_login
        || spec.shared_tools.is_some()
        || spec.shared_provider_sponsor.is_some()
    {
        return Err(SandboxError(
            "small guest requires the offline worker profile".into(),
        ));
    }
    let package = spec
        .worker
        .as_ref()
        .filter(|package| spec.space_id == Some(package.space_id))
        .ok_or_else(|| SandboxError("small guest has no matching worker".into()))?;
    if let Some(instance) = parked.take() {
        if prestart && worker::prepare(&instance.source, package).await.is_ok() {
            return Ok(instance);
        }
        instance.stop().await?;
    }
    let instance = Arc::new(PlainWorker::new(spec.thread_id));
    fs::create_dir(&instance.cgroup)
        .await
        .map_err(SandboxError::new)?;
    let result = async {
        instance.limits().await?;
        worker::prepare(&instance.source, package).await
    }
    .await;
    if let Err(error) = result {
        instance.stop().await?;
        return Err(error);
    }
    Ok(instance)
}

/// Park trusted code in a new empty staging directory and inspect its tasks
/// from the guest agent. A failed assertion yields a fresh snapshot only after
/// cleanup proves the candidate process is gone.
pub(super) async fn park() -> SandboxResult<Option<Arc<PlainWorker>>> {
    let instance = Arc::new(PlainWorker::new(Uuid::new_v4()));
    fs::create_dir(&instance.cgroup)
        .await
        .map_err(SandboxError::new)?;
    let ready = async {
        instance.limits().await?;
        fs::create_dir_all(&instance.source)
            .await
            .map_err(SandboxError::new)?;
        let mut exec = instance.spawn(String::new(), true)?;
        expect_marker(&mut exec, b"STANDBY\n").await?;
        instance.check_threads().await?;
        Ok::<_, SandboxError>(exec)
    }
    .await;
    match ready {
        Ok(exec) => {
            *instance.launch.lock().await = Launch::Standby(exec);
            Ok(Some(instance))
        }
        Err(error) => {
            tracing::warn!(%error, "small guest standby preparation refused");
            eprintln!("small guest standby preparation refused: {error}");
            instance.stop().await?;
            Ok(None)
        }
    }
}

impl PlainWorker {
    fn new(id: Uuid) -> Self {
        Self {
            id,
            source: PathBuf::from(PACKAGE_ROOT).join(id.to_string()),
            cgroup: PathBuf::from(CGROUP_ROOT).join(id.to_string()),
            launch: Mutex::new(Launch::Fresh),
        }
    }

    fn spawn(&self, stdin: String, standby: bool) -> SandboxResult<Exec> {
        let mut command = tokio::process::Command::new(LAUNCHER);
        command.arg(self.id.to_string()).env_clear();
        if standby {
            command.arg("--standby");
        }
        spawn_streaming(command, stdin, true, true).map(|(exec, _)| exec)
    }

    async fn limits(&self) -> SandboxResult<()> {
        for (file, limit) in [
            ("memory.max", MEMORY_BYTES.to_string()),
            ("memory.swap.max", "0".into()),
            ("pids.max", worker::LIMITS.pids.unwrap().to_string()),
            (
                "cpu.max",
                format!("{} 100000", worker::LIMITS.cpu_millis.unwrap() * 100),
            ),
        ] {
            fs::write(self.cgroup.join(file), limit)
                .await
                .map_err(SandboxError::new)?;
        }
        Ok(())
    }

    async fn check_threads(&self) -> SandboxResult<()> {
        let processes = fs::read_to_string(self.cgroup.join("cgroup.procs"))
            .await
            .map_err(SandboxError::new)?;
        let mut names = Vec::with_capacity(8);
        for pid in processes.split_whitespace() {
            let pid: u32 = pid.parse().map_err(SandboxError::new)?;
            let mut tasks = fs::read_dir(format!("/proc/{pid}/task"))
                .await
                .map_err(SandboxError::new)?;
            while let Some(task) = tasks.next_entry().await.map_err(SandboxError::new)? {
                let name = fs::read_to_string(task.path().join("comm"))
                    .await
                    .map_err(SandboxError::new)?;
                names.push(name);
            }
        }
        names.sort_unstable();
        // Node may omit its fixed signal-watcher thread when the inspector
        // is disabled. No additional process or thread name is permitted.
        if let Ok(index) = names.binary_search_by(|name| name.as_str().cmp("SignalInspector\n")) {
            names.remove(index);
        }
        let fixed = [
            "DelayedTaskSche\n",
            "MainThread\n",
            "V8Worker\n",
            "V8Worker\n",
            "V8Worker\n",
            "V8Worker\n",
            "calternal-worke\n",
        ];
        if names != fixed {
            return Err(SandboxError(format!(
                "unexpected standby task set: {names:?}"
            )));
        }
        Ok(())
    }

    async fn rearm_fresh(&self) -> SandboxResult<()> {
        self.kill_group().await?;
        fs::create_dir(&self.cgroup)
            .await
            .map_err(SandboxError::new)?;
        self.limits().await
    }

    async fn kill_group(&self) -> SandboxResult<()> {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(1);
        loop {
            match fs::write(self.cgroup.join("cgroup.kill"), "1").await {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
                Err(error) => return Err(SandboxError::new(error)),
            }
            match fs::remove_dir(&self.cgroup).await {
                Ok(()) => return Ok(()),
                Err(error)
                    if error.kind() == io::ErrorKind::ResourceBusy
                        && tokio::time::Instant::now() < deadline =>
                {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                Err(error) => return Err(SandboxError::new(error)),
            }
        }
    }
}

async fn expect_marker(exec: &mut Exec, marker: &[u8]) -> SandboxResult<()> {
    tokio::time::timeout(Duration::from_secs(2), async {
        let mut received = 0;
        loop {
            match exec.events.recv().await {
                Some(OutputEvent::Data(Stream::Stdout, bytes))
                    if marker[received..].starts_with(&bytes) =>
                {
                    received += bytes.len();
                    if received == marker.len() {
                        return Ok(());
                    }
                }
                Some(OutputEvent::Data(Stream::Stderr, bytes)) => {
                    eprintln!("small guest standby: {}", String::from_utf8_lossy(&bytes));
                }
                _ => return Err(SandboxError("standby control marker unavailable".into())),
            }
        }
    })
    .await
    .map_err(SandboxError::new)?
}

async fn send(exec: &Exec, bytes: Vec<u8>) -> SandboxResult<()> {
    exec.input
        .as_ref()
        .ok_or_else(|| SandboxError("standby input unavailable".into()))?
        .send(bytes)
        .await
        .map_err(SandboxError::new)
}

impl Sandbox for PlainWorker {
    fn exec(&self, request: ExecRequest) -> BoxFuture<'_, SandboxResult<Exec>> {
        Box::pin(async move {
            if !request.argv.iter().map(String::as_str).eq(worker::ARGV)
                || !request.env.is_empty()
                || !request.interactive
            {
                return Err(SandboxError(
                    "small guest only starts the verified package with pipes".into(),
                ));
            }
            let mut launch = self.launch.lock().await;
            match std::mem::replace(&mut *launch, Launch::Closed) {
                Launch::Fresh => {
                    eprintln!("small guest worker mode: fresh");
                    self.spawn(request.stdin, false)
                }
                Launch::Closed => Err(SandboxError("package exec already used or closed".into())),
                Launch::Standby(mut exec) => {
                    if let Err(error) = super::guest::small_ready().await {
                        *launch = Launch::Standby(exec);
                        return Err(error);
                    }
                    let refreshed = async {
                        self.check_threads().await?;
                        send(&exec, b"refresh\n".to_vec()).await?;
                        expect_marker(&mut exec, b"REFRESHED\n").await
                    }
                    .await;
                    if let Err(error) = refreshed {
                        tracing::warn!(%error, "small guest worker refresh refused");
                        eprintln!("small guest worker refresh refused: {error}");
                        drop(exec);
                        self.rearm_fresh().await?;
                        eprintln!("small guest worker mode: fallback");
                        return self.spawn(request.stdin, false);
                    }
                    // Only a refreshed launcher can receive the package load
                    // command. Once it is sent, errors must never replay code.
                    send(&exec, b"load\n".to_vec()).await?;
                    eprintln!("small guest worker mode: standby");
                    if !request.stdin.is_empty() {
                        send(&exec, request.stdin.into_bytes()).await?;
                    }
                    Ok(exec)
                }
            }
        })
    }

    fn freeze(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async {
            fs::write(self.cgroup.join("cgroup.freeze"), "1")
                .await
                .map_err(SandboxError::new)
        })
    }

    fn resume(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async {
            fs::write(self.cgroup.join("cgroup.freeze"), "0")
                .await
                .map_err(SandboxError::new)
        })
    }

    fn stop(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async {
            // Serialize teardown with exec and another release. A launcher
            // that has not joined by removal cannot open cgroup.procs.
            let mut launch = self.launch.lock().await;
            *launch = Launch::Closed;
            self.kill_group().await?;
            match fs::remove_dir_all(&self.source).await {
                Err(error) if error.kind() != io::ErrorKind::NotFound => {
                    Err(SandboxError::new(error))
                }
                _ => Ok(()),
            }
        })
    }

    fn release(&self) -> BoxFuture<'_, SandboxResult<()>> {
        self.stop()
    }
}
