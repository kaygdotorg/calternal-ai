//! Post-commit observations, assembled while the changed row is still current.

mod outbox;
use crate::declarations::Plugin;
use crate::maintenance::Slot;
use crate::space::{Row, Space};
use crate::worker_calls::Chain;
use crate::{Call, Host, identity, internal};
use calternal_format::Source;
use calternal_plugin_host::operations::{Delivery, Event, EventKind, Person};
use calternal_types::{Actor, Error, ErrorCode, Result, SpaceId};
pub(crate) use outbox::expire_pending;
use outbox::{Claim, OUTBOX_BYTES, OUTBOX_REQUESTS, has_outbox};
use rusqlite::params;
use serde::{Deserialize, Serialize};
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

#[derive(Serialize, Deserialize)]
struct Listener {
    plugin: String,
    selector: String,
    collection: Option<String>,
    kind: String,
}

#[derive(Serialize, Deserialize)]
struct Observation {
    plugin: String,
    selector: String,
    item: Uuid,
    key: String,
    before: Option<Value>,
    after: Value,
}

#[derive(Serialize, Deserialize)]
struct Before {
    row: Value,
    matches: Vec<bool>,
    bytes: usize,
}

#[derive(Serialize, Deserialize)]
struct Prepared {
    actor: Actor,
    key: String,
    before: BTreeMap<Uuid, Before>,
    listeners: Vec<Listener>,
}

#[derive(PartialEq, Eq)]
enum Disposition {
    Committed,
    Replayed,
}

