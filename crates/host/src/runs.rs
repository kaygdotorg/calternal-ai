//! Lifecycle and authority for session-scoped harness programs.
mod log;
mod output;

use crate::{Call, Host, decode, internal, space::Receipt};
use calternal_machine::{
    run_service::{
        ProgramBinding, RunAuthorizer, RunError, RunEvent, RunScope, RunService, StartRun,
    },
    sandbox::{BoxFuture, podman::PodmanBackend},
};
use calternal_types::{Actor, Error, ErrorCode, MAX_FRAME_BYTES, Result, SpaceId, Via};
use log::Log;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Arc, Weak,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::sync::{Mutex as AsyncMutex, broadcast};
use uuid::Uuid;

const MAX_RUNS_PER_SPACE: usize = 32;
const MAX_SEND_RECEIPTS: usize = 128;

pub(crate) use log::Batch as LogBatch;

pub(crate) struct Runs {
    pub(crate) start: Arc<AsyncMutex<()>>,
    entries: VecDeque<Arc<Run>>,
    pub(crate) logs: broadcast::Sender<()>,
}

impl Runs {
    pub(crate) fn is_active(&self) -> bool {
        self.entries
            .iter()
            .any(|run| !run.stopped.load(Ordering::Acquire))
            || Arc::strong_count(&self.start) > 1
    }
}

impl Default for Runs {
    fn default() -> Self {
        Self {
            start: Arc::default(),
            entries: VecDeque::new(),
            logs: broadcast::channel(32).0,
        }
    }
}

struct Run {
    grant: RunScopeGrant,
    actor: Actor,
    log: Log,
    notify: broadcast::Sender<()>,
    stopped: AtomicBool,
    sends: AsyncMutex<HashMap<String, blake3::Hash>>,
}

impl Run {
    fn publish(&self, event: RunEvent) {
        self.log.publish(event);
        let _ = self.notify.send(());
    }

    fn finish(&self, event: RunEvent) {
        self.stopped.store(true, Ordering::Release);
        self.publish(event);
    }
}

#[derive(Clone, Copy)]
pub(crate) struct RunScopeGrant {
    pub(crate) id: Uuid,
    space: SpaceId,
    pub(crate) item: Uuid,
}

