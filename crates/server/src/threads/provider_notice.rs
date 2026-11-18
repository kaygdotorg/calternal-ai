//! The sign-in notice contains only the provider label, outcome and time.
use crate::{api::Shared, auth::Directory};
use calternal_content::{MAX_DOCUMENT_BYTES, Request};
use calternal_types::{AccountId, Error, SpaceId};
use serde_json::json;
use uuid::Uuid;

/// The one line owned by a live flow, with its body offset before any concurrent append.
pub(crate) struct Notice {
    line: String,
    offset: usize,
}

/// Persist the attempted line before starting the official program. Crash recovery
/// keeps that line. One revision-fenced content write, with no challenge in the receipt.
pub(crate) fn append<D: Directory>(
    state: &Shared<D>,
    space: SpaceId,
    thread: Uuid,
    account: AccountId,
    flow: Uuid,
    provider: &str,
) -> Result<Notice, Error> {
    let stamp = time::OffsetDateTime::now_utc()
        .format(time::macros::format_description!(
            "[year]-[month]-[day] [hour]:[minute]:[second] UTC"
        ))
        .map_err(|_| Error::internal("could not format provider notice time"))?;
    let provider: String = provider
        .chars()
        .filter(|c| !c.is_control())
        .take(128)
        .collect();
    let line = format!("\n\n> Sign-in was attempted for {provider} at {stamp}.\n");
    let offset = update(state, space, thread, account, flow, "attempted", |body| {
        let offset = body.len();
        body.push_str(&line);
        Ok(offset)
    })?;
    Ok(Notice { line, offset })
}

/// Change the existing line to completion. The saved offset distinguishes
/// simultaneous flows for the same provider and second. An edited line is left alone.
pub(crate) fn complete<D: Directory>(
    state: &Shared<D>,
    space: SpaceId,
    thread: Uuid,
    account: AccountId,
    flow: Uuid,
    notice: &Notice,
) -> Result<(), Error> {
    update(state, space, thread, account, flow, "done", |body| {
        let mut range = notice.offset..notice.offset + notice.line.len();
        if body.get(range.clone()) != Some(notice.line.as_str()) {
            let mut hits = body.match_indices(&notice.line);
            let (offset, _) = hits
                .next()
                .ok_or_else(|| Error::conflict("thread sign-in notice was edited"))?;
            if hits.next().is_some() {
                return Err(Error::conflict("thread sign-in notice was moved"));
            }
            range = offset..offset + notice.line.len();
        }
        body.replace_range(range, &notice.line.replacen("was attempted", "was done", 1));
        Ok(notice.offset)
    })?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn update<D: Directory>(
    state: &Shared<D>,
    space: SpaceId,
    thread: Uuid,
    account: AccountId,
    flow: Uuid,
    phase: &str,
    edit: impl Fn(&mut String) -> Result<usize, Error>,
) -> Result<usize, Error> {
    let rid = format!("provider-sign-in-{flow}-{phase}");
    let fingerprint = crate::store::sha256_hex(rid.as_bytes());
    let content = state.store.content();
    for _ in 0..4 {
        let item = content.get(space, thread)?;
        if item.kind != "threads" || item.metadata["deleted"] == true {
            return Err(Error::not_found("thread not found"));
        }
        let mut body = super::document::body(&item)?.to_owned();
        let offset = edit(&mut body)?;
        let bytes = super::document::encode(&item.metadata, &body);
        if bytes.len() > MAX_DOCUMENT_BYTES {
            return Err(Error::conflict("thread is full"));
        }
        let mut metadata = item.metadata;
        if let Some(fields) = metadata.as_object_mut() {
            fields.remove(super::delta::METADATA_KEY);
        }
        match content.write(
            space,
            Some(thread),
            "threads",
            Some(&item.revision),
            metadata,
            bytes.as_bytes(),
            Request {
                account,
                via: None,
                id: &rid,
                operation: "threads.provider_sign_in",
                fingerprint: &fingerprint,
            },
            |_| json!({}),
        ) {
            Ok(_) => return Ok(offset),
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
        "thread changed while saving sign-in notice",
    ))
}
