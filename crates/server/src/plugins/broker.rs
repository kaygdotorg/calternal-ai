//! Trusted host-call brokers: the only way a worker reaches host state.
//!
//! One broker value exists per invocation and carries that invocation's authority
//! (acting account and its sign-in, content space, plugin and the activation
//! generation captured at admission). [`calternal_plugin_host::WorkerSession`] calls
//! [`InvocationHost::authorize`] before and after every effect and before every
//! delivery; each call rechecks the sign-in and reads the current directory grants
//! and activation row, so sign-out, a revoked installation or membership, or a
//! disabled/re-enabled plugin stops the invocation at its next boundary.
//!
//! Implemented host calls: `read_item` and `write_item`, both through
//! [`calternal_content`] via [`crate::store::Store`]; `start_job` and
//! `checkpoint_job` through [`super::jobs::Jobs`]; `request_secret_handle` through
//! [`crate::secrets::plugin_handle`]. Every other declared host call is
//! refused until its host service exists.

#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use calternal_core::{HostCallKind, ManifestCapability};
use calternal_plugin_host::Error;
use serde_json::{Map, Value, json};
use tokio::runtime::Handle;
use uuid::Uuid;

use calternal_secrets::{Actor, Owner, Via};

use super::jobs::Admission;
use super::live::Live;
use super::{Plugin, Session, activation, host_capability, runner};
use crate::api::Shared;
use crate::auth::{Directory, Identity};
use calternal_types::{Capability, SpaceId};
use crate::store::sha256_hex;

/// Fixed fixture text served to previews. Never user data.
pub const PREVIEW_FIXTURE: &str = "# Preview fixture\n\nThis is fixed sample text used to preview a plugin.\nIt is not one of your notes.\n";
/// Synthetic item ID used only by previews; a UUIDv7-shaped value no store allocates.
pub const PREVIEW_ITEM_ID: &str = "00000000-0000-7000-8000-000000000000";

/// Per-invocation trusted adapter called from the worker thread, never by plugin code.
/// Both methods may block on the directory and on the target space's lock.
pub trait InvocationHost: Send + Sync {
    /// Recheck current authority for `required`: grants, plugin activation and generation.
    fn authorize(&self, required: ManifestCapability) -> bool;
    /// Validate one host call's input and apply it under current authority.
    /// Errors reach the worker as `ok: false` with no detail.
    fn call(&self, method: HostCallKind, input: &Value) -> Result<Value, Error>;
    /// Receive one ordered chunk the worker emitted for `job_id`, after the session
    /// checked its sequence and current authority. The default drops it.
    fn chunk(&self, _job_id: &str, _data: &Value) {}
}

/// Broker for a real invocation by `identity` in one content space.
pub struct SpaceHost<D: Directory> {
    state: Shared<D>,
    identity: Identity,
    session: Session,
    runtime: Handle,
    space: SpaceId,
    plugin: Arc<Plugin>,
    generation: u32,
    request_id: String,
    writes: AtomicU32,
    starts: AtomicU32,
    run: Option<Run>,
}

/// The job a run invocation executes: it may checkpoint and stream for this job only,
/// and every boundary also requires the run to still be current (not cancelled).
pub struct Run {
    /// Job UUID.
    pub job: String,
    /// Run number from [`super::jobs::Jobs::claim`].
    pub run: u32,
    /// The job's live stream.
    pub live: Arc<Live>,
    /// Checkpoints written so far; scopes this run's write receipts so a run resumed
    /// from the same checkpoint replays its writes instead of repeating them.
    pub checkpoints: AtomicU32,
}

