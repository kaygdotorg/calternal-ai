//! Post-commit observations, assembled while the changed row is still current.
use crate::declarations::Plugin;
use crate::space::{Row, Space};
use crate::worker_calls::Chain;
use crate::{Call, Host, identity, internal};
use calternal_format::Source;
use calternal_plugin_host::operations::{Delivery, Event, EventKind, Person};
use calternal_types::{Actor, Result, SpaceId};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use uuid::Uuid;

// A qualified kind is the default selector. Prefix a collection with
// collection: if it shares a name with a kind.
pub(crate) fn resolve_selector(
    plugin: &Plugin,
    installed: &BTreeMap<String, Plugin>,
    selector: &str,
) -> Option<(Option<String>, String)> {
    let explicit_collection = selector.starts_with("collection:");
    let selector = selector.strip_prefix("collection:").unwrap_or(selector);
    let id = &plugin.manifest.id;
    let (owner, name) = selector.split_once('.').unwrap_or((id, selector));
    let source = if owner == id {
        plugin
    } else {
        installed.get(owner)?
    };
    let kind = if !explicit_collection && source.kinds.contains_key(name) {
        format!("{owner}.{name}")
    } else {
        let collection = source.collections.get(name)?;
        let [Source::Kind(kind)] = collection.sources.as_slice() else {
            return None;
        };
        let kind = if kind.contains('.') {
            kind.to_owned()
        } else {
            format!("{owner}.{kind}")
        };
        if collection.share_source == calternal_format::ShareSource::Shared
            || !(kind.starts_with(&format!("{id}.")) || plugin.manifest.reads.contains(&kind))
        {
            return None;
        }
        return Some((Some(format!("{owner}.{name}")), kind));
    };
    (owner == id || plugin.manifest.reads.contains(&kind)).then_some((None, kind))
}

struct Listener {
    plugin: String,
    selector: String,
    collection: Option<String>,
    kind: String,
}

struct Observation {
    plugin: String,
    selector: String,
    item: Uuid,
    key: String,
    before: Option<Value>,
    after: Value,
}

impl Host {
    pub(crate) async fn write_and_notify(
        self: Arc<Self>,
        actor: Actor,
        space: SpaceId,
        call: Call,
        chain: Chain,
    ) -> Result<Value> {
        let host = self.clone();
        let who = actor.clone();
        let ancestry = chain.plugins.clone();
        let (result, events) = tokio::task::spawn_blocking(move || {
            host.with_space(&who, space, |storage| {
                storage.observe_write(&who, call, &ancestry)
            })
        })
        .await
        .map_err(internal)??;
        self.live.notify(space);
        for event in events {
            let host = self.clone();
            let watcher = Actor::plugin(actor.account, &event.plugin);
            let checked = watcher.clone();
            let plugin = event.plugin.clone();
            let endpoint = tokio::task::spawn_blocking(move || {
                host.with_space(&checked, space, |storage| host.endpoint(storage, &plugin))
            })
            .await
            .map_err(internal)?;
            let delivery = async {
                let endpoint = endpoint?;
                let mut next = chain.clone();
                next.plugins.push(event.plugin);
                let request = next
                    .request
                    .get_or_insert_with(|| event.key.clone())
                    .clone();
                self.clone()
                    .deliver(
                        endpoint,
                        watcher,
                        next,
                        Delivery {
                            selector: event.selector,
                            input: event.after,
                            me: Person {
                                id: actor.account,
                                role: "owner".into(),
                            },
                            space,
                            event: Event {
                                kind: EventKind::Change,
                                key: event.key,
                                item: Some(event.item),
                                before: event.before,
                                request: Some(request),
                                selected_operation: None,
                            },
                        },
                    )
                    .await
            }
            .await;
            if let Err(error) = delivery {
                eprintln!("calternal-host: change delivery failed ({:?})", error.code);
            }
        }
        Ok(result)
    }
}

impl Space {
    fn listeners(&self, actor: &Actor, ancestry: &[String]) -> Vec<Listener> {
        if ancestry.len() >= 8 {
            return Vec::new();
        }
        let caller = identity(actor).0;
        self.code
            .iter()
            .filter(|(plugin, _)| plugin.as_str() != caller && !ancestry.contains(plugin))
            .flat_map(|(id, installed)| {
                installed
                    .registration
                    .changes
                    .iter()
                    .filter_map(move |selector| {
                        let (collection, kind) =
                            resolve_selector(&self.plugins[id], &self.plugins, selector)?;
                        Some(Listener {
                            plugin: id.clone(),
                            selector: selector.clone(),
                            collection,
                            kind,
                        })
                    })
            })
            .collect()
    }

    fn observe_write(
        &mut self,
        actor: &Actor,
        call: Call,
        ancestry: &[String],
    ) -> Result<(Value, Vec<Observation>)> {
        let listeners = self.listeners(actor, ancestry);
        if listeners.is_empty() || call.input.get("preview") == Some(&Value::Bool(true)) {
            return self.call(actor, call).map(|value| (value, Vec::new()));
        }
        let items = call.input["changes"]
            .as_array()
            .into_iter()
            .flatten()
            .take(1_000)
            .filter_map(|change| Uuid::parse_str(change.get("item")?.as_str()?).ok())
            .collect::<BTreeSet<_>>();
        let before = items
            .into_iter()
            .filter_map(|id| {
                let row = self.rows.get(&id)?;
                let snapshot = snapshot(row);
                let membership = listeners
                    .iter()
                    .map(|listener| self.matches(listener, row))
                    .collect::<Vec<_>>();
                Some((id, (snapshot, membership)))
            })
            .collect::<BTreeMap<_, _>>();
        let key = call
            .idempotency_key
            .clone()
            .unwrap_or_else(|| Uuid::now_v7().to_string());
        let result = self.call(actor, call)?;
        let ids = result["changes"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|change| Uuid::parse_str(change.get("id")?.as_str()?).ok())
            .collect::<BTreeSet<_>>();
        let mut events = Vec::new();
        for id in ids {
            let previous = before.get(&id);
            let current = self.rows.get(&id);
            let after = current.map(snapshot).unwrap_or_default();
            if previous.map(|(value, _)| value) == Some(&after) {
                continue;
            }
            for (index, listener) in listeners.iter().enumerate() {
                let before_member = previous.is_some_and(|(_, membership)| membership[index]);
                let after_member = current.is_some_and(|row| self.matches(listener, row));
                if before_member || after_member {
                    let key = blake3::Hash::from_bytes(
                        self.hash(
                            &serde_json::to_vec(&(
                                actor,
                                &key,
                                id,
                                &listener.plugin,
                                &listener.selector,
                            ))
                            .map_err(internal)?,
                        ),
                    )
                    .to_hex()
                    .to_string();
                    events.push(Observation {
                        plugin: listener.plugin.clone(),
                        selector: listener.selector.clone(),
                        item: id,
                        key,
                        before: previous.map(|(value, _)| value.clone()),
                        after: after.clone(),
                    });
                }
            }
        }
        Ok((result, events))
    }

    fn matches(&self, listener: &Listener, row: &Row) -> bool {
        if listener.kind != row.head.kind {
            return false;
        }
        match &listener.collection {
            None => true,
            Some(id) => self
                .collection(id)
                .and_then(|collection| crate::query::contains(self, collection, row))
                .unwrap_or_else(|error| {
                    let _ = internal(error);
                    false
                }),
        }
    }
}

fn snapshot(row: &Row) -> Value {
    json!({"id":row.head.id,"kind":row.head.kind,"fields":row.fields,"version":row.head.version})
}
