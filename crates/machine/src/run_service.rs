//! Bounded Podman run lifecycle for installed harness programs.
//!
//! The host supplies a manifest-checked program binding, fixed session targets,
//! and the actor. The machine runs that executable in a network-isolated
//! container, streams ordered output, checks actor identity on input, and asks
//! the host authority callback before stopping a run.
mod directory;
mod program;
pub use program::{ProgramBinding, RunScope};

use directory::RunDirectory;

use crate::{
    egress::{
        ConnectProxy, EgressDialer, EgressError, EgressResolver, RunControl, SystemResolver,
        TcpDialer,
    },
    protocol::{ExitInfo, HarnessSandbox, Limits, ProviderLoginScope, SandboxSpec, Stream},
    sandbox::{
        BoxFuture, ExecRequest, OUTPUT_CHANNEL, OutputEvent, Sandbox, SandboxError,
        podman::PodmanBackend,
    },
};
use calternal_types::{Actor, MAX_FRAME_BYTES, SpaceId};
use std::{sync::Arc, time::Duration};
use tokio::sync::{Mutex, RwLock, mpsc, watch};
use uuid::Uuid;

const MAX_RUNS: usize = 32;
const MAX_TARGETS: usize = 32;
const TERMINATE_LIMIT: Duration = Duration::from_millis(1800);
const SOCKET_IN_CONTAINER: &str = "/run/calternal-egress/proxy.sock";

/// A start request already authorized by the host route.
pub struct StartRun {
    /// Installed plugin program binding and its outbound policy.
    pub program: ProgramBinding,
    /// Personal or shared space containing the targets.
    pub space: SpaceId,
    /// Fixed scope for every call made by this run.
    pub scope: RunScope,
    /// Canonical item UUIDs resolved by the host before starting the run.
    pub targets: Vec<Uuid>,
    /// Actor whose authority starts the run and is the only actor allowed to send.
    pub actor: Actor,
}

/// Authorizes Stop against the run's original target scope.
pub trait RunAuthorizer: Send + Sync + 'static {
    /// Return true only when caller may read every fixed target in this scope.
    /// Implementations perform host authorization, not a plugin-supplied check.
    fn may_read<'a>(
        &'a self,
        caller: &'a Actor,
        space: SpaceId,
        scope: RunScope,
        targets: &'a [Uuid],
    ) -> BoxFuture<'a, bool>;
}

/// A sequenced log event from a run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunEvent {
    /// Monotonic position within this run, starting at 1.
    pub sequence: u64,
    /// Output bytes or the terminal process status.
    pub event: RunEventKind,
}

/// Run log content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunEventKind {
    /// Bytes from one adapter pipe, in the order received by the machine.
    Data {
        /// Source pipe.
        stream: Stream,
        /// Raw bytes; decoding belongs to the host run route.
        bytes: Vec<u8>,
    },
    /// Terminal process status; always the last event.
    Exit(ExitInfo),
}

/// A live run's stream and exit watch.
pub struct RunHandle {
    /// UUIDv7 assigned by the machine for this process family.
    pub id: Uuid,
    /// Bounded ordered log stream. A stalled reader applies backpressure.
    pub events: mpsc::Receiver<RunEvent>,
    /// Final status becomes available even if the log stream is not consumed.
    pub exit: watch::Receiver<Option<ExitInfo>>,
}

impl RunHandle {
    /// Wait until the machine reports the process exit.
    pub async fn wait_exit(&mut self) -> ExitInfo {
        loop {
            if let Some(exit) = self.exit.borrow().clone() {
                return exit;
            }
            if self.exit.changed().await.is_err() {
                return ExitInfo {
                    error: Some("run status channel closed".into()),
                    ..ExitInfo::default()
                };
            }
        }
    }
}

