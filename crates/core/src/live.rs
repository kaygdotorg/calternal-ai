//! One server connection follows journals; clients never reconcile file replicas.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// One space followed on a live connection. At most 16 distinct spaces per connection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct LiveSubscription {
    /// Full space UUID; possessing it grants no access.
    pub space_id: String,
    /// Opaque signed server bookmark; absent starts at the current head.
    pub bookmark: Option<String>,
}

/// SSE data payload. Entries are opaque journal values; only the server interprets them.
/// A reset stops this subscription; reload authorized state before reconnecting.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum LiveFrame {
    /// One currently authorized journal entry, in space order.
    Entry {
        /// Space whose journal advanced.
        space_id: String,
        /// Complete server journal entry, including its signed token and item UUID.
        #[ts(type = "unknown")]
        entry: serde_json::Value,
    },
    /// Backlog exhausted; subsequent entries continue from this bookmark.
    Ready {
        /// Space now caught up.
        space_id: String,
        /// Opaque signed bookmark, including positions skipped by access filtering.
        bookmark: String,
    },
    /// An expired, foreign or rolled-back bookmark requires a reload.
    Reset {
        /// Space whose derived state must be reloaded.
        space_id: String,
    },
}
