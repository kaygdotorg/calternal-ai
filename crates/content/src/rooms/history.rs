//! Ordered, author-tagged Yjs updates, committed with their owning revisions.
//! As in the reference app, the history lives until the item is purged. The
//! room's pruned checkpoints and the receipt retention window are independent.

use super::*;

pub(crate) const HISTORY_SCHEMA: &str = "
    CREATE TABLE room_history(
        item TEXT NOT NULL,
        seq INTEGER NOT NULL,
        ordinal INTEGER NOT NULL,
        lineage TEXT NOT NULL,
        author TEXT NOT NULL,
        via TEXT,
        source TEXT NOT NULL CHECK(source IN ('anchor','seed','edit','outside')),
        update_bytes BLOB NOT NULL CHECK(length(update_bytes) BETWEEN 2 AND 8388608),
        PRIMARY KEY(item,seq,ordinal),
        FOREIGN KEY(item,seq) REFERENCES versions(item,seq) ON DELETE CASCADE
    ) WITHOUT ROWID, STRICT;
";

pub(super) struct HistoryUpdate {
    source: &'static str,
    author: Option<calternal_types::AccountId>,
    bytes: Vec<u8>,
}

impl HistoryUpdate {
    pub(super) fn new(source: &'static str, bytes: Vec<u8>) -> Self {
        Self {
            source,
            bytes,
            author: None,
        }
    }
    pub(super) fn by(mut self, author: calternal_types::AccountId) -> Self {
        self.author = Some(author);
        self
    }

    pub(super) fn is_edit(&self) -> bool {
        self.source == "edit"
    }
}

pub(super) fn record(
    tx: &Transaction<'_>,
    item: &str,
    seq: i64,
    lineage: &str,
    account: &str,
    via: Option<&calternal_types::Via>,
    updates: &[HistoryUpdate],
) -> Result<()> {
    let via = via
        .map(serde_json::to_string)
        .transpose()
        .map_err(internal)?;
    let mut insert = tx
        .prepare_cached("INSERT INTO room_history VALUES(?1,?2,?3,?4,?5,?6,?7,?8)")
        .map_err(internal)?;
    for (ordinal, update) in updates.iter().enumerate() {
        let author = update.author.map(|id| id.to_string());
        insert
            .execute(params![
                item,
                seq,
                ordinal as i64,
                lineage,
                author.as_deref().unwrap_or(account),
                via,
                update.source,
                update.bytes,
            ])
            .map_err(internal)?;
    }
    Ok(())
}
