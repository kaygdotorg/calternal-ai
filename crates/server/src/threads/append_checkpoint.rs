//! Save scrubbed provider chunks as complete Markdown records.
use super::{delta, document, headings, persistence, records};
use crate::plugins::jobs::Job;
use calternal_content::{Content, Request};
use calternal_types::{Error, SpaceId};
use serde_json::{Value, json};
use uuid::Uuid;

/// Write just the next physical frame. The logical message retains its stable
/// ID across continuations and its original timestamp across terminal timing.
pub(super) fn save(
    content: &Content,
    space: SpaceId,
    id: Uuid,
    job: &Job,
    checkpoint: &Value,
    session: Value,
    request: Request<'_>,
) -> Result<(), Error> {
    let seq = checkpoint["provider_seq"].as_u64().unwrap_or(0);
    for _ in 0..4 {
        match append(content, space, id, job, checkpoint, &session, request, seq) {
            Err(error)
                if error
                    .details
                    .as_ref()
                    .is_some_and(|d| d["reason"] == "stale_revision") => {}
            result => return result,
        }
    }
    Err(Error::conflict(
        "thread changed while saving agent output; retry from its checkpoint",
    ))
}

fn append(
    content: &Content,
    space: SpaceId,
    id: Uuid,
    job: &Job,
    checkpoint: &Value,
    session: &Value,
    request: Request<'_>,
    seq: u64,
) -> Result<(), Error> {
    let (head, size) = content.thread_head_for_append(space, id)?;
    let mut metadata = head.metadata;
    let output = &metadata["output"];
    if metadata["deleted"] == true
        || metadata["pending"] != job.input["thread_request"]
        || output["job_id"] == job.id
            && output["run"]
                .as_u64()
                .is_some_and(|run| run > u64::from(job.runs))
    {
        return Err(Error::conflict(
            "this turn no longer owns the thread output",
        ));
    }
    let text = checkpoint["thread_text"].as_str().unwrap_or("");
    let control = text.is_empty() && checkpoint["message_timing"].is_null();
    if !session.is_null() && metadata[super::provider_session::KEY] != *session {
        metadata[super::provider_session::KEY] = session.clone();
    } else if control {
        return Ok(());
    }
    let prefix =
        document::prefix_bytes(&content.thread_prefix_for_append(space, id, &head.revision)?)?;
    let mut frame = String::from("\n");
    if control {
        frame.push_str(records::CONTROL);
    } else {
        let mut data = checkpoint.clone();
        let output = metadata
            .as_object_mut()
            .and_then(|meta| meta.remove("output"))
            .unwrap_or_default();
        let group = if output["job_id"] == job.id && output["run"] == job.runs {
            output["logical_id"].as_str().map(str::to_owned)
        } else {
            None
        };
        let attribution = &job.input["thread_request"];
        let at = if group.is_some() {
            output["at"]
                .as_str()
                .ok_or_else(|| Error::internal("saved thread timestamp missing"))?
                .to_owned()
        } else {
            headings::now(
                attribution["time_zone"].as_str(),
                attribution["client_offset_minutes"].as_i64().unwrap_or(0),
            )?
        };
        data["record_at"] = json!(at);
        if let Some(group) = &group {
            data["segment_of"] = json!(group);
            data["message_id"] = json!(Uuid::now_v7());
        }
        let mut record = String::new();
        persistence::append_output(&mut metadata, &mut record, job, seq, text, &data)?;
        metadata["output"]["logical_id"] =
            group.map_or_else(|| checkpoint["message_id"].clone(), Value::String);
        metadata["output"]["at"] = json!(at);
        frame.push_str(&record);
    }
    frame.push('\n');
    let start = size
        .checked_sub(1)
        .ok_or_else(|| Error::internal("saved thread is empty"))?;
    let body = start
        .checked_sub(prefix)
        .ok_or_else(|| Error::internal("invalid saved thread prefix"))?;
    let append = json!(delta::Append {
        base_revision: head.revision.clone(),
        offset: document::public_prefix_bytes(&metadata) + body,
        start,
        length: frame.len(),
    });
    let receipt = if text.is_empty() {
        metadata
            .as_object_mut()
            .expect("thread metadata")
            .remove(delta::METADATA_KEY);
        json!({"checkpoint":seq})
    } else {
        metadata[delta::METADATA_KEY] = append.clone();
        json!({"checkpoint":seq,"append":append})
    };
    content.append_thread_record(
        space,
        id,
        &head.revision,
        metadata,
        frame.as_bytes(),
        request,
        |_| receipt,
    )?;
    Ok(())
}
