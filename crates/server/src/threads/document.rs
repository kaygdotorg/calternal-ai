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
pub(super) fn encode(metadata: &Value, body: &str) -> String {
    let mut out = String::from("---\n");
    for key in [
        "title",
        "provider",
        "mode",
        "mode_for",
        "parent_thread",
        "spawned_by",
        "created_at",
        "archived",
        "deleted",
    ] {
        if let Some(value) = metadata.get(key) {
            out.push_str(&format!("{}: {}\n", key.replace('_', "-"), value));
        }
    }
    out.push_str(&format!(
        "updated-at: {}\n---\n",
        calternal_content::now_rfc3339()
    ));
    out.push_str(body);
    out
}
pub(super) fn view(item: &Item) -> Value {
    json!({"thread":{"id":item.id,"space_id":item.space,"address":format!("/threads/{}",item.id),"revision":item.revision,"metadata":item.metadata,"markdown":String::from_utf8_lossy(&item.bytes)}})
}
