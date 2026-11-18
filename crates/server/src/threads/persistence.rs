//! Bounded, receipted provider text checkpoints through generic content storage.
use super::delta::Append;
use super::provider_session::{KEY, ProviderSession};
use crate::{api::Shared, auth::Directory, plugins::jobs::Job};
use calternal_content::{MAX_DOCUMENT_BYTES, Request};
use calternal_types::{Error, SpaceId};
use serde_json::{Value, json};
use uuid::Uuid;

/// Stable reason for a saved turn whose next checkpoint cannot fit.
pub(crate) const THREAD_FULL: &str = "thread_full";

/// Flush at most 32 KiB of scrubbed output and any new provider session. The
/// content CAS serializes this against sends. A superseded turn cannot replace
/// the session binding. A saved cursor and receipt prevent duplicate output.
/// Cost is one receipt read and, for a new checkpoint, a content read and write
/// on a blocking worker outside jobs locks. Stale revisions retry up to four times.
pub(crate) fn checkpoint<D: Directory>(
    state: &Shared<D>,
    job: &Job,
    checkpoint: &Value,
    session: Option<ProviderSession>,
) -> Result<(), Error> {
    if job.input.get("thread_request").is_none() {
        return Ok(());
    }
    let text = checkpoint["thread_text"].as_str().unwrap_or("");
    if text.is_empty() && session.is_none() {
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
    let actor = calternal_types::Actor::agent(job.account, id);
    let seq = checkpoint["provider_seq"].as_u64().unwrap_or(0);
    let rid = format!("thread-output/{}/{}/{seq}", job.id, job.runs);
    let session = serde_json::to_value(session)
        .map_err(|_| Error::internal("invalid provider session record"))?;
    let fp = crate::api::fingerprint(
        "threads.checkpoint",
        &serde_json::from_value(
            json!({"job":job.id,"run":job.runs,"seq":seq,"text":text,(KEY):session}),
        )
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
        let mut metadata = item.metadata.clone();
        let new_session = !session.is_null()
            && metadata["pending"] == job.input["thread_request"]
            && metadata[KEY] != session;
        if new_session {
            metadata[KEY] = session.clone();
        } else if text.is_empty() {
            return Ok(());
        }
        let mut body = super::document::body(&item)?.to_owned();
        let range = if text.is_empty() {
            body.len()..body.len()
        } else {
            append_output(&mut metadata, &mut body, job, id, seq, text)?
        };
        let bytes = super::document::encode(&metadata, &body);
        if bytes.len() > MAX_DOCUMENT_BYTES {
            return Err(Error {
                details: Some(json!({"reason":THREAD_FULL})),
                ..Error::conflict(
                    "Thread reached its 2 MiB limit. Earlier output is saved; the latest output was not saved. Start a new thread to continue.",
                )
            });
        }
        let append = json!(Append {
            base_revision: item.revision.clone(),
            offset: range.start,
            start: bytes.len() - body.len() + range.start,
            length: range.len(),
        });
        metadata[super::delta::METADATA_KEY] = append.clone();
        match content.write(
            space,
            Some(id),
            "threads",
            Some(&item.revision),
            metadata,
            bytes.as_bytes(),
            Request {
                account: actor.account,
                via: actor.via.as_ref(),
                id: &rid,
                operation: "threads.checkpoint",
                fingerprint: &fp,
            },
            |_| json!({"checkpoint":seq,"append":append}),
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

fn append_output(
    metadata: &mut Value,
    body: &mut String,
    job: &Job,
    id: Uuid,
    seq: u64,
    text: &str,
) -> Result<std::ops::Range<usize>, Error> {
    let now = calternal_content::now_rfc3339();
    let output = &metadata["output"];
    let (end, range) = if output["job_id"] == job.id {
        let offset = output["end"]
            .as_u64()
            .and_then(|v| usize::try_from(v).ok())
            .filter(|&v| v <= body.len() && body.is_char_boundary(v))
            .ok_or_else(|| Error::internal("invalid thread output cursor"))?;
        body.insert_str(offset, text);
        super::document::record_activity(metadata, &now);
        let end = offset + text.len();
        (end, offset..end)
    } else {
        let start = body.len();
        body.push_str(&format!(
            "\n<!-- message:{} role:agent turn:{} actor:{} time:{now} -->\n",
            job.id,
            job.id,
            calternal_types::Actor::agent(job.account, id)
        ));
        body.push_str(text);
        let end = body.len();
        body.push_str("\n<!-- /message -->\n");
        super::document::record_message(metadata, &now);
        (end, start..body.len())
    };
    metadata["output"] = json!({"job_id":job.id,"run":job.runs,"checkpoint":seq,"end":end});
    Ok(range)
}