impl Host {
    pub(crate) async fn write_and_notify(
        self: Arc<Self>,
        actor: Actor,
        space: SpaceId,
        call: Call,
        chain: Chain,
    ) -> Result<Value> {
        let preview = call.input.get("preview") == Some(&Value::Bool(true));
        let host = self.clone();
        let who = actor.clone();
        let ancestry = chain.plugins.clone();
        let session = chain.session.clone();
        let (result, batch) = tokio::task::spawn_blocking(move || {
            host.with_space_slot(&who, space, |storage, slot| {
                let _admission = host.admit_session_blocking(session.as_ref())?;
                let (result, disposition, batch) =
                    storage.observe_write(&who, call, &ancestry, slot)?;
                if !preview && disposition == Disposition::Committed {
                    storage.live.notify(None);
                }
                Ok((result, batch))
            })
        })
        .await
        .map_err(internal)??;
        let Some((claim, events)) = batch else {
            return Ok(result);
        };
        for event in events {
            let event_key = event.key.clone();
            let host = self.clone();
            let watcher = match &chain.run {
                Some(grant) => Actor::run(actor.account, &event.plugin, grant.id),
                None => Actor::plugin(actor.account, &event.plugin),
            };
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
            match delivery {
                Ok(_) => {
                    let host = self.clone();
                    let actor = actor.clone();
                    let gesture = claim.id;
                    tokio::task::spawn_blocking(move || {
                        host.with_space(&actor, space, |storage| {
                            storage.ack_observation(&gesture, &event_key)
                        })
                    })
                    .await
                    .map_err(internal)??;
                }
                Err(error) => {
                    eprintln!("calternal-host: change delivery failed ({:?})", error.code);
                }
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
        mut call: Call,
        ancestry: &[String],
        slot: &Slot,
    ) -> Result<(Value, Disposition, Option<(Claim, Vec<Observation>)>)> {
        if call.input.get("preview") == Some(&Value::Bool(true)) {
            return self
                .call(actor, call)
                .map(|value| (value, Disposition::Committed, None));
        }
        let now = calternal_types::now_ms();
        let cutoff = now - self.retention_ms;
        let listeners = self.listeners(actor, ancestry);
        if !has_outbox(&self.db)? {
            if listeners.is_empty() {
                return self
                    .call(actor, call)
                    .map(|value| (value, Disposition::Committed, None));
            }
            self.initialize_outbox()?;
        }
        // Finish a committed preparation while its post-write row is still
        // current, including on the first new write after a restart.
        self.recover_observations(cutoff)?;
        let key = call
            .idempotency_key
            .get_or_insert_with(|| Uuid::now_v7().to_string())
            .clone();
        let writer = format!("write:{}", serde_json::to_string(actor).map_err(internal)?);
        let gesture = self.hash(key.as_bytes());
        let outbox_key = self.hash(&serde_json::to_vec(&(&writer, &gesture)).map_err(internal)?);
        let replayed = self
            .db
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM receipts WHERE actor=?1 AND gesture=?2 AND at>=?3)",
                params![writer, &gesture[..], cutoff],
                |row| row.get::<_, bool>(0),
            )
            .map_err(internal)?;
        if replayed {
            // Storage validates the original fingerprint before replaying the
            // receipt. A replay starts only deliveries still in the outbox.
            let result = self.call(actor, call)?;
            let batch = self.claim_observations(slot, outbox_key, cutoff)?;
            return Ok((result, Disposition::Replayed, batch));
        }
        if listeners.is_empty() {
            return self
                .call(actor, call)
                .map(|value| (value, Disposition::Committed, None));
        }
        expire_pending(&self.db, cutoff)?;
        let count = self
            .db
            .query_row(
                "SELECT count(*) FROM (
                SELECT gesture FROM pending_observations UNION
                SELECT gesture FROM prepared_observations
            )",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(internal)?;
        if count >= OUTBOX_REQUESTS {
            return Err(Error::unavailable(
                "Too many change deliveries are awaiting retry.",
            ));
        }
        let items = call.input["changes"]
            .as_array()
            .into_iter()
            .flatten()
            .take(1_000)
            .filter_map(|change| Uuid::parse_str(change.get("item")?.as_str()?).ok())
            .collect::<BTreeSet<_>>();
        let mut before = BTreeMap::new();
        let mut prior_bytes = 0usize;
        for id in items {
            let Some(row) = self.rows.get(&id) else {
                continue;
            };
            if !listeners
                .iter()
                .any(|listener| listener.kind == row.head.kind)
            {
                continue;
            }
            let bytes = row.head.prefix as usize;
            prior_bytes = prior_bytes.saturating_add(bytes);
            if prior_bytes > OUTBOX_BYTES {
                return Err(observation_limit());
            }
            let before_row = snapshot(row);
            let membership = listeners
                .iter()
                .map(|listener| self.matches(actor, listener, row))
                .collect::<Vec<_>>();
            before.insert(
                id,
                Before {
                    row: before_row,
                    matches: membership,
                    bytes,
                },
            );
        }
        let prepared = Prepared {
            actor: actor.clone(),
            key,
            before,
            listeners,
        };
        let data = calternal_plugin_host::bounded_json(&prepared, OUTBOX_BYTES)
            .map_err(|_| observation_limit())?;
        self.db
            .execute(
                "INSERT OR REPLACE INTO prepared_observations VALUES (?1,?2,?3,?4,?5)",
                params![&outbox_key[..], writer, &gesture[..], now, data],
            )
            .map_err(internal)?;
        let result = match self.call(actor, call) {
            Ok(result) => result,
            Err(error) => {
                // These refusals happen before publication. Keep the
                // preparation on internal failures that recovery may finish.
                if matches!(
                    error.code,
                    ErrorCode::InvalidInput
                        | ErrorCode::Forbidden
                        | ErrorCode::NotFound
                        | ErrorCode::TooLarge
                ) {
                    self.remove_prepared(&outbox_key)?;
                }
                return Err(error);
            }
        };
        if let Err(error) = self.finish_prepared(&prepared, &result, outbox_key, now) {
            eprintln!(
                "calternal-host: could not record change deliveries ({:?})",
                error.code
            );
        }
        let batch = self.claim_observations(slot, outbox_key, cutoff)?;
        Ok((result, Disposition::Committed, batch))
    }

    fn matches(&self, actor: &Actor, listener: &Listener, row: &Row) -> bool {
        if listener.kind != row.head.kind {
            return false;
        }
        let viewer = match &actor.via {
            Some(calternal_types::Via::Run { id, .. }) => {
                Actor::run(actor.account, &listener.plugin, *id)
            }
            _ => Actor::plugin(actor.account, &listener.plugin),
        };
        match &listener.collection {
            None => true,
            Some(id) => self
                .collection(id)
                .and_then(|collection| crate::query::contains(self, &viewer, collection, row))
                .unwrap_or_else(|error| {
                    let _ = internal(error);
                    false
                }),
        }
    }
}

fn observation_limit() -> Error {
    Error::new(
        ErrorCode::TooLarge,
        "Change deliveries exceed the observation budget.",
    )
}

fn snapshot(row: &Row) -> Value {
    json!({"id":row.head.id,"kind":row.head.kind,"fields":row.fields,"version":row.head.version})
}
