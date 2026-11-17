//! Journal values and token parser ported independently of the old engine.
use crate::{AccountId, Error, ErrorCode, Result};
use serde::{Deserialize, Serialize};
use std::fmt;

/// A position in one space’s journal.
///
/// The `epoch` names one generation of the journal. If the journal is ever
/// rebuilt, the epoch changes, old tokens stop working and clients refetch
/// what they show (a [`ErrorCode::SyncTokenExpired`] error).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncToken {
    /// Generation of the journal this token belongs to.
    pub epoch: String,
    /// Sequence number of the last entry seen. `0` means the beginning.
    pub seq: i64,
}

/// The two proofs a signed token carries after its position
/// (`epoch-seq-c-m`): `c` checks the journal entry at `seq`, `m` is a keyed
/// tag over the rest, made with a key that never leaves the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenProof {
    /// 32 bits hashed from the entry the token points at.
    pub check: u32,
    /// 32 bits keyed over `epoch ‖ seq ‖ check`.
    pub tag: u32,
}

impl SyncToken {
    /// Parses a token sent by a client. A signed token's proofs are read
    /// and ignored here; see [`Self::parse_signed`].
    /// Allocates the epoch string; no lock, writer queue or SQLite work.
    pub fn parse(s: &str) -> Result<Self> {
        Self::parse_signed(s).map(|(t, _)| t)
    }

    /// Parses `epoch-seq` or `epoch-seq-c-m`.
    /// Allocates the epoch string; no lock, writer queue or SQLite work.
    pub fn parse_signed(s: &str) -> Result<(Self, Option<TokenProof>)> {
        let bad = || {
            Error::new(
                ErrorCode::SyncTokenExpired,
                format!("unreadable sync token {s:?}"),
            )
        };
        let hex8 = |p: &str| {
            if p.len() == 8 {
                u32::from_str_radix(p, 16).ok()
            } else {
                None
            }
        };
        let mut parts = s.split('-');
        let (epoch, seq) = (parts.next().ok_or_else(bad)?, parts.next().ok_or_else(bad)?);
        let proof = match (parts.next(), parts.next(), parts.next()) {
            (None, _, _) => None,
            (Some(c), Some(m), None) => Some(TokenProof {
                check: hex8(c).ok_or_else(bad)?,
                tag: hex8(m).ok_or_else(bad)?,
            }),
            _ => return Err(bad()),
        };
        let seq: i64 = seq.parse().map_err(|_| bad())?;
        if epoch.is_empty() || !epoch.bytes().all(|b| b.is_ascii_hexdigit()) || seq < 0 {
            return Err(bad());
        }
        Ok((
            Self {
                epoch: epoch.to_owned(),
                seq,
            },
            proof,
        ))
    }
}

impl fmt::Display for SyncToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.epoch, self.seq)
    }
}

/// One change, as stored and as sent to clients.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JournalEntry {
    /// Acting account recorded when this entry was appended.
    pub author: AccountId,
    /// Position in the journal. Strictly increasing.
    pub seq: i64,
    /// The sync token right after this entry.
    pub token: String,
    /// When it happened, in milliseconds since the Unix epoch.
    pub at_ms: i64,
    /// What happened, namespaced by plugin: `notes.edited`.
    pub kind: String,
    /// Which item changed, in the plugin's own id space.
    pub item: String,
    /// Plugin-defined details.
    pub data: serde_json::Value,
    /// The client that made the change, if known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installation: Option<String>,
    /// The request that made the change, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_round_trip() {
        let t = SyncToken {
            epoch: "a1b2".into(),
            seq: 42,
        };
        assert_eq!(SyncToken::parse(&t.to_string()).unwrap(), t);
    }

    #[test]
    fn signed_tokens_parse_and_junk_proofs_do_not() {
        let (t, p) = SyncToken::parse_signed("ab12-7-0000beef-cafe0001").unwrap();
        assert_eq!((t.epoch.as_str(), t.seq), ("ab12", 7));
        assert_eq!(
            p,
            Some(TokenProof {
                check: 0xbeef,
                tag: 0xcafe0001
            })
        );
        assert_eq!(SyncToken::parse("ab12-7-0000beef-cafe0001").unwrap(), t);
        assert_eq!(SyncToken::parse_signed("ab12-7").unwrap().1, None);
        for junk in [
            "a-1-2",
            "a-1-0000beef",
            "a-1-zzzzzzzz-00000000",
            "a-1-0000beef-00000000-1",
        ] {
            assert!(SyncToken::parse_signed(junk).is_err(), "{junk:?}");
        }
    }

    #[test]
    fn junk_tokens_ask_for_a_refresh() {
        for junk in ["", "abc", "-5", "a--1", "a-x"] {
            let e = SyncToken::parse(junk).unwrap_err();
            assert_eq!(e.code, ErrorCode::SyncTokenExpired, "{junk:?}");
        }
    }
}
