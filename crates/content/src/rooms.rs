//! Durable state for the host live-editing service. A checkpoint is committed
//! with its content revision and outbox intent under the owning space lock.
mod diff;

use crate::{Content, FileFormat, Item, Result, SpaceId, db::Space, types::internal};
use calternal_types::Error;
use rusqlite::{OptionalExtension, Transaction, params};
use serde_json::Value;
use uuid::Uuid;

pub use diff::Edit as LiveTextEdit;

/// Text change for an opted-in Markdown kind. All changes share the normal
/// content revision, receipt and checked file placement path.
#[derive(Clone, Copy)]
pub enum LiveEdit<'a> {
    /// Add text at the current end. This operation does not take a base revision.
    Append(&'a str),
    /// Apply nonoverlapping UTF-16 edits against the specified retained base.
    Patch(&'a [LiveTextEdit]),
}

#[derive(Clone, Copy)]
pub(crate) enum Body<'a> {
    Replace(&'a [u8]),
    Live(LiveEdit<'a>),
}

impl Body<'_> {
    pub(crate) fn validate(self, metadata: &Value) -> Result<()> {
        match self {
            Self::Replace(bytes) => Content::validate_write_size(metadata, bytes),
            Self::Live(LiveEdit::Append(text)) => {
                Content::validate_write_size(metadata, text.as_bytes())
            }
            Self::Live(LiveEdit::Patch(edits)) => {
                let max = crate::MAX_DOCUMENT_BYTES;
                if edits.len() > 256
                    || edits
                        .iter()
                        .map(|e| e.insert.len().min(max + 1))
                        .sum::<usize>()
                        > max
                {
                    return Err(Error::invalid_input("live patch exceeds its size limit"));
                }
                Ok(())
            }
        }
    }
}

use yrs::updates::decoder::Decode;
use yrs::{
    ClientID, Doc, GetString, OffsetKind, Options, ReadTxn, StateVector, Text, Transact, Update,
};

pub(crate) const SCHEMA: &str = "
    CREATE TABLE room_kinds(kind TEXT PRIMARY KEY REFERENCES kinds(kind)) STRICT;
    CREATE TABLE room_checkpoints(
        item TEXT NOT NULL,
        seq INTEGER NOT NULL,
        lineage TEXT NOT NULL,
        state BLOB NOT NULL,
        PRIMARY KEY(item,seq),
        FOREIGN KEY(item,seq) REFERENCES versions(item,seq) ON DELETE CASCADE
    ) WITHOUT ROWID, STRICT;
";

const CHECKPOINT_COUNT: i64 = 64;
const CHECKPOINT_BYTES: i64 = 16 * 1024 * 1024;
const MAX_STATE_BYTES: usize = 8 * 1024 * 1024;
const TEXT: &str = "md";

struct Document {
    lineage: String,
    doc: Doc,
}

pub(crate) struct Checkpoint {
    lineage: String,
    state: Vec<u8>,
}

impl Document {
    fn seed(item: &Item) -> Result<Self> {
        let digest = blake3::hash(format!("room:{}:{}", item.id, item.revision).as_bytes());
        let writer = u64::from_le_bytes(digest.as_bytes()[..8].try_into().unwrap());
        let mut options = Options::with_client_id(ClientID::new((writer & ((1 << 53) - 1)).max(1)));
        options.offset_kind = OffsetKind::Utf16;
        let doc = Doc::with_options(options);
        let text = doc.get_or_insert_text(TEXT);
        text.insert(&mut doc.transact_mut(), 0, markdown(&item.bytes)?);
        Ok(Self {
            lineage: digest.to_hex()[..32].into(),
            doc,
        })
    }

    fn load(checkpoint: Checkpoint) -> Result<Self> {
        let doc = Doc::with_options(Options {
            offset_kind: OffsetKind::Utf16,
            ..Options::default()
        });
        doc.get_or_insert_text(TEXT);
        apply(&doc, &checkpoint.state)?;
        Ok(Self {
            lineage: checkpoint.lineage,
            doc,
        })
    }

    fn change(&self, body: Body<'_>) -> Result<Vec<u8>> {
        let edits = match body {
            Body::Replace(bytes) => diff::edits(&self.text(), markdown(bytes)?),
            Body::Live(LiveEdit::Patch(edits)) => checked_edits(&self.text(), edits)?,
            Body::Live(LiveEdit::Append(insert)) => {
                let text = self.doc.get_or_insert_text(TEXT);
                let mut txn = self.doc.transact_mut();
                let at = text.len(&txn);
                text.insert(&mut txn, at, insert);
                return Ok(txn.encode_update_v1());
            }
        };
        let text = self.doc.get_or_insert_text(TEXT);
        let mut txn = self.doc.transact_mut();
        for edit in edits {
            if edit.delete != 0 {
                text.remove_range(&mut txn, edit.at, edit.delete);
            }
            if !edit.insert.is_empty() {
                text.insert(&mut txn, edit.at, &edit.insert);
            }
        }
        Ok(txn.encode_update_v1())
    }

    fn text(&self) -> String {
        let text = self.doc.get_or_insert_text(TEXT);
        text.get_string(&self.doc.transact())
    }

    fn checkpoint(&self) -> Result<Checkpoint> {
        let state = self
            .doc
            .transact()
            .encode_state_as_update_v1(&StateVector::default());
        if state.len() > MAX_STATE_BYTES {
            return Err(Error::unavailable(
                "live document state exceeds its storage budget",
            ));
        }
        Ok(Checkpoint {
            lineage: self.lineage.clone(),
            state,
        })
    }
}

fn checked_edits(text: &str, edits: &[LiveTextEdit]) -> Result<Vec<LiveTextEdit>> {
    use unicode_segmentation::UnicodeSegmentation;
    let invalid =
        || Error::invalid_input("patch offsets overlap, split a grapheme, or exceed the base");
    let mut edits = edits.to_vec();
    edits.sort_unstable_by_key(|e| e.at);
    let mut end = 0;
    let mut previous = None;
    let mut boundaries = std::iter::once(0)
        .chain(text.graphemes(true).scan(0, |at, gr| {
            *at += gr.encode_utf16().count() as u32;
            Some(*at)
        }))
        .peekable();
    for edit in &edits {
        if previous == Some(edit.at) || edit.at < end {
            return Err(invalid());
        }
        previous = Some(edit.at);
        end = edit.at.checked_add(edit.delete).ok_or_else(invalid)?;
        for offset in [edit.at, end] {
            while boundaries.peek().is_some_and(|at| *at < offset) {
                boundaries.next();
            }
            if boundaries.peek() != Some(&offset) {
                return Err(invalid());
            }
        }
    }
    edits.reverse();
    Ok(edits)
}

fn markdown(bytes: &[u8]) -> Result<&str> {
    std::str::from_utf8(bytes).map_err(|_| Error::invalid_input("Markdown must be UTF-8"))
}

fn apply(doc: &Doc, bytes: &[u8]) -> Result<()> {
    let update = Update::decode_v1(bytes).map_err(internal)?;
    let mut txn = doc.transact_mut();
    txn.apply_update(update).map_err(internal)?;
    if txn.has_missing_updates() {
        return Err(internal("live checkpoint has an unresolved update"));
    }
    Ok(())
}

fn stale() -> Error {
    Error::conflict("live edit requires a retained base in the current lineage")
        .with_details(serde_json::json!({"reason":"stale_revision"}))
}

impl Content {
    /// Opt a registered Markdown kind in to live editing for this space.
    /// Non-Markdown and unknown kinds are refused. Repeating the opt-in is safe.
    /// Cost: one registration lookup, one indexed flag read, and a FULL
    /// transaction on first opt-in.
    pub fn enable_live_editing(&self, space: SpaceId, kind: &str) -> Result<()> {
        self.with_space(space, |space| {
            if space.registration(kind)?.1 != FileFormat::Markdown {
                return Err(Error::invalid_input(
                    "live editing requires a Markdown kind",
                ));
            }
            if !space.live_editing(kind)? {
                space
                    .conn
                    .execute("INSERT INTO room_kinds VALUES(?1)", [kind])
                    .map_err(internal)?;
            }
            Ok(())
        })
    }
}

impl Space {
    pub(crate) fn live_editing(&self, kind: &str) -> Result<bool> {
        self.conn
            .prepare_cached("SELECT EXISTS(SELECT 1 FROM room_kinds WHERE kind=?1)")
            .map_err(internal)?
            .query_row([kind], |r| r.get(0))
            .map_err(internal)
    }

    fn checkpoint(&self, id: Uuid, seq: i64) -> Result<Option<Checkpoint>> {
        self.conn
            .prepare_cached("SELECT lineage,state FROM room_checkpoints WHERE item=?1 AND seq=?2")
            .map_err(internal)?
            .query_row(params![id.to_string(), seq], |r| {
                Ok(Checkpoint {
                    lineage: r.get(0)?,
                    state: r.get(1)?,
                })
            })
            .optional()
            .map_err(internal)
    }

    /// Prepare a fork of the client's base update against the room head.
    /// The caller holds the space lock until the returned checkpoint and
    /// revision are committed, so a concurrent writer cannot overtake them.
    pub(crate) fn merge_room_write(
        &self,
        current: &Item,
        seq: i64,
        base: &str,
        metadata: &mut Value,
        body: Body<'_>,
    ) -> Result<(Vec<u8>, Checkpoint)> {
        let retained = self.checkpoint(current.id, seq)?;
        let live = match retained {
            Some(checkpoint) => Document::load(checkpoint)?,
            None if current.revision == base => Document::seed(current)?,
            None => return Err(stale()),
        };
        if base == current.revision {
            live.change(body)?;
        } else {
            let (base_seq, base_metadata): (i64, String) = self
                .conn
                .prepare_cached("SELECT seq,metadata FROM versions WHERE item=?1 AND revision=?2")
                .map_err(internal)?
                .query_row(params![current.id.to_string(), base], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })
                .optional()
                .map_err(internal)?
                .ok_or_else(stale)?;
            let checkpoint = self.checkpoint(current.id, base_seq)?.ok_or_else(stale)?;
            if checkpoint.lineage != live.lineage {
                return Err(stale());
            }
            let base_metadata: Value = serde_json::from_str(&base_metadata).map_err(internal)?;
            if matches!(body, Body::Replace(_)) && current.metadata != base_metadata {
                if *metadata == base_metadata {
                    *metadata = current.metadata.clone();
                } else if *metadata != current.metadata {
                    return Err(stale());
                }
            }
            let fork = Document::load(checkpoint)?;
            apply(&live.doc, &fork.change(body)?)?;
        }
        Ok((live.text().into_bytes(), live.checkpoint()?))
    }

    pub(crate) fn seed_checkpoint(&self, item: &Item) -> Result<Checkpoint> {
        Document::seed(item)?.checkpoint()
    }
}

impl Checkpoint {
    pub(crate) fn rewrite(self, bytes: &[u8]) -> Result<Self> {
        let doc = Document::load(self)?;
        doc.change(Body::Replace(bytes))?;
        doc.checkpoint()
    }

    pub(crate) fn write(&self, tx: &Transaction<'_>, item: Uuid, seq: i64) -> Result<()> {
        let item = item.to_string();
        tx.execute(
            "INSERT INTO room_checkpoints VALUES(?1,?2,?3,?4)",
            params![item, seq, self.lineage, self.state],
        )
        .map_err(internal)?;
        tx.execute(
            "DELETE FROM room_checkpoints WHERE item=?1 AND seq IN (
                SELECT seq FROM (SELECT seq,
                    ROW_NUMBER() OVER newest AS entries,
                    SUM(length(state)) OVER newest AS bytes
                FROM room_checkpoints WHERE item=?1
                WINDOW newest AS (ORDER BY seq DESC ROWS UNBOUNDED PRECEDING))
                WHERE entries>?2 OR bytes>?3)",
            params![item, CHECKPOINT_COUNT, CHECKPOINT_BYTES],
        )
        .map_err(internal)?;
        Ok(())
    }
}
