//! Trusted host-call brokers: the only way a worker reaches host state.
//!
//! One broker value exists per invocation and carries that invocation's authority
//! (acting account, content space, plugin and the activation generation captured at
//! admission). [`calternal_plugin_host::WorkerSession`] calls [`InvocationHost::authorize`]
//! before and after every effect and before every delivery; each call reads the
//! current directory grants and activation row, so a revoked membership or a
//! disabled/re-enabled plugin stops the invocation at its next boundary.
//!
//! Implemented host calls: `read_item` and `write_item`, both through
//! [`calternal_content`] via [`crate::store::Store`]. Every other declared host call
//! is refused until its host service exists.

#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use calternal_core::{HostCallKind, ManifestCapability};
use calternal_plugin_host::Error;
use serde_json::{Map, Value, json};
use uuid::Uuid;

use super::{Plugin, activation, host_capability};
use crate::api::Shared;
use crate::auth::{Directory, Identity};
use crate::authz::Capability;
use crate::store::sha256_hex;

/// Fixed fixture text served to previews. Never user data.
pub const PREVIEW_FIXTURE: &str = "# Preview fixture\n\nThis is fixed sample text used to preview a plugin.\nIt is not one of your notes.\n";
/// Synthetic item ID used only by previews; a UUIDv7-shaped value no store allocates.
pub const PREVIEW_ITEM_ID: &str = "00000000-0000-7000-8000-000000000000";

/// Per-invocation trusted adapter called from the worker thread, never by plugin code.
/// Both methods may block on directory and store locks.
pub trait InvocationHost: Send + Sync {
    /// Recheck current authority for `required`: grants, plugin activation and generation.
    fn authorize(&self, required: ManifestCapability) -> bool;
    /// Validate one host call's input and apply it under current authority.
    /// Errors reach the worker as `ok: false` with no detail.
    fn call(&self, method: HostCallKind, input: &Value) -> Result<Value, Error>;
}

/// Broker for a real invocation by `identity` in one content space.
pub struct SpaceHost<D: Directory> {
    state: Shared<D>,
    identity: Identity,
    space: String,
    plugin: Arc<Plugin>,
    generation: u32,
    request_id: String,
    writes: AtomicU32,
}

impl<D: Directory> SpaceHost<D> {
    /// Capture an invocation's authority. `space` is the content-space key used by
    /// the store and directory; `generation` is the activation generation read when
    /// the invocation was admitted; `request_id` scopes write receipts.
    pub fn new(
        state: Shared<D>,
        identity: Identity,
        space: String,
        plugin: Arc<Plugin>,
        generation: u32,
        request_id: String,
    ) -> Self {
        Self {
            state,
            identity,
            space,
            plugin,
            generation,
            request_id,
            writes: AtomicU32::new(0),
        }
    }

    fn has(&self, capability: Capability) -> bool {
        self.state
            .tokens
            .capabilities(&self.identity.id, &self.space)
            .is_some_and(|set| set.contains(capability))
    }

    fn read_item(&self, input: &Value) -> Result<Value, Error> {
        let (kind, id) = item_target(input, &["kind", "item_id"])?;
        let store = self.state.store.lock().map_err(|_| Error::Refused)?;
        let item = store
            .read_item(&self.space, kind, id)
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
        let receipt = format!(
            "plugin/{}/{}/{n}",
            self.plugin.manifest.id, self.request_id
        );
        let fingerprint = sha256_hex(input.to_string().as_bytes());
        let store = self.state.store.lock().map_err(|_| Error::Refused)?;
        let item = store
            .write_item(
                &self.identity.id,
                &receipt,
                &fingerprint,
                &self.space,
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
    /// Cost: one directory capability read plus one store lock and indexed
    /// activation read; no content I/O.
    fn authorize(&self, required: ManifestCapability) -> bool {
        let Some(required) = host_capability(required) else {
            return false;
        };
        if !self.has(required) {
            return false;
        }
        let Ok(store) = self.state.store.lock() else {
            return false;
        };
        activation(&store, &self.space, &self.plugin)
            .is_ok_and(|row| row.enabled && row.generation == self.generation)
    }

    /// Cost: `read_item` takes the store lock for one catalog route lookup and one
    /// item read; `write_item` adds a durable content write (journal, file and
    /// directory fsync). Other methods allocate nothing and are refused.
    fn call(&self, method: HostCallKind, input: &Value) -> Result<Value, Error> {
        match method {
            HostCallKind::ReadItem => self.read_item(input),
            HostCallKind::WriteItem => self.write_item(input),
            _ => Err(Error::Refused),
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
