//! Personal-space host for the Markdown plugin format.
#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod browser;
mod declarations;
mod installation;
pub mod live;
mod maintenance;
mod query;
mod reactions;
mod records;
mod runs;
mod session;
mod space;
mod worker_calls;
mod worker_runtime;
mod write;

pub use worker_runtime::WorkerRuntime;

use calternal_auth::Auth;
use calternal_types::{AccountId, Actor, Error, Result, SpaceId, Via};
use maintenance::Slot;
use serde::Deserialize;
use serde_json::{Value, json};
use session::Session;
use space::Space;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Maximum bytes in an encoded write request.
pub const WRITE_BYTES: usize = 16 * 1024 * 1024;

/// A request from a session or a trusted in-process adapter. The envelope
/// carries operation data and a person’s optional slot selection. The
/// adapter supplies identity separately.
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
    /// A person-picked slot and filler to check before dispatching coded work.
    pub slot_selection: Option<SlotSelection>,
}

/// An untrusted selection resolved against installed declarations by the
/// host before it can authorize a worker callback.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SlotSelection {
    /// Slot ID local to the plugin owning the coded operation.
    pub slot: String,
    /// Installed filler plugin ID selected for this slot.
    pub filler: String,
}

/// The one call dispatcher. It opens space storage on demand and serializes
/// publication only within the affected space.
pub struct Host {
    root: PathBuf,
    auth: Auth,
    retention_ms: i64,
    spaces: Mutex<HashMap<SpaceId, Slot>>,
    workers: Option<WorkerRuntime>,
    development_plugins: Vec<(String, PathBuf)>,
    runs: Mutex<Option<Arc<calternal_machine::run_service::RunService>>>,
    sessions: session::Registry,
    live_accounts: live::Accounts,
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
            development_plugins: Vec::new(),
            runs: Mutex::default(),
            sessions: session::Registry::default(),
            live_accounts: live::Accounts::default(),
        })
    }

    /// Resolve a bearer session token to its account ID.
    pub async fn identify_session(&self, bearer: &str) -> Option<AccountId> {
        self.auth.identify(bearer).await.map(|(account, _)| account)
    }

    /// Enable an operator-controlled launcher for new coded installations.
    /// Configuration alone starts no process. Spaces without code stay data-only.
    pub fn with_worker_runtime(mut self, runtime: WorkerRuntime) -> Self {
        self.workers = Some(runtime);
        self
    }

    /// Supply trusted local plugin bundles to seed the disposable browser
    /// host's personal space on sign-in and authenticated session status.
    /// Ordinary hosts have no seed bundles.
    pub fn with_development_plugins(
        mut self,
        plugins: impl IntoIterator<Item = (String, PathBuf)>,
    ) -> Self {
        self.development_plugins.extend(plugins);
        self
    }

    fn ensure_development_plugins(&self, owner: AccountId, id: SpaceId) -> Result<()> {
        let actor = Actor::account(owner);
        let missing = self.with_space(&actor, id, |space| {
            Ok(self
                .development_plugins
                .iter()
                .filter(|(plugin, _)| !space.plugins.contains_key(plugin))
                .collect::<Vec<_>>())
        })?;
        let mut result = Ok(());
        for (plugin, directory) in missing {
            if let Err(error) = self.install_local(owner, id, directory, true)
                && (error.code != calternal_types::ErrorCode::Conflict
                    || !self
                        .with_space(&actor, id, |space| Ok(space.plugins.contains_key(plugin)))?)
            {
                result = result.and(Err(error));
            }
        }
        result
    }

    /// Authenticate and bind the issuing installation to a shell call.
    /// Storage I/O and the per-space lock run on a blocking thread.
    pub async fn session_call(
        self: Arc<Self>,
        bearer: &str,
        space: SpaceId,
        call: Call,
    ) -> Result<Value> {
        let session = self.authenticate(bearer).await?;
        self.authorized_call(session, space, call).await
    }

    async fn authorized_call(
        self: Arc<Self>,
        session: Session,
        space: SpaceId,
        call: Call,
    ) -> Result<Value> {
        let actor = Actor::account(session.account);
        let chain = worker_calls::Chain::for_session(session);
        self.dispatch(actor, space, call, chain).await
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
        if matches!(actor.via, Some(Via::Agent { .. } | Via::Run { .. })) {
            return Err(Error::forbidden(
                "A run requires a host-owned session scope.",
            ));
        }
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
        if matches!(actor.via, Some(Via::Agent { .. } | Via::Run { .. })) {
            return Err(Error::forbidden(
                "A run requires a host-owned session scope.",
            ));
        }
        self.call_scoped(actor, id, call, None, None)
    }

    fn call_scoped(
        &self,
        actor: Actor,
        id: SpaceId,
        call: Call,
        session: Option<&Session>,
        run: Option<&runs::RunScopeGrant>,
    ) -> Result<Value> {
        if call.slot_selection.is_some() {
            return Err(Error::forbidden(
                "A slot selection requires a person’s coded operation.",
            ));
        }
        let recipient = match call.call.as_str() {
            "s.state" if call.input.get("op").and_then(Value::as_str) == Some("set") => {
                Some(Some(actor.account))
            }
            _ => None,
        };
        self.with_space(&actor, id, |space| {
            let _admission = self.admit_session_blocking(session)?;
            space.runs.admit(run)?;
            if call.call == "s.write" {
                let written = space.write_call(&actor, call)?;
                if written.disposition == crate::write::Disposition::Committed {
                    space.live.notify(None);
                }
                return Ok(written.value);
            }
            let output = space.call(&actor, call)?;
            if let Some(recipient) = recipient {
                space.live.notify(recipient);
            }
            Ok(output)
        })
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
        self.with_space_slot(actor, id, |space, _| f(space))
    }

    fn with_space_slot<T>(
        &self,
        actor: &Actor,
        id: SpaceId,
        f: impl FnOnce(&mut Space, &Slot) -> Result<T>,
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
        let slot = self.space_slot(id)?;
        // Keep the space lock across persistence so publication, recovery and
        // receipt lookup see the same version of the filesystem and ledger.
        let mut opened = slot.resident.lock().map_err(internal)?;
        opened.visited = std::time::Instant::now();
        if opened.space.is_none() {
            let display = self.owner_display(actor.account)?;
            let mut space = Space::open(&self.root, id, self.retention_ms)?;
            space.scope_existing_viewer_state(actor.account)?;
            space.owner_display = Some((actor.account, display));
            opened.space = Some(space);
        }
        let space = opened.space.as_mut().expect("space initialized");
        space.recover()?;
        space.expire_trash()?;
        f(space, &slot)
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
        Some(Via::Plugin { plugin } | Via::Run { plugin, .. }) => (plugin, actor.account),
        _ => ("host", actor.account),
    }
}

fn result(changes: Vec<Value>) -> Value {
    json!({"changes": changes})
}
