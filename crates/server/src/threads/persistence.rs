//! Bounded, receipted provider text checkpoints through generic content storage.
use crate::{api::Shared, auth::Directory, plugins::jobs::Job};
use calternal_content::Request;
use calternal_types::{Error, SpaceId};
use serde_json::{Value, json};
use uuid::Uuid;

/// Flush at most 32 KiB of scrubbed output; existing content CAS serializes it
/// against person messages. A saved cursor/receipt prevents duplicate checkpoint
/// blocks after a partial failure. Caller runs on a blocking worker, outside jobs locks.
pub(crate) fn checkpoint<D: Directory>(
    state: &Shared<D>,
    job: &Job,
    checkpoint: &Value,
) -> Result<(), Error> {
    if job.input.get("thread_request").is_none() {
        return Ok(());
    }
    let text = checkpoint["thread_text"].as_str().unwrap_or("");
    if text.is_empty() {
        return Ok(());
    }
    if text.len() > 32768 {
        return Err(Error::invalid_input("thread checkpoint exceeds 32 KiB"));
    }
    let space = SpaceId::parse(&job.space).map_err(|_| Error::internal("invalid thread space"))?;
    if !state
        .directory
        .capabilities(job.account, space)
        .is_some_and(|c| c.contains(calternal_types::Capability::PluginsUse))
    {
        return Err(Error::not_found("item not found"));
    }
    let plugin = state
        .registry
        .get("calternal-orchestrator")
        .ok_or_else(|| Error::not_found("item not found"))?;
    let activation = crate::plugins::activation(&state.store, space, plugin)?;
    if !activation.enabled || activation.generation != job.generation {
        return Err(Error::not_found("item not found"));
    }
    let id = job.input["request"]["thread_id"]
        .as_str()
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| Error::internal("invalid saved thread"))?;
    let seq = checkpoint["provider_seq"].as_u64().unwrap_or(0);
    let rid = format!("thread-output/{}/{}/{seq}", job.id, job.runs);
    let fp = crate::api::fingerprint(
        "threads.checkpoint",
        &serde_json::from_value(json!({"job":job.id,"run":job.runs,"seq":seq,"text":text}))
            .unwrap(),
    );
    let content = state.store.content();
    if let Some(receipt) = content.receipt(space, job.account, &rid)? {
        if receipt.fingerprint != fp {
            return Err(Error::request_reused());
        }
        return Ok(());
    }
    for _ in 0..4 {
        let item = content.get(space, id)?;
        if item.kind != "threads" || item.metadata["deleted"] == true {
            return Err(Error::not_found("item not found"));
        }
        let mut body = super::document::body(&item)?.to_owned();
        let mut metadata = item.metadata.clone();
        let output = &metadata["output"];
        let end = if output["job_id"] == job.id {
            let offset = output["end"]
                .as_u64()
                .and_then(|v| usize::try_from(v).ok())
                .filter(|&v| v <= body.len() && body.is_char_boundary(v))
                .ok_or_else(|| Error::internal("invalid thread output cursor"))?;
            body.insert_str(offset, text);
            offset + text.len()
        } else {
            body.push_str(&format!(
                "\n<!-- message:{} role:agent turn:{} actor:{} -->\n",
                job.id,
                job.id,
                calternal_types::Actor::agent(job.account, id)
            ));
            body.push_str(text);
            let end = body.len();
            body.push_str("\n<!-- /message -->\n");
            end
        };
        metadata["output"] = json!({"job_id":job.id,"run":job.runs,"checkpoint":seq,"end":end});
        let bytes = super::document::encode(&metadata, &body);
        match content.write(
            space,
            Some(id),
            "threads",
            Some(&item.revision),
            metadata,
            bytes.as_bytes(),
            Request {
                account: job.account,
                id: &rid,
                operation: "threads.checkpoint",
                fingerprint: &fp,
            },
            |_| json!({"checkpoint":seq}),
        ) {
            Ok(_) => return Ok(()),
            Err(e)
                if e.details
                    .as_ref()
                    .is_some_and(|d| d["reason"] == "stale_revision") =>
            {
                continue;
            }
            Err(e) => return Err(e),
        }
    }
    Err(Error::conflict(
        "thread changed while saving agent output; retry from its checkpoint",
    ))
}
