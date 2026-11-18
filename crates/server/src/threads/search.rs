//! Adapter between the current thread frame and the generic content index.
use calternal_content::{Content, MessageRecord};
use calternal_types::{Error, SpaceId};
use serde_json::{Value, json};
use uuid::Uuid;

/// Validate the exact saved thread before exposing UUIDs and byte windows.
pub(crate) fn decode(bytes: &[u8]) -> Result<Vec<MessageRecord>, Error> {
    let body = super::document::body_bytes(bytes)?;
    let base = bytes.as_ptr() as usize;
    Ok(super::records::read(body)?
        .into_iter()
        .map(|record| {
            let start = record.heading.as_ptr() as usize - base - 3;
            let text = record.text.as_ptr() as usize - base;
            let end = text + record.text.len();
            MessageRecord {
                id: record.id,
                stamp: record.instant.unix_timestamp(),
                offset_seconds: record.instant.offset().whole_seconds(),
                record: start..end,
                body: text..end,
            }
        })
        .collect())
}

/// Open a single record in the authorized live thread. A stale locator cannot
/// read a different frame or an old version after an edit or deletion.
pub(super) fn open(
    content: &Content,
    space: SpaceId,
    id: Uuid,
    message: &str,
) -> Result<Value, Error> {
    let missing = || Error::not_found("thread message not found");
    let message = Uuid::parse_str(message).map_err(|_| missing())?;
    let (revision, bytes) = content.search_message(space, id, message)?;
    let frame = std::str::from_utf8(&bytes).map_err(|_| missing())?;
    let mut records = super::records::read(frame)?;
    if records.len() != 1 || records[0].id != message {
        return Err(missing());
    }
    Ok(
        json!({"thread":{"id":id,"space_id":space,"address":format!("/threads/{id}"),
        "revision":revision,"messages":[records.remove(0)]}}),
    )
}
