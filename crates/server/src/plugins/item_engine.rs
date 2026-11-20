//! Typed v2 item callbacks on the existing blocking content writer.
use base64::{Engine, engine::general_purpose::STANDARD};
use calternal_content::{
    CheckedItemBody, CheckedItemPlacement, Content, ControlFence, DiscoveryWrite, FileFormat, Item,
    Placement, Request,
};
use calternal_core::ManifestPlacement;
use calternal_plugin_host::ManifestV2;
use calternal_types::{
    AccountId, Actor, Error, ItemRef, ReceiptRef, RequestId, RevisionRef, SpaceId,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Instant,
};
use uuid::Uuid;

mod attested;
mod commit;
mod provision;
mod read;
mod stages;
mod viewer;
mod wire;
pub use attested::{AttemptOutputs, AttemptText, CapturedInput};
use attested::{Attested, Composed, Placing};
pub use provision::kind_registrations;
pub use stages::ItemStages;
use commit::receipt_key;
use wire::*;

/// The protected check required at a callback boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemCheck {
    /// Recheck live create authority for the admitted package and kind.
    Create,
    /// Recheck live item writer and the exact invocation or attempt.
    Write,
    /// Check authority to learn the current revision and byte length.
    Head,
    /// Check whether one metadata field may be returned to this invocation.
    Metadata,
    /// Check live body authority plus the exact pinned revision.
    Range,
    /// Check authority to disclose the item and revision in this result.
    Deliver,
}

/// Item and revision from the checked invocation's space and kind.
#[derive(Clone, Debug)]
pub struct ItemAccess {
    /// Which authority boundary the storage call reached.
    pub check: ItemCheck,
    /// Checked item identity, absent before creation.
    pub item: Option<Uuid>,
    /// Exact revision when the access concerns one.
    pub revision: Option<RevisionRef>,
    /// Metadata key when the check concerns a single field.
    pub field: Option<String>,
}

/// Live authority for one invocation. It is called on the blocking worker
/// that runs the engine, including inside the content transaction for the
/// `Create` and `Write` checks, so it must not reenter content storage there.
pub type ItemAuthority = Arc<dyn Fn(ItemAccess) -> Result<(), Error> + Send + Sync>;

/// Authorized per-invocation resource for one package-owned kind. Call only
/// on a blocking worker. The caller owns live package, operation, space,
/// actor and attempt admission through the supplied authority check.
/// The write callback runs inside the content transaction. It must use the
/// checked authority source and may not reenter content storage. The caller
/// holds any attempt retirement permit through that transaction. The engine
/// owns shared handles only, so an async broker can move it between
/// blocking callbacks without holding a thread across worker think time.
pub struct ItemEngine {
    content: Arc<Content>,
    space: SpaceId,
    actor: Actor,
    package: Arc<ManifestV2>,
    operation: usize,
    kind: usize,
    invocation: RequestId,
    authorize: ItemAuthority,
    fence: Option<(String, Value)>,
    created: HashSet<Uuid>,
    ranges: HashMap<(Uuid, RevisionRef), Instant>,
    calls: usize,
    ticket: Uuid,
    stages: Option<Arc<ItemStages>>,
    discovery: bool,
    captured: Option<CapturedInput>,
    outputs: Option<AttemptOutputs>,
}

