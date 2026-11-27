//! Dispatches installed code through the same authorized route as session calls.
use crate::declarations::{Plugin, apply_fields};
use crate::space::Space;
use crate::{Call, Host, SlotSelection, internal};
use calternal_format::Operation;
use calternal_plugin_host::operations::{
    Delivery, Event, EventKind, Method, Operations, Person, Registrations,
};
use calternal_types::{Actor, Error, Result, SpaceId, Via};
use rusqlite::OptionalExtension;
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use uuid::Uuid;

pub(crate) struct InstalledCode {
    source: Vec<u8>,
    pub(crate) registration: Registrations,
    pub(crate) endpoint: Option<Arc<Operations>>,
}

impl InstalledCode {
    pub(crate) fn new(source: Vec<u8>, registration: Registrations) -> Self {
        Self {
            source,
            registration,
            endpoint: None,
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct Chain {
    pub(crate) plugins: Vec<String>,
    pub(crate) request: Option<String>,
    pub(crate) session_item: Option<Uuid>,
    pub(crate) run: Option<crate::runs::RunScopeGrant>,
    selection: Option<SlotGrant>,
}

#[derive(Clone)]
struct SlotGrant {
    owner: String,
    operation: String,
}

struct Invocation {
    plugin: String,
    selector: String,
    item: Option<Uuid>,
}

struct Target<'a> {
    plugin: &'a str,
    kind: &'a str,
    operation: &'a Operation,
}

impl Space {
    fn target(&self, name: &str) -> Result<Target<'_>> {
        let denied = || Error::forbidden("The coded operation is undeclared or unavailable.");
        let (plugin, rest) = name.split_once('.').ok_or_else(denied)?;
        let installed = self.plugins.get(plugin).ok_or_else(denied)?;
        if let Some((kind, name)) = rest.split_once('.') {
            let (kind, definition) = installed.kinds.get_key_value(kind).ok_or_else(denied)?;
            let operation = definition
                .operations
                .get(name)
                .filter(|op| op.is_coded())
                .ok_or_else(denied)?;
            let plugin = &installed.manifest.id;
            return Ok(Target {
                plugin,
                kind,
                operation,
            });
        }
        let mut declarations = installed.kinds.iter().filter_map(|(kind, definition)| {
            Some((
                kind,
                definition.operations.get(rest).filter(|op| op.is_coded())?,
            ))
        });
        let (kind, operation) = declarations.next().ok_or_else(denied)?;
        if declarations.next().is_some() {
            return Err(denied());
        }
        Ok(Target {
            plugin: &installed.manifest.id,
            kind,
            operation,
        })
    }

    fn select_filler(&self, owner: &str, selection: &SlotSelection) -> Result<SlotGrant> {
        let denied =
            || Error::forbidden("That plugin does not fill this person-picked operation slot.");
        self.plugins
            .get(owner)
            .and_then(|plugin| plugin.manifest.slots.get(&selection.slot))
            .filter(|slot| !slot.picked_by_space)
            .ok_or_else(denied)?;
        let operation = self
            .plugins
            .get(&selection.filler)
            .and_then(|plugin| plugin.manifest.operation_fill(owner, &selection.slot))
            .ok_or_else(denied)?;
        let target = self.target(&format!("{}.{operation}", selection.filler))?;
        Ok(SlotGrant {
            owner: owner.into(),
            operation: format!(
                "{}.{}.{}",
                target.plugin, target.kind, target.operation.name
            ),
        })
    }

    fn validate_designations(
        &self,
        actor: &Actor,
        value: &Value,
        scope: Option<Uuid>,
    ) -> Result<()> {
        match value {
            Value::Object(object) => {
                for value in object.values() {
                    self.validate_designations(actor, value, scope)?;
                }
            }
            Value::Array(values) => {
                for value in values {
                    self.validate_designations(actor, value, scope)?;
                }
            }
            Value::String(value) => {
                if let Ok(id) = Uuid::parse_str(value) {
                    if let Some(row) = self.rows.get(&id) {
                        if scope.is_some_and(|item| item != id) {
                            return Err(Error::forbidden(
                                "The item is outside this run's session.",
                            ));
                        }
                        self.permits(actor, &row.head.kind, false)?;
                    }
                } else if let Some(anchor) = value
                    .split_once("](<")
                    .and_then(|(_, path)| {
                        path.strip_suffix(">)")?
                            .rsplit_once("#^")
                            .map(|(_, anchor)| anchor)
                    })
                    .filter(|anchor| crate::records::valid_anchor(anchor))
                {
                    let id: Option<String> = self
                        .db
                        .query_row(
                            "SELECT item FROM records WHERE anchor=?1 AND (?2 IS NULL OR item=?2)",
                            rusqlite::params![anchor, scope.map(|item| item.to_string())],
                            |row| row.get(0),
                        )
                        .optional()
                        .map_err(internal)?;
                    let id = id.and_then(|id| Uuid::parse_str(&id).ok()).ok_or_else(|| {
                        if scope.is_some() {
                            Error::forbidden("The record link is outside this run's session.")
                        } else {
                            Error::not_found("Record link was not found.")
                        }
                    })?;
                    self.permits(actor, &self.row(id)?.head.kind, false)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn redact_rows(&self, actor: &Actor, value: &mut Value) {
        match value {
            Value::Object(object) => {
                if let Some(id) = object.get("id").and_then(Value::as_str)
                    && let Ok(uuid) = Uuid::parse_str(id)
                    && let Some(row) = self.rows.get(&uuid)
                    && self.permits(actor, &row.head.kind, false).is_err()
                {
                    *value = Value::String(id.to_owned());
                } else {
                    for value in object.values_mut() {
                        self.redact_rows(actor, value);
                    }
                }
            }
            Value::Array(values) => {
                for value in values {
                    self.redact_rows(actor, value);
                }
            }
            _ => {}
        }
    }

    pub(crate) fn coded(&self, name: &str) -> Result<()> {
        self.target(name).map(|_| ())
    }

    fn invocation(&self, actor: &Actor, name: &str, input: &Value) -> Result<Invocation> {
        let target = self.target(name)?;
        let selector = format!("{}.{}", target.kind, target.operation.name);
        let kind = format!("{}.{}", target.plugin, target.kind);
        if target.operation.condition.is_some() {
            return Err(Error::forbidden(
                "Conditional coded operations are outside this host slice.",
            ));
        }
        if !target.operation.inputs.is_empty() {
            let fields = serde_json::from_value(input.clone())
                .map_err(|_| Error::invalid_input("Expected the operation's declared inputs."))?;
            let mut normalized = serde_json::Map::new();
            apply_fields(&target.operation.inputs, &mut normalized, fields, false)?;
        }
        let item = if target.operation.inputs.contains_key("item") {
            let id = input
                .get("item")
                .and_then(Value::as_str)
                .and_then(|raw| Uuid::parse_str(raw).ok())
                .ok_or_else(|| Error::invalid_input("Supply the target item UUID under item."))?;
            let row = self.row(id)?;
            if row.head.kind != kind {
                return Err(Error::not_found("Item was not found."));
            }
            self.permits(actor, &row.head.kind, false)?;
            Some(id)
        } else {
            None
        };
        Ok(Invocation {
            plugin: target.plugin.into(),
            selector,
            item,
        })
    }
}

pub(crate) fn validate_registrations(
    plugin: &Plugin,
    installed: &BTreeMap<String, Plugin>,
    registration: &Registrations,
) -> Result<()> {
    let expected = plugin
        .kinds
        .iter()
        .flat_map(|(kind, definition)| {
            definition
                .operations
                .values()
                .filter(|op| op.is_coded())
                .map(move |op| format!("{kind}.{}", op.name))
        })
        .collect::<std::collections::BTreeSet<_>>();
    let actual = registration
        .calls
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    if expected != actual || actual.len() != registration.calls.len() {
        return Err(Error::invalid_input(
            "Worker exports do not match the declared coded operations.",
        ));
    }
    let mut seen = std::collections::BTreeSet::new();
    for change in &registration.changes {
        let Some((target, kind)) = crate::reactions::resolve_selector(plugin, installed, change)
        else {
            return Err(Error::invalid_input(
                "Worker change selector is not a readable kind or collection.",
            ));
        };
        if !seen.insert(target.unwrap_or(kind)) {
            return Err(Error::invalid_input(
                "Worker change selector is registered more than once.",
            ));
        }
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Nested {
    operation: String,
    input: Value,
}

type Outcome = Pin<Box<dyn Future<Output = Result<Value>> + Send>>;

impl Host {
    pub(crate) fn dispatch(
        self: Arc<Self>,
        actor: Actor,
        space: SpaceId,
        call: Call,
        chain: Chain,
    ) -> Outcome {
        Box::pin(async move {
            if let Some(grant) = &chain.run {
                grant.check(&actor, space, &call)?;
            }
            match call.call.as_str() {
                "s.run.send" | "s.run.stop" if call.slot_selection.is_none() => {
                    if call.call == "s.run.send" {
                        self.run_send(actor, space, call.input, call.idempotency_key)
                            .await
                    } else {
                        self.run_stop(actor, space, call.input).await
                    }
                }
                "s.write" if self.workers.is_some() && call.slot_selection.is_none() => {
                    self.write_and_notify(actor, space, call, chain).await
                }
                "s.declarations" | "s.query" | "s.read" | "s.write" | "s.state" | "s.me"
                | "s.space" | "index.rebuild" => {
                    tokio::task::spawn_blocking(move || self.call_scoped(actor, space, call))
                        .await
                        .map_err(internal)?
                }
                _ => self.dispatch_code(actor, space, call, chain).await,
            }
        })
    }

    async fn dispatch_code(
        self: Arc<Self>,
        actor: Actor,
        space: SpaceId,
        mut call: Call,
        mut chain: Chain,
    ) -> Result<Value> {
        if let Some(grant) = &chain.run {
            grant.check(&actor, space, &call)?;
        }
        let host = self.clone();
        let who = actor.clone();
        let caller_grant = chain.selection.take();
        let scope = chain.run.map(|grant| grant.item);
        let (invocation, endpoint, key, input, selection) =
            tokio::task::spawn_blocking(move || {
                host.with_space(&who, space, |storage| {
                    let invocation = storage.invocation(&who, &call.call, &call.input)?;
                    if scope.is_some() && scope != invocation.item {
                        return Err(Error::forbidden(
                            "The operation is outside this run's session.",
                        ));
                    }
                    if let Some(Via::Plugin { plugin } | Via::Run { plugin, .. }) = &who.via {
                        let canonical = format!("{}.{}", invocation.plugin, invocation.selector);
                        let declared = storage.plugins.get(plugin).is_some_and(|declaration| {
                            declaration
                                .manifest
                                .calls
                                .iter()
                                .any(|name| name == &call.call || name == &canonical)
                        });
                        let selected = caller_grant.as_ref().is_some_and(|grant| {
                            grant.owner == *plugin && grant.operation == canonical
                        });
                        if !declared && !selected {
                            return Err(Error::forbidden(
                                "The operation was not declared or selected by the person.",
                            ));
                        }
                        storage.validate_designations(&who, &call.input, scope)?;
                    }
                    let selection = call
                        .slot_selection
                        .as_ref()
                        .map(|selection| {
                            if who.via.is_some() {
                                return Err(Error::forbidden(
                                    "Only a person can select a slot filler.",
                                ));
                            }
                            storage.select_filler(&invocation.plugin, selection)
                        })
                        .transpose()?;
                    let original = call
                        .idempotency_key
                        .take()
                        .unwrap_or_else(|| Uuid::now_v7().to_string());
                    if original.is_empty()
                        || original.len() > 128
                        || original.chars().any(char::is_control)
                    {
                        return Err(Error::invalid_input("Invalid idempotency key."));
                    }
                    let key = blake3::Hash::from_bytes(
                        storage.hash(
                            &serde_json::to_vec(&(
                                &who,
                                &invocation.plugin,
                                &invocation.selector,
                                &original,
                            ))
                            .map_err(internal)?,
                        ),
                    )
                    .to_hex()
                    .to_string();
                    let endpoint = host.endpoint(storage, &invocation.plugin)?;
                    Ok((invocation, endpoint, key, call.input, selection))
                })
            })
            .await
            .map_err(internal)??;
        if chain.plugins.len() >= 8 || chain.plugins.contains(&invocation.plugin) {
            return Err(Error::forbidden(
                "The coded operation would repeat or exceed call depth.",
            ));
        }
        let selected_operation = selection.as_ref().map(|grant| grant.operation.clone());
        if chain.session_item.is_none() {
            chain.session_item = invocation.item;
        }
        chain.selection = selection;
        chain.plugins.push(invocation.plugin.clone());
        let request = chain.request.get_or_insert_with(|| key.clone()).clone();
        self.deliver(
            endpoint,
            match &chain.run {
                Some(grant) => Actor::run(actor.account, &invocation.plugin, grant.id),
                None => Actor::plugin(actor.account, &invocation.plugin),
            },
            chain,
            Delivery {
                selector: invocation.selector,
                input,
                me: Person {
                    id: actor.account,
                    role: "owner".into(),
                },
                space,
                event: Event {
                    kind: EventKind::Call,
                    key,
                    item: invocation.item,
                    before: None,
                    request: Some(request),
                    selected_operation,
                },
            },
        )
        .await
    }

    pub(crate) fn endpoint(&self, storage: &mut Space, plugin: &str) -> Result<Arc<Operations>> {
        let unavailable =
            || Error::unavailable("Install this coded plugin with an isolated worker configured.");
        let runtime = self.workers.as_ref().ok_or_else(unavailable)?;
        let code = storage.code.get_mut(plugin).ok_or_else(unavailable)?;
        if code.endpoint.is_none() {
            code.endpoint = Some(runtime.endpoint(storage.id, &code.source, &code.registration)?);
        }
        Ok(code
            .endpoint
            .as_ref()
            .expect("initialized worker endpoint")
            .clone())
    }

    pub(crate) async fn deliver(
        self: Arc<Self>,
        endpoint: Arc<Operations>,
        actor: Actor,
        chain: Chain,
        delivery: Delivery,
    ) -> Result<Value> {
        let space = delivery.space;
        endpoint
            .invoke(delivery, move |callback| {
                let (host, actor, mut chain) = (self.clone(), actor.clone(), chain.clone());
                async move {
                    let name = match callback.method {
                        Method::Query => "s.query",
                        Method::Read => "s.read",
                        Method::Write => "s.write",
                        Method::State => "s.state",
                        Method::Call => {
                            let nested: Nested = crate::decode(callback.input)?;
                            return host
                                .dispatch_code_call(
                                    actor,
                                    space,
                                    nested,
                                    callback.idempotency_key,
                                    chain,
                                )
                                .await;
                        }
                        Method::Run | Method::RunSend | Method::RunStop if chain.run.is_some() => {
                            return Err(Error::forbidden("A run cannot manage other runs."));
                        }
                        Method::Run => {
                            return host
                                .run_start(
                                    actor,
                                    space,
                                    callback.input,
                                    callback.idempotency_key,
                                    chain.request.clone(),
                                    chain.session_item,
                                )
                                .await;
                        }
                        Method::RunSend => {
                            return host
                                .run_send(actor, space, callback.input, callback.idempotency_key)
                                .await;
                        }
                        Method::RunStop => {
                            return host.run_stop(actor, space, callback.input).await;
                        }
                    };
                    chain.selection = None;
                    host.dispatch(
                        actor,
                        space,
                        Call {
                            call: name.into(),
                            input: callback.input,
                            idempotency_key: callback.idempotency_key,
                            slot_selection: None,
                        },
                        chain,
                    )
                    .await
                }
            })
            .await
    }

    fn dispatch_code_call(
        self: Arc<Self>,
        actor: Actor,
        space: SpaceId,
        nested: Nested,
        key: Option<String>,
        chain: Chain,
    ) -> Outcome {
        Box::pin(async move {
            let mut result = self
                .clone()
                .dispatch_code(
                    actor.clone(),
                    space,
                    Call {
                        call: nested.operation,
                        input: nested.input,
                        idempotency_key: key,
                        slot_selection: None,
                    },
                    chain,
                )
                .await?;
            tokio::task::spawn_blocking(move || {
                self.with_space(&actor, space, |storage| {
                    storage.redact_rows(&actor, &mut result);
                    Ok(result)
                })
            })
            .await
            .map_err(internal)?
        })
    }
}
