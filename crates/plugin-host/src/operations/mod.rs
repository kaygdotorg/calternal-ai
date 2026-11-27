//! Drives one installed plugin's coded operations through a private child pipe.
//!
//! The registry supplies one endpoint per plugin and space. The endpoint
//! serializes deliveries, and the call route owns authorization, durable
//! receipts and change delivery. A trusted launcher provides isolation.
mod wire;

use std::future::Future;
use std::pin::Pin;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use calternal_types::{AccountId, Error, SpaceId};
use serde::Serialize;
use serde_json::Value;
use tokio::io::BufReader;
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc, oneshot};
use uuid::Uuid;

pub use wire::{Code, Method};

/// Supported ways to deliver work to a plugin worker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    /// An authorized, declared coded operation.
    Call,
    /// A post-commit change matching a registered kind or collection.
    Change,
}

/// Current acting person and role resolved by the host at delivery.
#[derive(Serialize)]
pub struct Person {
    /// Identifier of the verified person.
    pub id: AccountId,
    /// Current space role for this delivery.
    pub role: String,
}

/// Host-issued context carried across retries of one logical delivery.
#[derive(Serialize)]
pub struct Event {
    /// Selects the event registration used for this delivery.
    pub kind: EventKind,
    /// Opaque, stable key from the host's call or committed change.
    pub key: String,
    /// Item addressed by this delivery, if there is one.
    pub item: Option<Uuid>,
    /// Authorized pre-change data, when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before: Option<Value>,
    /// Host request identifier, when available for this event.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request: Option<String>,
}

/// A host-admitted event and its caller-visible input.
pub struct Delivery {
    /// Declared operation, or subscribed kind or collection.
    pub selector: String,
    /// Form input for a call, or the authorized changed item.
    pub input: Value,
    /// Current actor checked by the call route.
    pub me: Person,
    /// Space containing the installed plugin.
    pub space: SpaceId,
    /// Host-owned event context for this delivery.
    pub event: Event,
}

/// A callback to the single host route, using the delivery's trusted context.
pub struct Call {
    /// The wire method, drawn from the slice's fixed method table.
    pub method: Method,
    /// Method-specific JSON for the route to validate before applying effects.
    pub input: Value,
    /// Stable per-effect key for a write, nested call or run start. The route
    /// scopes receipts and uses a nested call key as the callee event key.
    pub idempotency_key: Option<String>,
}

type RouteFuture = Pin<Box<dyn Future<Output = Result<Value, Error>> + Send>>;
type Route = Box<dyn FnMut(Call) -> RouteFuture + Send>;
type Declaration = Box<dyn Fn(EventKind, &str) -> bool + Send + Sync>;

struct Job {
    delivery: Delivery,
    route: Route,
    result: oneshot::Sender<Result<Value, Error>>,
    // Held while an invocation can send callbacks. It forbids a cycle from
    // waiting on an occupied worker, including through a different plugin.
    _permit: OwnedSemaphorePermit,
}

/// On-demand operation worker for one registered plugin in one space.
///
/// Construct inside a Tokio runtime. A disabled installation has no endpoint.
/// The internal task owns the process and its pipes, with no locks across I/O.
pub struct Operations {
    space: SpaceId,
    declared: Declaration,
    jobs: mpsc::Sender<Job>,
    permit: Arc<Semaphore>,
}

impl Operations {
    /// Register an endpoint without starting the child. The registry must
    /// allocate only one endpoint per installation. The launcher must return
    /// promptly and configure an isolated worker from a verified installation.
    /// Its own process boundary must deny ambient filesystem, network and
    /// credentials and terminate descendants when the supervisor exits.
    /// The supervisor clears inherited environment and owns all three pipes.
    /// Cost is one idle task and one admission slot per installed coded plugin.
    pub fn new(
        space: SpaceId,
        declared: impl Fn(EventKind, &str) -> bool + Send + Sync + 'static,
        launch: impl Fn() -> Command + Send + Sync + 'static,
        idle: Duration,
        deadline: Duration,
    ) -> Self {
        let (jobs, pending) = mpsc::channel(1);
        tokio::spawn(serve(pending, launch, idle, deadline));
        Self {
            space,
            declared: Box::new(declared),
            jobs,
            permit: Arc::new(Semaphore::new(1)),
        }
    }

    /// Deliver an event and forward its callbacks through the supplied route.
    /// The route captures the verified actor, plugin, run context and call
    /// ancestry, and rechecks authority before each effect. A busy endpoint
    /// returns `limit`; a trusted scheduler can retry outside an active chain.
    /// Dropping this future closes the invocation and terminates its worker.
    /// Cost is one queue handoff and one pipe exchange per callback.
    pub async fn invoke<F, Fut>(&self, delivery: Delivery, mut route: F) -> Result<Value, Error>
    where
        F: FnMut(Call) -> Fut + Send + 'static,
        Fut: Future<Output = Result<Value, Error>> + Send + 'static,
    {
        if delivery.space != self.space || delivery.event.key.is_empty() {
            return Err(Code::Invalid.error(
                "The plugin delivery context is invalid.",
                "Retry from the host.",
            ));
        }
        if !(self.declared)(delivery.event.kind, &delivery.selector) {
            return Err(Code::NotAllowed.error(
                "The plugin did not declare this operation or event.",
                "Choose a declared operation.",
            ));
        }
        let permit = self.permit.clone().try_acquire_owned().map_err(|_| {
            Code::Limit.error(
                "The plugin is already handling an event.",
                "Retry the operation.",
            )
        })?;
        let (result, reply) = oneshot::channel();
        self.jobs
            .try_send(Job {
                delivery,
                route: Box::new(move |call| Box::pin(route(call))),
                result,
                _permit: permit,
            })
            .map_err(|_| wire::disconnected())?;
        reply.await.map_err(|_| wire::disconnected())?
    }
}

