//! Bounded sign-in notices in the transcript prefix, separate from authored records.
use crate::{api::Shared, auth::Directory};
use calternal_content::Request;
use calternal_types::{AccountId, Error, SpaceId};
use serde_json::{Value, json};
use std::fmt::Write as _;
use uuid::Uuid;

pub(super) const KEY: &str = "provider_sign_in_notices";
const MAX_NOTICES: usize = 16;
const START: &str = "> Sign-in was ";

/// Render the same credential-free prefix for disk, full reads and delta offsets.
pub(super) fn render(metadata: &Value, out: &mut String) {
    if let Some(notices) = metadata[KEY].as_array() {
        for notice in notices {
            let status = if notice["done"] == true {
                "done"
            } else {
                "attempted"
            };
            let label = notice["label"].as_str().expect("stored provider label");
            let stamp = notice["at"].as_str().expect("stored provider sign-in time");
            write!(out, "{START}{status} for {label} at {stamp}.\n\n").expect("write to String");
        }
    }
}

/// Skip managed blockquotes before the first message. Authored content begins
/// at an H2 and is never inspected for notice-shaped lines.
pub(super) fn skip(mut body: &str) -> Result<&str, Error> {
    while body.starts_with(START) {
        body = body
            .split_once("\n\n")
            .map(|(_, rest)| rest)
            .ok_or_else(|| Error::internal("invalid thread sign-in framing"))?;
    }
    Ok(body)
}

/// Record an attempted flow once. Receipts and the content revision guard
/// serialize competing writes. The latest sixteen flows are kept per thread.
pub(crate) fn append<D: Directory>(
    state: &Shared<D>,
    space: SpaceId,
    thread: Uuid,
    account: AccountId,
    flow: Uuid,
    provider: &str,
) -> Result<(), Error> {
    let label: String = provider
        .chars()
        .filter(|c| !c.is_control())
        .take(128)
        .collect();
    let label = super::headings::label(&label);
    let (_, zone) = super::headings::profile(state, account)?;
    let at = super::headings::time(&super::headings::now(zone.as_deref(), 0)?)?;
    update(
        state,
        space,
        thread,
        account,
        flow,
        "attempted",
        |notices| {
            if notices.iter().any(|row| row["id"] == json!(flow)) {
                return false;
            }
            if notices.len() >= MAX_NOTICES {
                notices.remove(0);
            }
            notices.push(json!({"id":flow,"label":label,"at":at,"done":false}));
            true
        },
    )
}

/// Mark the same flow complete, preserving the line's original timestamp.
/// A flow already completed or removed by retention needs no write.
pub(crate) fn complete<D: Directory>(
    state: &Shared<D>,
    space: SpaceId,
    thread: Uuid,
    account: AccountId,
    flow: Uuid,
) -> Result<(), Error> {
    update(state, space, thread, account, flow, "done", |notices| {
        let Some(row) = notices.iter_mut().find(|row| row["id"] == json!(flow)) else {
            return false;
        };
        if row["done"] == true {
            return false;
        }
        row["done"] = json!(true);
        true
    })
}

#[allow(clippy::too_many_arguments)]
fn update<D: Directory>(
    state: &Shared<D>,
    space: SpaceId,
    thread: Uuid,
    account: AccountId,
    flow: Uuid,
    phase: &str,
    edit: impl Fn(&mut Vec<Value>) -> bool,
) -> Result<(), Error> {
    let rid = format!("provider-sign-in-{flow}-{phase}");
    let fingerprint = crate::store::sha256_hex(rid.as_bytes());
    let content = state.store.content();
    for _ in 0..4 {
        if content.receipt(space, account, &rid)?.is_some() {
            return Ok(());
        }
        let item = content.get(space, thread)?;
        if item.kind != "threads" || item.metadata["deleted"] == true {
            return Err(Error::not_found("thread not found"));
        }
        let mut metadata = item.metadata.clone();
        let mut notices = metadata[KEY].as_array().cloned().unwrap_or_default();
        if !edit(&mut notices) {
            return Ok(());
        }
        let notice = notices
            .iter()
            .find(|row| row["id"] == json!(flow))
            .map(|row| json!({"provider":row["label"],"at":row["at"],"status":phase}))
            .unwrap_or(Value::Null);
        metadata[KEY] = json!(notices);
        metadata
            .as_object_mut()
            .expect("thread metadata")
            .remove(super::delta::METADATA_KEY);
        let bytes = super::document::encode(&metadata, super::document::body(&item)?);
        super::persistence::check_size(bytes.len())?;
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
            |_| json!({"notice":notice}),
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
        "thread changed while saving sign-in notice",
    ))
}
