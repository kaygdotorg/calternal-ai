//! Adapter between the current thread frame and the generic content index.
use calternal_content::{Content, MessageRecord};
use calternal_types::{Error, SpaceId};
use serde_json::{Value, json};
use uuid::Uuid;

#[cfg(feature = "rebuild-probe")]
pub(crate) fn fixture(first: usize, count: usize) -> (Value, String) {
    const SAVED: &str = "2026-11-18T12:00:00+00:00";
    const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut body = String::new();
    for i in first..first + count {
        let common = if i % 2 == 0 { "canvas" } else { "forest" };
        let rare = if i % 8191 == 0 { "rarest" } else { "common" };
        let phrase = match i % 64 {
            0 => "readyx spacer",
            2 => "spacer readyx",
            _ => "spacer restxx",
        };
        let mut text = format!("## Plan\n\n{common} {phrase} {rare} ");
        for j in 0..30 {
            let n = (i.wrapping_mul(2_654_435_761) + j * 97) % 8192;
            text.push('t');
            text.extend([n / 1296, n / 36 % 36, n % 36].map(|digit| DIGITS[digit] as char));
            text.push(' ');
        }
        let id = Uuid::from_u128(0x019a0000_0000_7000_8000_000000000000 | (i as u128 + 1));
        super::records::append(
            &mut body,
            id,
            "person",
            "Person · 2026-11-18 12:00:00 +00:00",
            &text,
            None,
        );
    }
    let metadata = json!({"title":format!("Corpus {first:09}"),"provider":"calternal-claude-code",
        "created_at":SAVED,"updated_at":SAVED,"last_activity_at":SAVED,"message_count":count,
        "deleted":false,"archived":false});
    let bytes = super::document::encode(&metadata, &body);
    (metadata, bytes)
}

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
