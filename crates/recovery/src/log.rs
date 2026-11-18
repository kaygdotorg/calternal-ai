//! Checksummed framing and the logical image shared by replay and migration.
use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Entry {
    pub(super) installation: Uuid,
    pub(super) sequence: u64,
    pub(super) previous: String,
    pub(super) body: Body,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) enum Body {
    Proposed {
        transaction: Uuid,
        delta: BTreeMap<String, Option<Record>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        receipts: Option<receipts::Changes>,
    },
    Resolved {
        committed: bool,
    },
}
pub(super) fn frame(entry: &Entry) -> Result<Vec<u8>> {
    let bytes = encode(entry)?;
    let mut out = (bytes.len() as u64).to_le_bytes().to_vec();
    out.extend(bytes);
    Ok(out)
}
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Image {
    pub(super) version: u32,
    pub(super) head: Checkpoint,
    pub(super) transaction: Uuid,
    pub(super) proposal_digest: String,
    pub(super) committed_sequence: u64,
    pub(super) segment: String,
    pub(super) records: BTreeMap<String, Record>,
    #[serde(default, skip_serializing_if = "receipts::Rows::is_empty")]
    pub(super) receipts: receipts::Rows,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) retired_receipt_ms: Option<i64>,
}

impl Image {
    pub(super) fn snapshot(self, installation: Uuid) -> Result<Snapshot> {
        if self
            .receipts
            .0
            .keys()
            .any(|key| !self.records.contains_key(&format!("1{}", key.0.simple())))
        {
            return Err(invalid());
        }
        let mut snapshot = state::assemble(
            installation,
            self.transaction,
            self.committed_sequence,
            self.records.into_values().collect(),
        )?;
        let table = snapshot
            .tables
            .get_mut("space_receipts")
            .ok_or_else(invalid)?;
        for (key, receipt) in self.receipts.0 {
            table.rows.push(receipts::cells(&key, &receipt));
        }
        table
            .rows
            .sort_by_cached_key(|r| serde_json::to_vec(r).expect("cells serialize"));
        Ok(snapshot)
    }
}
/// A proposal and its one canonical frame hash, reused by chain validation,
/// witness resolution and application. Entries remain immutable until applied.
pub(super) struct Pending {
    pub(super) entry: Entry,
    pub(super) digest: String,
}
pub(super) struct Loaded {
    pub(super) image: Image,
    pub(super) pending: Option<Pending>,
    pub(super) tail: Vec<u8>,
    pub(super) length: u64,
    pub(super) stamps: (Vec<u64>, Vec<u64>),
    pub(super) needs_sync: bool,
    pub(super) compared: bool,
}
impl Loaded {
    pub(super) fn resolution(&self, committed: bool) -> Result<(Vec<u8>, Checkpoint)> {
        let pending = self.pending.as_ref().ok_or_else(invalid)?;
        let marker = Entry {
            installation: pending.entry.installation,
            sequence: pending.entry.sequence.checked_add(1).ok_or_else(invalid)?,
            previous: pending.digest.clone(),
            body: Body::Resolved { committed },
        };
        let bytes = frame(&marker)?;
        let head = Checkpoint {
            installation: marker.installation,
            sequence: marker.sequence,
            digest: digest(&bytes),
        };
        Ok((bytes, head))
    }
    pub(super) fn apply(&mut self, committed: bool, head: Checkpoint) -> Result<()> {
        let Pending { entry, digest } = self.pending.take().ok_or_else(invalid)?;
        self.image.proposal_digest = digest;
        let Body::Proposed {
            transaction,
            delta,
            receipts,
        } = entry.body
        else {
            return Err(invalid());
        };
        if committed {
            for (key, record) in delta {
                if let Some(record) = record {
                    self.image.records.insert(key, record);
                } else {
                    self.image.records.remove(&key);
                }
            }
            if let Some(changes) = receipts {
                for (key, receipt) in changes.rows {
                    let previous = if let Some(receipt) = receipt {
                        self.image.receipts.0.insert(key, receipt)
                    } else {
                        self.image.receipts.0.remove(&key)
                    };
                    if previous.is_some() {
                        self.image.retired_receipt_ms = Some(
                            self.image
                                .retired_receipt_ms
                                .map_or(changes.at_ms, |at| at.min(changes.at_ms)),
                        );
                    }
                }
            }
            self.image.transaction = transaction;
            self.image.committed_sequence = entry.sequence;
        }
        self.image.head = head;
        Ok(())
    }
}
