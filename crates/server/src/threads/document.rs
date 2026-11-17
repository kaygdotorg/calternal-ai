//! Small Markdown envelope. Message content is never parsed for authority.
use calternal_content::Item;
use calternal_types::Error;
use serde_json::{Value, json};

pub(super) fn validate_metadata(metadata: &Value) -> Result<(), Error> {
    for (key, max) in [("title", 256), ("provider", 80)] {
        let value = metadata[key].as_str().unwrap_or("");
        if value.trim().is_empty() || value.len() > max || value.chars().any(char::is_control) {
            return Err(Error::invalid_input(
                "title/provider is empty or exceeds its bounds",
            ));
        }
    }
    Ok(())
}
pub(super) fn body(item: &Item) -> Result<&str, Error> {
    let text =
        std::str::from_utf8(&item.bytes).map_err(|_| Error::internal("invalid thread encoding"))?;
    text.strip_prefix("---\n")
        .and_then(|s| s.split_once("\n---\n"))
        .map(|(_, body)| body)
        .ok_or_else(|| Error::internal("invalid thread envelope"))
}
/// Encode a readable frontmatter envelope and copy the body without interpreting
/// its markers. The content service adds the item identity during placement.
pub(super) fn encode(metadata: &Value, body: &str) -> String {
    let mut out = String::from("---\n");
    for key in [
        "title",
        "provider",
        "model",
        "message_count",
        "mode",
        "mode_for",
        "parent_thread",
        "spawned_by",
        "created_at",
        "updated_at",
        "last_activity_at",
        "archived",
        "deleted",
    ] {
        if let Some(value) = metadata.get(key) {
            out.push_str(&format!("{}: {}\n", key.replace('_', "-"), value));
        }
    }
    out.push_str("---\n");
    out.push_str(body);
    out
}
/// Encode the existing document response with one allocation proportional to
/// the stored UTF-8 body. Callers add their current authorized job view.
pub(super) fn view(item: &Item) -> Value {
    json!({"thread":{"id":item.id,"space_id":item.space,"address":format!("/threads/{}",item.id),"revision":item.revision,"metadata":item.metadata,"markdown":String::from_utf8_lossy(&item.bytes)}})
}

/// The configured provider default, if the trusted manifest declares one. No I/O.
pub(super) fn default_model<D: crate::auth::Directory>(
    state: &crate::api::Shared<D>,
    provider: &str,
) -> Value {
    state
        .registry
        .get(provider)
        .and_then(|p| p.manifest.provider.as_ref())
        .map_or(Value::Null, |p| json!(p.default_model))
}

/// Record the timestamp and count when a message block is added, in constant time.
pub(super) fn record_message(metadata: &mut Value, now: &str) {
    metadata["message_count"] = json!(metadata["message_count"].as_u64().unwrap_or(0) + 1);
    record_activity(metadata, now);
}

/// Record the time new conversation text was saved, in constant time.
pub(super) fn record_activity(metadata: &mut Value, now: &str) {
    metadata["updated_at"] = json!(now);
    metadata["last_activity_at"] = json!(now);
}
