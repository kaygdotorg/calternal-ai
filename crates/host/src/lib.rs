//! Personal-space host for the Markdown plugin format.
#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod declarations;
mod installation;
mod query;
mod reactions;
mod records;
mod space;
mod worker_calls;
mod worker_runtime;
mod write;

pub use worker_runtime::WorkerRuntime;

use calternal_auth::Auth;
use calternal_types::{AccountId, Actor, Error, Result, SpaceId, Via};
use serde::Deserialize;
use serde_json::{Value, json};
use space::Space;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Maximum bytes in an encoded write request.
pub const WRITE_BYTES: usize = 16 * 1024 * 1024;

/// A request from a session or a trusted in-process adapter. The envelope
/// carries only operation data. The adapter supplies identity separately.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Call {
    /// Name from the host call table.
    pub call: String,
    /// Arguments specific to the call.
    #[serde(default)]
    pub input: Value,
    /// Optional per-gesture or per-handler-write retry key.
    pub idempotency_key: Option<String>,
}

type Slot = Arc<Mutex<Option<Space>>>;

/// The one call dispatcher. It opens space storage on demand and serializes
/// publication only within the affected space.
pub struct Host {
    root: PathBuf,
    auth: Auth,
    retention_ms: i64,
    spaces: Mutex<HashMap<SpaceId, Slot>>,
    workers: Option<WorkerRuntime>,
}

impl Host {
    /// Attach the directory and a host-owned storage root. Performs no I/O.
    /// Rejects an invalid operator-supplied receipt retention period.
    pub fn new(root: PathBuf, auth: Auth, retention_days: u32) -> Result<Self> {
        if retention_days == 0 {
            return Err(Error::invalid_input("receipt retention must be positive"));
        }
        Ok(Self {
            root,
            auth,
            retention_ms: i64::from(retention_days) * 86_400_000,
            spaces: Mutex::default(),
            workers: None,
        })
    }

    /// Enable an operator-controlled launcher for new coded installations.
    /// Configuration alone starts no process. Spaces without code stay data-only.
    pub fn with_worker_runtime(mut self, runtime: WorkerRuntime) -> Self {
        self.workers = Some(runtime);
        self
    }

    /// Authenticate a bearer session and dispatch a shell call. Storage I/O
    /// and the per-space lock run on a blocking thread.
    pub async fn session_call(
        self: Arc<Self>,
        bearer: &str,
        space: SpaceId,
        call: Call,
    ) -> Result<Value> {
        let (account, _) = self
            .auth
            .identify(bearer)
            .await
            .ok_or_else(|| Error::unauthorized("Sign in to use this space."))?;
        self.trusted_call(Actor::account(account), space, call)
            .await
    }

    /// Dispatch from a trusted asynchronous adapter on a blocking thread.
    /// The adapter verifies the person and any plugin identity before calling.
    /// Storage and authorization errors are returned through the shared error type.
    pub async fn trusted_call(
        self: Arc<Self>,
        actor: Actor,
        space: SpaceId,
        call: Call,
    ) -> Result<Value> {
        self.dispatch(actor, space, call, worker_calls::Chain::default())
            .await
    }

    /// Install a trusted local plugin bundle for the space's owner. The
    /// operator supplies the source directory and whether it is official.
    /// Costs one bounded bundle read plus durable publication. Called on a
    /// blocking thread, or from the host binary's local operator command.
    pub fn install_local(
        &self,
        owner: AccountId,
        id: SpaceId,
        directory: &Path,
        official: bool,
    ) -> Result<()> {
        let actor = Actor::account(owner);
        self.with_space(&actor, id, |_| Ok(()))?;
        let files = installation::bundle(directory)?;
        let registration = match (self.workers.as_ref(), files.get("operations.ts")) {
            (Some(runtime), Some(source)) => {
                let snapshot = WorkerRuntime::snapshot(source)?;
                Some(block_on(runtime.probe(snapshot.path()))??)
            }
            _ => None,
        };
        self.with_space(&actor, id, |space| {
            space.install(files, official, registration)
        })
    }

    /// Execute a verified in-process call. A worker adapter supplies its
    /// plugin via the shared actor type. A run needs a scope-aware adapter
    /// before it can call the host. This method blocks on space storage.
    pub fn call(&self, actor: Actor, id: SpaceId, call: Call) -> Result<Value> {
        self.with_space(&actor, id, |space| space.call(&actor, call))
    }

    fn owner_display(&self, account: AccountId) -> Result<String> {
        let key = account.to_string();
        let lookup = self.auth.find(&key);
        let (_, name) = block_on(lookup)?.map_err(internal)?;
        Ok(name)
    }

    fn with_space<T>(
        &self,
        actor: &Actor,
        id: SpaceId,
        f: impl FnOnce(&mut Space) -> Result<T>,
    ) -> Result<T> {
        if matches!(actor.via, Some(Via::Agent { .. })) {
            return Err(Error::forbidden("Run scope is required for this call."));
        }
        let directory = self.auth.spaces();
        if !directory
            .is_personal_space(actor.account, id)
            .map_err(internal)?
            || directory
                .capabilities(actor.account, id)
                .map_err(internal)?
                .is_none()
        {
            return Err(Error::not_found("Personal space was not found."));
        }
        let slot = {
            let mut slots = self.spaces.lock().map_err(internal)?;
            slots.entry(id).or_default().clone()
        };
        // Keep the space lock across persistence so publication, recovery and
        // receipt lookup see the same version of the filesystem and ledger.
        let mut opened = slot.lock().map_err(internal)?;
        if opened.is_none() {
            let display = self.owner_display(actor.account)?;
            let mut space = Space::open(&self.root, id, self.retention_ms)?;
            space.owner_display = Some((actor.account, display));
            *opened = Some(space);
        }
        let space = opened.as_mut().expect("space initialized");
        space.recover()?;
        space.expire_trash()?;
        f(space)
    }
}

fn block_on<F: std::future::Future>(future: F) -> Result<F::Output> {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => Ok(handle.block_on(future)),
        Err(_) => Ok(tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(internal)?
            .block_on(future)),
    }
}

fn internal(error: impl std::fmt::Display) -> Error {
    eprintln!("calternal-host: {error}");
    Error::internal("The host could not complete the call.")
}

fn decode<T: serde::de::DeserializeOwned>(input: Value) -> Result<T> {
    serde_json::from_value(input).map_err(|_| Error::invalid_input("Invalid call arguments."))
}

fn identity(actor: &Actor) -> (&str, AccountId) {
    match &actor.via {
        Some(Via::Plugin { plugin }) => (plugin, actor.account),
        _ => ("host", actor.account),
    }
}

fn result(changes: Vec<Value>) -> Value {
    json!({"changes": changes})
}