impl<D: Directory> SpaceHost<D> {
    /// Capture an invocation's authority. `session` is the caller's sign-in, rechecked
    /// at every boundary; `space` is the space the invocation runs in; `generation` is the activation generation read when the invocation
    /// was admitted; `request_id` scopes write receipts. Must be called inside the
    /// async runtime, whose handle the blocking session checks use.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        state: Shared<D>,
        identity: Identity,
        session: Session,
        space: SpaceId,
        plugin: Arc<Plugin>,
        generation: u32,
        request_id: String,
    ) -> Self {
        Self {
            state,
            identity,
            session,
            runtime: Handle::current(),
            space,
            plugin,
            generation,
            request_id,
            writes: AtomicU32::new(0),
            starts: AtomicU32::new(0),
            run: None,
        }
    }

    /// Bind this broker to a job's run invocation. Cost: none.
    pub fn for_run(mut self, run: Run) -> Self {
        self.run = Some(run);
        self
    }

    /// Receipt scope of this invocation's effects: its request, or for a job run the
    /// job and its checkpoint count. Cost: one small allocation.
    fn scope(&self) -> String {
        match &self.run {
            Some(run) => format!(
                "job/{}/{}",
                run.job,
                run.checkpoints.load(Ordering::Acquire)
            ),
            None => self.request_id.clone(),
        }
    }

    /// Admit a job whose run operation is declared by this plugin and has no item
    /// target. Input: `{ operation, input, parent_id? }`. The admission key is the
    /// receipt scope plus the call's position, so a retried request returns the job
    /// it admitted before. The job is scheduled once its record is durable.
    fn start_job(&self, input: &Value) -> Result<Value, Error> {
        let object = exact(input, &["operation", "input", "parent_id"])?;
        let operation = str_field(object, "operation")?.ok_or(Error::Invalid)?;
        let run_input = object.get("input").cloned().ok_or(Error::Invalid)?;
        let parent = str_field(object, "parent_id")?.map(str::to_owned);
        if self
            .plugin
            .operation(operation)
            .is_none_or(|op| op.target.is_some())
        {
            return Err(Error::Refused);
        }
        let n = self.starts.fetch_add(1, Ordering::Relaxed);
        let id = self
            .state
            .jobs
            .admit(Admission {
                space: self.space.to_string(),
                plugin: self.plugin.manifest.id.clone(),
                account: self.identity.id,
                session: self.session.clone(),
                operation: operation.to_owned(),
                input: run_input,
                parent,
                generation: self.generation,
                request_key: format!("{}/{}/{n}", self.identity.id, self.scope()),
            })
            .map_err(|_| Error::Refused)?;
        let _guard = self.runtime.enter();
        runner::schedule(&self.state, self.space.to_string(), id.clone());
        Ok(json!({ "job_id": id }))
    }

    /// `request_secret_handle { name }`: an opaque handle to the space's secret for this
    /// plugin, owned by the space's UUID (never the content-space key). When the secret
    /// is missing, one pending request is recorded for the trusted prompt and the call
    /// is refused. The worker session has already checked `name` is declared, and
    /// rechecks sign-in, membership and enablement around the call.
    fn secret_handle(&self, input: &Value) -> Result<Value, Error> {
        let object = exact(input, &["name"])?;
        let name = str_field(object, "name")?.ok_or(Error::Invalid)?;
        let (space, account) = (self.space, self.identity.id);
        let secrets = self.state.store.secrets();
        let actor = Actor {
            account,
            via: Via::Plugin(self.plugin.manifest.id.clone()),
        };
        crate::secrets::plugin_handle(
            secrets.as_ref(),
            &Owner::Space(space),
            &actor,
            &self.plugin.manifest.id,
            name,
        )
    }

    /// Durably checkpoint the bound job: `{ job_id, checkpoint }`. Answers the stream
    /// sequence the checkpoint covers.
    fn checkpoint_job(&self, input: &Value) -> Result<Value, Error> {
        let run = self.run.as_ref().ok_or(Error::Refused)?;
        let object = exact(input, &["job_id", "checkpoint"])?;
        if str_field(object, "job_id")? != Some(run.job.as_str()) {
            return Err(Error::Refused);
        }
        let checkpoint = object.get("checkpoint").ok_or(Error::Invalid)?;
        let n = self
            .state
            .jobs
            .checkpoint(&self.space.to_string(), &run.job, run.run, &run.live, checkpoint)
            .map_err(|_| Error::Refused)?;
        run.checkpoints.store(n, Ordering::Release);
        self.writes.store(0, Ordering::Release);
        self.starts.store(0, Ordering::Release);
        Ok(json!({ "checkpoint_seq": run.live.next_seq() }))
    }

    fn has(&self, capability: Capability) -> bool {
        self.state
            .tokens
            .capabilities(self.identity.id, self.space)
            .is_some_and(|set| set.contains(capability))
    }

    fn read_item(&self, input: &Value) -> Result<Value, Error> {
        let (kind, id) = item_target(input, &["kind", "item_id"])?;
        let item = self
            .state
            .store
            .read_item(self.space, kind, id)
            .map_err(|_| Error::Refused)?;
        Ok(item_json(&item))
    }

    fn write_item(&self, input: &Value) -> Result<Value, Error> {
        let object = exact(
            input,
            &["kind", "item_id", "base_revision", "metadata", "text"],
        )?;
        let kind = str_field(object, "kind")?.ok_or(Error::Invalid)?;
        let text = object
            .get("text")
            .and_then(Value::as_str)
            .ok_or(Error::Invalid)?;
        let id = str_field(object, "item_id")?
            .map(|s| Uuid::parse_str(s).map_err(|_| Error::Invalid))
            .transpose()?;
        let base = str_field(object, "base_revision")?;
        let metadata = match object.get("metadata") {
            None => json!({}),
            Some(m @ Value::Object(_)) => m.clone(),
            Some(_) => return Err(Error::Invalid),
        };
        // A plugin writes only its own kinds, and only with current write authority.
        if !self.plugin.declares_kind(kind) || !self.has(Capability::ContentWrite) {
            return Err(Error::Refused);
        }
        // Deterministic per-invocation receipt IDs make a retried request replay
        // its earlier writes instead of duplicating them.
        let n = self.writes.fetch_add(1, Ordering::Relaxed);
        let receipt = format!("plugin/{}/{}/{n}", self.plugin.manifest.id, self.scope());
        let fingerprint = sha256_hex(input.to_string().as_bytes());
        let item = self
            .state
            .store
            .write_item(
                self.identity.id,
                &receipt,
                &fingerprint,
                self.space,
                kind,
                id,
                base,
                metadata,
                text,
            )
            .map_err(|_| Error::Refused)?;
        Ok(json!({"id": item.id, "revision": item.revision}))
    }
}

