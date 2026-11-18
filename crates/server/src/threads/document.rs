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
/// Locate framed records, ignoring the optional host session and the final
/// framing newline. This never scans authored Markdown after the prefix.
pub(super) fn body(item: &Item) -> Result<&str, Error> {
    body_bytes(&item.bytes)
}

pub(super) fn body_bytes(bytes: &[u8]) -> Result<&str, Error> {
    let text =
        std::str::from_utf8(bytes).map_err(|_| Error::internal("invalid thread encoding"))?;
    let (front, body) = text
        .strip_prefix("---\n")
        .and_then(|s| s.split_once("\n---\n"))
        .ok_or_else(|| Error::internal("invalid thread envelope"))?;
    if !front.lines().any(|line| line == "format: calternal-thread") {
        return Err(Error::conflict("invalid thread format"));
    }
    let (_, tail) = body
        .strip_prefix("\n# ")
        .and_then(|s| s.split_once("\n\n"))
        .ok_or_else(|| Error::internal("invalid thread title heading"))?;
    let tail = if tail.starts_with("<!-- provider-session:") {
        tail.split_once(" -->\n\n")
            .map(|(_, tail)| tail)
            .ok_or_else(|| Error::internal("invalid thread session marker"))?
    } else {
        tail
    };
    super::provider_notice::skip(tail)?
        .strip_suffix('\n')
        .ok_or_else(|| Error::internal("invalid thread framing"))
}

fn headline(metadata: &Value) -> String {
    format!(
        "\n# {}\n\n",
        super::headings::label(metadata["title"].as_str().unwrap_or(""))
    )
}

/// Encode the one readable format. The content host injects calternal-id when
/// materializing the file. Private session state stays outside public reads.
pub(super) fn encode(metadata: &Value, body: &str) -> String {
    format_document(metadata, body, None, true)
}

fn format_document(metadata: &Value, body: &str, id: Option<uuid::Uuid>, private: bool) -> String {
    use std::fmt::Write as _;
    let mut out = String::from("---\n");
    if let Some(id) = id {
        writeln!(out, "calternal-id: {id}").expect("write to String");
    }
    out.push_str("format: calternal-thread\n");
    let mut title = String::new();
    for c in metadata["title"].to_string().chars() {
        if matches!(c, '#' | '\u{2028}' | '\u{2029}' | '\u{fffe}' | '\u{ffff}') {
            write!(title, "\\u{:04x}", u32::from(c)).expect("write to String");
        } else {
            title.push(c);
        }
    }
    writeln!(
        out,
        "title: {title}\ndate: {}\nlast edited: {}",
        metadata["created_at"].as_str().unwrap_or(""),
        metadata["updated_at"].as_str().unwrap_or("")
    )
    .expect("write to String");
    if metadata["archived"] == true {
        out.push_str("archived: true\n");
    }
    out.push_str("---\n");
    out.push_str(&headline(metadata));
    if private && let Some(session) = metadata.get(super::provider_session::KEY) {
        let session = session.to_string().replace('>', "\\u003e");
        writeln!(out, "<!-- provider-session:{session} -->\n").expect("write to String");
    }
    super::provider_notice::render(metadata, &mut out);
    out.push_str(body);
    out.push('\n');
    out
}

/// Size of the public body's generated prefix for translating delta offsets.
pub(super) fn public_prefix_bytes(metadata: &Value) -> usize {
    let mut prefix = headline(metadata);
    super::provider_notice::render(metadata, &mut prefix);
    prefix.len()
}

/// Exclude transport internals and the account's private native session binding.
pub(super) fn public_metadata(mut metadata: Value) -> Value {
    if let Some(fields) = metadata.as_object_mut() {
        for key in [
            super::delta::METADATA_KEY,
            super::provider_session::KEY,
            "output",
            "pending",
            super::provider_notice::KEY,
        ] {
            fields.remove(key);
        }
    }
    metadata
}

/// Validate records and return their presentation alongside the public Markdown.
/// The view holds no provider session state or machine membership references.
pub(super) fn checked_view(item: &Item) -> Result<Value, Error> {
    let body = body(item)?;
    let messages = super::records::read(body)?;
    let mut metadata = public_metadata(item.metadata.clone());
    metadata["message_count"] = json!(messages.len());
    Ok(
        json!({"thread":{"id":item.id,"space_id":item.space,"address":format!("/threads/{}",item.id),
        "revision":item.revision,"metadata":metadata,"messages":messages,
        "markdown":format_document(&item.metadata, body, Some(item.id), false)}}),
    )
}

/// Encode a just-written snapshot for its receipt. The writer constructs the
/// record framing and validated inputs before entering the content transaction.
pub(super) fn view(item: &Item) -> Value {
    checked_view(item).expect("generated thread snapshot has valid framing")
}

/// Share one parsed snapshot between the content receipt and the operation reply.
pub(super) fn receipt_view(out: &mut Value) -> impl FnOnce(&Item) -> Value + '_ {
    |item| {
        *out = view(item);
        out.clone()
    }
}

/// The configured provider default, if the trusted manifest declares one. No I/O.
pub(super) fn default_model<D: crate::auth::Directory>(
    state: &crate::api::Shared<D>,
    provider: &str,
) -> Value {
    state
        .registry
        .snapshot()
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
