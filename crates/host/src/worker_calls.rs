//! Dispatches installed code through the same authorized route as session calls.
use crate::declarations::{Plugin, apply_fields};
use crate::space::Space;
use crate::{Call, Host, internal};
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

    fn validate_designations(&self, actor: &Actor, value: &Value) -> Result<()> {
        match value {
            Value::Object(object) => {
                for value in object.values() {
                    self.validate_designations(actor, value)?;
                }
            }
            Value::Array(values) => {
                for value in values {
                    self.validate_designations(actor, value)?;
                }
            }
            Value::String(value) => {
                if let Ok(id) = Uuid::parse_str(value) {
                    if let Some(row) = self.rows.get(&id) {
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
                            "SELECT item FROM records WHERE anchor=?1",
                            [anchor],
                            |row| row.get(0),
                        )
                        .optional()
                        .map_err(internal)?;
                    let id = id
                        .and_then(|id| Uuid::parse_str(&id).ok())
                        .ok_or_else(|| Error::not_found("Record link was not found."))?;
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
        let item = input.as_object().and_then(|inputs| {
            inputs
                .values()
                .filter_map(|value| Uuid::parse_str(value.as_str()?).ok())
                .find(|id| self.rows.get(id).is_some_and(|row| row.head.kind == kind))
        });
        if let Some(id) = item {
            self.permits(
                &Actor::plugin(actor.account, target.plugin),
                &self.row(id)?.head.kind,
                false,
            )?;
        }
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
            match call.call.as_str() {
                "s.write" if self.workers.is_some() => {
                    self.write_and_notify(actor, space, call, chain).await
                }
                "s.query" | "s.read" | "s.write" | "s.state" | "s.me" | "s.space"
                | "index.rebuild" => {
                    tokio::task::spawn_blocking(move || self.call(actor, space, call))
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
        let host = self.clone();
        let who = actor.clone();
        let (invocation, endpoint, key, input) = tokio::task::spawn_blocking(move || {
            host.with_space(&who, space, |storage| {
                let invocation = storage.invocation(&who, &call.call, &call.input)?;
                if let Some(Via::Plugin { plugin }) = &who.via {
                    let canonical = format!("{}.{}", invocation.plugin, invocation.selector);
                    let declared = storage.plugins.get(plugin).is_some_and(|declaration| {
                        declaration
                            .manifest
                            .calls
                            .iter()
                            .any(|name| name == &call.call || name == &canonical)
                    });
                    if !declared {
                        return Err(Error::forbidden(
                            "The caller did not declare this operation.",
                        ));
                    }
                    storage.validate_designations(&who, &call.input)?;
                }
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
                Ok((invocation, endpoint, key, call.input))
            })
        })
        .await
        .map_err(internal)??;
        if chain.plugins.len() >= 8 || chain.plugins.contains(&invocation.plugin) {
            return Err(Error::forbidden(
                "The coded operation would repeat or exceed call depth.",
            ));
        }
        chain.plugins.push(invocation.plugin.clone());
        let request = chain.request.get_or_insert_with(|| key.clone()).clone();
        self.deliver(
            endpoint,
            Actor::plugin(actor.account, &invocation.plugin),
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
                let (host, actor, chain) = (self.clone(), actor.clone(), chain.clone());
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
                        Method::Run | Method::RunSend | Method::RunStop => {
                            return Err(Error::forbidden(
                                "No scope-aware run adapter is attached.",
                            ));
                        }
                    };
                    host.dispatch(
                        actor,
                        space,
                        Call {
                            call: name.into(),
                            input: callback.input,
                            idempotency_key: callback.idempotency_key,
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
