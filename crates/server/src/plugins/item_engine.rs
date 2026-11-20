//! Typed v2 item callbacks on the existing blocking content writer.
use base64::{Engine, engine::general_purpose::STANDARD};
use calternal_content::{
    CheckedItemBody, CheckedItemPlacement, Content, DiscoveryWrite, FileFormat, Item, Placement,
    Request,
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

mod read;
mod stages;
mod wire;
pub use stages::ItemStages;
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

/// Authorized per-invocation resource for one package-owned kind. Call only
/// on a blocking worker. The caller owns live package, operation, space,
/// actor and attempt admission through the supplied authority check.
/// The write callback runs inside the content transaction. It must use the
/// checked authority source and may not reenter content storage. The caller
/// holds any attempt retirement permit through that transaction.
pub struct ItemEngine<'a> {
    content: &'a Arc<Content>,
    space: SpaceId,
    actor: Actor,
    package: &'a ManifestV2,
    operation: &'a Value,
    kind: &'a Value,
    invocation: &'a RequestId,
    authorize: &'a dyn Fn(ItemAccess) -> Result<(), Error>,
    created: HashSet<Uuid>,
    ranges: HashMap<(Uuid, RevisionRef), Instant>,
    calls: usize,
    ticket: Uuid,
    stages: Option<&'a ItemStages>,
    discovery: bool,
}

