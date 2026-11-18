//! Production Markdown projections exposed only to the disposable performance driver.
use calternal_types::Error;
use serde_json::Value;

/// Read the same heading, tags and relative-link projection used by Notes saves.
/// The fallback title represents the retained file route for headingless notes.
/// Work is bounded by supplied file bytes. Errors use the Notes save contract.
pub fn note_metadata(fallback_title: &str, markdown: &str) -> Result<Value, Error> {
    let title = crate::notes::heading_title(markdown).unwrap_or(fallback_title);
    calternal_core::validate_title(title)?;
    crate::notes::metadata(title, markdown)
}

/// Fit a synthetic corpus into the readable thread format. Each fixture record
/// uses 30 generated terms and explicit common, rare and phrase controls.
/// Returns the current metadata and staged Markdown for one bounded thread.
pub fn message_fixture(first: usize, count: usize) -> (Value, String) {
    crate::threads::search::fixture(first, count)
}