/// Failure to start, send to, authorize, or stop a run.
#[derive(Debug, thiserror::Error)]
pub enum RunError {
    /// The host supplied an invalid ID or plugin-relative path.
    #[error("invalid program binding")]
    InvalidProgram,
    /// Image-backed bindings need an image loader that is not in this runner.
    #[error("this runner accepts executable program bindings only")]
    ImageProgramUnsupported,
    /// The resolved executable is missing, non-file, or outside its plugin folder.
    #[error("program executable is not a regular file inside its plugin folder")]
    ProgramPath,
    /// The run scope or target list is unsupported.
    #[error("run scope requires 1 to 32 session targets")]
    InvalidScope,
    /// The machine already has the maximum active run count.
    #[error("machine run capacity is full")]
    Capacity,
    /// No active run has this ID.
    #[error("run is not active")]
    NotActive,
    /// Input must come from the exact actor that started the run.
    #[error("only the starting actor may send input to this run")]
    InputActor,
    /// Input exceeds the shared worker frame bound.
    #[error("run input exceeds the frame limit")]
    InputTooLarge,
    /// The bounded input queue is full or its process has ended.
    #[error("run input is not currently available")]
    InputUnavailable,
    /// The caller cannot read every target in the run's fixed scope.
    #[error("caller may not read this run's target")]
    StopNotAllowed,
    /// Fast hard termination exceeded the two-second Stop budget.
    #[error("run process family did not stop within two seconds")]
    StopTimeout,
    /// The host did not provide a current authority implementation.
    #[error("run authority is unavailable")]
    AuthorityUnavailable,
    /// Host-side egress setup failed.
    #[error(transparent)]
    Egress(#[from] EgressError),
    /// Podman or sandbox setup failed.
    #[error(transparent)]
    Sandbox(#[from] SandboxError),
    /// Temporary-directory or executable metadata access failed.
    #[error("run setup I/O failed: {0}")]
    Io(#[from] std::io::Error),
    /// A machine task ended unexpectedly.
    #[error("run task failed")]
    Task,
}

/// Starts and tracks isolated harness runs on one Podman backend.
pub struct RunService {
    inner: Arc<RunServiceInner>,
}

struct RunServiceInner {
    backend: Arc<PodmanBackend>,
    image: String,
    authorizer: Arc<dyn RunAuthorizer>,
    resolver: Arc<dyn EgressResolver>,
    dialer: Arc<dyn EgressDialer>,
    runs: RwLock<std::collections::HashMap<Uuid, Arc<RunState>>>,
    permits: Arc<tokio::sync::Semaphore>,
}

struct RunState {
    actor: Actor,
    space: SpaceId,
    scope: RunScope,
    targets: Vec<Uuid>,
    input: mpsc::Sender<Vec<u8>>,
    resources: Arc<RunResources>,
    stopping: watch::Sender<bool>,
}

struct RunResources {
    sandbox: Arc<dyn Sandbox>,
    proxy: Mutex<Option<ConnectProxy>>,
    directory: RunDirectory,
    _permit: tokio::sync::OwnedSemaphorePermit,
    _control: RunControl,
}

impl RunResources {
    async fn cleanup(&self) {
        if let Some(proxy) = self.proxy.lock().await.take() {
            let _ = proxy.shutdown().await;
        }
        let _ = self.sandbox.stop().await;
        self.directory.cleanup().await;
    }
}

impl RunService {
    /// Use the system resolver and numeric-address TCP dialer for host egress.
    pub fn new(
        backend: Arc<PodmanBackend>,
        image: impl Into<String>,
        authorizer: Arc<dyn RunAuthorizer>,
    ) -> Self {
        Self::with_egress(
            backend,
            image,
            authorizer,
            Arc::new(SystemResolver::default()),
            Arc::new(TcpDialer),
        )
    }

    /// Construct a runner with injectable host DNS and dialing for isolated tests.
    pub fn with_egress(
        backend: Arc<PodmanBackend>,
        image: impl Into<String>,
        authorizer: Arc<dyn RunAuthorizer>,
        resolver: Arc<dyn EgressResolver>,
        dialer: Arc<dyn EgressDialer>,
    ) -> Self {
        Self {
            inner: Arc::new(RunServiceInner {
                backend,
                image: image.into(),
                authorizer,
                resolver,
                dialer,
                runs: RwLock::new(std::collections::HashMap::new()),
                permits: Arc::new(tokio::sync::Semaphore::new(MAX_RUNS)),
            }),
        }
    }

    /// Start the declared executable for fixed session targets and one actor.
    /// Target resolution and initial host authorization happen before this call.
    pub async fn start(&self, request: StartRun) -> Result<RunHandle, RunError> {
        if request.targets.is_empty() || request.targets.len() > MAX_TARGETS {
            return Err(RunError::InvalidScope);
        }
        if request.program.image {
            return Err(RunError::ImageProgramUnsupported);
        }
        let permit = Arc::clone(&self.inner.permits)
            .try_acquire_owned()
            .map_err(|_| RunError::Capacity)?;
        let plugin_root = tokio::fs::canonicalize(&request.program.plugin_root).await?;
        let candidate =
            tokio::fs::canonicalize(plugin_root.join(&request.program.relative_path)).await?;
        if !candidate.starts_with(&plugin_root) {
            return Err(RunError::ProgramPath);
        }
        let metadata = tokio::fs::metadata(&candidate).await?;
        if !metadata.is_file() {
            return Err(RunError::ProgramPath);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o111 == 0 {
                return Err(RunError::ProgramPath);
            }
        }

        let directory = tokio::task::spawn_blocking(tempfile::tempdir)
            .await
            .map_err(|_| RunError::Task)??;
        let directory = RunDirectory::from_tempdir(directory);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            tokio::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
                .await?;
        }
        let proxy = ConnectProxy::start(
            &directory.path().join("proxy.sock"),
            request.program.policy.clone(),
            Arc::clone(&self.inner.resolver),
            Arc::clone(&self.inner.dialer),
        )
        .await?;
        let mut control = RunControl::start(&directory.path().join("control.sock")).await?;
        let run_id = Uuid::now_v7();
        let spec = SandboxSpec {
            code_directory: None,
            space_id: Some(request.space),
            provider_login: false,
            shared_provider_sponsor: None,
            shared_tools: None,
            account_id: request.actor.account.as_uuid(),
            thread_id: run_id,
            image: Some(self.inner.image.clone()),
            worker: None,
            limits: Limits::default(),
        };
        let harness = HarnessSandbox {
            provider_id: request.program.plugin_id.clone(),
            login_scope: ProviderLoginScope::Account(request.actor.account),
            workspace_read_only: true,
            disposable_profile: true,
            legacy_profile: false,
            guest_image_digest: None,
        };
        let program = format!(
            "{}.{}",
            request.program.plugin_id, request.program.program_id
        );
        let sandbox = match self
            .inner
            .backend
            .start_harness_run(&spec, &harness, &plugin_root, directory.path(), &program)
            .await
        {
            Ok(sandbox) => sandbox,
            Err(error) => {
                let _ = proxy.shutdown().await;
                return Err(RunError::Sandbox(error));
            }
        };
        let relative = request
            .program
            .relative_path
            .to_str()
            .ok_or(RunError::InvalidProgram)?;
        let mut exec = match sandbox
            .exec(ExecRequest {
                argv: vec![format!("/opt/calternal-plugin/{relative}")],
                interactive: true,
                stdin: String::new(),
                env: vec![
                    ("CALTERNAL_EGRESS_SOCKET".into(), SOCKET_IN_CONTAINER.into()),
                    (
                        "CALTERNAL_RUN_CONTROL_SOCKET".into(),
                        "/run/calternal-egress/control.sock".into(),
                    ),
                ],
            })
            .await
        {
            Ok(exec) => exec,
            Err(error) => {
                let _ = sandbox.release().await;
                let _ = proxy.shutdown().await;
                return Err(RunError::Sandbox(error));
            }
        };
        let Some(input) = exec.input.take() else {
            let _ = sandbox.release().await;
            let _ = proxy.shutdown().await;
            return Err(RunError::InputUnavailable);
        };
        if let Err(error) = control.wait_ready().await {
            let _ = sandbox.release().await;
            let _ = proxy.shutdown().await;
            return Err(RunError::Egress(error));
        }
        let resources = Arc::new(RunResources {
            sandbox: Arc::clone(&sandbox),
            proxy: Mutex::new(Some(proxy)),
            directory,
            _permit: permit,
            _control: control,
        });
        let (log_tx, log_rx) = mpsc::channel(OUTPUT_CHANNEL);
        let (exit_tx, exit_rx) = watch::channel(None);
        let (stopping_tx, stopping_rx) = watch::channel(false);
        let state = Arc::new(RunState {
            actor: request.actor,
            space: request.space,
            scope: request.scope,
            targets: request.targets,
            input,
            resources: Arc::clone(&resources),
            stopping: stopping_tx,
        });
        {
            let mut runs = self.inner.runs.write().await;
            if runs.contains_key(&run_id) {
                drop(runs);
                resources.cleanup().await;
                return Err(RunError::Task);
            }
            runs.insert(run_id, Arc::clone(&state));
        }
        let weak = Arc::downgrade(&self.inner);
        tokio::spawn(monitor_run(
            run_id,
            exec.events,
            log_tx,
            exit_tx,
            resources,
            weak,
            stopping_rx,
        ));
        Ok(RunHandle {
            id: run_id,
            events: log_rx,
            exit: exit_rx,
        })
    }

    /// Send one bounded frame from the exact actor that started the run.
    pub async fn send_input(
        &self,
        run_id: Uuid,
        actor: &Actor,
        bytes: Vec<u8>,
    ) -> Result<(), RunError> {
        if bytes.len() > MAX_FRAME_BYTES + 1 {
            return Err(RunError::InputTooLarge);
        }
        let state = self
            .inner
            .runs
            .read()
            .await
            .get(&run_id)
            .cloned()
            .ok_or(RunError::NotActive)?;
        if &state.actor != actor {
            return Err(RunError::InputActor);
        }
        state
            .input
            .try_send(bytes)
            .map_err(|_| RunError::InputUnavailable)
    }

    /// Stop after the host authority callback confirms the caller can read all
    /// original targets. The hard process-family kill is bounded to 1.8 seconds.
    pub async fn stop(&self, run_id: Uuid, caller: &Actor) -> Result<(), RunError> {
        let state = self
            .inner
            .runs
            .read()
            .await
            .get(&run_id)
            .cloned()
            .ok_or(RunError::NotActive)?;
        if !self
            .inner
            .authorizer
            .may_read(caller, state.space, state.scope, &state.targets)
            .await
        {
            return Err(RunError::StopNotAllowed);
        }
        Self::terminate_state(&state).await
    }

    /// Terminate a known run by UUID after trusted host authorization or target
    /// retirement. No target lookup is needed. Bounds the process kill to 1.8 seconds.
    pub async fn terminate(&self, run_id: Uuid) -> Result<(), RunError> {
        let state = self
            .inner
            .runs
            .read()
            .await
            .get(&run_id)
            .cloned()
            .ok_or(RunError::NotActive)?;
        Self::terminate_state(&state).await
    }

    async fn terminate_state(state: &RunState) -> Result<(), RunError> {
        state.stopping.send_replace(true);
        match tokio::time::timeout(TERMINATE_LIMIT, state.resources.sandbox.terminate()).await {
            Ok(result) => result.map_err(RunError::Sandbox),
            Err(_) => Err(RunError::StopTimeout),
        }
    }
}

async fn monitor_run(
    run_id: Uuid,
    mut events: mpsc::Receiver<OutputEvent>,
    log: mpsc::Sender<RunEvent>,
    exit: watch::Sender<Option<ExitInfo>>,
    resources: Arc<RunResources>,
    service: std::sync::Weak<RunServiceInner>,
    mut stopping: watch::Receiver<bool>,
) {
    let mut sequence = 0_u64;
    let mut log_open = true;
    let info = loop {
        match events.recv().await {
            Some(OutputEvent::Exit(info)) => break info,
            Some(OutputEvent::Data(stream, bytes)) if log_open => {
                let event = RunEvent {
                    sequence: sequence + 1,
                    event: RunEventKind::Data { stream, bytes },
                };
                log_open = send_run_event(&log, &mut stopping, event).await;
                if log_open {
                    sequence += 1;
                }
            }
            Some(OutputEvent::Data(..)) => {}
            None => {
                break ExitInfo {
                    error: Some("sandbox output ended without an exit event".into()),
                    ..ExitInfo::default()
                };
            }
        }
    };
    let _ = resources.sandbox.terminate().await;
    exit.send_replace(Some(info.clone()));
    if let Some(service) = service.upgrade() {
        service.runs.write().await.remove(&run_id);
    }
    let terminal = RunEvent {
        sequence: sequence + 1,
        event: RunEventKind::Exit(info),
    };
    if *stopping.borrow() {
        let _ = log.try_send(terminal);
    } else if log_open {
        let _ = send_run_event(&log, &mut stopping, terminal).await;
    }
    resources.cleanup().await;
}

async fn send_run_event(
    log: &mpsc::Sender<RunEvent>,
    stopping: &mut watch::Receiver<bool>,
    event: RunEvent,
) -> bool {
    if *stopping.borrow() {
        return false;
    }
    tokio::select! {
        result = log.send(event) => result.is_ok(),
        changed = stopping.changed() => {
            let _ = changed;
            false
        }
    }
}
