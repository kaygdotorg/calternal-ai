//! Exact checkpoint deltas, addressed by adjacent revisions and UTF-8 bytes.
use calternal_content::Content;
use calternal_types::{Error, SpaceId};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

/// Version metadata field containing the range sent by the checkpoint journal.
pub(super) const METADATA_KEY: &str = "checkpoint_append";

/// Coordinates for one body insertion between adjacent committed versions.
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(super) struct Append {
    /// Opaque revision that the client must already hold.
    pub(super) base_revision: String,
    /// Insertion point relative to the previous Markdown body.
    pub(super) offset: usize,
    /// Beginning of the inserted bytes in the target revision's stored Markdown.
    pub(super) start: usize,
    /// Number of appended UTF-8 bytes, including any new message markers.
    pub(super) length: usize,
}

/// Read the exact checkpoint addition published in the journal. One indexed
/// content lookup and at most one bounded BLOB read, no userdata or job lookup. An invalid pair or offset
/// prompts a full read so a client can safely replace its cached revision.
pub(super) fn read(
    content: &Content,
    space: SpaceId,
    id: Uuid,
    input: &Value,
) -> Result<Value, Error> {
    let append: Append = serde_json::from_value(input["append"].clone())
        .map_err(|_| Error::invalid_input("invalid checkpoint range"))?;
    let revision = input["revision"].as_str().unwrap_or_default();
    let reset = || Error {
        details: Some(json!({"reason":"checkpoint_unavailable"})),
        ..Error::conflict("checkpoint delta unavailable; read the whole thread")
    };
    let end = append.start.checked_add(append.length).ok_or_else(reset)?;
    let (metadata, bytes) = content
        .revision_slice(
            space,
            id,
            "threads",
            &append.base_revision,
            revision,
            append.start..end,
        )?
        .ok_or_else(reset)?;
    let expected: Append =
        serde_json::from_value(metadata[METADATA_KEY].clone()).map_err(|_| reset())?;
    if append != expected {
        return Err(reset());
    }
    let markdown =
        String::from_utf8(bytes).map_err(|_| Error::internal("invalid checkpoint encoding"))?;
    Ok(json!({
        "delta": {"id":id, "base_revision":append.base_revision, "revision":revision,
                  "offset":append.offset,"markdown":markdown}
    }))
}