impl<D: Directory> InvocationHost for SpaceHost<D> {
    /// Cost: one session check (see [`Session::is_live`]), one directory capability
    /// read and one pooled activation read; no content I/O. The worker
    /// session calls this up to four times per host call.
    fn authorize(&self, required: ManifestCapability) -> bool {
        let Some(required) = host_capability(required) else {
            return false;
        };
        if self.run.as_ref().is_some_and(|r| !r.live.is_current(r.run))
            || !self
                .session
                .is_live_blocking(&self.state.tokens, &self.runtime)
            || !self.has(required)
        {
            return false;
        }
        activation(&self.state.store, self.space, &self.plugin)
            .is_ok_and(|row| row.enabled && row.generation == self.generation)
    }

    /// Cost: `read_item` is one pooled catalog route read and one item read under
    /// the space's lock; `write_item` adds a durable content write (journal, file and
    /// directory fsync); `start_job` and `checkpoint_job` are one FULL transaction on
    /// the space's job file; `request_secret_handle` one secret-database read (plus one
    /// fsync'd insert when it records a request). Other methods are refused.
    fn call(&self, method: HostCallKind, input: &Value) -> Result<Value, Error> {
        match method {
            HostCallKind::ReadItem => self.read_item(input),
            HostCallKind::WriteItem => self.write_item(input),
            HostCallKind::StartJob => self.start_job(input),
            HostCallKind::CheckpointJob => self.checkpoint_job(input),
            HostCallKind::RequestSecretHandle => self.secret_handle(input),
            _ => Err(Error::Refused),
        }
    }

    /// Only a run invocation streams, and only for its own job; anything else is
    /// dropped. Cost: one stream lock and the chunk's serialization.
    fn chunk(&self, job_id: &str, data: &Value) {
        if let Some(run) = self.run.as_ref().filter(|r| r.job == job_id) {
            run.live.publish(run.run, data);
        }
    }
}

/// Broker for `plugins.preview`: serves the fixed fixture as the only readable item
/// and refuses everything else, so a preview never touches user data or effects.
/// The caller checks the requester's space access before the preview starts.
pub struct PreviewHost {
    /// Kind the previewed operation reads (its manifest `target` kind).
    pub kind: String,
}

impl InvocationHost for PreviewHost {
    /// Only read-only authority is ever granted. Cost: O(1).
    fn authorize(&self, required: ManifestCapability) -> bool {
        required == ManifestCapability::SpaceRead
    }

    /// Cost: one small fixture allocation for the single permitted read.
    fn call(&self, method: HostCallKind, input: &Value) -> Result<Value, Error> {
        if method != HostCallKind::ReadItem {
            return Err(Error::Refused);
        }
        let (kind, id) = item_target(input, &["kind", "item_id"])?;
        if kind != self.kind || id != PREVIEW_ITEM_ID {
            return Err(Error::Refused);
        }
        Ok(json!({
            "id": PREVIEW_ITEM_ID,
            "kind": kind,
            "revision": "preview-fixture",
            "updated_at": "1970-01-01T00:00:00.000Z",
            "metadata": {"title": "Preview fixture"},
            "text": PREVIEW_FIXTURE,
        }))
    }
}

/// Wire shape of an item returned to a worker: identity, revision, metadata and
/// the body as UTF-8 text (`null` when the body is not text).
fn item_json(item: &calternal_content::Item) -> Value {
    json!({
        "id": item.id,
        "kind": item.kind,
        "revision": item.revision,
        "updated_at": item.updated_at,
        "metadata": item.metadata,
        "text": std::str::from_utf8(&item.bytes).ok(),
    })
}

fn exact<'a>(input: &'a Value, allowed: &[&str]) -> Result<&'a Map<String, Value>, Error> {
    let object = input.as_object().ok_or(Error::Invalid)?;
    if object.keys().any(|k| !allowed.contains(&k.as_str())) {
        return Err(Error::Invalid);
    }
    Ok(object)
}

fn str_field<'a>(object: &'a Map<String, Value>, key: &str) -> Result<Option<&'a str>, Error> {
    match object.get(key) {
        None => Ok(None),
        Some(Value::String(s)) if !s.is_empty() && s.len() <= 2 * 1024 * 1024 => Ok(Some(s)),
        Some(_) => Err(Error::Invalid),
    }
}

fn item_target<'a>(input: &'a Value, keys: &[&str]) -> Result<(&'a str, &'a str), Error> {
    let object = exact(input, keys)?;
    let kind = str_field(object, "kind")?.ok_or(Error::Invalid)?;
    let id = str_field(object, "item_id")?.ok_or(Error::Invalid)?;
    Ok((kind, id))
}
