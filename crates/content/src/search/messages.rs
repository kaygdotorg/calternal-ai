//! Byte-counted thread records projected into per-space SQLite postings.
mod codec;
mod query;
mod source;
mod writer;

use super::{Result, internal};
use calternal_types::SpaceId;
use std::{
    ops::Range,
    path::{Path, PathBuf},
};
use uuid::Uuid;

pub(super) use query::search;
pub(super) use writer::{Progress, advance};
const WIDTH: usize = 16_384;
const GENERATION: &str = concat!("1/", env!("CARGO_PKG_VERSION"));

/// One record in a validated immutable thread revision. Ranges address that exact
/// revision's staged Markdown, excluding any materialization-only identity line.
pub struct MessageRecord {
    /// Stable message UUID.
    pub id: Uuid,
    /// Saved instant in Unix seconds.
    pub stamp: i64,
    /// Original offset east of UTC, in seconds.
    pub offset_seconds: i32,
    /// Byte range of the complete framed record.
    pub record: Range<usize>,
    /// Authored body inside the record, borrowed during tokenization.
    pub body: Range<usize>,
}

/// Parser supplied by the host owning the thread format. It must validate
/// framing before returning any ranges and must not read files.
pub type MessageDecoder = fn(&[u8]) -> Result<Vec<MessageRecord>>;

struct MessageRange {
    revision: String,
    bytes: Range<usize>,
}

fn path(root: &Path, space: SpaceId) -> PathBuf {
    crate::db::space_path(root, space).with_file_name("search-messages-v1.all.db")
}

/// Fold alphanumeric tokens using Unicode lowercase. Work is linear in input
/// length. Query and indexing callers bound their input sizes before calling.
pub fn message_terms(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split(|ch: char| !ch.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(|word| word.chars().flat_map(char::to_lowercase).collect())
}

fn uuid(row: &rusqlite::Row<'_>, col: usize) -> rusqlite::Result<Uuid> {
    let bytes = row.get_ref(col)?.as_blob()?;
    Uuid::from_slice(bytes).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(col, rusqlite::types::Type::Blob, Box::new(e))
    })
}