impl RunScopeGrant {
    pub(crate) fn check(&self, actor: &Actor, space: SpaceId, call: &Call) -> Result<()> {
        let is_item = |value: &Value| {
            value
                .get("item")
                .and_then(Value::as_str)
                .and_then(|raw| Uuid::parse_str(raw).ok())
                == Some(self.item)
        };
        let allowed = match call.call.as_str() {
            "s.me" | "s.space" => true,
            "s.read" | "s.state" => is_item(&call.input),
            "s.write" => call
                .input
                .get("changes")
                .and_then(Value::as_array)
                .is_some_and(|changes| {
                    changes
                        .iter()
                        .all(|change| is_item(change) && change["op"] == "append record")
                        && changes.len() == 1
                }),
            name if !name.starts_with("s.") && name != "index.rebuild" => is_item(&call.input),
            _ => false,
        };
        if space != self.space
            || !allowed
            || !matches!(actor.via, Some(Via::Run { id, .. }) if id == self.id)
        {
            return Err(Error::forbidden("The call is outside this run's session."));
        }
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StartInput {
    program: String,
    scope: String,
    targets: Vec<Uuid>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SendInput {
    id: Uuid,
    input: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StopInput {
    id: Uuid,
}

struct HostRunAuthorizer(Weak<Host>);

impl RunAuthorizer for HostRunAuthorizer {
    fn may_read<'a>(
        &'a self,
        caller: &'a Actor,
        space: SpaceId,
        _scope: RunScope,
        targets: &'a [Uuid],
    ) -> BoxFuture<'a, bool> {
        Box::pin(async move {
            let Some(host) = self.0.upgrade() else {
                return false;
            };
            let [item] = targets else { return false };
            host.check_run_access(space, caller.clone(), *item)
                .await
                .is_ok()
        })
    }
}

impl Host {
    /// Attach the operator-configured Podman runner. Takes no process or storage action.
    pub fn with_podman_runs(
        self: Arc<Self>,
        backend: Arc<PodmanBackend>,
        image: impl Into<String>,
    ) -> Arc<Self> {
        let service = RunService::new(
            backend,
            image,
            Arc::new(HostRunAuthorizer(Arc::downgrade(&self))),
        );
        *self.runs.lock().expect("run configuration lock") = Some(Arc::new(service));
        self
    }

    pub(crate) async fn run_start(
        self: Arc<Self>,
        actor: Actor,
        space: SpaceId,
        input: Value,
        key: Option<String>,
        gesture: Option<String>,
        session: Option<Uuid>,
        installation: Option<crate::Session>,
    ) -> Result<Value> {
        // Cancellation of a worker invocation cannot abandon a partially launched sandbox.
        tokio::spawn(self.start_run(actor, space, input, key, gesture, session, installation))
            .await
            .map_err(internal)?
    }

    async fn start_run(
        self: Arc<Self>,
        actor: Actor,
        space: SpaceId,
        input: Value,
        key: Option<String>,
        gesture: Option<String>,
        session: Option<Uuid>,
        installation: Option<crate::Session>,
    ) -> Result<Value> {
        let actor_key = format!("run-start:{}", actor.account);
        let gesture =
            gesture.ok_or_else(|| Error::invalid_input("A run start needs a gesture key."))?;
        let fingerprint = serde_json::to_vec(&(&actor.via, &key, &input)).map_err(internal)?;
        let input: StartInput = decode(input)?;
        let item = session
            .filter(|item| input.targets.as_slice() == [*item])
            .ok_or_else(|| Error::forbidden("A run may target only the current session."))?;
        if input.scope != "session" {
            return Err(Error::forbidden("Only session scope is available."));
        }
        let Some(Via::Plugin { plugin }) = &actor.via else {
            return Err(Error::forbidden(
                "Only an installed plugin can start a run.",
            ));
        };
        let plugin = plugin.clone();
        if key.is_none() {
            return Err(Error::invalid_input("A run start needs a request key."));
        }
        let gate = self
            .run_storage(actor.clone(), space, |storage| {
                Ok(Arc::clone(&storage.runs.start))
            })
            .await?;
        // Serialize start receipt lookup and process launch within one space.
        let _start = gate.lock().await;
        let caller = actor.clone();
        let bundle_root = self.root.join("spaces").join(space.to_string());
        let (binding, receipt, notify) = self
            .run_storage(actor.clone(), space, move |storage| {
                let actor = caller;
                let kind = storage.row(item)?.head.kind.clone();
                storage.permits(&actor, &kind, false)?;
                let receipt = storage
                    .receipt(&actor_key, Some(&gesture), &fingerprint)?
                    .expect("start receipt key");
                if receipt.replay.is_some() {
                    return Ok((None, receipt, storage.runs.logs.clone()));
                }
                if storage.runs.entries.len() >= MAX_RUNS_PER_SPACE {
                    let Some(index) = storage
                        .runs
                        .entries
                        .iter()
                        .position(|run| run.stopped.load(Ordering::Acquire))
                    else {
                        return Err(Error::unavailable("This space has too many active runs."));
                    };
                    storage.runs.entries.remove(index);
                }
                let manifest = &storage
                    .plugins
                    .get(&plugin)
                    .ok_or_else(|| Error::forbidden("The harness plugin is unavailable."))?
                    .manifest;
                let declaration = manifest.programs.get(&input.program).ok_or_else(|| {
                    Error::forbidden("The program is not declared by the harness.")
                })?;
                let path = declaration
                    .path
                    .as_deref()
                    .ok_or_else(|| Error::invalid_input("The program needs a bundle path."))?;
                let program = ProgramBinding::new(
                    plugin.clone(),
                    input.program,
                    bundle_root.join(".calternal/plugins").join(&plugin),
                    path,
                    declaration.image,
                    manifest.destinations.iter().map(String::as_str),
                )
                .map_err(run_error)?;
                let reply = kind == "calternal-chat.chats"
                    && manifest.calls.iter().any(|name| {
                        matches!(
                            name.as_str(),
                            "calternal-chat.send" | "calternal-chat.chats.send"
                        )
                    });
                Ok((Some((program, reply)), receipt, storage.runs.logs.clone()))
            })
            .await?;
        let Some((program, reply)) = binding else {
            drop(self.admit_session(installation.as_ref()).await?);
            let mut result = receipt.replay.expect("replayed start receipt");
            let id: StopInput = decode(json!({"id":result["id"]}))?;
            result["status"] = json!(if self
                .find_run(actor, space, id.id)
                .await
                .is_ok_and(|run| !run.stopped.load(Ordering::Acquire))
            {
                "running"
            } else {
                "stopped"
            });
            return Ok(result);
        };
        let service = self.run_service()?;
        // The launched run uses the account and fixed session target only.
        // A revoked browser cannot publish a run that was still starting.
        drop(self.admit_session(installation.as_ref()).await?);
        let handle = service
            .start(StartRun {
                program,
                space,
                scope: RunScope::Session,
                targets: vec![item],
                actor: actor.clone(),
            })
            .await
            .map_err(run_error)?;
        let id = handle.id;
        let run = Arc::new(Run {
            grant: RunScopeGrant { id, space, item },
            actor: actor.clone(),
            log: Log::default(),
            notify,
            stopped: AtomicBool::new(false),
            sends: AsyncMutex::default(),
        });
        let publish = Arc::clone(&run);
        let result = json!({"id":id,"status":"running"});
        let stored = result.clone();
        let host = self.clone();
        if let Err(error) = self
            .run_storage(actor.clone(), space, move |storage| {
                let _admission = host.admit_session_blocking(installation.as_ref())?;
                save_start(storage, &receipt, &stored)?;
                storage.runs.entries.push_back(publish);
                Ok(())
            })
            .await
        {
            let _ = service.stop(id, &actor).await;
            return Err(error);
        }
        tokio::spawn(output::monitor(Arc::downgrade(&self), run, handle, reply));
        Ok(result)
    }

    pub(crate) async fn run_send(
        self: Arc<Self>,
        actor: Actor,
        space: SpaceId,
        input: Value,
        key: Option<String>,
        installation: Option<crate::Session>,
    ) -> Result<Value> {
        let input: SendInput = decode(input)?;
        let run = self
            .find_run(actor.clone(), space, input.id)
            .await
            .map_err(|error| {
                if error.code == ErrorCode::NotFound {
                    Error::forbidden("The caller cannot send to this run.")
                } else {
                    error
                }
            })?;
        if run.actor.account != actor.account || (actor.via.is_some() && actor != run.actor) {
            return Err(Error::forbidden(
                "Only the starting person may send to this run.",
            ));
        }
        let mut sends = run.sends.lock().await;
        let digest = blake3::hash(&serde_json::to_vec(&input.input).map_err(internal)?);
        if let Some(previous) = key.as_ref().and_then(|key| sends.get(key)) {
            return if previous == &digest {
                Ok(Value::Null)
            } else {
                Err(Error::request_reused())
            };
        }
        if run.stopped.load(Ordering::Acquire) {
            return Err(Error::not_found("Run has stopped."));
        }
        if key.is_some() && sends.len() >= MAX_SEND_RECEIPTS {
            return Err(Error::unavailable("This run has too many input receipts."));
        }
        let value = output::resolve_prompt(&self, &run, input.input).await?;
        let mut bytes = serde_json::to_vec(&value).map_err(internal)?;
        bytes.push(b'\n');
        if bytes.len() > MAX_FRAME_BYTES + 1 {
            return Err(Error::new(
                ErrorCode::TooLarge,
                "Run input exceeds the frame limit.",
            ));
        }
        let _admission = self.admit_session(installation.as_ref()).await?;
        self.run_service()?
            .send_input(input.id, &run.actor, bytes)
            .await
            .map_err(run_error)?;
        if let Some(key) = key {
            sends.insert(key, digest);
        }
        Ok(Value::Null)
    }

    pub(crate) async fn run_stop(
        self: Arc<Self>,
        actor: Actor,
        space: SpaceId,
        input: Value,
        installation: Option<crate::Session>,
    ) -> Result<Value> {
        let input: StopInput = decode(input)?;
        let run = self.find_run(actor.clone(), space, input.id).await?;
        let _admission = self.admit_session(installation.as_ref()).await?;
        if !run.stopped.load(Ordering::Acquire) {
            match self.run_service()?.stop(input.id, &actor).await {
                Ok(()) | Err(RunError::NotActive) => run.stopped.store(true, Ordering::Release),
                Err(error) => return Err(run_error(error)),
            }
        }
        Ok(Value::Null)
    }

    pub(crate) async fn run_log(
        self: &Arc<Self>,
        actor: Actor,
        space: SpaceId,
        id: Uuid,
        offset: Option<u64>,
    ) -> Result<LogBatch> {
        self.find_run(actor, space, id).await?.log.after(id, offset)
    }

    fn run_service(&self) -> Result<Arc<RunService>> {
        self.runs
            .lock()
            .map_err(internal)?
            .clone()
            .ok_or_else(|| Error::unavailable("The run service is not configured."))
    }

    async fn check_run_access(
        self: &Arc<Self>,
        space: SpaceId,
        actor: Actor,
        item: Uuid,
    ) -> Result<()> {
        self.run_storage(actor.clone(), space, move |storage| {
            storage.permits(&actor, &storage.row(item)?.head.kind, false)
        })
        .await
    }

    async fn find_run(
        self: &Arc<Self>,
        actor: Actor,
        space: SpaceId,
        id: Uuid,
    ) -> Result<Arc<Run>> {
        self.run_storage(actor.clone(), space, move |storage| {
            let run = storage
                .runs
                .entries
                .iter()
                .find(|run| run.grant.id == id)
                .ok_or_else(|| Error::not_found("Run was not found."))?;
            storage.permits(&actor, &storage.row(run.grant.item)?.head.kind, false)?;
            Ok(Arc::clone(run))
        })
        .await
    }

    async fn run_storage<T: Send + 'static>(
        self: &Arc<Self>,
        actor: Actor,
        space: SpaceId,
        f: impl FnOnce(&mut crate::space::Space) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let host = Arc::clone(self);
        tokio::task::spawn_blocking(move || host.with_space(&actor, space, f))
            .await
            .map_err(internal)?
    }
}

fn save_start(storage: &mut crate::space::Space, receipt: &Receipt, result: &Value) -> Result<()> {
    let tx = storage.db.transaction().map_err(internal)?;
    receipt.store(&tx, result, calternal_types::now_ms())?;
    tx.commit().map_err(internal)
}

fn run_error(error: RunError) -> Error {
    match error {
        RunError::InputActor | RunError::StopNotAllowed => {
            Error::forbidden("The run request is not allowed.")
        }
        RunError::InvalidProgram | RunError::InvalidScope | RunError::ProgramPath => {
            Error::invalid_input("The run declaration or scope is invalid.")
        }
        RunError::InputTooLarge => {
            Error::new(ErrorCode::TooLarge, "Run input exceeds the frame limit.")
        }
        RunError::NotActive => Error::not_found("Run has stopped."),
        error => {
            eprintln!("calternal-host: run service: {error}");
            Error::unavailable("The run service could not complete the request.")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scoped_route_checks_every_target_before_dispatch() {
        let grant = RunScopeGrant {
            id: Uuid::now_v7(),
            space: SpaceId::new(),
            item: Uuid::now_v7(),
        };
        let actor = Actor::run(calternal_types::AccountId::new(), "harness", grant.id);
        let other = Uuid::now_v7();
        for (call, input) in [
            ("s.read", json!({"item":other})),
            (
                "s.state",
                json!({"item":other,"op":"get","key":"read through"}),
            ),
            ("s.query", json!({"from":"calternal-chat.chats"})),
            (
                "calternal-chat.chats.send",
                json!({"item":other,"text":"outside"}),
            ),
            (
                "s.write",
                json!({"changes":[{"op":"append record","item":grant.item},{"op":"append record","item":other}]}),
            ),
        ] {
            let call = Call {
                call: call.into(),
                input,
                idempotency_key: None,
                slot_selection: None,
            };
            assert_eq!(
                grant.check(&actor, grant.space, &call).unwrap_err().code,
                ErrorCode::Forbidden
            );
        }
        let read = Call {
            call: "s.read".into(),
            input: json!({"item":grant.item}),
            idempotency_key: None,
            slot_selection: None,
        };
        assert!(grant.check(&actor, grant.space, &read).is_ok());
        assert!(
            grant
                .check(&Actor::account(actor.account), grant.space, &read)
                .is_err()
        );
        assert!(grant.check(&actor, SpaceId::new(), &read).is_err());
    }
}