impl<'a> ItemEngine<'a> {
    /// Bind one installed operation and its package-owned kind to a checked
    /// invocation. A target operation must be bound to its declared kind.
    /// Manifest lookup is bounded by its installation limits. No I/O.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        content: &'a Arc<Content>,
        space: SpaceId,
        account: AccountId,
        package: &'a ManifestV2,
        operation: &str,
        kind: &str,
        invocation: &'a RequestId,
        authorize: &'a dyn Fn(ItemAccess) -> Result<(), Error>,
    ) -> Result<Self, Error> {
        let operation = declaration(package, "operations", operation).ok_or_else(denied)?;
        if operation["target"]["kind"]
            .as_str()
            .is_some_and(|target| target != kind)
        {
            return Err(denied());
        }
        let kind = declaration(package, "kinds", kind).ok_or_else(denied)?;
        Ok(Self {
            content,
            space,
            actor: Actor::plugin(account, package.id()),
            package,
            operation,
            kind,
            invocation,
            authorize,
            created: HashSet::new(),
            ranges: HashMap::new(),
            calls: 0,
            ticket: Uuid::new_v4(),
            stages: None,
            discovery: false,
        })
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
        if self.discovery && !matches!(method, "item_head" | "item_range") {
            return Err(denied());
        }
        if self.calls == calternal_plugin_host::MAX_CALLBACKS {
            return Err(Error::unavailable("item callback limit reached"));
        }
        self.calls += 1;
        if !self.operation["methods"]
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
            "item_create" => {
                let create: Create = decode(operand)?;
                if create.kind != self.kind_id() {
                    return Err(denied());
                }
                self.access(ItemCheck::Create, None, None)?;
                let bytes = body(create.body.as_deref().unwrap_or_default(), 128 * 1024)?;
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
                })?;
                let reference: ItemRef = decode(&placed["item"])?;
                let item = self.resolve(&reference)?;
                self.retain_pin(item, decode(&placed["revision"])?)?;
                if self.operation["creates"] == self.kind["id"] {
                    self.created.insert(item);
                }
                Ok(placed)
            }
            "item_save_begin" => self.begin_stage(decode(operand)?, operand),
            "item_save_chunk" => self.pool()?.chunk(self, decode(operand)?),
            "item_save_seal" => self.pool()?.seal(self, decode(operand)?),
            "item_save" => {
                let save: Save = decode(operand)?;
                let item = self.resolve(&save.item)?;
                let mut stage = None;
                if save.staged.is_some() {
                    self.access(ItemCheck::Write, Some(item), Some(&save.expected))?;
                    if let Some(saved) = self.load_receipt(&save.request_id)? {
                        let fp = self.fingerprint("item_save", operand)?;
                        return self
                            .replay(saved, "item_save", &fp, Some(item))
                            .and_then(|(id, result)| self.deliver(id, result));
                    }
                    stage = Some(self.pool()?.sealed(self, &save)?);
                }
                let bytes = save
                    .body
                    .as_deref()
                    .map(|encoded| body(encoded, 128 * 1024))
                    .transpose()?;
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
            "item_append" if self.kind["append_file"] == true => {
                let append: Append = decode(operand)?;
                let bytes = body(&append.bytes, 64 * 1024)?;
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
            return if self.operation.get("creates").is_some() {
                Err(denied())
            } else {
                Ok(None)
            };
        };
        let reference = decode::<ItemRef>(value)?;
        let item = self.resolve(&reference)?;
        if output.as_object().is_none_or(|object| object.len() != 1)
            || self.operation["creates"] != self.kind["id"]
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
            && self.kind["attempt_discovery"] == "item_policy"
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
        self.kind["id"].as_str().expect("validated kind")
    }

    fn write(&self, write: Write<'_>) -> Result<Value, Error> {
        let check = if write.target.is_some() {
            ItemCheck::Write
        } else {
            ItemCheck::Create
        };
        let guard = || self.access(check, write.target, write.expected);
        guard()?;
        if self.kind.get("extractors").is_some() {
            return Err(Error::unavailable("item extractors are unavailable"));
        }
        let fingerprint = self.fingerprint(write.method, write.operand)?;
        let key = receipt_key(&write.id);
        let request = Request {
            account: self.actor.account,
            via: self.actor.via.as_ref(),
            id: &key,
            operation: write.method,
            fingerprint: &fingerprint,
        };
        let encode = |item: &Item| {
            let receipt = ReceiptRef(write.id.0.clone());
            let revision = RevisionRef(item.revision.clone());
            let result = if write.method == "item_append" {
                json!({"revision":revision,"end_offset":item.bytes.len(),"receipt":receipt})
            } else {
                json!({"item":write.reference, "revision":revision, "locator":locator(self.space,item.id),"receipt":receipt})
            };
            match write.stage_begin {
                None => result,
                Some(begin) => json!({"stage_begin":begin,"result":result}),
            }
        };
        let replay = || -> Result<Option<(Uuid, Value)>, Error> {
            let Some(saved) = self.load_receipt(&write.id)? else {
                return Ok(None);
            };
            guard()?;
            self.replay(saved, write.method, &fingerprint, write.target)
                .map(Some)
        };
        let (item, result) = if let Some(replay) = replay()? {
            replay
        } else {
            let discovery = if self.kind["attempt_discovery"] == "item_policy" {
                DiscoveryWrite::Checked {
                    allow_creator_only: write.marker == Some("creator_only"),
                    protect: write.marker == Some("consent_only"),
                }
            } else {
                if write.marker.is_some() {
                    return Err(denied());
                }
                DiscoveryWrite::Unmanaged
            };
            match self.content.write_checked_item(
                self.space,
                self.kind_id(),
                write.target,
                write.expected,
                write.placement,
                write.metadata,
                write.body,
                discovery,
                request,
                guard,
                |value| {
                    self.package
                        .check_item_metadata(self.kind_id(), value)
                        .map_err(|_| Error::invalid_input("invalid item metadata"))
                },
                encode,
            ) {
                Ok(item) => (item.id, wire_result(encode(&item))),
                Err(error) => replay()?.ok_or(error)?,
            }
        };
        self.deliver(item, result)
    }

    fn fingerprint(&self, method: &str, operand: &Value) -> Result<String, Error> {
        let bytes = serde_json::to_vec(&(
            self.package.id(),
            &self.package.declaration()["worker"]["sha256"],
            &self.operation["id"],
            self.invocation,
            method,
            operand,
        ))
        .map_err(|_| Error::invalid_input("invalid item operand"))?;
        Ok(crate::store::sha256_hex(&bytes))
    }

    fn load_receipt(&self, id: &RequestId) -> Result<Option<calternal_content::Receipt>, Error> {
        self.content
            .receipt(self.space, self.actor.account, &receipt_key(id))
    }

    fn replay(
        &self,
        saved: calternal_content::Receipt,
        method: &str,
        fingerprint: &str,
        target: Option<Uuid>,
    ) -> Result<(Uuid, Value), Error> {
        if saved.operation != method || saved.fingerprint != fingerprint {
            return Err(Error::request_reused());
        }
        let item = saved.item.ok_or_else(denied)?;
        if target.is_some_and(|target| target != item) {
            return Err(denied());
        }
        Ok((item, wire_result(saved.result)))
    }

    fn deliver(&self, item: Uuid, result: Value) -> Result<Value, Error> {
        let revision = decode::<RevisionRef>(&result["revision"])?;
        self.access(ItemCheck::Deliver, Some(item), Some(&revision))?;
        Ok(result)
    }

    /// Provision the installed kind for this space at activation on a
    /// blocking worker. Existing equal placement is reused. This reads the
    /// registration once and commits it only on first activation.
    pub fn provision_kind(&self) -> Result<(), Error> {
        let placement = match decode::<ManifestPlacement>(&self.kind["storage"])? {
            ManifestPlacement::Visible { folder } => Placement::Visible { folder },
            ManifestPlacement::VisibleTree {
                folder,
                subdirectories,
            } => Placement::VisibleTree {
                folder,
                subdirectories,
            },
            ManifestPlacement::Hidden { .. } => Placement::Hidden {
                plugin_id: self.package.id().to_owned(),
            },
        };
        let format = if self.kind["claimed_text"] == false {
            FileFormat::Opaque {
                extension: decode(&self.kind["extension"])?,
            }
        } else {
            FileFormat::Markdown
        };
        self.content
            .register_kind(self.space, self.kind_id(), placement, format)?;
        if self.kind["live_room"] == "yjs-v1" {
            self.content
                .enable_live_editing(self.space, self.kind_id())?;
        }
        Ok(())
    }
}

impl Drop for ItemEngine<'_> {
    fn drop(&mut self) {
        if let Some(stages) = self.stages {
            stages.retire(self.ticket);
        }
    }
}

fn receipt_key(id: &RequestId) -> String {
    format!("plugin/v2/{}", crate::store::sha256_hex(id.0.as_bytes()))
}

fn wire_result(mut result: Value) -> Value {
    if result.get("stage_begin").is_some() {
        result["result"].take()
    } else {
        result
    }
}

fn declaration<'a>(package: &'a ManifestV2, section: &str, id: &str) -> Option<&'a Value> {
    package.declaration()[section]
        .as_array()?
        .iter()
        .find(|entry| entry["id"] == id)
}

fn denied() -> Error {
    Error::not_found("item not found")
}

#[cfg(test)]
mod tests;