impl ItemEngine {
    /// Bind one installed operation and its package-owned kind to a checked
    /// invocation. A target operation must be bound to its declared kind.
    /// Manifest lookup is bounded by its installation limits. No I/O.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        content: Arc<Content>,
        space: SpaceId,
        account: AccountId,
        package: Arc<ManifestV2>,
        operation: &str,
        kind: &str,
        invocation: RequestId,
        authorize: ItemAuthority,
    ) -> Result<Self, Error> {
        let (operation, declared) =
            position(&package, "operations", operation).ok_or_else(denied)?;
        if declared["target"]["kind"]
            .as_str()
            .is_some_and(|target| target != kind)
        {
            return Err(denied());
        }
        let (kind, _) = position(&package, "kinds", kind).ok_or_else(denied)?;
        Ok(Self {
            content,
            space,
            actor: Actor::plugin(account, package.id()),
            package,
            operation,
            kind,
            invocation,
            authorize,
            fence: None,
            created: HashSet::new(),
            ranges: HashMap::new(),
            calls: 0,
            ticket: Uuid::new_v4(),
            stages: None,
            discovery: false,
            captured: None,
            outputs: None,
        })
    }

    /// Require this exact control value, normally the owning package's
    /// activation stamp, at every item commit. The content writer compares
    /// it under the space lock that also serializes activation changes.
    /// No I/O until a write.
    pub fn with_activation(mut self, key: String, value: Value) -> Self {
        self.fence = Some((key, value));
        self
    }

    fn operation(&self) -> &Value {
        &self.package.declaration()["operations"][self.operation]
    }

    fn kind(&self) -> &Value {
        &self.package.declaration()["kinds"][self.kind]
    }

    /// Limit an engine used for attempt discovery to checked read methods.
    /// Opted-in kinds admit only the creator's still-discoverable items.
    /// Ordinary owned-item invocations use their existing authority checks.
    pub fn for_attempt_discovery(mut self) -> Self {
        self.discovery = true;
        self
    }

    /// Validate and dispatch one declared item callback. Caller-safe failures
    /// leave the invocation authority unchanged. Delivery and protected writes
    /// recheck authority at their boundary. Storage uses the space's content writer.
    pub fn call(&mut self, method: &str, operand: &Value) -> Result<Value, Error> {
        if self.discovery && !matches!(method, "item_head" | "item_range" | "item_spans") {
            return Err(denied());
        }
        if self.calls == calternal_plugin_host::MAX_CALLBACKS {
            return Err(Error::unavailable("item callback limit reached"));
        }
        self.calls += 1;
        if !self.operation()["methods"]
            .as_array()
            .is_some_and(|methods| methods.iter().any(|m| m == method))
        {
            return Err(denied());
        }
        ManifestV2::check_item_operand(method, operand)
            .map_err(|_| Error::invalid_input("invalid item operand"))?;
        self.dispatch(method, operand)
    }

    fn dispatch(&mut self, method: &str, operand: &Value) -> Result<Value, Error> {
        match method {
            "item_head" => {
                let input = decode::<Head>(operand)?;
                let item = self.resolve(&input.item)?;
                let head = self.head(item)?;
                self.retain_pin(item, decode(&head["revision"])?)?;
                Ok(head)
            }
            "item_range" => {
                let range: Range = decode(operand)?;
                let item = self.resolve(&range.item)?;
                self.check_range(item, &range.revision)?;
                let (bytes, next) = self.content.read_revision_page(
                    self.space,
                    item,
                    self.kind_id(),
                    &range.revision,
                    range.offset,
                    range.length,
                )?;
                self.check_range(item, &range.revision)?;
                let mut page = json!({"bytes":STANDARD.encode(bytes)});
                if let Some(offset) = next {
                    page["next_offset"] = json!(offset);
                }
                Ok(page)
            }
            "item_spans" => self.spans(decode(operand)?),
            "viewer_state_read" => self.viewer_read(decode(operand)?),
            "viewer_state_write" => self.viewer_write(decode(operand)?, operand),
            "item_create" => {
                let create: Create = decode(operand)?;
                if create.kind != self.kind_id() {
                    return Err(denied());
                }
                self.access(ItemCheck::Create, None, None)?;
                let composed = create
                    .attested
                    .map(|attested| self.compose(attested, Placing::Create, 128 * 1024))
                    .transpose()?;
                let bytes = match &composed {
                    Some(record) => record.bytes.clone(),
                    None => body(create.body.as_deref().unwrap_or_default(), 128 * 1024)?,
                };
                let reference = ItemRef(Uuid::new_v4().to_string());
                let placed = self.write(Write {
                    method: "item_create",
                    operand,
                    id: create.request_id,
                    target: None,
                    reference: &reference,
                    expected: None,
                    placement: CheckedItemPlacement::Create {
                        folders: &create.folders,
                        name: &create.name,
                        reference: &reference,
                    },
                    metadata: create.metadata,
                    body: CheckedItemBody::Replace(&bytes),
                    marker: create.attempt_discovery.as_deref(),
                    stage_begin: None,
                    record: composed.as_ref(),
                    opaque: composed.is_none() && !bytes.is_empty(),
                })?;
                let reference: ItemRef = decode(&placed["item"])?;
                let item = self.resolve(&reference)?;
                self.retain_pin(item, decode(&placed["revision"])?)?;
                if self.operation()["creates"] == self.kind()["id"] {
                    self.created.insert(item);
                }
                Ok(placed)
            }
            "item_save_begin" => self.begin_stage(decode(operand)?, operand),
            "item_save_chunk" => self.pool()?.chunk(self, decode(operand)?),
            "item_save_seal" => self.pool()?.seal(self, decode(operand)?),
            "item_save" => {
                let mut save: Save = decode(operand)?;
                let item = self.resolve(&save.item)?;
                if save.replace_span.is_some() {
                    return Err(Error::unavailable(
                        "recorded span replacement is unavailable",
                    ));
                }
                let composed = save
                    .attested
                    .take()
                    .map(|attested| {
                        self.compose(attested, Placing::Snapshot(&save.item), 128 * 1024)
                    })
                    .transpose()?;
                let mut stage = None;
                if save.staged.is_some() {
                    self.access(ItemCheck::Write, Some(item), Some(&save.expected))?;
                    if let Some(saved) = self.load_receipt(&save.request_id)? {
                        let fp = self.fingerprint("item_save", operand, None)?;
                        return self
                            .replay(saved, "item_save", &fp, Some(item))
                            .and_then(|(id, result)| self.deliver(id, result));
                    }
                    stage = Some(self.pool()?.sealed(self, &save)?);
                }
                let bytes = match &composed {
                    Some(record) => Some(record.bytes.clone()),
                    None => save
                        .body
                        .as_deref()
                        .map(|encoded| body(encoded, 128 * 1024))
                        .transpose()?,
                };
                let opaque = composed.is_none() && (stage.is_some() || bytes.is_some());
                let reply = self.write(Write {
                    method: "item_save",
                    operand,
                    id: save.request_id,
                    target: Some(item),
                    reference: &save.item,
                    expected: Some(&save.expected),
                    placement: CheckedItemPlacement::KeepFolder {
                        name: save.name.as_deref(),
                    },
                    metadata: save.patch.unwrap_or_else(|| json!({})),
                    body: stage
                        .as_ref()
                        .map(|(bytes, _)| bytes.as_slice())
                        .or(bytes.as_deref())
                        .map_or(CheckedItemBody::Keep, CheckedItemBody::Replace),
                    marker: save.attempt_discovery.as_deref(),
                    stage_begin: stage.as_ref().map(|(_, begin)| begin.as_str()),
                    record: composed.as_ref(),
                    opaque,
                });
                if let Ok(result) = &reply {
                    if let Some(handle) = save.staged.as_deref() {
                        self.pool()?.consume(self.ticket, handle);
                    }
                    self.retain_pin(item, decode(&result["revision"])?)?;
                }
                match reply {
                    Err(error)
                        if error
                            .details
                            .as_ref()
                            .is_some_and(|d| d["reason"] == "stale_revision") =>
                    {
                        Ok(json!({"status":409,"code":"head_conflict","current":self.head(item)?}))
                    }
                    reply => reply,
                }
            }
            "item_append" if self.kind()["append_file"] == true => {
                let append: Append = decode(operand)?;
                let composed = append
                    .attested
                    .map(|attested| {
                        self.compose(attested, Placing::Append(&append.item), 64 * 1024)
                    })
                    .transpose()?;
                let bytes = match (&composed, append.bytes.as_deref()) {
                    (Some(record), None) => record.bytes.clone(),
                    (None, Some(encoded)) => body(encoded, 64 * 1024)?,
                    _ => return Err(Error::invalid_input("invalid item operand")),
                };
                let item = self.resolve(&append.item)?;
                let result = self
                    .write(Write {
                        method: "item_append",
                        operand,
                        id: append.request_id,
                        target: Some(item),
                        reference: &append.item,
                        expected: Some(&append.expected_revision),
                        placement: CheckedItemPlacement::KeepFolder { name: None },
                        metadata: json!({}),
                        body: CheckedItemBody::Append(&bytes),
                        marker: append.attempt_discovery.as_deref(),
                        stage_begin: None,
                        record: composed.as_ref(),
                        opaque: composed.is_none(),
                    })
                    .map_err(|error| {
                        if error
                            .details
                            .as_ref()
                            .is_some_and(|d| d["reason"] == "stale_revision")
                        {
                            Error::conflict("item has a newer revision")
                        } else {
                            error
                        }
                    })?;
                self.retain_pin(item, decode(&result["revision"])?)?;
                Ok(result)
            }
            _ => Err(denied()),
        }
    }

    /// Accept the closed created result for this invocation. Only an item this
    /// broker recorded from a checked create or exact replay may drive routing.
    pub fn created(&self, output: &Value) -> Result<Option<ItemRef>, Error> {
        let Some(value) = output.get("created") else {
            return if self.operation().get("creates").is_some() {
                Err(denied())
            } else {
                Ok(None)
            };
        };
        let reference = decode::<ItemRef>(value)?;
        let item = self.resolve(&reference)?;
        if output.as_object().is_none_or(|object| object.len() != 1)
            || self.operation()["creates"] != self.kind()["id"]
            || !self.created.contains(&item)
        {
            return Err(denied());
        }
        self.access(ItemCheck::Deliver, Some(item), None)?;
        Ok(Some(reference))
    }

    fn access(
        &self,
        check: ItemCheck,
        item: Option<Uuid>,
        revision: Option<&RevisionRef>,
    ) -> Result<(), Error> {
        (self.authorize)(ItemAccess {
            check,
            item,
            revision: revision.cloned(),
            field: None,
        })?;
        if self.discovery
            && self.kind()["attempt_discovery"] == "item_policy"
            && !self.content.creator_discoverable(
                self.space,
                item.ok_or_else(denied)?,
                self.kind_id(),
                self.actor.account,
            )?
        {
            return Err(denied());
        }
        Ok(())
    }

    fn resolve(&self, reference: &ItemRef) -> Result<Uuid, Error> {
        self.content
            .resolve_item_ref(self.space, self.kind_id(), reference)
    }

    fn kind_id(&self) -> &str {
        self.kind()["id"].as_str().expect("validated kind")
    }
}

impl Drop for ItemEngine {
    fn drop(&mut self) {
        if let Some(stages) = &self.stages {
            stages.retire(self.ticket);
        }
    }
}

fn position<'a>(package: &'a ManifestV2, section: &str, id: &str) -> Option<(usize, &'a Value)> {
    package.declaration()[section]
        .as_array()?
        .iter()
        .enumerate()
        .find(|(_, entry)| entry["id"] == id)
}

fn denied() -> Error {
    Error::not_found("item not found")
}

#[cfg(test)]
mod tests;