async fn serve(
    mut jobs: mpsc::Receiver<Job>,
    launch: impl Fn() -> Command + Send + Sync + 'static,
    idle: Duration,
    deadline: Duration,
) {
    let launch = Arc::new(launch);
    let mut worker: Option<Worker> = None;
    loop {
        let pending = if let Some(running) = worker.as_mut() {
            tokio::select! {
                biased;
                _ = running.child.wait() => {
                    worker = None;
                    continue;
                }
                pending = jobs.recv() => pending,
                _ = tokio::time::sleep(idle) => {
                    stop(&mut worker).await;
                    continue;
                }
            }
        } else {
            jobs.recv().await
        };
        let Some(Job {
            delivery,
            mut route,
            mut result,
            _permit,
        }) = pending
        else {
            stop(&mut worker).await;
            return;
        };
        if result.is_closed() {
            continue;
        }
        let invocation = Uuid::new_v4();
        let first = match wire::invoke(invocation, &delivery) {
            Ok(first) => first,
            Err(error) => {
                drop(_permit);
                let _ = result.send(Err(error));
                continue;
            }
        };
        if worker.is_none() {
            let launcher = Arc::clone(&launch);
            match tokio::task::spawn_blocking(move || Worker::spawn(launcher())).await {
                Ok(Ok(running)) => worker = Some(running),
                _ => {
                    let error = wire::disconnected();
                    drop(_permit);
                    let _ = result.send(Err(error));
                    continue;
                }
            }
        }
        let running = worker.as_mut().expect("worker started");
        let work = exchange(
            &mut running.input,
            &mut running.output,
            invocation,
            &first,
            &delivery.event,
            &mut route,
        );
        let outcome = tokio::select! {
            biased;
            _ = result.closed() => None,
            _ = running.child.wait() => Some(Err(wire::disconnected())),
            outcome = tokio::time::timeout(deadline, work) => Some(outcome.unwrap_or_else(|_| {
                Err(Code::Timeout.error("The plugin worker did not finish.", "Retry the operation."))
            })),
        };
        if !matches!(outcome, Some(Ok(_))) {
            stop(&mut worker).await;
        }
        drop(_permit);
        if let Some(outcome) = outcome {
            let _ = result.send(outcome.and_then(|answer| answer));
        }
    }
}

async fn stop(worker: &mut Option<Worker>) {
    if let Some(mut running) = worker.take() {
        let _ = running.child.kill().await;
    }
}

struct Worker {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
}

impl Worker {
    fn spawn(mut command: Command) -> Result<Self, Error> {
        let mut child = command
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|_| wire::disconnected())?;
        let input = child.stdin.take().expect("child stdin was piped");
        let output = BufReader::new(child.stdout.take().expect("child stdout was piped"));
        Ok(Self {
            child,
            input,
            output,
        })
    }
}

async fn exchange(
    input: &mut ChildStdin,
    output: &mut BufReader<ChildStdout>,
    invocation: Uuid,
    first: &[u8],
    event: &Event,
    route: &mut Route,
) -> Result<Result<Value, Error>, Error> {
    wire::send(input, first).await?;
    let mut id = 1u32;
    let mut writes = 0u32;
    let mut calls = 0u32;
    let mut runs = 0u32;
    loop {
        let (received, method, args) = match wire::receive(output, invocation, id).await? {
            wire::Message::Call { id, method, input } => (id, method, input),
            wire::Message::Complete(result) => {
                return Ok(result.map(|value| match event.kind {
                    EventKind::Call => value,
                    EventKind::Change => Value::Null,
                }));
            }
        };
        let ordinal = match method {
            Method::Write => Some(("write", &mut writes)),
            Method::Call => Some(("call", &mut calls)),
            Method::Run => Some(("run", &mut runs)),
            _ => None,
        };
        let idempotency_key = ordinal.map(|(effect, count)| {
            let key = format!("{}:{effect}:{count}", event.key);
            *count += 1;
            key
        });
        let result = if event.kind == EventKind::Change
            && matches!(method, Method::Run | Method::RunSend | Method::RunStop)
        {
            Err(Code::NotAllowed.error(
                "A change event cannot control a run.",
                "Start or control the run from an explicit operation.",
            ))
        } else {
            route(Call {
                method,
                input: args,
                idempotency_key,
            })
            .await
        };
        let reply = match wire::reply(invocation, received, &result) {
            Ok(frame) => frame,
            Err(error) => wire::reply(invocation, received, &Err(error))?,
        };
        wire::send(input, &reply).await?;
        id = id.checked_add(1).ok_or_else(wire::oversized)?;
    }
}
