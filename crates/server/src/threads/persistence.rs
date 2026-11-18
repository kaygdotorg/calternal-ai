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
    let timing = &checkpoint["message_timing"];
    if text.is_empty() && session.is_none() && timing.is_null() {
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
    let catalog = state.registry.snapshot();
    let plugin = catalog
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
            json!({"job":job.id,"run":job.runs,"seq":seq,"text":text,"timing":timing,(KEY):session}),
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
        if item.metadata["pending"] != job.input["thread_request"]
            || item.metadata["output"]["job_id"] == job.id
                && item.metadata["output"]["run"]
                    .as_u64()
                    .is_some_and(|run| run > u64::from(job.runs))
        {
            return Err(Error::conflict(
                "this turn no longer owns the thread output",
            ));
        }
        let mut metadata = item.metadata.clone();
        let new_session = !session.is_null()
            && metadata["pending"] == job.input["thread_request"]
            && metadata[KEY] != session;
        if new_session {
            metadata[KEY] = session.clone();
        } else if text.is_empty() && timing.is_null() {
            return Ok(());
        }
        let mut body = super::document::body(&item)?.to_owned();
        let range = if text.is_empty() && timing.is_null() {
            Some(body.len()..body.len())
        } else {
            let provider = job.input["request"]["provider_id"].as_str().unwrap_or("");
            let manifest = state.registry.get(provider).map(|plugin| &plugin.manifest);
            append_output(
                &mut metadata,
                &mut body,
                job,
                seq,
                text,
                checkpoint,
                manifest,
            )?
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
        let mut receipt = json!({"checkpoint":seq});
        metadata
            .as_object_mut()
            .expect("thread metadata object")
            .remove(super::delta::METADATA_KEY);
        if let Some(range) = range {
            let append = json!(Append {
                base_revision: item.revision.clone(),
                offset: super::document::public_prefix_bytes(&metadata) + range.start,
                start: bytes.len() - body.len() - 1 + range.start,
                length: range.len(),
            });
            metadata[super::delta::METADATA_KEY] = append.clone();
            receipt["append"] = append;
        }
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
            |_| receipt,
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
    seq: u64,
    text: &str,
    checkpoint: &Value,
    manifest: Option<&calternal_core::ManifestV1>,
) -> Result<Option<std::ops::Range<usize>>, Error> {
    let now = super::headings::now(
        job.input["thread_request"]["client_offset_minutes"]
            .as_i64()
            .unwrap_or(0),
    )?;
    let id = checkpoint["message_id"]
        .as_str()
        .and_then(|v| Uuid::parse_str(v).ok())
        .ok_or_else(|| Error::internal("saved assistant message ID missing"))?;
    let turn = Uuid::parse_str(&job.id).map_err(|_| Error::internal("invalid saved turn ID"))?;
    let timing = &checkpoint["message_timing"];
    let request = &job.input["request"];
    let selected = request["model"].as_str().unwrap_or("");
    let model = manifest
        .and_then(|m| m.provider.as_ref())
        .and_then(|p| p.models.iter().find(|m| m.id == selected))
        .map(|m| m.name.as_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("Unknown model");
    let harness = manifest
        .map(|m| m.name.as_str())
        .unwrap_or("Unknown harness");
    let serving = checkpoint["serving_provider"]
        .as_str()
        .unwrap_or("Unknown provider");
    let reasoning = request["options"]["effort"].as_str();
    let continuation = if job.runs <= 1 {
        String::new()
    } else if job.checkpoint["retry"]["retry_count"]
        .as_u64()
        .is_some_and(|n| n > 0)
    {
        format!(" · Retry {}", job.runs)
    } else {
        " · Resumed".into()
    };
    let rendered = |at: &str| {
        super::headings::agent(
            harness,
            serving,
            model,
            reasoning,
            at,
            timing,
            &continuation,
        )
    };
    let provenance = super::records::Provenance {
        provider_id: request["provider_id"]
            .as_str()
            .filter(|id| !id.is_empty())
            .unwrap_or("calternal-claude-code")
            .to_owned(),
        model_id: (!selected.is_empty()).then(|| selected.to_owned()),
        reasoning_level: reasoning.map(str::to_owned),
        machine_id: checkpoint["message_machine"].as_str().map(str::to_owned),
        duration_ms: timing["duration_ms"].as_u64(),
        reasoning_ms: timing["reasoning_ms"].as_u64(),
    };
    let output = &metadata["output"];
    let (start, range) = if output["job_id"] == job.id && output["run"] == job.runs {
        let start = output["start"]
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .ok_or_else(|| Error::internal("invalid thread output cursor"))?;
        let current = super::records::tail(body, start, id)?;
        let heading = if timing.is_null() {
            current.heading.to_owned()
        } else {
            rendered(&current.at)?
        };
        let mut combined = String::with_capacity(current.text.len() + text.len());
        combined.push_str(current.text);
        combined.push_str(text);
        body.truncate(start);
        // The indexed start already excludes the separator.
        let mut replacement = String::new();
        super::records::append(
            &mut replacement,
            id,
            "agent",
            &heading,
            &combined,
            Some((turn, &provenance)),
        );
        body.push_str(&replacement);
        super::document::record_activity(metadata, &now);
        (start, None)
    } else {
        let offset = body.len();
        let start = super::records::append(
            body,
            id,
            "agent",
            &rendered(&now)?,
            text,
            Some((turn, &provenance)),
        );
        super::document::record_message(metadata, &now);
        (start, Some(offset..body.len()))
    };
    metadata["output"] = json!({"job_id":job.id,"run":job.runs,"checkpoint":seq,"start":start});
    Ok(range)
}
