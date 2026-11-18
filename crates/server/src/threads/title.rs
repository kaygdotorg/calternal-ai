//! Opportunistic titles on the account's own machine after a saved turn ends.
use super::{document, headings, persistence, records};
use crate::{
    api::Shared, auth::Directory, machines::MachineService, plugins::jobs::Job, turns::blocking,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use calternal_content::{Content, Item, Request};
use calternal_machine::protocol::{ChunkEvent, Stream};
use calternal_types::{Capability, Error, SpaceId};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};
use uuid::Uuid;

const MIN_MESSAGES: u64 = 5;
const MAX_ATTEMPTS: u64 = 2;
const DEADLINE: Duration = Duration::from_secs(35);

/// Use the same receipted, guarded title change for a person and for the
/// provider result. The caller passes the current revision, including any
/// completed manual rename. Cost: one ordinary content write.
pub(super) fn rename(
    content: &Content,
    mut item: Item,
    title: Value,
    automatic: bool,
    now: &str,
    request: Request<'_>,
    result: impl FnOnce(&Item) -> Value,
) -> Result<(), Error> {
    item.metadata["title"] = title;
    item.metadata["title_source"] = json!(if automatic { "automatic" } else { "manual" });
    item.metadata["updated_at"] = json!(now);
    document::validate_metadata(&item.metadata)?;
    let bytes = document::encode(&item.metadata, document::body(&item)?);
    persistence::check_size(bytes.len())?;
    content.write(
        item.space,
        Some(item.id),
        "threads",
        Some(&item.revision),
        item.metadata,
        bytes.as_bytes(),
        request,
        result,
    )?;
    Ok(())
}

/// Submit best-effort background work with an independent queue bound. There
/// are at most eight admitted tasks, including retries, and no waiting backlog
/// when that budget is full. No provider request is awaited by the turn.
pub(crate) fn schedule<D: Directory>(state: Shared<D>, machines: Arc<MachineService>, job: Job) {
    let provider = job.input["request"]["provider_id"].as_str().unwrap_or("");
    if provider != "calternal-claude-code"
        || !state.registry.snapshot().get(provider).is_some_and(|p| {
            p.manifest
                .provider
                .as_ref()
                .is_some_and(|p| p.title_generation)
        })
    {
        return;
    }
    let Ok(permit) = machines.title_slots.clone().try_acquire_owned() else {
        return;
    };
    tokio::spawn(async move {
        let _permit = permit;
        let _ = generate(state, machines, job).await;
    });
}

#[derive(Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum Reply {
    Title { title: String },
    Skip { reason: String },
}

async fn generate<D: Directory>(
    state: Shared<D>,
    machines: Arc<MachineService>,
    job: Job,
) -> Result<(), Error> {
    let space = SpaceId::parse(&job.space).map_err(|_| Error::not_found("space not found"))?;
    let thread = uuid(&job.input["request"]["thread_id"])?;
    let account = job.account;
    let provider = job.input["request"]["provider_id"]
        .as_str()
        .unwrap_or("")
        .to_owned();
    let key = Uuid::now_v7().to_string();
    for index in 0..MAX_ATTEMPTS {
        if index > 0 {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        let (st, svc, provider_id) = (state.clone(), machines.clone(), provider.clone());
        let request_id = format!("thread-title/{key}/{index}");
        let (space_key, turn_id, run) = (job.space.clone(), job.id.clone(), job.runs);
        let attempt = blocking(move || {
            if !may_title(&st, account, space) {
                return Ok(None);
            }
            let Some(latest) = st
                .jobs
                .thread_latest(&space_key, &thread.to_string())?
                .filter(|j| {
                    j.id == turn_id && j.runs == run && j.state == calternal_core::JobState::Done
                })
            else {
                return Ok(None);
            };
            let machine = uuid(&latest.checkpoint["machine_id"])?;
            let content = st.store.content();
            let item = content.get(space, thread)?;
            if !eligible(&item) {
                return Ok(None);
            }
            let catalog = st.registry.snapshot();
            let p = catalog
                .get(&provider_id)
                .ok_or_else(|| Error::not_found("provider not found"))?;
            let spec = p
                .manifest
                .provider
                .as_ref()
                .filter(|p| p.title_generation)
                .ok_or_else(|| Error::not_found("title adapter is disabled"))?;
            if !crate::plugins::activation(&st.store, space, p)?.enabled {
                return Ok(None);
            }
            let prompt = sample(document::body(&item)?)?;
            let lease = svc.request_title(space, account, machine)?.lease_id;
            let opened = reserve(content, item, account, &request_id).and_then(|()| {
                if !may_title(&st, account, space) {
                    return Err(Error::not_found("title target is unavailable"));
                }
                svc.title_exec(account, lease, spec.executable.clone(), prompt)
            });
            match opened {
                Ok(exec) => Ok(Some((lease, exec, machine))),
                Err(error) => {
                    let _ = svc.release_probe(account, lease);
                    Err(error)
                }
            }
        })
        .await;
        let (lease, exec, machine) = match attempt {
            Ok(Some(opened)) => opened,
            Ok(None) => return Ok(()),
            Err(_) => continue,
        };
        let reply = tokio::time::timeout(DEADLINE, read_reply(&machines, account, exec)).await;
        let (svc, provider_id) = (machines.clone(), provider.clone());
        let policy = match &reply {
            Ok(Ok(Reply::Skip { reason })) if reason == "managed_policy" => {
                Some(Some("managed_policy"))
            }
            Ok(Ok(Reply::Skip { reason })) if reason == "subscription_unverified" => {
                Some(Some("subscription_unverified"))
            }
            Ok(Ok(Reply::Title { .. })) => Some(None),
            _ => None,
        };
        blocking(move || {
            let observed = policy.map_or(Ok(()), |reason| {
                svc.record_title_policy(account, lease, machine, provider_id, reason)
            });
            let released = svc.release_probe(account, lease);
            observed.and(released)
        })
        .await?;
        match reply {
            Ok(Ok(Reply::Title { title })) => {
                let (st, key) = (state.clone(), key.clone());
                return blocking(move || apply_title(&st, account, space, thread, &key, title))
                    .await;
            }
            Ok(Ok(Reply::Skip { reason }))
                if matches!(
                    reason.as_str(),
                    "managed_policy" | "subscription_unverified"
                ) =>
            {
                return Ok(());
            }
            _ => {}
        }
    }
    Ok(())
}

fn uuid(value: &Value) -> Result<Uuid, Error> {
    value
        .as_str()
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| Error::not_found("title target is unavailable"))
}

fn may_title<D: Directory>(
    state: &Shared<D>,
    account: calternal_types::AccountId,
    space: SpaceId,
) -> bool {
    state
        .directory
        .capabilities(account, space)
        .is_some_and(|caps| {
            caps.contains(Capability::ContentWrite) && caps.contains(Capability::PluginsUse)
        })
}

fn pending(item: &Item) -> bool {
    item.kind == "threads"
        && item.metadata["title_source"] == "pending"
        && item.metadata["archived"] != true
        && item.metadata["deleted"] != true
}

fn eligible(item: &Item) -> bool {
    pending(item)
        && item.metadata["message_count"].as_u64().unwrap_or(0) >= MIN_MESSAGES
        && item.metadata["title_attempts"].as_u64().unwrap_or(0) < MAX_ATTEMPTS
}

fn reserve(
    content: &Content,
    mut item: Item,
    account: calternal_types::AccountId,
    id: &str,
) -> Result<(), Error> {
    item.metadata["title_attempts"] =
        json!(item.metadata["title_attempts"].as_u64().unwrap_or(0) + 1);
    content.write(
        item.space,
        Some(item.id),
        "threads",
        Some(&item.revision),
        item.metadata,
        &item.bytes,
        Request {
            account,
            via: None,
            id,
            operation: "threads.title_attempt",
            fingerprint: id,
        },
        |_| Value::Null,
    )?;
    Ok(())
}

fn sample(body: &str) -> Result<String, Error> {
    let mut prompt = String::new();
    for message in records::view(body)?.into_iter().take(MIN_MESSAGES as usize) {
        let label = if message["role"] == "agent" {
            "Assistant"
        } else {
            "Person"
        };
        let text = message["text"].as_str().unwrap_or("");
        prompt.push_str(label);
        prompt.push_str(": ");
        prompt.push_str(&text[..text.floor_char_boundary(3000)]);
        prompt.push('\n');
    }
    Ok(prompt)
}

async fn read_reply(
    machines: &Arc<MachineService>,
    account: calternal_types::AccountId,
    exec: Uuid,
) -> Result<Reply, Error> {
    let mut cursor = 0;
    let mut output = Vec::new();
    loop {
        let batch = machines
            .read_output(account, exec, cursor, DEADLINE)
            .await?;
        if batch.lost_before.is_some() {
            return Err(Error::unavailable("title output lost"));
        }
        cursor = batch.next;
        for chunk in batch.chunks {
            if let ChunkEvent::Data {
                stream: Stream::Stdout,
                data,
            } = chunk.event
            {
                if data.len() > 2048 {
                    return Err(Error::invalid_input("title output exceeds limit"));
                }
                let bytes = STANDARD
                    .decode(data)
                    .map_err(|_| Error::internal("invalid title output"))?;
                if output.len() + bytes.len() > 2048 {
                    return Err(Error::invalid_input("title output exceeds limit"));
                }
                output.extend(bytes);
            }
        }
        if let Some(exit) = batch.exit {
            if exit.code != Some(0) {
                return Err(Error::unavailable("title command ended"));
            }
            return serde_json::from_slice(&output)
                .map_err(|_| Error::internal("invalid title output"));
        }
    }
}

fn apply_title<D: Directory>(
    state: &Shared<D>,
    account: calternal_types::AccountId,
    space: SpaceId,
    thread: Uuid,
    key: &str,
    title: String,
) -> Result<(), Error> {
    let content = state.store.content();
    let item = content.get(space, thread)?;
    if !pending(&item) || !may_title(state, account, space) {
        return Ok(());
    }
    let (_, zone) = headings::profile(state, account)?;
    let now = headings::now(zone.as_deref(), 0)?;
    let request = Request {
        account,
        via: None,
        id: key,
        operation: "threads.rename",
        fingerprint: key,
    };
    rename(
        content,
        item,
        json!(title),
        true,
        &now,
        request,
        document::view,
    )
}
